use crate::*;
use amm_core::pool_state::PoolState;
use anchor_lang::prelude::*;

/// Handler for checking rebalance conditions
///
/// This function evaluates whether a position should be rebalanced based on:
/// 1. Time since last rebalance (cooldown period)
/// 2. Current market volatility
/// 3. Current price relative to position boundaries
/// 4. Virtual reserves imbalance
pub fn handler(ctx: Context<CheckRebalanceCondition>, position_id: Pubkey) -> Result<()> {
    let rebalance_state = &mut ctx.accounts.rebalance_state;
    let volatility_state = &ctx.accounts.volatility_state;
    let il_params = &ctx.accounts.il_params;

    // Get current timestamp
    let current_timestamp = Clock::get()?.unix_timestamp;

    // Check if cooldown period has passed
    let time_since_last_rebalance = current_timestamp - rebalance_state.last_rebalance;
    require!(
        time_since_last_rebalance >= il_params.rebalance_cooldown as i64,
        ErrorCode::RebalanceCooldownNotMet
    );

    // Check if volatility exceeds threshold
    // For this check, we'll use a weighted combination of short, medium, and long-term volatility
    let effective_volatility = calculate_effective_volatility(volatility_state);

    // Create a pool state instance to access virtual reserves
    let pool = &mut ctx.accounts.pool;
    let pool_state = PoolState::new(pool);

    // Get virtual reserves to analyze pool state
    let virtual_reserves = pool_state.get_virtual_reserves()?;
    let reserve_ratio = calculate_reserve_ratio(virtual_reserves.0, virtual_reserves.1)?;

    // Check if reserve imbalance exceeds threshold (indicating high IL potential)
    let reserve_imbalance_threshold = il_params.reserve_imbalance_threshold;
    let reserve_imbalance_factor = calculate_reserve_imbalance_factor(reserve_ratio);

    // Current price information
    let current_tick = pool.current_tick;
    let current_lower_tick = rebalance_state.original_lower_tick;
    let current_upper_tick = rebalance_state.original_upper_tick;

    // Determine if rebalance is needed based on combined factors
    let needs_rebalance = effective_volatility >= il_params.adjustment_threshold
        || reserve_imbalance_factor >= reserve_imbalance_threshold
        || is_position_at_boundary(current_tick, current_lower_tick, current_upper_tick);

    require!(needs_rebalance, ErrorCode::NoRebalanceNeeded);

    // Calculate optimal boundaries based on current conditions
    let (optimal_lower_tick, optimal_upper_tick) = calculate_optimal_boundaries(
        current_lower_tick,
        current_upper_tick,
        effective_volatility,
        il_params.max_adjustment_factor,
        reserve_ratio,
        current_tick,
    )?;

    // Update the rebalance state with the new optimal boundaries
    rebalance_state.optimal_lower_tick = optimal_lower_tick;
    rebalance_state.optimal_upper_tick = optimal_upper_tick;

    // Calculate and store estimated IL savings
    let estimated_savings = estimate_il_savings(
        virtual_reserves.0,
        virtual_reserves.1,
        current_lower_tick,
        current_upper_tick,
        optimal_lower_tick,
        optimal_upper_tick,
        effective_volatility,
    )?;

    rebalance_state.estimated_il_saved = estimated_savings;
    rebalance_state.last_rebalance = current_timestamp;

    Ok(())
}

/// Calculate a weighted effective volatility from multiple time frames
fn calculate_effective_volatility(volatility_state: &VolatilityState) -> u64 {
    // Weighted combination giving more weight to short-term volatility,
    // but also considering medium and long-term trends
    let weight_short = 60; // 60%
    let weight_medium = 30; // 30%
    let weight_long = 10; // 10%

    let effective_vol = (volatility_state.short_term_volatility * weight_short
        + volatility_state.medium_term_volatility * weight_medium
        + volatility_state.long_term_volatility * weight_long)
        / 100;

    // If volatility is accelerating rapidly, increase the effective value
    if volatility_state.volatility_acceleration > 100 {
        return effective_vol + (volatility_state.volatility_acceleration as u64 / 10);
    }

    effective_vol
}

