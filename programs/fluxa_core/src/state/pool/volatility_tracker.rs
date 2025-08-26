use crate::error::PoolError;
use crate::math::core_arithmetic::{mul_div_q64, sqrt_x64, Q64x64};
use crate::utils::constants::STANDARD_LAMBDA;
use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

/// Exponentially-weighted moving average (EWMA) volatility tracker for on-chain risk management.
///
/// # Why this structure?
/// - Designed for O(1) updates and integer-only arithmetic, making it efficient and predictable for Solana's compute model.
/// - All fields are fixed-size and aligned, with no dynamic allocations, ensuring safety and deterministic account size.
/// - EWMA is chosen for its simplicity, low memory footprint, and ability to react to changing market conditions without expensive on-chain computation.
///
/// ## Usage
/// This struct is used for dynamic fee adjustment, risk controls, and monitoring, referenced by pool config and security logic.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Pod, Zeroable, InitSpace)]
#[repr(C)]
pub struct EwmaVolatilityTracker {
    /// EWMA parameters and state.
    ///
    /// - `lambda`: Decay factor for the EWMA, in Q16.16 fixed point. Why: Controls responsiveness to new data; Q16.16 allows fine-tuning while remaining compact.
    /// - `current_volatility`: Current volatility estimate, in basis points. Why: Integer basis points are easy to use for fee/risk logic and avoid floating-point errors.
    /// - `previous_price`: Last observed price, in Q64.64. Why: Needed for log-return calculation; Q64.64 ensures precision and overflow safety.
    pub lambda: u32, // Decay factor in fixed point (e.g., 0.94 * 2^16)
    pub current_volatility: u32, // Current volatility estimate (basis points)
    pub _volatility_padding: [u8; 8], // Alignment padding for u128 field
    pub previous_price: Q64x64,  // Previous price for return calculation

    /// Rate limiting and update tracking.
    ///
    /// - `last_update_slot`: Last slot when volatility was updated. Why: Prevents excessive computation and enables replay protection.
    /// - `min_update_interval`: Minimum slots between updates. Why: Ensures updates are not spammed, protecting against DoS and wasted compute.
    /// - `update_count`: Total number of updates. Why: Useful for monitoring and debugging, and for adaptive logic if needed.
    pub last_update_slot: u64,
    pub update_count: u32,        // Total number of updates
    pub min_update_interval: u16, // Minimum slots between updates

    /// Configuration and safety controls.
    ///
    /// - `enabled`: Whether the tracker is active. Why: Allows for protocol upgrades and emergency disables without redeploying.
    /// - `volatility_cap`: Maximum allowed volatility, in basis points. Why: Prevents runaway fees or risk logic due to oracle errors or attacks.
    /// - `_padding`: Ensures 8-byte alignment for Anchor zero-copy safety and future extensibility.
    pub enabled: u8,
    pub _padding: u8,          // Padding for alignment
    pub volatility_cap: u32,   // Maximum volatility in basis points
    pub reserved: [u8; 4],     // Align to 8-byte boundary
    pub _end_padding: [u8; 8], // Final alignment padding for struct
}

/// EWMA Volatility Tracker implementation.
///
/// # Why this approach?
/// - All math is performed using integer/fixed-point arithmetic for on-chain safety and predictability.
/// - O(1) update complexity ensures that volatility tracking does not become a DoS vector.
/// - Volatility is capped and rate-limited to prevent manipulation or runaway computation.
impl EwmaVolatilityTracker {
    /// Initialize with safe, conservative defaults.
    ///
    /// # Why this pattern?
    /// - Sets a standard lambda and minimum update interval to balance responsiveness and safety.
    /// - All fields are initialized to known values, preventing uninitialized state and making upgrades safer.
    ///
    /// # Arguments
    /// * `min_update_interval` - Minimum slots between updates to prevent excessive computation
    /// # Returns
    /// A new instance of `EwmaVolatilityTracker`
    pub fn new(min_update_interval: u16) -> Self {
        Self {
            lambda: STANDARD_LAMBDA,
            current_volatility: 0,
            _volatility_padding: [0; 8],
            previous_price: Q64x64::zero(),
            last_update_slot: 0,
            min_update_interval,
            update_count: 0,
            enabled: 1,
            _padding: 0,           // Padding for alignment
            volatility_cap: 10000, // 100% volatility cap
            reserved: [0; 4],
            _end_padding: [0; 8],
        }
    }

