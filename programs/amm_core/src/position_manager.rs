// Position Manager Module
//
// This module provides comprehensive position management functionality for Fluxa's AMM Core.
// It handles tracking, querying, and analyzing liquidity provider positions, supporting
// features like efficient position lookup, position metrics, and IL (Impermanent Loss) calculation.
//
// The position management system is designed to work seamlessly with the IL Mitigation module,
// providing the necessary data structures and interfaces for dynamic position adjustments.

use crate::errors::ErrorCode;
use crate::math;
use crate::pool_state::PoolState;
use crate::utils::price_range::calculate_impermanent_loss;
use crate::utils::price_range::PriceRange;
use crate::{Pool, Position};
use anchor_lang::prelude::*;
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Position snapshot for tracking position changes over time
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PositionSnapshot {
    /// Timestamp when the snapshot was taken
    pub timestamp: i64,

    /// The position's liquidity at snapshot time
    pub liquidity: u128,

    /// The pool's price at snapshot time
    pub price: u128,

    /// Total accumulated fees for token A
    pub fees_a: u64,

    /// Total accumulated fees for token B
    pub fees_b: u64,

    /// The position's value in terms of token A
    pub value_in_a: u64,

    /// The position's value in terms of token B
    pub value_in_b: u64,

    /// Estimated impermanent loss at snapshot time
    pub impermanent_loss_bps: u16,
}

/// Cached position data for analytics
struct PositionData {
    /// Position key for lookup
    key: Pubkey,
    /// Owner of the position
    owner: Pubkey,
    /// Lower tick boundary
    lower_tick: i32,
    /// Upper tick boundary
    upper_tick: i32,
}

/// Manages position tracking and analytics across a liquidity pool
///
/// This struct extends the basic position tracking in PoolState with more advanced
/// features like position lookup by owner, position metrics calculation, and
/// support for the IL mitigation system.
pub struct PositionManager<'a> {
    /// Reference to the pool state that this manager operates on
    pub pool_state: &'a mut PoolState<'a>,

    /// Position lookup by owner for efficient querying
    position_by_owner: HashMap<Pubkey, Vec<Pubkey>>,

    /// Position snapshots for historical comparison
    position_snapshots: HashMap<Pubkey, Vec<PositionSnapshot>>,

    /// Cached position data for analytics
    position_data: Vec<PositionData>,

    /// Latest price used for position valuation
    latest_price: u128,

    /// Latest price timestamp
    latest_price_timestamp: i64,
}

impl<'a> PositionManager<'a> {
    /// Creates a new PositionManager instance
    ///
    /// Initializes the position manager and builds the lookup tables for
    /// efficient position querying and analytics.
    pub fn new(pool_state: &'a mut PoolState<'a>) -> Result<Self> {
        let latest_price = pool_state.pool.sqrt_price;
        let latest_price_timestamp = Clock::get()?.unix_timestamp;

        let mut manager = PositionManager {
            pool_state,
            position_by_owner: HashMap::new(),
            position_snapshots: HashMap::new(),
            position_data: Vec::new(),
            latest_price,
            latest_price_timestamp,
        };

        // Build the lookup tables
        manager.rebuild_lookup_tables()?;

        Ok(manager)
    }

    /// Rebuilds all lookup tables for efficient position querying
    fn rebuild_lookup_tables(&mut self) -> Result<()> {
        self.position_by_owner.clear();
        self.position_data.clear();

        // First, build the position_data cache
        for position in self.pool_state.positions.iter() {
            // Clone necessary data first
            let key = self.generate_position_key_from_data(
                position.owner,
                position.pool,
                position.lower_tick,
                position.upper_tick,
            );

            let data = PositionData {
                key,
                owner: position.owner,
                lower_tick: position.lower_tick,
                upper_tick: position.upper_tick,
            };
            self.position_data.push(data);
        }

        // Then use the cache to build the owner lookup
        for data in &self.position_data {
            self.position_by_owner
                .entry(data.owner)
                .or_default()
                .push(data.key);
        }

        Ok(())
    }

    /// Generates a stable key for a position based on its properties
    fn generate_position_key(&self, position: &Position) -> Pubkey {
        self.generate_position_key_from_data(
            position.owner,
            position.pool,
            position.lower_tick,
            position.upper_tick,
        )
    }

    /// Generates a key from raw position data
    fn generate_position_key_from_data(
        &self,
        owner: Pubkey,
        pool: Pubkey,
        lower_tick: i32,
        upper_tick: i32,
    ) -> Pubkey {
        let mut hasher = DefaultHasher::new();
        owner.hash(&mut hasher);
        pool.hash(&mut hasher);
        lower_tick.hash(&mut hasher);
        upper_tick.hash(&mut hasher);

        // Create a deterministic key from the hash
        let hash = hasher.finish();
        let bytes = hash.to_be_bytes();
        let mut pubkey_bytes = [0u8; 32];

        // Use more efficient slice operations to copy bytes
        pubkey_bytes[..8].copy_from_slice(&bytes);
        pubkey_bytes[8..16].copy_from_slice(&bytes);
        pubkey_bytes[16..24].copy_from_slice(&bytes);
        pubkey_bytes[24..32].copy_from_slice(&bytes);

        Pubkey::new_from_array(pubkey_bytes)
    }

