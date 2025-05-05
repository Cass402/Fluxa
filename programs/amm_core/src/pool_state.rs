//! Pool State Management Module
//!
//! This module handles the tracking and management of liquidity positions within Fluxa pools.
//! It implements advanced state tracking to efficiently manage active liquidity across all
//! price ranges, enabling the core concentrated liquidity functionality of the protocol.
//!
//! The implementation uses a sparse data structure to track liquidity changes at tick boundaries,
//! allowing for capital-efficient positions and precise fee accounting per liquidity unit.

use crate::constants::{MAX_TICK, MIN_TICK, PROTOCOL_FEE_DENOMINATOR};
use crate::errors::ErrorCode;
use crate::math::{self, Q64, U128MAX};
use crate::tick_bitmap::TickBitmap;
use crate::{Pool, Position};
use anchor_lang::prelude::*;

// Constants for fee tiers and corresponding tick spacings
pub const FEE_TIER_LOW: u16 = 5; // 0.05%
pub const FEE_TIER_MEDIUM: u16 = 30; // 0.3%
pub const FEE_TIER_HIGH: u16 = 100; // 1%

pub const TICK_SPACING_LOW: i32 = 10;
pub const TICK_SPACING_MEDIUM: i32 = 60;
pub const TICK_SPACING_HIGH: i32 = 200;

/// Represents the state for a specific tick in the price range
///
/// Ticks are the discrete price points that define the boundaries of positions.
/// Each tick stores the net liquidity change when the price crosses that tick,
/// as well as accumulated fee data for precise fee distribution.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct Tick {
    /// Net liquidity change when price crosses this tick
    /// Positive when entering the tick range, negative when exiting
    pub liquidity_net: i128,

    /// Total liquidity gross amount - sum of all liquidity that references this tick
    /// Used for garbage collection and optimization
    pub liquidity_gross: u128,

    /// Fee growth outside of this tick range for token A, stored as Q64.64
    /// Used to calculate fees for positions that span this tick
    pub fee_growth_outside_a: u128,

    /// Fee growth outside of this tick range for token B, stored as Q64.64
    /// Used to calculate fees for positions that span this tick
    pub fee_growth_outside_b: u128,

    /// Flag indicating if this tick is initialized
    /// Helps with optimization and quick checks
    pub initialized: bool,

    /// Sequential reference count for positions using this tick boundary
    /// Used to track when a tick can be deleted
    pub reference_count: u16,
}

impl Tick {
    /// Creates a new uninitialized tick
    pub fn new() -> Self {
        Tick {
            liquidity_net: 0,
            liquidity_gross: 0,
            fee_growth_outside_a: 0,
            fee_growth_outside_b: 0,
            initialized: false,
            reference_count: 0,
        }
    }

    /// Initialize a tick with the provided liquidity change
    pub fn initialize(&mut self, liquidity_delta: i128) {
        self.initialized = true;
        self.liquidity_net = liquidity_delta;
        self.liquidity_gross = liquidity_delta.unsigned_abs();
        self.reference_count = 1;
    }

    /// Update tick with additional liquidity
    pub fn update(&mut self, liquidity_delta: i128) {
        self.liquidity_net += liquidity_delta;
        self.liquidity_gross += liquidity_delta.unsigned_abs();
        self.reference_count += 1;
    }

    /// Remove liquidity from this tick
    pub fn remove_liquidity(&mut self, liquidity_delta: i128) -> Result<()> {
        require!(self.reference_count > 0, ErrorCode::InvalidTickReference);

        self.liquidity_net -= liquidity_delta;
        self.liquidity_gross -= liquidity_delta.unsigned_abs();
        self.reference_count -= 1;

        // If there are no more references, we can clear the tick
        if self.reference_count == 0 {
            self.initialized = false;
        }

        Ok(())
    }

    /// Flip the growth variables when crossing a tick
    ///
    /// This is a key operation for concentrated liquidity fee accounting,
    /// ensuring that fees are properly tracked as price moves across tick boundaries.
    pub fn cross(&mut self, fee_growth_global_a: u128, fee_growth_global_b: u128) {
        // Flip the tracked growth variables
        self.fee_growth_outside_a = fee_growth_global_a.wrapping_sub(self.fee_growth_outside_a);
        self.fee_growth_outside_b = fee_growth_global_b.wrapping_sub(self.fee_growth_outside_b);
    }
}

impl Default for Tick {
    /// Default implementation that returns a new uninitialized tick
    fn default() -> Self {
        Self::new()
    }
}

/// Parameters for fee growth calculations
#[derive(Debug, Clone, Copy)]
pub struct FeeGrowthParams {
    /// Current tick index
    pub tick_current: i32,
    /// Lower tick boundary
    pub tick_lower: i32,
    /// Upper tick boundary
    pub tick_upper: i32,
    /// Global fee growth for token A
    pub fee_growth_global_a: u128,
    /// Global fee growth for token B
    pub fee_growth_global_b: u128,
    /// Fee growth outside lower tick for token A
    pub fee_growth_outside_lower_a: u128,
    /// Fee growth outside lower tick for token B
    pub fee_growth_outside_lower_b: u128,
    /// Fee growth outside upper tick for token A
    pub fee_growth_outside_upper_a: u128,
    /// Fee growth outside upper tick for token B
    pub fee_growth_outside_upper_b: u128,
}

