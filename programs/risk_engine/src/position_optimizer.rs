//! This module calculates optimal liquidity boundaries for a position.
//! It uses fixed-point arithmetic throughout to avoid floating-point numbers.
use crate::errors::RiskEngineError as ErrorCode; // Assuming this is the correct path
use amm_core::constants::{MAX_TICK, MIN_TICK}; // Assuming these are pub
use amm_core::math as amm_math;
use anchor_lang::prelude::*; // For tick_to_sqrt_price_q64 and sqrt_price_q64_to_tick
use primitive_types::U256;

/// Scaling factor for general precision in intermediate calculations. 10^12.
const PRECISION_SCALE: u128 = 1_000_000_000_000;

/// Scaling factor for the input annualized volatility.
/// This should match the scaling factor used when calculating volatility (e.g., from volatility_detector.rs).
/// Assuming it's 10^9 as per volatility_detector.rs example.
const VOLATILITY_INPUT_SCALE: u128 = 1_000_000_000;

/// Alpha factor numerator for price range calculation (e.g., 1.5 = 3/2).
const ALPHA_MVP_NUM: u128 = 3;
/// Alpha factor denominator for price range calculation.
const ALPHA_MVP_DEN: u128 = 2;

/// Time horizon for range calculation, in days (numerator). E.g., 1 day.
const TIME_HORIZON_DAYS_NUM: u128 = 1;
/// Time horizon for range calculation, days in a year (denominator). E.g., 365 days.
const DAYS_IN_YEAR_DEN: u128 = 365;

/// Calculates the integer square root of a u128 number using the Babylonian method.
/// Returns floor(sqrt(n)).
/// Note: In a larger project, this would ideally be in a shared math utility module.
fn isqrt_u128(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + n / x) / 2;
    if y >= x {
        if x * x > n && x > 0 {
            return x - 1;
        }
        return x;
    }
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