    /// Registers a new position with the manager
    pub fn register_position(&mut self, position: &Position) -> Result<()> {
        // Clone the position data to avoid borrowing issues
        let owner = position.owner;
        let pool = position.pool;
        let lower_tick = position.lower_tick;
        let upper_tick = position.upper_tick;
        let liquidity = position.liquidity;
        let tokens_owed_a = position.tokens_owed_a;
        let tokens_owed_b = position.tokens_owed_b;

        // Generate the position key from cloned data
        let position_key =
            self.generate_position_key_from_data(owner, pool, lower_tick, upper_tick);

        // Check if position is already in data cache
        let already_tracked = self
            .position_data
            .iter()
            .any(|data| data.key == position_key);

        if !already_tracked {
            // Add to data cache
            let position_data = PositionData {
                key: position_key,
                owner,
                lower_tick,
                upper_tick,
            };
            self.position_data.push(position_data);

            // Add to owner lookup
            self.position_by_owner
                .entry(owner)
                .or_default()
                .push(position_key);

            // Create initial snapshot
            let (value_a, value_b) = self.calculate_position_value_from_data(
                lower_tick,
                upper_tick,
                liquidity,
                tokens_owed_a,
                tokens_owed_b,
            )?;

            let snapshot = PositionSnapshot {
                timestamp: Clock::get()?.unix_timestamp,
                liquidity,
                price: self.latest_price,
                fees_a: tokens_owed_a,
                fees_b: tokens_owed_b,
                value_in_a: value_a,
                value_in_b: value_b,
                impermanent_loss_bps: 0,
            };

            self.position_snapshots
                .entry(position_key)
                .or_default()
                .push(snapshot);
        }

        Ok(())
    }

    /// Finds all positions owned by a specific account
    pub fn get_positions_by_owner(&self, owner: Pubkey) -> Vec<PositionInfo> {
        let mut results = Vec::new();

        // Get the position keys for this owner
        if let Some(keys) = self.position_by_owner.get(&owner) {
            for key in keys {
                if let Some(pos_idx) = self.find_position_index_by_key(*key) {
                    let position = &self.pool_state.positions[pos_idx];

                    results.push(PositionInfo {
                        key: *key,
                        owner: position.owner,
                        lower_tick: position.lower_tick,
                        upper_tick: position.upper_tick,
                        liquidity: position.liquidity,
                        tokens_owed_a: position.tokens_owed_a,
                        tokens_owed_b: position.tokens_owed_b,
                    });
                }
            }
        }

        results
    }

    /// Finds positions that are active in a specific price range
    pub fn get_positions_in_range(&self, min_price: u128, max_price: u128) -> Vec<PositionInfo> {
        let mut results = Vec::new();

        // Use the cached position data to filter positions
        for data in &self.position_data {
            let lower_price = math::tick_to_sqrt_price(data.lower_tick).unwrap_or(0);
            let upper_price = math::tick_to_sqrt_price(data.upper_tick).unwrap_or(u128::MAX);

            // Check if ranges overlap
            if lower_price <= max_price && upper_price >= min_price {
                // Find the actual position
                if let Some(pos_idx) = self.find_position_index_by_key(data.key) {
                    let position = &self.pool_state.positions[pos_idx];

                    results.push(PositionInfo {
                        key: data.key,
                        owner: position.owner,
                        lower_tick: position.lower_tick,
                        upper_tick: position.upper_tick,
                        liquidity: position.liquidity,
                        tokens_owed_a: position.tokens_owed_a,
                        tokens_owed_b: position.tokens_owed_b,
                    });
                }
            }
        }

        results
    }

    /// Helper to find position index by key
    fn find_position_index_by_key(&self, key: Pubkey) -> Option<usize> {
        self.pool_state
            .positions
            .iter()
            .position(|pos| self.generate_position_key(pos) == key)
    }

    /// Finds positions that are in or near a specific tick
    pub fn get_positions_near_tick(&self, tick: i32, range: i32) -> Vec<PositionInfo> {
        let min_tick = tick - range;
        let max_tick = tick + range;
        let mut results = Vec::new();

        // Use the cached position data for efficient filtering
        for data in &self.position_data {
            if data.lower_tick <= max_tick && data.upper_tick >= min_tick {
                if let Some(pos_idx) = self.find_position_index_by_key(data.key) {
                    let position = &self.pool_state.positions[pos_idx];

                    results.push(PositionInfo {
                        key: data.key,
                        owner: position.owner,
                        lower_tick: position.lower_tick,
                        upper_tick: position.upper_tick,
                        liquidity: position.liquidity,
                        tokens_owed_a: position.tokens_owed_a,
                        tokens_owed_b: position.tokens_owed_b,
                    });
                }
            }
        }

        results
    }

