use crate::constants::BPS_DENOMINATOR;
use crate::constants::MAX_SQRT_PRICE;
use crate::errors::ErrorCode;
use crate::math;
use crate::tick::TickData;
use crate::tick_bitmap;
use anchor_lang::prelude::{AccountLoader, *}; // Added AccountLoader
use std::collections::BTreeMap; // MIN_SQRT_PRICE is 0, handled by direct check

/// Maximum expected size for the serialized tick_bitmap_data in bytes.
const MAX_SERIALIZED_BITMAP_BYTES: usize = 1280; // Based on original LEN: (2+8)*128

/// Defines the state for a liquidity pool in the Fluxa AMM.
///
/// For the MVP, this struct holds the core attributes necessary for pool
/// operation, including token information, fee parameters, current price state,
/// liquidity, and a simplified tick bitmap stored directly in the account.
#[account]
#[derive(Default, Debug)]
pub struct Pool {
    /// Bump seed for PDA.
    pub bump: u8,
    /// The factory that created this pool.
    /// Can be a placeholder (e.g., system_program) for MVP if no factory instruction.
    pub factory: Pubkey,
    /// The mint address of the first token (token0).
    pub token0_mint: Pubkey,
    /// The mint address of the second token (token1).
    pub token1_mint: Pubkey,
    /// The vault holding token0 for this pool.
    pub token0_vault: Pubkey,
    /// The vault holding token1 for this pool.
    pub token1_vault: Pubkey,
    /// Fee rate in basis points (e.g., 30 for 0.3%).
    pub fee_rate: u16,
    /// The spacing between usable ticks.
    pub tick_spacing: u16,
    /// The current square root of the price, in Q64.64 fixed-point format (sqrt(P) * 2^64).
    pub sqrt_price_q64: u128,
    /// The current tick index.
    pub current_tick: i32,
    /// The total active liquidity within the current tick's price range.
    pub liquidity: u128,
    /// Stores initialized tick data directly for MVP simplicity.
    /// Serialized BTreeMap<i16, u64> mapping compressed_tick_word_index to the bitmap.
    pub tick_bitmap_data: Vec<u8>,
    // MVP Simplification: Skipping fee_growth_global_..., protocol_fees_..., oracle_...
}

/// Parameters for initializing a new pool.
#[derive(Clone)]
pub struct InitializePoolParams {
    pub bump: u8,
    pub factory: Pubkey,
    pub token0_mint: Pubkey,
    pub token1_mint: Pubkey,
    pub token0_vault: Pubkey,
    pub token1_vault: Pubkey,
    pub initial_sqrt_price_q64: u128,
    pub fee_rate: u16,
    pub tick_spacing: u16,
}