/// Calculate fee growth inside a specific tick range
///
/// This function is used to determine how much fee growth has occurred
/// within a position's tick range, accounting for fee growth that
/// happened outside the range.
///
/// # Parameters
/// * `params` - The fee growth parameters
///
/// # Returns
/// * `(u128, u128)` - The fee growth inside the range for tokens A and B
pub fn compute_fee_growth_inside(params: &FeeGrowthParams) -> (u128, u128) {
    // Calculate fee growth inside based on where current tick is relative to the range
    let (fee_growth_below_a, fee_growth_below_b) = if params.tick_current >= params.tick_lower {
        (
            params.fee_growth_outside_lower_a,
            params.fee_growth_outside_lower_b,
        )
    } else {
        (
            params
                .fee_growth_global_a
                .wrapping_sub(params.fee_growth_outside_lower_a),
            params
                .fee_growth_global_b
                .wrapping_sub(params.fee_growth_outside_lower_b),
        )
    };

    let (fee_growth_above_a, fee_growth_above_b) = if params.tick_current < params.tick_upper {
        (
            params.fee_growth_outside_upper_a,
            params.fee_growth_outside_upper_b,
        )
    } else {
        (
            params
                .fee_growth_global_a
                .wrapping_sub(params.fee_growth_outside_upper_a),
            params
                .fee_growth_global_b
                .wrapping_sub(params.fee_growth_outside_upper_b),
        )
    };

    // Fee growth inside = global growth - growth below - growth above
    let fee_growth_inside_a = params
        .fee_growth_global_a
        .wrapping_sub(fee_growth_below_a)
        .wrapping_sub(fee_growth_above_a);

    let fee_growth_inside_b = params
        .fee_growth_global_b
        .wrapping_sub(fee_growth_below_b)
        .wrapping_sub(fee_growth_above_b);

    (fee_growth_inside_a, fee_growth_inside_b)
}

/// Manages the global state of a liquidity pool
///
/// This struct provides methods for tracking active positions, computing fees,
/// and maintaining the pool state as positions are created, modified, and liquidated.
/// It leverages the sparse tick data structure for capital efficiency.
pub struct PoolState<'a> {
    /// Reference to the pool account being managed
    pub pool: &'a mut Pool,

    /// Active positions within the pool
    pub positions: Vec<&'a mut Position>,

    /// Ticks mapped by their index (sparse representation)
    /// This approach allows efficient storage of only initialized ticks
    pub ticks: Vec<(i32, Tick)>,

    /// Tick bitmap for efficient tick traversal during swaps
    pub tick_bitmap: TickBitmap,
}

impl<'a> PoolState<'a> {
    /// Creates a new PoolState instance for managing a pool's liquidity positions
    pub fn new(pool: &'a mut Pool) -> Self {
        PoolState {
            pool,
            positions: Vec::new(),
            ticks: Vec::new(),
            tick_bitmap: TickBitmap::new(),
        }
    }

    /// Loads a position into the state manager
    pub fn load_position(&mut self, position: &'a mut Position) {
        self.positions.push(position);
    }

    /// Finds or initializes a tick at the given index
    pub fn get_or_create_tick(&mut self, tick_index: i32) -> &mut Tick {
        // First, try to find the tick
        let tick_position = self.ticks.iter().position(|(idx, _)| *idx == tick_index);

        // If found, return a mutable reference to it
        if let Some(position) = tick_position {
            return &mut self.ticks[position].1;
        }

        // Tick doesn't exist, create and initialize it
        let new_tick = Tick::new();
        self.ticks.push((tick_index, new_tick));

        // Return a reference to the newly added tick
        // We know it's the last one, so we can use last_mut() safely
        &mut self.ticks.last_mut().unwrap().1
    }

    /// Creates a new position in the pool with specified price range
    pub fn create_position(
        &mut self,
        position: &'a mut Position,
        lower_tick: i32,
        upper_tick: i32,
        liquidity_delta: u128,
    ) -> Result<(u64, u64)> {
        // Validate tick range
        require!(lower_tick < upper_tick, ErrorCode::InvalidTickRange);
        require!(
            lower_tick >= MIN_TICK && upper_tick <= MAX_TICK,
            ErrorCode::InvalidTickRange
        );

        // Initialize position data
        position.lower_tick = lower_tick;
        position.upper_tick = upper_tick;
        position.liquidity = liquidity_delta;

        // Get the current tick and price from the pool
        let current_tick = self.pool.current_tick;
        let sqrt_price = self.pool.sqrt_price;

        // Initialize or update the ticks at position boundaries
        let lower = self.get_or_create_tick(lower_tick);
        if !lower.initialized {
            lower.initialize(liquidity_delta as i128);
        } else {
            lower.update(liquidity_delta as i128);
        }

        let upper = self.get_or_create_tick(upper_tick);
        if !upper.initialized {
            upper.initialize(-(liquidity_delta as i128));
        } else {
            upper.update(-(liquidity_delta as i128));
        }

        // Update position fee tracking - properly handle the Result
        self.update_position_fees(position)?;

        // Update global pool liquidity if position is in range
        if current_tick >= lower_tick && current_tick < upper_tick {
            self.pool.liquidity = self
                .pool
                .liquidity
                .checked_add(liquidity_delta)
                .ok_or(ErrorCode::MathOverflow)?;
        }

        // Calculate token amounts required for the position
        let (amount_a, amount_b) = self.calculate_token_amounts(
            lower_tick,
            upper_tick,
            liquidity_delta,
            sqrt_price,
            current_tick,
        )?;

        // Add position to our tracked positions
        self.positions.push(position);

        Ok((amount_a, amount_b))
    }