    /// Updates position fees for all tracked positions
    pub fn update_all_position_fees(&mut self) -> Result<()> {
        // Use the pool's current state to update fees
        let fee_growth_global_a = self.pool_state.pool.fee_growth_global_a;
        let fee_growth_global_b = self.pool_state.pool.fee_growth_global_b;

        // Update each position separately
        for i in 0..self.pool_state.positions.len() {
            // Get a separate mutable reference to the position
            let position = &mut self.pool_state.positions[i];

            // Calculate fee growth delta
            let fee_growth_delta_a = fee_growth_global_a.wrapping_sub(position.fee_growth_inside_a);
            let fee_growth_delta_b = fee_growth_global_b.wrapping_sub(position.fee_growth_inside_b);

            // Calculate tokens owed
            if position.liquidity > 0 {
                // Formula: tokens_owed += liquidity * fee_growth_delta / Q64
                let delta_a = position
                    .liquidity
                    .checked_mul(fee_growth_delta_a)
                    .map(|n| n / math::Q64)
                    .unwrap_or(0) as u64;

                let delta_b = position
                    .liquidity
                    .checked_mul(fee_growth_delta_b)
                    .map(|n| n / math::Q64)
                    .unwrap_or(0) as u64;

                position.tokens_owed_a = position.tokens_owed_a.saturating_add(delta_a);
                position.tokens_owed_b = position.tokens_owed_b.saturating_add(delta_b);
            }

            // Update fee growth tracking
            position.fee_growth_inside_a = fee_growth_global_a;
            position.fee_growth_inside_b = fee_growth_global_b;
        }

        // Take snapshots in a separate loop to avoid double mutable borrow
        for i in 0..self.pool_state.positions.len() {
            let position = &self.pool_state.positions[i];
            let position_key = self.generate_position_key_from_data(
                position.owner,
                position.pool,
                position.lower_tick,
                position.upper_tick,
            );

            // Capture position data for snapshot
            let lower_tick = position.lower_tick;
            let upper_tick = position.upper_tick;
            let liquidity = position.liquidity;
            let tokens_owed_a = position.tokens_owed_a;
            let tokens_owed_b = position.tokens_owed_b;

            // Use the captured position data to create a snapshot
            self.snapshot_position_if_needed(
                position_key,
                lower_tick,
                upper_tick,
                liquidity,
                tokens_owed_a,
                tokens_owed_b,
            )?;
        }

        Ok(())
    }

    /// Helper function to snapshot a position if needed
    fn snapshot_position_if_needed(
        &mut self,
        position_key: Pubkey,
        lower_tick: i32,
        upper_tick: i32,
        liquidity: u128,
        tokens_owed_a: u64,
        tokens_owed_b: u64,
    ) -> Result<()> {
        let current_price = self.pool_state.pool.sqrt_price;
        let timestamp = Clock::get()?.unix_timestamp;

        // Only create a new snapshot if price has changed significantly or time has passed
        let should_snapshot = match self.position_snapshots.get(&position_key) {
            Some(snapshots) if !snapshots.is_empty() => {
                let last_snapshot = snapshots.last().unwrap();
                let price_diff_pct = (current_price as f64 - last_snapshot.price as f64).abs()
                    / (last_snapshot.price as f64)
                    * 100.0;
                let time_diff = timestamp - last_snapshot.timestamp;

                // Snapshot if price changed >1% or 1 hour passed
                price_diff_pct > 1.0 || time_diff > 3600
            }
            _ => true, // Always snapshot if no previous snapshots
        };

        if should_snapshot {
            // Calculate position value
            let (value_in_a, value_in_b) = self.calculate_position_value_from_data(
                lower_tick,
                upper_tick,
                liquidity,
                tokens_owed_a,
                tokens_owed_b,
            )?;

            // Calculate IL using position data
            let il_bps = self.calculate_il_bps_from_data(position_key, lower_tick, upper_tick);

            let new_snapshot = PositionSnapshot {
                timestamp,
                liquidity,
                price: current_price,
                fees_a: tokens_owed_a,
                fees_b: tokens_owed_b,
                value_in_a,
                value_in_b,
                impermanent_loss_bps: il_bps,
            };

            self.position_snapshots
                .entry(position_key)
                .or_default()
                .push(new_snapshot);
        }

        Ok(())
    }

