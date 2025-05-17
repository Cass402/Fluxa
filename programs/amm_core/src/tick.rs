use crate::errors::ErrorCode;
/// Defines the state and basic logic for individual initialized ticks.
///
/// In Fluxa's concentrated liquidity model, the price range is divided into discrete
/// ticks. Each tick represents a specific price point. When liquidity providers
/// create positions, they specify a lower and upper tick for their liquidity range.
/// The `TickData` account stores information about each tick that has been initialized
/// (i.e., has some liquidity associated with it).
use anchor_lang::prelude::*;

/// Represents the state of an initialized tick.
///
/// For the MVP, this struct focuses on core liquidity tracking.
/// Fee growth and oracle-related fields are omitted for simplification as per
/// the MVP scope.
///
/// Accounts of this type would typically be PDAs derived from the pool
/// and the tick index.
#[account(zero_copy)]
#[repr(C)]
#[derive(Debug, Default)]
pub struct TickData {
    // MVP Simplification: Skipping fee_growth_outside_... and oracle fields.
    /// total gross liquidity (16-byte align)
    pub liquidity_gross: u128, // offset 0
    /// net liquidity change        (16-byte align)
    pub liquidity_net: i128, // offset 16
    /// pool pubkey                (1-byte align)
    pub pool: Pubkey, // offset 32
    /// the index                  (4-byte align)
    pub index: i32, // offset 64
    /// initialized flag           (1-byte align)
    pub initialized: u8, // offset 68
    // split the 59 bytes into two chunks ≤ 32
    pub _padding0: [u8; 32], // offset 69..100
    pub _padding1: [u8; 27], // offset 101..127
}

impl TickData {
    /// Total size of the fields: 16 (liquidity_gross) + 16 (liquidity_net) + 32 (pool) + 4 (index) + 1 (initialized) + 32 (_padding0) + 27 (_padding1) = 128 bytes.
    /// Anchor's `#[account(zero_copy)]` handles the 8-byte discriminator separately.
    pub const LEN: usize = 128;

    /// Initializes a new tick with default values.
    ///
    /// # Arguments
    ///
    /// * `pool` - The pubkey of the pool this tick belongs to.
    /// * `index` - The index of this tick.
    pub fn initialize(&mut self, pool: Pubkey, index: i32) {
        self.pool = pool;
        self.index = index;
        self.liquidity_gross = 0;
        self.liquidity_net = 0;
        self.initialized = 0; // 0 for false
        self._padding0 = [0; 32];
        self._padding1 = [0; 27];
    }

    /// Updates the tick's liquidity values when a position referencing this tick changes.
    ///
    /// # Arguments
    ///
    /// * `liquidity_delta` - The change in liquidity. Positive if adding liquidity,
    ///   negative if removing.
    /// * `is_upper_tick` - True if this tick is the upper boundary of the position,
    ///   false if it's the lower boundary.
    pub fn update_on_liquidity_change(
        &mut self,
        liquidity_delta: i128,
        is_upper_tick: bool,
    ) -> Result<()> {
        let abs_delta_u128 = liquidity_delta.unsigned_abs();

        if liquidity_delta > 0 {
            self.liquidity_gross = self
                .liquidity_gross
                .checked_add(abs_delta_u128)
                .ok_or_else(|| error!(ErrorCode::MathOverflow))?;
        } else {
            self.liquidity_gross = self
                .liquidity_gross
                .checked_sub(abs_delta_u128)
                .ok_or_else(|| error!(ErrorCode::MathOverflow))?;
        }
        self.liquidity_net = if is_upper_tick {
            self.liquidity_net.checked_sub(liquidity_delta)
        } else {
            self.liquidity_net.checked_add(liquidity_delta)
        }
        .ok_or_else(|| error!(ErrorCode::MathOverflow))?;

        self.initialized = if self.liquidity_gross > 0 { 1 } else { 0 };
        Ok(())
    }
}