    /// Calculate amounts of token A and B required for a position
    fn calculate_token_amounts(
        &self,
        lower_tick: i32,
        upper_tick: i32,
        liquidity: u128,
        sqrt_price: u128,
        current_tick: i32,
    ) -> Result<(u64, u64)> {
        // Calculate lower and upper sqrt price bounds
        let sqrt_price_lower = math::tick_to_sqrt_price(lower_tick)?;
        let sqrt_price_upper = math::tick_to_sqrt_price(upper_tick)?;

        let mut amount_a: u64 = 0;
        let mut amount_b: u64 = 0;

        // Calculate amounts based on the current price in relation to position bounds
        if current_tick < lower_tick {
            // Current price is below the position range
            // Only token A is needed
            amount_a = math::get_amount_a_delta_for_price_range(
                liquidity,
                sqrt_price_lower,
                sqrt_price_upper,
                true, // Rounding up for deposits
            )? as u64;
        } else if current_tick >= upper_tick {
            // Current price is above the position range
            // Only token B is needed
            amount_b = math::get_amount_b_delta_for_price_range(
                liquidity,
                sqrt_price_lower,
                sqrt_price_upper,
                true, // Rounding up for deposits
            )? as u64;
        } else {
            // Current price is within the position range
            // Both tokens are needed
            amount_a = math::get_amount_a_delta_for_price_range(
                liquidity,
                sqrt_price,
                sqrt_price_upper,
                true, // Rounding up for deposits
            )? as u64;

            amount_b = math::get_amount_b_delta_for_price_range(
                liquidity,
                sqrt_price_lower,
                sqrt_price,
                true, // Rounding up for deposits
            )? as u64;
        }

        Ok((amount_a, amount_b))
    }

    /// Updates the fee tracking data for a position
    pub fn update_position_fees(&mut self, position: &mut Position) -> Result<()> {
        // Get the current global fee growth values
        let fee_growth_global_a = self.pool.fee_growth_global_a;
        let fee_growth_global_b = self.pool.fee_growth_global_b;

        // Find lower and upper ticks
        let lower_tick_idx = position.lower_tick;
        let upper_tick_idx = position.upper_tick;

        // Calculate fee growth inside the position's range
        let (fee_growth_inside_a, fee_growth_inside_b) = self.get_fee_growth_inside(
            lower_tick_idx,
            upper_tick_idx,
            self.pool.current_tick,
            fee_growth_global_a,
            fee_growth_global_b,
        )?;

        // Calculate accumulated fees if position already has liquidity
        if position.liquidity > 0 {
            // Calculate fee growth since last update
            let fee_growth_delta_a = fee_growth_inside_a.wrapping_sub(position.fee_growth_inside_a);
            let fee_growth_delta_b = fee_growth_inside_b.wrapping_sub(position.fee_growth_inside_b);

            // Calculate fees owed based on liquidity and fee growth
            let fees_owed_a = U128::from(position.liquidity)
                .mul_div_floor(U128::from(fee_growth_delta_a), U128::from(Q64))
                .map_err(|_| ErrorCode::MathOverflow)?;

            let fees_owed_b = U128::from(position.liquidity)
                .mul_div_floor(U128::from(fee_growth_delta_b), U128::from(Q64))
                .map_err(|_| ErrorCode::MathOverflow)?;

            // Accumulate fees in the position
            position.tokens_owed_a = position
                .tokens_owed_a
                .checked_add(fees_owed_a.as_u64())
                .ok_or(ErrorCode::MathOverflow)?;

            position.tokens_owed_b = position
                .tokens_owed_b
                .checked_add(fees_owed_b.as_u64())
                .ok_or(ErrorCode::MathOverflow)?;
        }

        // Update position with current fee growth values
        position.fee_growth_inside_a = fee_growth_inside_a;
        position.fee_growth_inside_b = fee_growth_inside_b;

        Ok(())
    }

    /// Calculates the fee growth inside a specific tick range
    fn get_fee_growth_inside(
        &self,
        lower_tick_idx: i32,
        upper_tick_idx: i32,
        current_tick: i32,
        fee_growth_global_a: u128,
        fee_growth_global_b: u128,
    ) -> Result<(u128, u128)> {
        // Find the tick state at boundaries
        let mut fee_growth_below_a = 0;
        let mut fee_growth_below_b = 0;
        let mut fee_growth_above_a = 0;
        let mut fee_growth_above_b = 0;

        // Get lower tick fee growth outside values
        for (idx, tick) in &self.ticks {
            if *idx == lower_tick_idx && tick.initialized {
                if current_tick >= *idx {
                    fee_growth_below_a = tick.fee_growth_outside_a;
                    fee_growth_below_b = tick.fee_growth_outside_b;
                } else {
                    fee_growth_below_a =
                        fee_growth_global_a.wrapping_sub(tick.fee_growth_outside_a);
                    fee_growth_below_b =
                        fee_growth_global_b.wrapping_sub(tick.fee_growth_outside_b);
                }
                break;
            }
        }

        // Get upper tick fee growth outside values
        for (idx, tick) in &self.ticks {
            if *idx == upper_tick_idx && tick.initialized {
                if current_tick >= *idx {
                    fee_growth_above_a =
                        fee_growth_global_a.wrapping_sub(tick.fee_growth_outside_a);
                    fee_growth_above_b =
                        fee_growth_global_b.wrapping_sub(tick.fee_growth_outside_b);
                } else {
                    fee_growth_above_a = tick.fee_growth_outside_a;
                    fee_growth_above_b = tick.fee_growth_outside_b;
                }
                break;
            }
        }

        // Calculate growth inside the range by subtracting outside growth
        let fee_growth_inside_a = fee_growth_global_a
            .wrapping_sub(fee_growth_below_a)
            .wrapping_sub(fee_growth_above_a);

        let fee_growth_inside_b = fee_growth_global_b
            .wrapping_sub(fee_growth_below_b)
            .wrapping_sub(fee_growth_above_b);

        Ok((fee_growth_inside_a, fee_growth_inside_b))
    }