    /// Helper to calculate IL in basis points from position data
    fn calculate_il_bps_from_data(
        &self,
        position_key: Pubkey,
        lower_tick: i32,
        upper_tick: i32,
    ) -> u16 {
        // Get current price and creation price
        let current_price = self.pool_state.pool.sqrt_price;

        // Lookup creation price from snapshots
        let creation_price = self
            .position_snapshots
            .get(&position_key)
            .and_then(|snapshots| snapshots.first())
            .map(|snapshot| snapshot.price)
            .unwrap_or(current_price);

        // If price hasn't changed, no IL
        if creation_price == current_price {
            return 0;
        }

        // Get position boundaries
        let lower_price = math::tick_to_sqrt_price(lower_tick).unwrap_or(0);
        let upper_price = math::tick_to_sqrt_price(upper_tick).unwrap_or(u128::MAX);

        // Calculate IL
        let il =
            calculate_impermanent_loss(creation_price, current_price, lower_price, upper_price);
        (il * 10000.0).round() as u16
    }

    /// Creates a snapshot of a position's current state for historical tracking
    pub fn create_position_snapshot(&self, position: &Position) -> Result<PositionSnapshot> {
        let current_price = self.pool_state.pool.sqrt_price;
        let timestamp = Clock::get()?.unix_timestamp;

        // Calculate position value
        let (value_in_a, value_in_b) = self.calculate_position_value_from_data(
            position.lower_tick,
            position.upper_tick,
            position.liquidity,
            position.tokens_owed_a,
            position.tokens_owed_b,
        )?;

        // Calculate IL
        let position_key = self.generate_position_key_from_data(
            position.owner,
            position.pool,
            position.lower_tick,
            position.upper_tick,
        );

        let il_bps =
            self.calculate_il_bps_from_data(position_key, position.lower_tick, position.upper_tick);

        Ok(PositionSnapshot {
            timestamp,
            liquidity: position.liquidity,
            price: current_price,
            fees_a: position.tokens_owed_a,
            fees_b: position.tokens_owed_b,
            value_in_a,
            value_in_b,
            impermanent_loss_bps: il_bps,
        })
    }

    /// Takes a snapshot of all positions for historical comparison
    pub fn snapshot_all_positions(&mut self) -> Result<()> {
        let timestamp = Clock::get()?.unix_timestamp;

        // Iterate through each position
        for i in 0..self.pool_state.positions.len() {
            let position = &self.pool_state.positions[i];

            // Capture data we need from the position to avoid borrowing issues
            let position_key = self.generate_position_key_from_data(
                position.owner,
                position.pool,
                position.lower_tick,
                position.upper_tick,
            );

            let lower_tick = position.lower_tick;
            let upper_tick = position.upper_tick;
            let liquidity = position.liquidity;
            let tokens_owed_a = position.tokens_owed_a;
            let tokens_owed_b = position.tokens_owed_b;

            // Calculate position value
            let (value_a, value_b) = self.calculate_position_value_from_data(
                lower_tick,
                upper_tick,
                liquidity,
                tokens_owed_a,
                tokens_owed_b,
            )?;

            // Calculate IL
            let il_bps = self.calculate_il_bps_from_data(position_key, lower_tick, upper_tick);

            // Create and store snapshot
            let snapshot = PositionSnapshot {
                timestamp,
                liquidity,
                price: self.latest_price,
                fees_a: tokens_owed_a,
                fees_b: tokens_owed_b,
                value_in_a: value_a,
                value_in_b: value_b,
                impermanent_loss_bps: il_bps,
            };

            self.position_snapshots
                .entry(position_key)
                .or_default()
                .push(snapshot);
        }

        // Update latest price timestamp
        self.latest_price = self.pool_state.pool.sqrt_price;
        self.latest_price_timestamp = timestamp;

        Ok(())
    }

    /// Calculate position value from raw position data
    pub fn calculate_position_value_from_data(
        &self,
        lower_tick: i32,
        upper_tick: i32,
        liquidity: u128,
        tokens_owed_a: u64,
        tokens_owed_b: u64,
    ) -> Result<(u64, u64)> {
        let current_sqrt_price = self.pool_state.pool.sqrt_price;

        // Calculate position boundaries
        let sqrt_price_lower = math::tick_to_sqrt_price(lower_tick)?;
        let sqrt_price_upper = math::tick_to_sqrt_price(upper_tick)?;

        // Calculate amounts based on current price
        let token_a = math::get_token_a_from_liquidity(
            liquidity,
            sqrt_price_lower,
            sqrt_price_upper,
            current_sqrt_price,
        )? as u64;

        let token_b = math::get_token_b_from_liquidity(
            liquidity,
            sqrt_price_lower,
            sqrt_price_upper,
            current_sqrt_price,
        )? as u64;

        // Add uncollected fees
        let total_a = token_a.saturating_add(tokens_owed_a);
        let total_b = token_b.saturating_add(tokens_owed_b);

        Ok((total_a, total_b))
    }

    /// Calculate the current total value of a position in terms of both tokens
    pub fn calculate_position_value(&self, position: &Position) -> Result<(u64, u64)> {
        self.calculate_position_value_from_data(
            position.lower_tick,
            position.upper_tick,
            position.liquidity,
            position.tokens_owed_a,
            position.tokens_owed_b,
        )
    }