/// Calculate optimal tick boundaries based on volatility and virtual reserves
fn calculate_optimal_boundaries(
    current_lower_tick: i32,
    current_upper_tick: i32,
    volatility: u64,
    max_adjustment_factor: u64,
    reserve_ratio: f64,
    current_tick: i32,
) -> Result<(i32, i32)> {
    // Calculate the current range width
    let current_width = current_upper_tick - current_lower_tick;

    // Calculate adjustment factor based on volatility, capped by maximum
    // Higher volatility = wider range to reduce IL
    let volatility_factor = std::cmp::min(volatility, max_adjustment_factor);

    // Calculate range expansion (as a percentage of current width)
    let mut expansion_percentage = volatility_factor as f64 / 10000.0; // convert basis points to percentage

    // Adjust expansion based on reserve ratio imbalance
    // If reserves are imbalanced, we need a wider range
    let balance_factor = (reserve_ratio - 1.0).abs().min(1.0);
    expansion_percentage += balance_factor * 0.1; // Add up to 10% extra expansion for imbalanced reserves

    // Calculate tick expansion as a proportion of current width
    let tick_expansion = (current_width as f64 * expansion_percentage) as i32;

    // Ensure at least 1 tick expansion if adjustment is needed
    let tick_expansion = std::cmp::max(tick_expansion, 1);

    // Calculate new boundaries with adjustments to center around current price
    // This helps rebalance the position to better include the current price
    let price_centered_adjustment =
        (current_tick - (current_lower_tick + current_upper_tick) / 2) / 2;

    let new_lower_tick = current_lower_tick - tick_expansion + price_centered_adjustment;
    let new_upper_tick = current_upper_tick + tick_expansion + price_centered_adjustment;

    // TODO: Validate ticks are within allowed range for the protocol

    Ok((new_lower_tick, new_upper_tick))
}

/// Calculate the ratio between token reserves to measure imbalance
fn calculate_reserve_ratio(reserve_a: u64, reserve_b: u64) -> Result<f64> {
    // Avoid division by zero
    require!(
        reserve_a > 0 && reserve_b > 0,
        ErrorCode::InvalidReserveAmount
    );

    // Calculate ratio of reserves (A:B)
    let ratio = reserve_a as f64 / reserve_b as f64;

    // Return the ratio in a way that it's always >= 1.0
    // This simplifies threshold comparisons
    Ok(if ratio >= 1.0 { ratio } else { 1.0 / ratio })
}

/// Calculate a factor representing how imbalanced the reserves are
/// Returns a value from 0 (perfectly balanced) to 10000 (highly imbalanced)
fn calculate_reserve_imbalance_factor(reserve_ratio: f64) -> u64 {
    // Convert reserve ratio to an imbalance factor in basis points
    // A ratio of 1.0 means perfectly balanced (factor = 0)
    // Higher ratio means more imbalance
    let imbalance = (reserve_ratio - 1.0).min(1.0); // Cap at 100% imbalance
    (imbalance * 10000.0) as u64 // Convert to basis points
}

/// Check if position is at or near a boundary, suggesting repositioning might be needed
fn is_position_at_boundary(current_tick: i32, lower_tick: i32, upper_tick: i32) -> bool {
    // Calculate how close the current price is to either boundary as a percentage
    let range_width = upper_tick - lower_tick;
    let lower_distance = current_tick - lower_tick;
    let upper_distance = upper_tick - current_tick;

    // If price is within 10% of either boundary, consider rebalancing
    const BOUNDARY_THRESHOLD: i32 = 10; // 10%
    let threshold_ticks = std::cmp::max(range_width * BOUNDARY_THRESHOLD / 100, 1);

    lower_distance <= threshold_ticks || upper_distance <= threshold_ticks
}

/// Estimate IL savings from rebalancing based on current market conditions
fn estimate_il_savings(
    virtual_reserve_a: u64,
    virtual_reserve_b: u64,
    current_lower_tick: i32,
    current_upper_tick: i32,
    new_lower_tick: i32,
    new_upper_tick: i32,
    volatility: u64,
) -> Result<u64> {
    // Simple model: savings are proportional to:
    // 1. Total value in the position (reserve_a + reserve_b, simplified)
    // 2. Volatility level
    // 3. How much the range is being expanded

    let position_value = virtual_reserve_a.saturating_add(virtual_reserve_b);
    let current_width = (current_upper_tick - current_lower_tick) as u64;
    let new_width = (new_upper_tick - new_lower_tick) as u64;

    // No savings if not expanding
    if new_width <= current_width {
        return Ok(0);
    }

    // Calculate width increase percentage
    let width_increase = ((new_width - current_width) * 10000) / current_width; // in basis points

    // IL reduction model: higher volatility and larger width increase = more savings
    // This is a simplification - a real model would use price movement simulations
    let volatility_factor = volatility / 100; // Scale down volatility
    let il_reduction_bps = volatility_factor * width_increase / 100;

    // Calculate estimated savings in token units (simplified)
    let savings = (position_value as u128 * il_reduction_bps as u128 / 10000) as u64;

    Ok(savings)
}