    /// Modify an existing position's liquidity
    pub fn modify_position(
        &mut self,
        position: &mut Position,
        liquidity_delta: i128,
        is_increase: bool,
    ) -> Result<(u64, u64)> {
        // Update position fees before modifying liquidity
        self.update_position_fees(position)?;

        let lower_tick = position.lower_tick;
        let upper_tick = position.upper_tick;
        let current_tick = self.pool.current_tick;
        let sqrt_price = self.pool.sqrt_price;

        // Calculate absolute delta (always positive)
        let abs_delta = liquidity_delta.unsigned_abs();

        // Check if we're decreasing more than available
        if !is_increase {
            require!(
                position.liquidity >= abs_delta,
                ErrorCode::PositionLiquidityTooLow
            );
        }

        // Update the position's liquidity
        if is_increase {
            position.liquidity = position
                .liquidity
                .checked_add(abs_delta)
                .ok_or(ErrorCode::MathOverflow)?;
        } else {
            position.liquidity -= abs_delta;
        }

        // Update the ticks at position boundaries
        let signed_delta = if is_increase {
            abs_delta as i128
        } else {
            -(abs_delta as i128)
        };

        // Update lower tick
        let lower = self.get_or_create_tick(lower_tick);
        if is_increase {
            if !lower.initialized {
                lower.initialize(signed_delta);
            } else {
                lower.update(signed_delta);
            }
        } else {
            lower.remove_liquidity(signed_delta)?;
        }

        // Update upper tick with negative delta
        let upper = self.get_or_create_tick(upper_tick);
        if is_increase {
            if !upper.initialized {
                upper.initialize(-signed_delta);
            } else {
                upper.update(-signed_delta);
            }
        } else {
            upper.remove_liquidity(-signed_delta)?;
        }

        // Update global pool liquidity if position is in current price range
        if current_tick >= lower_tick && current_tick < upper_tick {
            if is_increase {
                self.pool.liquidity = self
                    .pool
                    .liquidity
                    .checked_add(abs_delta)
                    .ok_or(ErrorCode::MathOverflow)?;
            } else {
                self.pool.liquidity = self
                    .pool
                    .liquidity
                    .checked_sub(abs_delta)
                    .ok_or(ErrorCode::MathOverflow)?;
            }
        }

        // Calculate token amounts required for the position modification
        let (mut amount_a, mut amount_b) = self.calculate_token_amounts(
            lower_tick,
            upper_tick,
            abs_delta,
            sqrt_price,
            current_tick,
        )?;

        // If we're removing liquidity, we receive tokens, not pay them
        if !is_increase {
            // Calculate with exact amounts for withdrawals (rounding down)
            (amount_a, amount_b) = self.calculate_exact_token_amounts(
                lower_tick,
                upper_tick,
                abs_delta,
                sqrt_price,
                current_tick,
            )?;
        }

        Ok((amount_a, amount_b))
    }

    /// Calculate exact token amounts (for withdrawals, using floor rounding)
    fn calculate_exact_token_amounts(
        &self,
        lower_tick: i32,
        upper_tick: i32,
        liquidity: u128,
        sqrt_price: u128,
        current_tick: i32,
    ) -> Result<(u64, u64)> {
        // Similar to calculate_token_amounts but with floor rounding for withdrawals
        let sqrt_price_lower = math::tick_to_sqrt_price(lower_tick)?;
        let sqrt_price_upper = math::tick_to_sqrt_price(upper_tick)?;

        let mut amount_a: u64 = 0;
        let mut amount_b: u64 = 0;

        if current_tick < lower_tick {
            amount_a = math::get_amount_a_delta_for_price_range(
                liquidity,
                sqrt_price_lower,
                sqrt_price_upper,
                false, // Rounding down for withdrawals
            )? as u64;
        } else if current_tick >= upper_tick {
            amount_b = math::get_amount_b_delta_for_price_range(
                liquidity,
                sqrt_price_lower,
                sqrt_price_upper,
                false, // Rounding down for withdrawals
            )? as u64;
        } else {
            amount_a = math::get_amount_a_delta_for_price_range(
                liquidity,
                sqrt_price,
                sqrt_price_upper,
                false, // Rounding down for withdrawals
            )? as u64;

            amount_b = math::get_amount_b_delta_for_price_range(
                liquidity,
                sqrt_price_lower,
                sqrt_price,
                false, // Rounding down for withdrawals
            )? as u64;
        }

        Ok((amount_a, amount_b))
    }

    /// Update pool state when price crosses a tick boundary
    pub fn cross_tick(&mut self, tick_idx: i32) -> Result<i128> {
        // Find the tick we're crossing
        for (idx, tick) in &mut self.ticks {
            if *idx == tick_idx && tick.initialized {
                // Update fee growth tracking when crossing a tick
                tick.cross(self.pool.fee_growth_global_a, self.pool.fee_growth_global_b);

                // Return the liquidity delta to apply to global liquidity
                return Ok(tick.liquidity_net);
            }
        }

        // If we don't find the tick, return zero liquidity change
        Ok(0)
    }