    /// Estimates IL (Impermanent Loss) for a position based on current price
    pub fn estimate_impermanent_loss(&self, position: &Position) -> f64 {
        let current_price = self.pool_state.pool.sqrt_price;
        let position_key = self.generate_position_key_from_data(
            position.owner,
            position.pool,
            position.lower_tick,
            position.upper_tick,
        );

        // Get creation price from snapshots or use current price if no history
        let creation_price = self
            .position_snapshots
            .get(&position_key)
            .and_then(|snapshots| snapshots.first())
            .map(|snapshot| snapshot.price)
            .unwrap_or(current_price);

        // If price hasn't changed, no IL
        if creation_price == current_price {
            return 0.0;
        }

        // Get position boundaries
        let lower_tick = position.lower_tick;
        let upper_tick = position.upper_tick;
        let lower_price = math::tick_to_sqrt_price(lower_tick).unwrap_or(0);
        let upper_price = math::tick_to_sqrt_price(upper_tick).unwrap_or(u128::MAX);

        // Calculate IL
        calculate_impermanent_loss(creation_price, current_price, lower_price, upper_price)
    }

    /// Gets positions that match criteria for IL mitigation
    pub fn get_positions_for_il_mitigation(&self, volatility_threshold: f64) -> Vec<PositionInfo> {
        let mut candidates = Vec::new();

        for position in self.pool_state.positions.iter() {
            // Calculate IL for this position
            let il = self.estimate_impermanent_loss(position);

            // Check if IL exceeds our threshold
            if il > volatility_threshold / 100.0 {
                candidates.push(PositionInfo {
                    key: self.generate_position_key(position),
                    owner: position.owner,
                    lower_tick: position.lower_tick,
                    upper_tick: position.upper_tick,
                    liquidity: position.liquidity,
                    tokens_owed_a: position.tokens_owed_a,
                    tokens_owed_b: position.tokens_owed_b,
                });
            }
        }

        candidates
    }

    /// Gets position history with snapshots for a given position
    pub fn get_position_history(&self, position_pubkey: &Pubkey) -> Vec<&PositionSnapshot> {
        match self.position_snapshots.get(position_pubkey) {
            Some(snapshots) => snapshots.iter().collect(),
            None => Vec::new(),
        }
    }

    /// Sorts positions by a given metric (useful for analytics)
    pub fn sort_positions_by<F>(&self, mut comparator: F) -> Vec<PositionInfo>
    where
        F: FnMut(&PositionInfo, &PositionInfo) -> Ordering,
    {
        // Create a vector of PositionInfo objects
        let mut position_infos = Vec::new();

        for position in self.pool_state.positions.iter() {
            position_infos.push(PositionInfo {
                key: self.generate_position_key(position),
                owner: position.owner,
                lower_tick: position.lower_tick,
                upper_tick: position.upper_tick,
                liquidity: position.liquidity,
                tokens_owed_a: position.tokens_owed_a,
                tokens_owed_b: position.tokens_owed_b,
            });
        }

        // Sort by the given comparator
        position_infos.sort_by(|a, b| comparator(a, b));
        position_infos
    }

    /// Gets positions that may benefit from rebalancing based on current market conditions
    pub fn get_positions_for_rebalancing(&self) -> Result<Vec<(PositionInfo, f64)>> {
        let mut rebalance_candidates = Vec::new();

        for position in self.pool_state.positions.iter() {
            let il = self.estimate_impermanent_loss(position);

            // Check if we have price movement history and the position has significant IL
            if il > 0.01 {
                // 1% IL threshold as an example
                let info = PositionInfo {
                    key: self.generate_position_key(position),
                    owner: position.owner,
                    lower_tick: position.lower_tick,
                    upper_tick: position.upper_tick,
                    liquidity: position.liquidity,
                    tokens_owed_a: position.tokens_owed_a,
                    tokens_owed_b: position.tokens_owed_b,
                };

                // Add this position to candidates with its IL value
                rebalance_candidates.push((info, il));
            }
        }

        // Sort by highest IL first
        rebalance_candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

        Ok(rebalance_candidates)
    }

    /// Records user adjustment of a position for future analysis
    pub fn record_position_adjustment(&mut self, position: &Position) -> Result<()> {
        // Create a snapshot to record the state after adjustment
        let snapshot = self.create_position_snapshot(position)?;
        let position_key = self.generate_position_key_from_data(
            position.owner,
            position.pool,
            position.lower_tick,
            position.upper_tick,
        );

        // Add to position history
        self.position_snapshots
            .entry(position_key)
            .or_default()
            .push(snapshot);

        Ok(())
    }
}

/// Position info for safely returning position data
#[derive(Debug, Clone)]
pub struct PositionInfo {
    /// Position unique identifier
    pub key: Pubkey,

    /// The owner of this position
    pub owner: Pubkey,

    /// The lower tick boundary of the position
    pub lower_tick: i32,

    /// The upper tick boundary of the position
    pub upper_tick: i32,

