use crate::error::PoolError;
use crate::math::core_arithmetic::{mul_div_q64, sqrt_x64, Q64x64};
use crate::utils::constants::STANDARD_LAMBDA;
use anchor_lang::prelude::*;

/// EWMA-based volatility tracker optimized for on-chain computation
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
#[repr(C)]
pub struct EwmaVolatilityTracker {
    /// EWMA parameters
    /// 'lambda' - Decay factor for the exponential moving average (EMA) in fixed point (Q16.16)
    /// 'current_volatility' - Current volatility estimate in basis points
    /// 'previous_price' - Previous price for calculating returns (Q64.64 fixed point)
    pub lambda: u32, // Decay factor in fixed point (e.g., 0.94 * 2^16)
    pub current_volatility: u32, // Current volatility estimate (basis points)
    pub previous_price: u128,    // Previous price for return calculation

    /// Rate limiting and updates
    /// 'last_update_slot' - Slot of the last update
    /// 'min_update_interval' - Minimum slots between updates to prevent excessive computation
    /// 'update_count' - Total number of updates for tracking
    pub last_update_slot: u64,
    pub min_update_interval: u16, // Minimum slots between updates
    pub update_count: u32,        // Total number of updates

    /// Configuration
    /// 'enabled' - Whether the volatility tracker is active
    /// 'volatility_cap' - Maximum volatility in basis points to prevent excessive fees
    /// '_padding' - Padding for alignment
    pub enabled: bool,
    pub volatility_cap: u32, // Maximum volatility in basis points
    pub _padding: [u8; 6],   // Align to 8-byte boundary
}

/// EWMA Volatility Tracker implementation
impl EwmaVolatilityTracker {
    /// Initialize with standard parameters
    /// This sets a default lambda and minimum update interval
    /// # Arguments:
    /// * `min_update_interval` - Minimum slots between updates to prevent excessive computation
    /// # Returns: A new instance of `EwmaVolatilityTracker`
    pub fn new(min_update_interval: u16) -> Self {
        Self {
            lambda: STANDARD_LAMBDA,
            current_volatility: 0,
            previous_price: 0,
            last_update_slot: 0,
            min_update_interval,
            update_count: 0,
            enabled: true,
            volatility_cap: 10000, // 100% volatility cap
            _padding: [0; 6],
        }
    }

    /// Update volatility using EWMA - O(1) complexity, integer-only arithmetic
    /// # Arguments:
    /// * `current_price` - Current price in Q64.64 format
    /// * `current_slot` - Current slot for rate limiting
    /// # Returns: `Ok(())` on success, or an error if the update is too frequent
    /// # Errors: `PoolError::VolatilityUpdateTooFrequent` if the update is too frequent
    pub fn update_volatility(&mut self, current_price: Q64x64, current_slot: u64) -> Result<()> {
        // Ensure the tracker is enabled
        if !self.enabled {
            return Ok(());
        }

        // Rate limiting check
        if current_slot < self.last_update_slot + self.min_update_interval as u64 {
            return Err(PoolError::VolatilityUpdateTooFrequent.into());
        }

        // Skip first update (no previous price)
        if self.previous_price == 0 {
            self.previous_price = current_price.raw();
            self.last_update_slot = current_slot;
            return Ok(());
        }

        let prev_price = Q64x64::from_raw(self.previous_price);

        // Calculate log return approximation using integer arithmetic
        // For small changes: ln(p1/p0) ≈ (p1-p0)/p0
        let price_change = if current_price.raw() >= self.previous_price {
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

        // Squared return for variance calculation
        let squared_return = (price_change.checked_mul(price_change)?).raw() >> 64; // Q64.64 to integer

        // EWMA update: σ²(t) = λ * σ²(t-1) + (1-λ) * r²(t)
        // Using fixed-point arithmetic: lambda is scaled by 65536
        let lambda_scaled = self.lambda as u128;
        let one_minus_lambda = 65536u128 - lambda_scaled;

        // Previous variance calculation ()
        let previous_variance =
            (self.current_volatility as u128) * (self.current_volatility as u128);
        // New variance calculation
        let new_variance =
            (lambda_scaled * previous_variance + one_minus_lambda * squared_return) >> 16;

        // Convert new variance back to Q64.64 format for volatility calculation
        let new_variance_q64 = Q64x64::from_raw(new_variance << 64);

        // Calculate new volatility as square root of variance
        let new_volatility_q64 = sqrt_x64(new_variance_q64)?;

        // Convert to integer basis points
        // Note: new_volatility_q64 is in Q64.64 format, we need to convert it to basis points
        // by shifting right 64 bits (to get the integer part)
        let new_volatility = new_volatility_q64.raw() >> 64; // Convert back to integer

        // Update current volatility
        self.current_volatility = new_volatility as u32; // Convert back to basis points

        // Apply volatility cap
        if self.current_volatility > self.volatility_cap {
            self.current_volatility = self.volatility_cap;
        }

        // Update state
        self.previous_price = current_price.raw();
        self.last_update_slot = current_slot;
        self.update_count += 1;

        Ok(())
    }

    /// Get current volatility in basis points
    /// # Returns: Current volatility estimate
    #[inline(always)]
    pub fn get_volatility(&self) -> u32 {
        self.current_volatility
    }

    /// Check if volatility is above threshold (used for dynamic fees)
    /// # Arguments:
    /// * `threshold` - Volatility threshold in basis points
    /// # Returns: `true` if current volatility exceeds threshold, `false` otherwise
    #[inline(always)]
    pub fn is_high_volatility(&self, threshold: u32) -> bool {
        self.current_volatility > threshold
    }
}