    /// Process a price update in the pool, handling tick crossings
    pub fn update_price(&mut self, new_sqrt_price: u128, new_tick: i32) -> Result<()> {
        let old_tick = self.pool.current_tick;

        // Update pool state with new price
        self.pool.sqrt_price = new_sqrt_price;
        self.pool.current_tick = new_tick;

        // Handle tick crossings if price moves across tick boundaries
        match new_tick.cmp(&old_tick) {
            std::cmp::Ordering::Greater => {
                // Price moving up, cross all ticks in the range (old_tick, new_tick]
                for tick_idx in (old_tick + 1)..=new_tick {
                    let liquidity_delta = self.cross_tick(tick_idx)?;
                    if liquidity_delta != 0 {
                        self.pool.liquidity = if liquidity_delta > 0 {
                            self.pool
                                .liquidity
                                .checked_add(liquidity_delta as u128)
                                .ok_or(ErrorCode::MathOverflow)?
                        } else {
                            self.pool
                                .liquidity
                                .checked_sub((-liquidity_delta) as u128)
                                .ok_or(ErrorCode::MathOverflow)?
                        };
                    }
                }
            }
            std::cmp::Ordering::Less => {
                // Price moving down, cross all ticks in the range [new_tick + 1, old_tick]
                for tick_idx in ((new_tick + 1)..=old_tick).rev() {
                    let liquidity_delta = self.cross_tick(tick_idx)?;
                    if liquidity_delta != 0 {
                        // When moving down, we apply the negative of the liquidity delta
                        self.pool.liquidity = if liquidity_delta > 0 {
                            self.pool
                                .liquidity
                                .checked_sub(liquidity_delta as u128)
                                .ok_or(ErrorCode::MathOverflow)?
                        } else {
                            self.pool
                                .liquidity
                                .checked_add((-liquidity_delta) as u128)
                                .ok_or(ErrorCode::MathOverflow)?
                        };
                    }
                }
            }
            std::cmp::Ordering::Equal => {
                // No tick crossing if the tick hasn't changed
            }
        }

        Ok(())
    }

    /// Update accumulated fees in the pool
    ///
    /// This function updates the global fee accumulators when fees are collected from trades.
    /// It properly handles the scaling of fees according to available liquidity and ensures
    /// that even with zero liquidity, fee accounting remains consistent.
    ///
    /// # Arguments
    /// * `fee_amount_a` - Amount of token A fees to add to global accumulators
    /// * `fee_amount_b` - Amount of token B fees to add to global accumulators
    ///
    /// # Returns
    /// * `Result<()>` - Result indicating success or containing an error code
    pub fn update_fees(&mut self, fee_amount_a: u64, fee_amount_b: u64) -> Result<()> {
        // If there's no active liquidity, we don't update fee growth
        // as there are no LPs to attribute the fees to
        if self.pool.liquidity == 0 {
            return Ok(());
        }

        // Calculate protocol fees (if applicable)
        let protocol_fee_bps = self.pool.protocol_fee;
        let protocol_fee_a = if protocol_fee_bps > 0 {
            (fee_amount_a as u128)
                .checked_mul(protocol_fee_bps as u128)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(PROTOCOL_FEE_DENOMINATOR as u128)
                .ok_or(ErrorCode::MathOverflow)? as u64
        } else {
            0
        };

        let protocol_fee_b = if protocol_fee_bps > 0 {
            (fee_amount_b as u128)
                .checked_mul(protocol_fee_bps as u128)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(PROTOCOL_FEE_DENOMINATOR as u128)
                .ok_or(ErrorCode::MathOverflow)? as u64
        } else {
            0
        };

        // Calculate LP fees (total fees minus protocol fees)
        let lp_fee_amount_a = fee_amount_a.saturating_sub(protocol_fee_a);
        let lp_fee_amount_b = fee_amount_b.saturating_sub(protocol_fee_b);

        // Convert to Q64.64 fixed-point with proper scaling
        // Convert u64 to u128 using into() before passing to U128::from
        let fee_growth_a_delta = U128::from(lp_fee_amount_a as u128)
            .mul_div_floor(U128::from(Q64), U128::from(self.pool.liquidity))
            .map_err(|_| ErrorCode::MathOverflow)?;

        let fee_growth_b_delta = U128::from(lp_fee_amount_b as u128)
            .mul_div_floor(U128::from(Q64), U128::from(self.pool.liquidity))
            .map_err(|_| ErrorCode::MathOverflow)?;

        // Add to global fee growth accumulator using wrapping addition to handle overflow
        self.pool.fee_growth_global_a = self
            .pool
            .fee_growth_global_a
            .wrapping_add(fee_growth_a_delta.as_u128());

        self.pool.fee_growth_global_b = self
            .pool
            .fee_growth_global_b
            .wrapping_add(fee_growth_b_delta.as_u128());

        // Store protocol fees for later collection (in a separate ProtocolFeeCollect instruction)
        // For the hackathon implementation, protocol fees can be added to a tracking account
        // For now, we'll just emit an event for transparency
        if protocol_fee_a > 0 || protocol_fee_b > 0 {
            // Get the account info for the pool to access its public key
            // We can't directly call key() on &mut Pool, so we use the authority field's public key
            // as an identifier, which is safe since each pool has a unique authority
            emit!(ProtocolFeeEvent {
                pool: self.pool.authority,
                token_a_amount: protocol_fee_a,
                token_b_amount: protocol_fee_b,
                timestamp: Clock::get()?.unix_timestamp,
            });
        }

        Ok(())
    }

    /// Calculate current virtual reserves for the pool
    ///
    /// Virtual reserves represent the effective amounts of tokens at the current price point,
    /// which are useful for price impact calculations and other analytics.
    ///
    /// # Returns
    /// * `Result<(u64, u64)>` - A tuple of (virtual_reserve_a, virtual_reserve_b), or an error
    pub fn get_virtual_reserves(&self) -> Result<(u64, u64)> {
        // If the pool doesn't have any liquidity, return zeros
        if self.pool.liquidity == 0 {
            return Ok((0, 0));
        }

        // Use the math library to calculate virtual reserves from current liquidity and sqrt price
        Ok(math::calculate_virtual_reserves(
            self.pool.liquidity,
            self.pool.sqrt_price,
        )?)
    }