    /// Amount of liquidity contributed to the pool within this range
    pub liquidity: u128,

    /// Uncollected token A fees, ready for withdrawal
    pub tokens_owed_a: u64,

    /// Uncollected token B fees, ready for withdrawal
    pub tokens_owed_b: u64,
}

/// Extension trait for Position to add analytics methods
pub trait PositionAnalyticsTrait {
    /// Calculates the percentage of total pool liquidity this position represents
    fn percentage_of_pool(&self, pool: &Pool) -> f64;

    /// Calculates the range width as a percentage
    fn range_width_percentage(&self) -> f64;

    /// Checks if the position is currently in active range
    fn is_active(&self, current_tick: i32) -> bool;

    /// Gets human-readable price range
    fn price_range_display(&self) -> (f64, f64);

    /// Calculates uncollected fees in USD terms (if price feeds available)
    fn uncollected_fees_value(&self, token_a_usd_price: f64, token_b_usd_price: f64) -> f64;

    /// Calculates the capital efficiency of this position
    fn capital_efficiency(&self, current_tick: i32) -> f64;
}

impl PositionAnalyticsTrait for Position {
    fn percentage_of_pool(&self, pool: &Pool) -> f64 {
        if pool.liquidity == 0 {
            return 0.0;
        }

        (self.liquidity as f64) / (pool.liquidity as f64) * 100.0
    }

    fn range_width_percentage(&self) -> f64 {
        // Convert ticks to prices for percentage calculation
        let lower_price = PriceRange::tick_to_price(self.lower_tick);
        let upper_price = PriceRange::tick_to_price(self.upper_tick);

        // Calculate percentage width
        ((upper_price - lower_price) / lower_price) * 100.0
    }

    fn is_active(&self, current_tick: i32) -> bool {
        current_tick >= self.lower_tick && current_tick < self.upper_tick
    }

    fn price_range_display(&self) -> (f64, f64) {
        // Convert from stored 6 decimal fixed-point to float
        let lower = (self.lower_price as f64) / 1_000_000.0;
        let upper = (self.upper_price as f64) / 1_000_000.0;

        (lower, upper)
    }

    fn uncollected_fees_value(&self, token_a_usd_price: f64, token_b_usd_price: f64) -> f64 {
        let fees_a_value = (self.tokens_owed_a as f64) * token_a_usd_price;
        let fees_b_value = (self.tokens_owed_b as f64) * token_b_usd_price;

        fees_a_value + fees_b_value
    }

    fn capital_efficiency(&self, current_tick: i32) -> f64 {
        // If position is out of range, efficiency is 0%
        if current_tick < self.lower_tick || current_tick >= self.upper_tick {
            return 0.0;
        }

        // Capital efficiency is approximately inverse of range width
        // 1/width * adjustment factor
        // Narrower positions have higher capital efficiency
        let width = self.range_width_percentage();
        if width <= 0.0 {
            return 0.0; // Prevent division by zero
        }

        // Factor to normalize for standard positioning around current price
        // This is an approximation and would be refined in production
        let standard_width = 50.0; // 50% width as a reference
        (standard_width / width) * 100.0
    }
}

// We also implement the PositionAnalytics trait for PositionInfo
impl PositionAnalyticsTrait for PositionInfo {
    fn percentage_of_pool(&self, pool: &Pool) -> f64 {
        if pool.liquidity == 0 {
            return 0.0;
        }

        (self.liquidity as f64) / (pool.liquidity as f64) * 100.0
    }

    fn range_width_percentage(&self) -> f64 {
        // Convert ticks to prices for percentage calculation
        let lower_price = PriceRange::tick_to_price(self.lower_tick);
        let upper_price = PriceRange::tick_to_price(self.upper_tick);

        // Calculate percentage width
        ((upper_price - lower_price) / lower_price) * 100.0
    }

    fn is_active(&self, current_tick: i32) -> bool {
        current_tick >= self.lower_tick && current_tick < self.upper_tick
    }

    fn price_range_display(&self) -> (f64, f64) {
        // Convert from ticks to prices
        let lower = PriceRange::tick_to_price(self.lower_tick);
        let upper = PriceRange::tick_to_price(self.upper_tick);

        (lower, upper)
    }

    fn uncollected_fees_value(&self, token_a_usd_price: f64, token_b_usd_price: f64) -> f64 {
        let fees_a_value = (self.tokens_owed_a as f64) * token_a_usd_price;
        let fees_b_value = (self.tokens_owed_b as f64) * token_b_usd_price;

        fees_a_value + fees_b_value
    }

    fn capital_efficiency(&self, current_tick: i32) -> f64 {
        // If position is out of range, efficiency is 0%
        if current_tick < self.lower_tick || current_tick >= self.upper_tick {
            return 0.0;
        }

        // Capital efficiency is approximately inverse of range width
        // 1/width * adjustment factor
        // Narrower positions have higher capital efficiency
        let width = self.range_width_percentage();
        if width <= 0.0 {
            return 0.0; // Prevent division by zero
        }

        // Factor to normalize for standard positioning around current price
        // This is an approximation and would be refined in production
        let standard_width = 50.0; // 50% width as a reference
        (standard_width / width) * 100.0
    }
}

