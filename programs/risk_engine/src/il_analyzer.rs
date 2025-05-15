//! Calculates Impermanent Loss (IL) percentage for a liquidity position.
//! This implementation uses fixed-point arithmetic with u128/i128 and U256 for on-chain compatibility.
//!
//! The calculation is based on the formula: IL = (2 * sqrt(k)) / (1 + k) - 1, where k = P_current / P_initial.
//! This is equivalent to: IL = -(sqrt(k) - 1)^2 / (sqrt(k)^2 + 1).
//! We use sqrt(k) = S_current / S_initial, where S is the square root price (sqrt_price_q64).
//!
//! The output is an i128 representing the IL percentage scaled by `IL_PERCENTAGE_SCALE`.
//! E.g., a return value of -50_000_000 means -5% IL if `IL_PERCENTAGE_SCALE` is 10^9.
use anchor_lang::prelude::*;
// Assuming AmmPositionData is a simplified struct mirroring necessary fields
// from amm_core::PositionData for IL calculation.
// Or, you pass the amm_core::PositionData account directly.
use amm_core::math as amm_math;
use primitive_types::U256; // For U256 operations
/// Scaling factor for the final IL percentage result.
/// A value of 10^9 means 9 decimal places of precision for the percentage.
pub(crate) const IL_PERCENTAGE_SCALE: u128 = 1_000_000_000; // 10^9

// Simplified IL calculation based on Section 3.1 & 3.2.1
// This is a conceptual guide; actual implementation needs careful fixed-point math.
// For MVP, we might focus on the percentage IL.
pub fn calculate_current_il_percentage(
    position_tick_lower: i32,
    position_tick_upper: i32,
    position_entry_sqrt_price_q64: u128, // Sqrt price when position was entered/last rebalanced
    current_sqrt_price_q64: u128,
) -> Result<i128> {
    // Return scaled 0 if initial price was zero or current price is zero.
    // If initial price is zero, the ratio is undefined.
    // If current price is zero, sqrt(k) is zero, IL is (0/1) - 1 = -1 (-100%).
    // However, a zero price is often an invalid state for IL calculation context.
    // Let's return 0 scaled for simplicity in these edge cases, matching the original f64 0.0.
    if position_entry_sqrt_price_q64 == 0 {
        return Ok(0);
    }

    // Check if current price tick is within the position range.
    // If outside, IL calculation is different (comparing value of assets if held vs one-sided LP).
    // For simplicity, we return 0 scaled if outside the range, matching the original f64 0.0 behavior.
    // In a real system, you might calculate the actual out-of-range IL.
    let p_current_tick = amm_math::sqrt_price_q64_to_tick(current_sqrt_price_q64)?;

    if p_current_tick >= position_tick_lower && p_current_tick < position_tick_upper {
        // Calculate IL using fixed-point arithmetic.
        // Formula: IL = -(S_current - S_initial)^2 / (S_current^2 + S_initial^2)
        // where S = sqrt_price_q64

        let s_current_u256 = U256::from(current_sqrt_price_q64);
        let s_initial_u256 = U256::from(position_entry_sqrt_price_q64);

        // Calculate (S_current - S_initial)^2
        let diff_u256 = if s_current_u256 >= s_initial_u256 {
            s_current_u256 - s_initial_u256
        } else {
            s_initial_u256 - s_current_u256
        };
        let diff_sq_u256 = diff_u256 * diff_u256;

        // Calculate S_current^2 + S_initial^2
        let s_current_sq_u256 = s_current_u256 * s_current_u256;
        let s_initial_sq_u256 = s_initial_u256 * s_initial_u256;
        let denominator_u256 = s_current_sq_u256 + s_initial_sq_u256;

        // Avoid division by zero. This should not happen if initial_sqrt_price is non-zero,
        // as s_initial_sq_u256 will be non-zero.
        if denominator_u256.is_zero() {
            return Ok(0); // Should be caught by initial check, but defensive
        }

        // Calculate the ratio: diff_sq_u256 / denominator_u256
        // Scale the numerator before division to maintain precision for the fractional part.
        // We want the result scaled by 100 * IL_PERCENTAGE_SCALE for percentage.
        let total_scale_u256 = U256::from(100) * U256::from(IL_PERCENTAGE_SCALE);
        let numerator_scaled_u256 = diff_sq_u256 * total_scale_u256;

        let ratio_scaled_u256 = numerator_scaled_u256 / denominator_u256;

        // The formula result is -(ratio). Convert to i128 and negate.
        // ratio_scaled_u256 is scaled by 100 * IL_PERCENTAGE_SCALE.
        // The result fits in i128 because the maximum absolute value of IL is 1 (or 100%).
        // So the maximum scaled value is 1 * 100 * IL_PERCENTAGE_SCALE, which fits in i128.
        let il_percentage_scaled_abs_i128 = ratio_scaled_u256.as_u128() as i128;

        Ok(-il_percentage_scaled_abs_i128)
    } else {
        // Position is out of range, IL calculation is different (value of assets if held vs one-sided LP)
        // For MVP, can return 0 or a simplified out-of-range IL.
        Ok(0) // Simplified for MVP
    }
}