    /// Calculate virtual reserve of token A
    ///
    /// # Returns
    /// * `Result<u64>` - The virtual reserve of token A, or an error
    pub fn get_virtual_reserve_a(&self) -> Result<u64> {
        Ok(math::calculate_virtual_reserve_a(
            self.pool.liquidity,
            self.pool.sqrt_price,
        )?)
    }

    /// Calculate virtual reserve of token B
    ///
    /// # Returns
    /// * `Result<u64>` - The virtual reserve of token B, or an error
    pub fn get_virtual_reserve_b(&self) -> Result<u64> {
        Ok(math::calculate_virtual_reserve_b(
            self.pool.liquidity,
            self.pool.sqrt_price,
        )?)
    }

    /// Verifies that pool state satisfies the constant product formula
    ///
    /// This is useful for validating that the pool state is consistent after operations.
    /// The constant product formula (x * y = k) should hold for virtual reserves,
    /// where k is approximately equal to the square of the pool's liquidity.
    ///
    /// # Returns
    /// * `bool` - True if the virtual reserves match the constant product formula
    pub fn verify_constant_product(&self) -> bool {
        // If the pool doesn't have any liquidity, the test passes trivially
        if self.pool.liquidity == 0 {
            return true;
        }

        // Get virtual reserves
        if let Ok((reserve_a, reserve_b)) = self.get_virtual_reserves() {
            // Check if they satisfy the constant product formula using the math utility function
            math::verify_virtual_reserves_invariant(reserve_a, reserve_b, self.pool.liquidity)
        } else {
            // If we can't calculate virtual reserves, the invariant check fails
            false
        }
    }

    /// Find the nearest initialized tick below the current tick
    #[allow(dead_code)]
    fn find_nearest_lower_initialized_tick(&self, current_tick: i32) -> i32 {
        let mut nearest_lower = current_tick;
        let mut found_initialized = false;

        for (tick_idx, tick) in &self.ticks {
            if *tick_idx < current_tick
                && tick.initialized
                && (!found_initialized || *tick_idx > nearest_lower)
            {
                nearest_lower = *tick_idx;
                found_initialized = true;
            }
        }

        // If we didn't find an initialized tick below, use protocol minimum
        if !found_initialized {
            nearest_lower = MIN_TICK;
        }

        nearest_lower
    }

    /// Find the nearest initialized tick above the current tick
    #[allow(dead_code)]
    fn find_nearest_upper_initialized_tick(&self, current_tick: i32) -> i32 {
        let mut nearest_upper = current_tick;
        let mut found_initialized = false;

        for (tick_idx, tick) in &self.ticks {
            if *tick_idx > current_tick
                && tick.initialized
                && (!found_initialized || *tick_idx < nearest_upper)
            {
                nearest_upper = *tick_idx;
                found_initialized = true;
            }
        }

        // If we didn't find an initialized tick above, use protocol maximum
        if !found_initialized {
            nearest_upper = MAX_TICK;
        }

        nearest_upper
    }

    /// Calculate price impact of a swap based on virtual reserves
    ///
    /// # Parameters
    /// * `amount_in` - Amount of input token
    /// * `is_token_a` - Whether the input is token A (true) or token B (false)
    ///
    /// # Returns
    /// * `Result<u64>` - Price impact in basis points (e.g., 100 = 1%)
    pub fn calculate_price_impact(&self, amount_in: u64, is_token_a: bool) -> Result<u64> {
        // Get current virtual reserves
        let (reserve_a, reserve_b) = self.get_virtual_reserves()?;

        // Avoid division by zero
        if reserve_a == 0 || reserve_b == 0 {
            return Ok(0);
        }

        // Calculate k = reserve_a * reserve_b before swap
        let k_before = reserve_a as u128 * reserve_b as u128;

        // Calculate new reserves after swap (simplified, ignoring fees)
        let (new_reserve_a, new_reserve_b) = if is_token_a {
            // Swapping token A for token B
            let new_a = reserve_a.saturating_add(amount_in);
            // B = k / A for constant product
            let new_b = (k_before / new_a as u128) as u64;
            (new_a, new_b)
        } else {
            // Swapping token B for token A
            let new_b = reserve_b.saturating_add(amount_in);
            // A = k / B for constant product
            let new_a = (k_before / new_b as u128) as u64;
            (new_a, new_b)
        };

        // Calculate price before: reserve_b / reserve_a
        let price_before = (reserve_b as f64) / (reserve_a as f64);

        // Calculate price after: new_reserve_b / new_reserve_a
        let price_after = (new_reserve_b as f64) / (new_reserve_a as f64);

        // Calculate price change as a percentage
        let price_change_pct = ((price_after - price_before) / price_before).abs() * 100.0;

        // Convert to basis points (1% = 100 basis points)
        let impact_bps = (price_change_pct * 100.0) as u64;

        Ok(impact_bps)
    }