/// Struct for position adjustment recommendations from IL mitigation system
#[derive(Debug)]
pub struct PositionAdjustmentRecommendation {
    /// The position to adjust
    pub position: Pubkey,

    /// Recommended new lower tick
    pub new_lower_tick: i32,

    /// Recommended new upper tick
    pub new_upper_tick: i32,

    /// Estimated IL reduction if adjusted
    pub estimated_il_reduction_bps: u16,

    /// Reason for the adjustment
    pub adjustment_reason: String,

    /// Timestamp of the recommendation
    pub timestamp: i64,
}

/// Enum for position status categorization
pub enum PositionStatus {
    /// Position is in active range and earning fees
    Active,

    /// Position is outside current price range
    OutOfRange,

    /// Position has significant IL risk
    HighILRisk,

    /// Position would benefit from rebalancing
    NeedsRebalance,

    /// Position has uncollected fees
    HasUncollectedFees,

    /// Position is approaching range boundary
    ApproachingBoundary,
}

/// Position Manager Module
///
/// This module implements operations for managing liquidity positions in the AMM.
/// It provides methods for creating, modifying, and analyzing positions.
/// Creates a new position in a pool with the specified tick range
///
/// # Parameters
/// * `position_account` - The account where position data will be stored
/// * `pool_state` - The state manager for the pool
/// * `lower_tick` - The lower tick boundary of the position
/// * `upper_tick` - The upper tick boundary of the position
/// * `liquidity_delta` - The amount of liquidity to add to the position
///
/// # Returns
/// * `Result<(u64, u64)>` - The amounts of token A and B required for the position
pub fn create_position<'a>(
    position_account: &'a mut Position,
    pool_state: &'a mut PoolState<'a>,
    lower_tick: i32,
    upper_tick: i32,
    liquidity_delta: u128,
) -> Result<(u64, u64)> {
    // Call into pool state manager to create the position
    pool_state.create_position(position_account, lower_tick, upper_tick, liquidity_delta)
}

/// Increases liquidity in an existing position
///
/// # Parameters
/// * `position` - The position to modify
/// * `pool_state` - The state manager for the pool
/// * `liquidity_delta` - The amount of liquidity to add
///
/// # Returns
/// * `Result<(u64, u64)>` - The amounts of token A and B required for the added liquidity
pub fn increase_liquidity(
    position: &mut Position,
    pool_state: &mut PoolState,
    liquidity_delta: u128,
) -> Result<(u64, u64)> {
    // Call into pool state manager to modify the position
    pool_state.modify_position(position, liquidity_delta as i128, true)
}

/// Decreases liquidity in an existing position
///
/// # Parameters
/// * `position` - The position to modify
/// * `pool_state` - The state manager for the pool
/// * `liquidity_delta` - The amount of liquidity to remove
///
/// # Returns
/// * `Result<(u64, u64)>` - The amounts of token A and B returned from the removed liquidity
pub fn decrease_liquidity(
    position: &mut Position,
    pool_state: &mut PoolState,
    liquidity_delta: u128,
) -> Result<(u64, u64)> {
    // Ensure we're not removing more liquidity than the position has
    require!(
        position.liquidity >= liquidity_delta,
        ErrorCode::PositionLiquidityTooLow
    );

    // Call into pool state manager to modify the position
    pool_state.modify_position(position, liquidity_delta as i128, false)
}

/// Collects accumulated fees from a position
///
/// # Parameters
/// * `position` - The position to collect fees from
/// * `pool_state` - The state manager for the pool
///
/// # Returns
/// * `Result<(u64, u64)>` - The amounts of token A and B fees collected
pub fn collect_fees(position: &mut Position, pool_state: &mut PoolState) -> Result<(u64, u64)> {
    // Update fee accounting in the position
    pool_state.update_position_fees(position)?;

    // Get the fees that have accumulated
    let fees_a = position.tokens_owed_a;
    let fees_b = position.tokens_owed_b;

    // Reset fees owed in the position
    position.tokens_owed_a = 0;
    position.tokens_owed_b = 0;

    Ok((fees_a, fees_b))
}