// Simplified version of Section 4.1.2 for MVP
// Returns (new_lower_sqrt_price_q64, new_upper_sqrt_price_q64)
pub fn calculate_optimal_boundaries_mvp(
    current_sqrt_price_q64: u128,
    volatility_annualized_scaled: u128, // e.g., 800_000_000 for 80% annualized vol if VOLATILITY_INPUT_SCALE is 10^9
    pool_tick_spacing: u16,
) -> Result<(i32, i32)> {
    if current_sqrt_price_q64 == 0 {
        return Ok((MIN_TICK, MAX_TICK)); // Default to full range or error
    }

    // Calculate price_range_factor = alpha * sigma * sqrt(T) using fixed-point arithmetic.
    // All components will be scaled by PRECISION_SCALE or VOLATILITY_INPUT_SCALE.

    // alpha_scaled = (ALPHA_MVP_NUM / ALPHA_MVP_DEN) * PRECISION_SCALE
    let alpha_scaled: u128 = (ALPHA_MVP_NUM * PRECISION_SCALE) / ALPHA_MVP_DEN;

    // sigma_scaled is volatility_annualized_scaled (input, scaled by VOLATILITY_INPUT_SCALE)

    // sqrt_T_scaled = sqrt(TIME_HORIZON_DAYS_NUM / DAYS_IN_YEAR_DEN) * PRECISION_SCALE
    // sqrt(A/B) * S = sqrt(A*B*S*S) / B
    let sqrt_t_numerator =
        isqrt_u128(TIME_HORIZON_DAYS_NUM * DAYS_IN_YEAR_DEN * PRECISION_SCALE * PRECISION_SCALE);
    let sqrt_t_scaled: u128 = sqrt_t_numerator / DAYS_IN_YEAR_DEN;

    // price_range_factor_numerator_u256 has scale: PRECISION_SCALE * VOLATILITY_INPUT_SCALE * PRECISION_SCALE
    let price_range_factor_numerator_u256: U256 = U256::from(alpha_scaled)
        * U256::from(volatility_annualized_scaled)
        * U256::from(sqrt_t_scaled);

    // We want price_range_factor_scaled to have scale: PRECISION_SCALE
    // So, divide by (VOLATILITY_INPUT_SCALE * PRECISION_SCALE)
    let price_range_factor_denominator_u256: U256 =
        U256::from(VOLATILITY_INPUT_SCALE) * U256::from(PRECISION_SCALE);

    if price_range_factor_denominator_u256.is_zero() {
        // This should not happen if scales are non-zero
        return Err(ErrorCode::CalculationError.into());
    }

    let price_range_factor_scaled: u128 =
        (price_range_factor_numerator_u256 / price_range_factor_denominator_u256).as_u128();

    // Calculate multipliers: (1 +/- price_range_factor_scaled/PRECISION_SCALE)
    // lower_multiplier_scaled = (1.0 - price_range_factor) * PRECISION_SCALE
    // upper_multiplier_scaled = (1.0 + price_range_factor) * PRECISION_SCALE
    let one_scaled = PRECISION_SCALE;
    let min_lower_multiplier_scaled = PRECISION_SCALE / 100; // Corresponds to 0.01

    let mut lower_multiplier_scaled = one_scaled.saturating_sub(price_range_factor_scaled);
    lower_multiplier_scaled = lower_multiplier_scaled.max(min_lower_multiplier_scaled);

    let upper_multiplier_scaled = one_scaled + price_range_factor_scaled;

    // new_sqrt_price = current_sqrt_price * sqrt(multiplier_actual)
    // sqrt(multiplier_actual) = sqrt(multiplier_scaled / PRECISION_SCALE)
    //                         = sqrt(multiplier_scaled * PRECISION_SCALE) / PRECISION_SCALE
    // Let sqrt_multiplier_intermediate = isqrt(multiplier_scaled * PRECISION_SCALE)
    // This intermediate is sqrt(multiplier_actual) * PRECISION_SCALE

    let sqrt_lower_multiplier_intermediate = isqrt_u128(
        lower_multiplier_scaled
            .checked_mul(PRECISION_SCALE)
            .ok_or(ErrorCode::Overflow)?,
    );
    let sqrt_upper_multiplier_intermediate = isqrt_u128(
        upper_multiplier_scaled
            .checked_mul(PRECISION_SCALE)
            .ok_or(ErrorCode::Overflow)?,
    );

    // new_sqrt_price_q64 = (current_sqrt_price_q64 * sqrt_multiplier_intermediate) / PRECISION_SCALE
    let new_lower_sqrt_price_q64 = (U256::from(current_sqrt_price_q64)
        * U256::from(sqrt_lower_multiplier_intermediate)
        / U256::from(PRECISION_SCALE))
    .as_u128();
    let new_upper_sqrt_price_q64 = (U256::from(current_sqrt_price_q64)
        * U256::from(sqrt_upper_multiplier_intermediate)
        / U256::from(PRECISION_SCALE))
    .as_u128();

    let mut new_lower_tick = amm_math::sqrt_price_q64_to_tick(new_lower_sqrt_price_q64)?;
    let mut new_upper_tick = amm_math::sqrt_price_q64_to_tick(new_upper_sqrt_price_q64)?;

    // Align to tick_spacing
    let tick_spacing_i32 = pool_tick_spacing as i32;
    new_lower_tick = (new_lower_tick / tick_spacing_i32) * tick_spacing_i32;
    new_upper_tick =
        ((new_upper_tick + tick_spacing_i32 - 1) / tick_spacing_i32) * tick_spacing_i32; // Ceiling division for upper

    // Ensure lower < upper and within bounds
    if new_lower_tick >= new_upper_tick {
        // Fallback or error, e.g., make a minimum width range around current price
        let current_tick = amm_math::sqrt_price_q64_to_tick(current_sqrt_price_q64)?;
        new_lower_tick = ((current_tick - tick_spacing_i32) / tick_spacing_i32) * tick_spacing_i32;
        new_upper_tick = ((current_tick + tick_spacing_i32) / tick_spacing_i32) * tick_spacing_i32;
        if new_lower_tick >= new_upper_tick {
            // if current_tick was 0 and spacing makes them overlap
            new_upper_tick = new_lower_tick + tick_spacing_i32;
        }
    }

    Ok((
        new_lower_tick.clamp(MIN_TICK, MAX_TICK - tick_spacing_i32),
        new_upper_tick.clamp(MIN_TICK + tick_spacing_i32, MAX_TICK),
    ))
}