    /// Update volatility using EWMA, with O(1) complexity and integer-only arithmetic.
    ///
    /// # Why this method?
    /// - Integer/fixed-point math is used throughout to avoid rounding errors and overflows, which are critical for on-chain safety.
    /// - Rate limiting is enforced to prevent DoS and excessive compute usage.
    /// - Skips the first update to avoid spurious volatility spikes from uninitialized state.
    ///
    /// # Arguments
    /// * `current_price` - Current price in Q64.64 format
    /// * `current_slot` - Current slot for rate limiting
    /// # Returns
    /// `Ok(())` on success, or an error if the update is too frequent
    /// # Errors
    /// `PoolError::VolatilityUpdateTooFrequent` if the update is too frequent
    pub fn update_volatility(&mut self, current_price: Q64x64, current_slot: u64) -> Result<()> {
        // Ensure the tracker is enabled
        if self.enabled == 0 {
            return Ok(());
        }

        // Rate limiting check
        if current_slot < self.last_update_slot + self.min_update_interval as u64 {
            return Err(PoolError::VolatilityUpdateTooFrequent.into());
        }

        // Skip first update (no previous price)
        if self.previous_price == Q64x64::zero() {
            self.previous_price = current_price;
            self.last_update_slot = current_slot;
            return Ok(());
        }

        let prev_price = self.previous_price;

        // Log return approximation using integer arithmetic.
        // Why: For small changes, ln(p1/p0) ≈ (p1-p0)/p0, which is cheap and robust on-chain.
        let price_change = if current_price >= self.previous_price {
            mul_div_q64(
                current_price.checked_sub(prev_price)?,
                Q64x64::from_int(10000),
                prev_price,
            )?
        } else {
            mul_div_q64(
                prev_price.checked_sub(current_price)?,
                Q64x64::from_int(10000),
                prev_price,
            )?
        };

        // Squared return for variance calculation.
        // Why: Variance is the EWMA of squared returns; Q64.64 math ensures precision.
        let squared_return = (price_change.checked_mul(price_change)?).raw() >> 64; // Q64.64 to integer

        // EWMA update: σ²(t) = λ * σ²(t-1) + (1-λ) * r²(t)
        // Why: This formula provides a robust, memory-efficient estimate of volatility that reacts to new data but is resistant to manipulation.
        // All math is performed in fixed-point for on-chain safety.
        let lambda_scaled = self.lambda as u128;
        let one_minus_lambda = 65536u128 - lambda_scaled;

        // Previous variance is the square of the current volatility (in basis points).
        let previous_variance =
            (self.current_volatility as u128) * (self.current_volatility as u128);
        // New variance is the EWMA of previous variance and new squared return.
        let new_variance =
            (lambda_scaled * previous_variance + one_minus_lambda * squared_return) >> 16;

        // Convert new variance to Q64.64 for square root calculation.
        let new_variance_q64 = Q64x64::from_raw(new_variance << 64);

        // Volatility is the square root of variance, in Q64.64.
        // Why: Square root is needed to convert variance to standard deviation (volatility).
        let new_volatility_q64 = sqrt_x64(new_variance_q64)?;

        // Convert Q64.64 volatility to integer basis points for storage and downstream logic.
        let new_volatility = new_volatility_q64.raw() >> 64; // Convert back to integer

        // Store new volatility estimate (basis points).
        self.current_volatility = new_volatility as u32; // Convert back to basis points

        // Cap volatility to prevent runaway fees or risk logic due to oracle errors or attacks.
        if self.current_volatility > self.volatility_cap {
            self.current_volatility = self.volatility_cap;
        }

        // Update state for next round.
        self.previous_price = current_price;
        self.last_update_slot = current_slot;
        self.update_count += 1;

        Ok(())
    }

    /// Get current volatility in basis points.
    ///
    /// # Why this method?
    /// - Returns integer basis points for easy integration with fee/risk logic.
    /// - Marked inline for performance, as this is called frequently.
    /// # Returns
    /// Current volatility estimate (basis points)
    #[inline(always)]
    pub fn get_volatility(&self) -> u32 {
        self.current_volatility
    }

    /// Check if volatility exceeds a given threshold (for dynamic fees or risk logic).
    ///
    /// # Why this method?
    /// - Enables dynamic fee/risk logic based on real-time volatility.
    /// - Marked inline for performance, as this is called frequently.
    /// # Arguments
    /// * `threshold` - Volatility threshold in basis points
    /// # Returns
    /// `true` if current volatility exceeds threshold, `false` otherwise
    #[inline(always)]
    pub fn is_high_volatility(&self, threshold: u32) -> bool {
        self.current_volatility > threshold
    }
}