/// Gets the current virtual reserves for a specific position
///
/// This function calculates how much of the virtual reserves is attributable to a
/// specific position based on its share of the liquidity in the active range.
///
/// # Parameters
/// * `position` - The position to analyze
/// * `pool_state` - The state manager for the pool
///
/// # Returns
/// * `Result<(u64, u64)>` - The position's contribution to virtual reserves (A, B)
pub fn get_position_virtual_reserves(
    position: &Position,
    pool_state: &PoolState,
) -> Result<(u64, u64)> {
    // Get total pool virtual reserves
    let (pool_reserve_a, pool_reserve_b) = pool_state.get_virtual_reserves()?;

    // Get current tick and check if position is in range
    let current_tick = pool_state.pool.current_tick;
    let is_in_range = current_tick >= position.lower_tick && current_tick < position.upper_tick;

    // If position is not in active range, return zero for both reserves
    if !is_in_range || pool_state.pool.liquidity == 0 {
        return Ok((0, 0));
    }

    // Calculate position's share of the total liquidity
    let position_share = (position.liquidity as f64) / (pool_state.pool.liquidity as f64);

    // Calculate position's portion of virtual reserves
    let position_reserve_a = (pool_reserve_a as f64 * position_share) as u64;
    let position_reserve_b = (pool_reserve_b as f64 * position_share) as u64;

    Ok((position_reserve_a, position_reserve_b))
}

/// Gets the distribution of tokens in a position at the current price
///
/// This function provides a detailed breakdown of a position's token distribution,
/// including both real token amounts and virtual reserves. This is useful for
/// analytics and impermanent loss calculations.
///
/// # Parameters
/// * `position` - The position to analyze
/// * `pool_state` - The state manager for the pool
///
/// # Returns
/// * `Result<PositionAnalytics>` - Detailed position analytics
pub fn get_position_analytics(
    position: &Position,
    pool_state: &PoolState,
) -> Result<PositionAnalytics> {
    let current_tick = pool_state.pool.current_tick;
    let sqrt_price = pool_state.pool.sqrt_price;

    // Calculate the sqrt prices at position boundaries
    let sqrt_price_lower = math::tick_to_sqrt_price(position.lower_tick)?;
    let sqrt_price_upper = math::tick_to_sqrt_price(position.upper_tick)?;

    // Determine if position is in current price range
    let is_in_range = current_tick >= position.lower_tick && current_tick < position.upper_tick;

    // Get real token amounts in the position
    let (real_token_a, real_token_b) = if is_in_range {
        // If in range, calculate real token amounts based on current price
        let amount_a = math::get_amount_a_delta_for_price_range(
            position.liquidity,
            sqrt_price,
            sqrt_price_upper,
            false, // Round down for more conservative estimate
        )? as u64;

        let amount_b = math::get_amount_b_delta_for_price_range(
            position.liquidity,
            sqrt_price_lower,
            sqrt_price,
            false, // Round down for more conservative estimate
        )? as u64;

        (amount_a, amount_b)
    } else if current_tick < position.lower_tick {
        // All value is in token A
        let amount_a = math::get_amount_a_delta_for_price_range(
            position.liquidity,
            sqrt_price_lower,
            sqrt_price_upper,
            false, // Round down for more conservative estimate
        )? as u64;

        (amount_a, 0)
    } else {
        // All value is in token B
        let amount_b = math::get_amount_b_delta_for_price_range(
            position.liquidity,
            sqrt_price_lower,
            sqrt_price_upper,
            false, // Round down for more conservative estimate
        )? as u64;

        (0, amount_b)
    };

    // Get position's virtual reserves contribution
    let (virtual_reserve_a, virtual_reserve_b) =
        get_position_virtual_reserves(position, pool_state)?;

    // Calculate fees owed (without updating position state)
    let fees_a = position.tokens_owed_a;
    let fees_b = position.tokens_owed_b;

    // Generate a position ID using position data
    let position_id = Pubkey::new_unique(); // Using a unique key for this instance

    // Create and return the analytics struct
    Ok(PositionAnalytics {
        position_id,
        lower_tick: position.lower_tick,
        upper_tick: position.upper_tick,
        liquidity: position.liquidity,
        is_in_range,
        real_token_a,
        real_token_b,
        virtual_reserve_a,
        virtual_reserve_b,
        fees_owed_a: fees_a,
        fees_owed_b: fees_b,
        current_tick,
        sqrt_price,
    })
}

/// Detailed analytics for a position
///
/// This struct contains comprehensive information about a position's state,
/// including both real token amounts and virtual reserves distribution.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PositionAnalytics {
    /// The position's unique identifier
    pub position_id: Pubkey,

    /// Lower tick boundary of the position
    pub lower_tick: i32,

    /// Upper tick boundary of the position
    pub upper_tick: i32,

    /// Liquidity amount in the position
    pub liquidity: u128,

    /// Whether the position is in the active price range
    pub is_in_range: bool,

    /// Actual amount of token A in the position
    pub real_token_a: u64,

    /// Actual amount of token B in the position
    pub real_token_b: u64,

    /// Position's contribution to virtual reserve A
    pub virtual_reserve_a: u64,

    /// Position's contribution to virtual reserve B
    pub virtual_reserve_b: u64,

    /// Fees owed to the position in token A
    pub fees_owed_a: u64,

    /// Fees owed to the position in token B
    pub fees_owed_b: u64,

    /// Current tick of the pool
    pub current_tick: i32,

    /// Current sqrt price of the pool
    pub sqrt_price: u128,
}