impl<'info> Pool {
    /// The size of the Pool account in bytes.
    pub const LEN: usize = 8 // discriminator
        + 1 // bump
        + 32 // factory
        + 32 // token0_mint
        + 32 // token1_mint
        + 32 // token0_vault
        + 32 // token1_vault
        + 2 // fee_rate
        + 2 // tick_spacing
        + 16 // sqrt_price_q64
        + 4 // current_tick
        + 16 // liquidity
        + 4 + MAX_SERIALIZED_BITMAP_BYTES; // tick_bitmap_data: Vec<u8> (4 for len + data)

    /// Initializes the state of a new pool.
    ///
    /// # Arguments
    /// * `bump` - The bump seed for the pool's PDA.
    /// * `factory` - The Pubkey of the factory that created this pool.
    /// * `token0_mint` - Mint of the first token.
    /// * `token1_mint` - Mint of the second token.
    /// * `token0_vault` - Vault for the first token.
    /// * `token1_vault` - Vault for the second token.
    /// * `initial_sqrt_price_q64` - The initial sqrt price for the pool.
    /// * `fee_rate` - The fee rate for swaps in this pool, in basis points.
    /// * `tick_spacing` - The tick spacing for this pool.
    pub fn initialize(&mut self, params: InitializePoolParams) -> Result<()> {
        if params.token0_mint == params.token1_mint {
            return err!(ErrorCode::MintsMustDiffer);
        }
        if params.initial_sqrt_price_q64 == 0 || params.initial_sqrt_price_q64 > MAX_SQRT_PRICE {
            return err!(ErrorCode::InvalidInitialPrice);
        }
        if params.tick_spacing == 0 {
            return err!(ErrorCode::InvalidTickSpacing);
        }

        self.bump = params.bump;
        self.factory = params.factory;
        self.token0_mint = params.token0_mint;
        self.token1_mint = params.token1_mint;
        self.token0_vault = params.token0_vault;
        self.token1_vault = params.token1_vault;
        self.fee_rate = params.fee_rate;
        self.tick_spacing = params.tick_spacing;
        self.sqrt_price_q64 = params.initial_sqrt_price_q64;
        self.current_tick = math::sqrt_price_q64_to_tick(params.initial_sqrt_price_q64)?;
        self.liquidity = 0;
        self.tick_bitmap_data = borsh::to_vec(&BTreeMap::<i16, u64>::new())
            .expect("Failed to serialize empty BTreeMap");

        Ok(())
    }

    /// Updates a tick's state after a liquidity change and flips its status in the bitmap.
    ///
    /// # Arguments
    /// * `tick_index` - The index of the tick being updated.
    /// * `liquidity_delta` - The change in liquidity affecting this tick.
    /// * `is_upper_tick` - True if this tick is the upper boundary of the position.
    /// * `tick_data_account` - The account holding the data for the tick.
    fn _process_tick_liquidity_change(
        &mut self,
        tick_index: i32,
        liquidity_delta: i128,
        is_upper_tick: bool,
        tick_data: &mut TickData, // Changed to take &mut TickData directly
    ) -> Result<()> {
        let mut map: BTreeMap<i16, u64> =
            borsh::BorshDeserialize::try_from_slice(&self.tick_bitmap_data)
                .expect("Failed to deserialize tick_bitmap_data");

        tick_data.update_on_liquidity_change(liquidity_delta, is_upper_tick)?;

        tick_bitmap::flip_tick_initialized_status(
            &mut map,
            tick_index,
            self.tick_spacing,
            tick_data.initialized != 0,
        )?;
        self.tick_bitmap_data = borsh::to_vec(&map).expect("Failed to serialize tick_bitmap_data");
        Ok(())
    }

    /// Modifies liquidity for a given range, updating ticks and pool liquidity.
    ///
    /// # Arguments
    /// * `tick_lower_index` - The lower tick boundary of the position.
    /// * `tick_upper_index` - The upper tick boundary of the position.
    /// * `liquidity_delta` - The change in liquidity (positive to add, negative to remove).
    /// * `tick_lower_data` - Account for the lower tick's data.
    /// * `tick_upper_data` - Account for the upper tick's data.
    pub fn modify_liquidity(
        &mut self,
        tick_lower_index: i32,
        tick_upper_index: i32,
        liquidity_delta: i128,
        tick_lower_loader: &AccountLoader<'info, TickData>,
        tick_upper_loader: &AccountLoader<'info, TickData>,
    ) -> Result<()> {
        let mut tick_lower_data = tick_lower_loader.load_mut()?;
        let mut tick_upper_data = tick_upper_loader.load_mut()?;

        // Update the lower tick
        self._process_tick_liquidity_change(
            tick_lower_index,
            liquidity_delta,
            false, // Not an upper tick
            &mut tick_lower_data,
        )?;

        // Update the upper tick
        self._process_tick_liquidity_change(
            tick_upper_index,
            liquidity_delta,
            true, // Is an upper tick
            &mut tick_upper_data,
        )?;

        // If the current price is within the modified range, update pool's active liquidity
        if self.current_tick >= tick_lower_index && self.current_tick < tick_upper_index {
            if liquidity_delta > 0 {
                self.liquidity = self
                    .liquidity
                    .checked_add(liquidity_delta as u128)
                    .ok_or_else(|| error!(ErrorCode::MathOverflow))?;
            } else if liquidity_delta < 0 {
                // liquidity_delta is negative, so unsigned_abs gives the positive amount to subtract
                // We need to convert it to u128 for subtraction
                let liq_delta_abs = liquidity_delta.unsigned_abs();
                self.liquidity = self
                    .liquidity
                    .checked_sub(liq_delta_abs)
                    .ok_or_else(|| error!(ErrorCode::MathOverflow))?;
            }
            // If liquidity_delta is 0, self.liquidity remains unchanged.
        }
        Ok(())
    }

    /// Test-only version of modify_liquidity that accepts `&mut TickData` directly.
    /// This allows testing with `MockAccount<TickData>` by passing its inner `data`.
    #[cfg(test)]
    pub fn modify_liquidity_for_test(
        &mut self,
        tick_lower_index: i32,
        tick_upper_index: i32,
        liquidity_delta: i128,
        tick_lower_data: &mut TickData, // Accepts &mut TickData
        tick_upper_data: &mut TickData, // Accepts &mut TickData
    ) -> Result<()> {
        let mut map: BTreeMap<i16, u64> =
            borsh::BorshDeserialize::try_from_slice(&self.tick_bitmap_data)
                .expect("Failed to deserialize tick_bitmap_data for test");

        // Update the lower tick
        tick_lower_data.update_on_liquidity_change(liquidity_delta, false)?;
        tick_bitmap::flip_tick_initialized_status(
            &mut map,
            tick_lower_index,
            self.tick_spacing,
            tick_lower_data.initialized != 0,
        )?;

        // Update the upper tick
        tick_upper_data.update_on_liquidity_change(liquidity_delta, true)?;
        tick_bitmap::flip_tick_initialized_status(
            &mut map,
            tick_upper_index,
            self.tick_spacing,
            tick_upper_data.initialized != 0,
        )?;

        self.tick_bitmap_data =
            borsh::to_vec(&map).expect("Failed to serialize tick_bitmap_data for test");

        if self.current_tick >= tick_lower_index && self.current_tick < tick_upper_index {
            if liquidity_delta > 0 {
                self.liquidity = self
                    .liquidity
                    .checked_add(liquidity_delta as u128)
                    .ok_or_else(|| error!(ErrorCode::MathOverflow))?;
            } else if liquidity_delta < 0 {
                self.liquidity = self
                    .liquidity
                    .checked_sub(liquidity_delta.unsigned_abs())
                    .ok_or_else(|| error!(ErrorCode::MathOverflow))?;
            }
            // If liquidity_delta is 0, self.liquidity remains unchanged.
        }
        Ok(())
    }

    /// Calculates the result of a single swap step.
    ///
    /// # Arguments
    /// * `sqrt_price_current_q64` - The current sqrt price.
    /// * `sqrt_price_target_q64` - The target sqrt price for this step (e.g., next tick or price limit).
    /// * `step_liquidity` - The liquidity available for this step.
    /// * `amount_remaining_gross_input` - The gross amount of input token remaining to be swapped.
    /// * `fee_rate_bps` - The fee rate in basis points.
    /// * `zero_for_one` - True if swapping token0 for token1, false otherwise.
    ///
    /// # Returns
    /// A tuple: `(gross_amount_in_consumed, net_amount_out_produced, next_sqrt_price_q64)`
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn swap_step(
        &self,
        sqrt_price_current_q64: u128,
        sqrt_price_target_q64: u128,
        step_liquidity: u128,
        amount_remaining_gross_input: u128,
        fee_rate_bps: u16,
        zero_for_one: bool,
    ) -> Result<(u128, u128, u128)> {
        if step_liquidity == 0 {
            return Ok((0, 0, sqrt_price_current_q64));
        }

        let exact_input = true; // For MVP, assuming exact input

        let gross_amount_in_consumed: u128;
        let net_amount_out_produced: u128;
        let next_sqrt_price_q64: u128;

        if exact_input {
            // Calculate net input after fee
            let fee_rate_u128 = fee_rate_bps as u128;
            let net_amount_remaining_input = amount_remaining_gross_input
                .checked_mul(
                    BPS_DENOMINATOR
                        .checked_sub(fee_rate_u128)
                        .ok_or(ErrorCode::MathOverflow)?,
                )
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(BPS_DENOMINATOR)
                .ok_or(ErrorCode::MathOverflow)?; // floor division

            // Calculate max net input to reach target price
            let max_net_input_to_reach_target = if zero_for_one {
                // Swapping token0 for token1, price decreases. Target is lower or equal.
                math::get_amount_0_delta(
                    sqrt_price_target_q64,  // lower bound for delta calc
                    sqrt_price_current_q64, // upper bound for delta calc
                    step_liquidity,
                    true, // round up input
                )?
            } else {
                // Swapping token1 for token0, price increases. Target is higher or equal.
                math::get_amount_1_delta(
                    sqrt_price_current_q64, // lower bound for delta calc
                    sqrt_price_target_q64,  // upper bound for delta calc
                    step_liquidity,
                    true, // round up input
                )?
            };

            if net_amount_remaining_input >= max_net_input_to_reach_target {
                // Can reach target price
                let net_amount_in_consumed = max_net_input_to_reach_target;
                gross_amount_in_consumed = math::round_up_div(
                    net_amount_in_consumed
                        .checked_mul(BPS_DENOMINATOR)
                        .ok_or(ErrorCode::MathOverflow)?,
                    BPS_DENOMINATOR
                        .checked_sub(fee_rate_u128)
                        .ok_or(ErrorCode::MathOverflow)?,
                );
                next_sqrt_price_q64 = sqrt_price_target_q64;
            } else {
                // Cannot reach target price, limited by remaining input
                let net_amount_in_consumed = net_amount_remaining_input;
                gross_amount_in_consumed = amount_remaining_gross_input; // All remaining gross input is consumed

                next_sqrt_price_q64 = if zero_for_one {
                    math::compute_next_sqrt_price_from_amount0_in(
                        sqrt_price_current_q64,
                        step_liquidity,
                        net_amount_in_consumed, // Use net amount for price calculation
                    )?
                } else {
                    math::compute_next_sqrt_price_from_amount1_in(
                        sqrt_price_current_q64,
                        step_liquidity,
                        net_amount_in_consumed, // Use net amount for price calculation
                    )?
                };
            }

            // Calculate net_amount_out_produced based on the price change and liquidity
            net_amount_out_produced = if zero_for_one {
                math::get_amount_1_delta(
                    next_sqrt_price_q64,    // new lower bound
                    sqrt_price_current_q64, // old upper bound
                    step_liquidity,
                    false, // round down output
                )?
            } else {
                math::get_amount_0_delta(
                    sqrt_price_current_q64, // old lower bound
                    next_sqrt_price_q64,    // new upper bound
                    step_liquidity,
                    false, // round down output
                )?
            };
        } else {
            // TODO: Implement exact output logic if needed for future versions
            return err!(ErrorCode::InvalidInput); // Placeholder for not implemented
        }

        // If no input was consumed, no output should be produced, and price doesn't change.
        if gross_amount_in_consumed == 0 {
            return Ok((0, 0, sqrt_price_current_q64));
        }

        Ok((
            gross_amount_in_consumed,
            net_amount_out_produced,
            next_sqrt_price_q64,
        ))
    }

    /// Executes a swap.
    ///
    /// # Arguments
    /// * `zero_for_one` - True if swapping token0 for token1, false otherwise.
    /// * `amount_specified` - The gross amount of input token to swap. Must be positive.
    /// * `sqrt_price_limit_q64` - The price limit for the swap.
    /// * `tick_loaders` - A slice of `AccountLoader` for `TickData` accounts expected to be crossed.
    /// * `current_timestamp` - The current blockchain timestamp.
    pub fn swap(
        // Removed shadowed 'info lifetime
        &mut self,
        zero_for_one: bool,
        amount_specified: i128, // For exact_input, this will be positive.
        sqrt_price_limit_q64: u128,
        pool_key: &Pubkey, // Pass the pool's own key for validation
        tick_loaders: &[&AccountLoader<'info, TickData>],
        _current_timestamp: i64, // Parameter included, but not used in this MVP logic
    ) -> Result<(u128, u128)> {
        if amount_specified <= 0 {
            // For swap_exact_input, amount_specified should be positive.
            // If it could be negative (e.g. for swap_exact_output), this check would change.
            if amount_specified == 0 {
                return Ok((0, 0));
            } else {
                return err!(ErrorCode::InvalidInput); // Or a more specific error
            }
        }
        let amount_to_swap_gross: u128 = amount_specified.unsigned_abs();

        if amount_to_swap_gross == 0 {
            return Ok((0, 0));
        }

        let mut total_amount_in_gross: u128 = 0;
        let mut total_amount_out_net: u128 = 0;
        let mut amount_remaining_gross = amount_to_swap_gross;
        let mut current_sqrt_price_q64 = self.sqrt_price_q64;
        let mut current_tick_effective = self.current_tick;

        while amount_remaining_gross > 0 {
            if (zero_for_one && current_sqrt_price_q64 <= sqrt_price_limit_q64)
                || (!zero_for_one && current_sqrt_price_q64 >= sqrt_price_limit_q64)
            {
                break; // Price limit reached
            }

            let current_tick_bitmap: BTreeMap<i16, u64> =
                borsh::BorshDeserialize::try_from_slice(&self.tick_bitmap_data)
                    .expect("Failed to deserialize tick_bitmap for swap");

            let next_initialized_tick_index_opt = tick_bitmap::next_initialized_tick(
                &current_tick_bitmap,
                current_tick_effective,
                self.tick_spacing,
                zero_for_one,
            )?;

            let sqrt_price_at_next_tick_q64 =
                if let Some(tick_index) = next_initialized_tick_index_opt {
                    math::tick_to_sqrt_price_q64(tick_index)?
                } else {
                    // No more initialized ticks in this direction, so target is the overall limit
                    sqrt_price_limit_q64
                };

            let sqrt_price_target_for_step_q64 = if zero_for_one {
                // Price decreasing
                sqrt_price_at_next_tick_q64.max(sqrt_price_limit_q64)
            } else {
                // Price increasing
                sqrt_price_at_next_tick_q64.min(sqrt_price_limit_q64)
            };

            let (step_gross_in, step_net_out, next_step_sqrt_price_q64) = self.swap_step(
                current_sqrt_price_q64,
                sqrt_price_target_for_step_q64,
                self.liquidity,
                amount_remaining_gross,
                self.fee_rate,
                zero_for_one,
            )?;

            total_amount_in_gross = total_amount_in_gross
                .checked_add(step_gross_in)
                .ok_or(ErrorCode::MathOverflow)?;
            total_amount_out_net = total_amount_out_net
                .checked_add(step_net_out)
                .ok_or(ErrorCode::MathOverflow)?;
            amount_remaining_gross = amount_remaining_gross
                .checked_sub(step_gross_in)
                .ok_or(ErrorCode::MathOverflow)?;
            current_sqrt_price_q64 = next_step_sqrt_price_q64;

            // If no gross input was consumed in this step, it means no progress was made on the amount.
            // This can happen if, for example, the target price for the step was the current price,
            // or if liquidity for the step was zero (though self.liquidity is constant here for MVP).
            // Break to prevent an infinite loop if amount_remaining_gross is still > 0 (which is implied by the while loop condition).
            if step_gross_in == 0 {
                break;
            }

            if current_sqrt_price_q64 == sqrt_price_at_next_tick_q64
                && next_initialized_tick_index_opt.is_some()
            {
                let next_tick_idx = next_initialized_tick_index_opt.unwrap();
                let mut found_tick_loader: Option<&AccountLoader<'info, TickData>> = None;

                for loader in tick_loaders.iter() {
                    // Peek at the index without fully loading if possible, or load and check.
                    // For AccountLoader, we need to load to access its fields.
                    // This assumes the client provides the correct tick accounts.
                    // A more robust system might involve deriving the PDA for the tick
                    // and ensuring the provided account matches that PDA.
                    let tick_data_ref = loader.load()?;
                    if tick_data_ref.index == next_tick_idx && tick_data_ref.pool == *pool_key {
                        found_tick_loader = Some(loader);
                        break;
                    }
                }

                if let Some(tick_loader) = found_tick_loader {
                    let tick_data = tick_loader.load()?; // Load again or use already loaded ref
                    let liquidity_net_change = tick_data.liquidity_net;

                    msg!(
                        "Crossed tick {}, liquidity_net: {}. Current pool liquidity: {}",
                        next_tick_idx,
                        liquidity_net_change,
                        self.liquidity
                    );

                    // Update pool liquidity based on liquidity_net_change
                    // If zero_for_one (price decreasing), liquidity_net is subtracted.
                    // If !zero_for_one (price increasing), liquidity_net is added.
                    self.liquidity = (self.liquidity as i128)
                        .checked_add(if zero_for_one {
                            -liquidity_net_change
                        } else {
                            liquidity_net_change
                        })
                        .ok_or(ErrorCode::MathOverflow)?
                        as u128;
                } else {
                    // Critical: A tick indicated by the bitmap was not provided by the client.
                    // This is an error condition as the swap cannot proceed accurately.
                    return err!(ErrorCode::TickNotFound);
                }

                current_tick_effective = next_tick_idx;
            } else {
                // Did not reach the next tick, or no next tick, or hit price limit
                // The loop will break if amount_remaining_gross is 0 or price limit is hit.
            }
        }

        self.sqrt_price_q64 = current_sqrt_price_q64;
        self.current_tick = math::sqrt_price_q64_to_tick(self.sqrt_price_q64)?;

        Ok((total_amount_in_gross, total_amount_out_net))
    }
}