    /// Examine impermanent loss implications given the current virtual reserves
    /// compared to the initial position's virtual reserves
    ///
    /// # Parameters
    /// * `initial_sqrt_price` - The sqrt price when position was created
    /// * `initial_liquidity` - The liquidity when position was created
    /// * `current_tick` - The current tick index
    ///
    /// # Returns
    /// * `Result<(u64, f64)>` - (IL amount in smallest token units, IL percentage)
    pub fn estimate_impermanent_loss(
        &self,
        initial_sqrt_price: u128,
        initial_liquidity: u128,
    ) -> Result<(u64, f64)> {
        if initial_liquidity == 0 {
            return Ok((0, 0.0));
        }

        // Calculate initial virtual reserves
        let (init_virt_a, init_virt_b) =
            math::calculate_virtual_reserves(initial_liquidity, initial_sqrt_price)?;

        // Calculate current virtual reserves for same liquidity
        let current_sqrt_price = self.pool.sqrt_price;
        let (curr_virt_a, curr_virt_b) =
            math::calculate_virtual_reserves(initial_liquidity, current_sqrt_price)?;

        // Calculate hodl value (if user just held the initial tokens)
        let init_value_a = init_virt_a;
        let init_value_b = init_virt_b;

        // Calculate current LP value using the current price
        let current_value_a = curr_virt_a;
        let current_value_b = curr_virt_b;

        // Convert to a common denominator using current price
        let price_in_a_terms = (current_sqrt_price as f64 / math::Q96 as f64).powi(2);

        // Calculate total values in terms of token A
        let hodl_value = init_value_a as f64 + (init_value_b as f64 * price_in_a_terms);
        let lp_value = current_value_a as f64 + (current_value_b as f64 * price_in_a_terms);

        // Calculate IL as percentage
        let il_percentage = if hodl_value > 0.0 {
            ((hodl_value - lp_value) / hodl_value) * 100.0
        } else {
            0.0
        };

        // Calculate IL amount in token A units
        let il_amount = if hodl_value > lp_value {
            (hodl_value - lp_value) as u64
        } else {
            0
        };

        Ok((il_amount, il_percentage))
    }

    /// Get virtual reserves for a specific position
    ///
    /// # Parameters
    /// * `position` - The position to calculate virtual reserves for
    ///
    /// # Returns
    /// * `Result<(u64, u64)>` - Virtual reserves for the position
    pub fn get_position_virtual_reserves(&self, position: &Position) -> Result<(u64, u64)> {
        let sqrt_price = self.pool.sqrt_price;
        let current_tick = self.pool.current_tick;
        let position_liquidity = position.liquidity;

        // If there's no liquidity in the position, there are no virtual reserves
        if position_liquidity == 0 {
            return Ok((0, 0));
        }

        // Get position boundaries
        let lower_tick = position.lower_tick;
        let upper_tick = position.upper_tick;

        // Convert ticks to sqrt prices
        let sqrt_price_lower = math::tick_to_sqrt_price(lower_tick)?;
        let sqrt_price_upper = math::tick_to_sqrt_price(upper_tick)?;

        // Calculate virtual reserves based on position's state relative to current price
        let (virtual_reserve_a, virtual_reserve_b) = if current_tick < lower_tick {
            // Position is entirely above current price
            // Only token A is in the position
            let virt_a = math::calculate_virtual_reserve_a(position_liquidity, sqrt_price_lower)?;
            (virt_a, 0)
        } else if current_tick >= upper_tick {
            // Position is entirely below current price
            // Only token B is in the position
            let virt_b = math::calculate_virtual_reserve_b(position_liquidity, sqrt_price_upper)?;
            (0, virt_b)
        } else {
            // Current price is within position bounds
            // Calculate partial virtual reserves for both tokens
            let virt_a = math::get_amount_a_delta_for_price_range(
                position_liquidity,
                sqrt_price,
                sqrt_price_upper,
                false,
            )? as u64;

            let virt_b = math::get_amount_b_delta_for_price_range(
                position_liquidity,
                sqrt_price_lower,
                sqrt_price,
                false,
            )? as u64;

            (virt_a, virt_b)
        };

        Ok((virtual_reserve_a, virtual_reserve_b))
    }

    /// Calculate the virtual reserves for a specific tick range
    ///
    /// # Parameters
    /// * `lower_tick` - Lower tick of the range
    /// * `upper_tick` - Upper tick of the range
    ///
    /// # Returns
    /// * `Result<(u64, u64)>` - Virtual reserves in the range as (token_a, token_b)
    pub fn get_virtual_reserves_in_range(
        &self,
        lower_tick: i32,
        upper_tick: i32,
    ) -> Result<(u64, u64)> {
        // Get the sqrt prices for the ticks
        let lower_sqrt_price = math::tick_to_sqrt_price(lower_tick)?;
        let upper_sqrt_price = math::tick_to_sqrt_price(upper_tick)?;

        // Get the liquidity in the range
        let liquidity_in_range = self.get_liquidity_in_range(lower_tick, upper_tick)?;

        Ok(math::calculate_virtual_reserves_in_range(
            liquidity_in_range,
            self.pool.sqrt_price,
            lower_sqrt_price,
            upper_sqrt_price,
        )?)
    }

    /// Get the liquidity in a specific tick range
    ///
    /// This function calculates how much liquidity is currently available in a given
    /// tick range. It's used for virtual reserve calculations and range-specific queries.
    ///
    /// # Parameters
    /// * `lower_tick` - Lower tick of the range
    /// * `upper_tick` - Upper tick of the range
    ///
    /// # Returns
    /// * `Result<u128>` - Liquidity in the range
    fn get_liquidity_in_range(&self, lower_tick: i32, upper_tick: i32) -> Result<u128> {
        // In a full concentrated liquidity AMM implementation, this would need to:
        // 1. Track liquidity at each initialized tick boundary
        // 2. Aggregate it across the specified range
        // 3. Account for user positions properly

        // For this implementation, we check if the current price is within the range
        // If so, we return the pool's total liquidity, otherwise zero

        let current_tick = self.pool.current_tick;

        if current_tick >= lower_tick && current_tick < upper_tick {
            Ok(self.pool.liquidity)
        } else {
            // If the current price is outside the range, there's no active liquidity
            Ok(0)
        }
    }

    /// Calculate the virtual reserves for the entire pool based on current liquidity and price
    pub fn calculate_virtual_reserves(&self) -> Result<(u64, u64)> {
        Ok(math::calculate_virtual_reserves(
            self.pool.liquidity,
            self.pool.sqrt_price,
        )?)
    }

    /// Calculate the virtual reserves for a specific price range
    pub fn calculate_virtual_reserves_in_range(
        &self,
        lower_tick: i32,
        upper_tick: i32,
    ) -> Result<(u64, u64)> {
        let liquidity_in_range = self.get_liquidity_in_range(lower_tick, upper_tick)?;
        let lower_sqrt_price = math::tick_to_sqrt_price(lower_tick)?;
        let upper_sqrt_price = math::tick_to_sqrt_price(upper_tick)?;

        Ok(math::calculate_virtual_reserves_in_range(
            liquidity_in_range,
            self.pool.sqrt_price,
            lower_sqrt_price,
            upper_sqrt_price,
        )?)
    }

    /// Get virtual reserves ratio (price)
    pub fn get_virtual_reserves_ratio(&self) -> Result<f64> {
        let (reserve_a, reserve_b) = self.get_virtual_reserves()?;

        if reserve_a == 0 {
            return Err(ErrorCode::ZeroReserveAmount.into());
        }

        Ok(reserve_b as f64 / reserve_a as f64)
    }

    /// Calculate fee growth inside a specific tick range
    ///
    /// This function is used to determine how much fee growth has occurred
    /// within a position's tick range, accounting for fee growth that
    /// happened outside the range.
    ///
    /// # Parameters
    /// * `params` - The fee growth parameters
    ///
    /// # Returns
    /// * `(u128, u128)` - The fee growth inside the range for tokens A and B
    pub fn compute_fee_growth_inside(params: &FeeGrowthParams) -> (u128, u128) {
        // Delegate to the standalone function that already uses the FeeGrowthParams struct
        compute_fee_growth_inside(params)
    }

    /// Gets the appropriate tick spacing for a fee tier
    ///
    /// Different fee tiers have different tick spacings to balance
    /// gas costs with price precision.
    ///
    /// # Parameters
    /// * `fee_tier` - The fee tier in basis points
    ///
    /// # Returns
    /// * `Result<i32>` - The tick spacing for this fee tier or an error
    pub fn get_tick_spacing_for_fee_tier(fee_tier: u16) -> Result<i32> {
        match fee_tier {
            FEE_TIER_LOW => Ok(TICK_SPACING_LOW),
            FEE_TIER_MEDIUM => Ok(TICK_SPACING_MEDIUM),
            FEE_TIER_HIGH => Ok(TICK_SPACING_HIGH),
            _ => Err(ErrorCode::InvalidFeeTier.into()),
        }
    }

    /// Initialize or update a tick in the bitmap
    pub fn update_tick_bitmap(&mut self, tick_index: i32, initialized: bool) -> Result<()> {
        // Get the tick spacing from the pool's fee tier
        let tick_spacing = Self::get_tick_spacing_for_fee_tier(self.pool.fee_tier)? as u16;

        // Update the bitmap
        self.tick_bitmap
            .update_bitmap(tick_index, tick_spacing, initialized)
    }

    /// Find the next initialized tick using the bitmap
    pub fn next_initialized_tick(&self, tick: i32) -> Result<i32> {
        let tick_spacing = Self::get_tick_spacing_for_fee_tier(self.pool.fee_tier)? as u16;
        self.tick_bitmap.next_initialized_tick(tick, tick_spacing)
    }

    /// Find the previous initialized tick using the bitmap
    pub fn prev_initialized_tick(&self, tick: i32) -> Result<i32> {
        let tick_spacing = Self::get_tick_spacing_for_fee_tier(self.pool.fee_tier)? as u16;
        self.tick_bitmap.prev_initialized_tick(tick, tick_spacing)
    }
}

/// Event emitted when protocol fees are collected
#[event]
pub struct ProtocolFeeEvent {
    /// The pool where the fees were collected
    pub pool: Pubkey,

    /// The amount of token A protocol fees
    pub token_a_amount: u64,

    /// The amount of token B protocol fees
    pub token_b_amount: u64,

    /// The timestamp when the fee was recorded
    pub timestamp: i64,
}

/// Helper struct for 128-bit unsigned math operations
///
/// This implementation provides safe arithmetic operations for u128 values,
/// particularly for the fixed-point math used in AMM calculations.
pub struct U128(u128);

impl U128 {
    /// Create from a u128 value
    pub fn from(value: u128) -> Self {
        Self(value)
    }

    /// Convert to u64, checking for overflow
    pub fn as_u64(&self) -> u64 {
        assert!(self.0 <= u64::MAX as u128, "Value exceeds u64 range");
        self.0 as u64
    }

    /// Convert to u128
    pub fn as_u128(&self) -> u128 {
        self.0
    }

    /// Multiply and then divide, with floor rounding
    pub fn mul_div_floor(&self, mul: Self, div: Self) -> Result<Self> {
        if div.0 == 0 {
            return Err(ErrorCode::MathOverflow.into());
        }

        // For small enough values, we can do direct multiplication and division
        if self.0 <= U128MAX / mul.0 {
            return Ok(Self(self.0 * mul.0 / div.0));
        }

        // For larger values, we need to be careful about overflow
        // Use a standard technique: (a * b) / c = (a / c) * b + (a % c) * b / c
        let q = self.0 / div.0;
        let r = self.0 % div.0;

        // Calculate (q * mul) + (r * mul / div) with overflow checking
        let q_mul = q.checked_mul(mul.0).ok_or(ErrorCode::MathOverflow)?;
        let r_mul = r.checked_mul(mul.0).ok_or(ErrorCode::MathOverflow)?;
        let r_mul_div = r_mul / div.0;

        q_mul
            .checked_add(r_mul_div)
            .map(Self)
            .ok_or(ErrorCode::MathOverflow.into())
    }
}
