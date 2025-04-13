use crate::*;
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<CheckRebalanceCondition>,
    position_id: Pubkey,
) -> Result<()> {
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
    require!(
        effective_volatility >= il_params.adjustment_threshold,
        ErrorCode::VolatilityBelowThreshold
    );
    
    // TODO: Fetch the position details from AMM core program (cross-program invocation)
    // For now, we'll use the data stored in the rebalance state
    let current_lower_tick = rebalance_state.original_lower_tick;
    let current_upper_tick = rebalance_state.original_upper_tick;
    
    // Calculate optimal boundaries based on current volatility
    let (optimal_lower_tick, optimal_upper_tick) = calculate_optimal_boundaries(
        current_lower_tick,
        current_upper_tick,
        effective_volatility,
        il_params.max_adjustment_factor,
    )?;
    
    // Update the rebalance state with the new optimal boundaries
    rebalance_state.optimal_lower_tick = optimal_lower_tick;
    rebalance_state.optimal_upper_tick = optimal_upper_tick;
    
    // TODO: Calculate and update estimated IL savings
    
    Ok(())
}

/// Calculate a weighted effective volatility from multiple time frames
fn calculate_effective_volatility(volatility_state: &VolatilityState) -> u64 {
    // Weighted combination giving more weight to short-term volatility,
    // but also considering medium and long-term trends
    let weight_short = 60;   // 60%
    let weight_medium = 30;  // 30%
    let weight_long = 10;    // 10%
    
    let effective_vol = 
        (volatility_state.short_term_volatility * weight_short +
         volatility_state.medium_term_volatility * weight_medium +
         volatility_state.long_term_volatility * weight_long) / 100;
    
    // If volatility is accelerating rapidly, increase the effective value
    if volatility_state.volatility_acceleration > 100 {
        return effective_vol + (volatility_state.volatility_acceleration as u64 / 10);
    }
    
    effective_vol
}

/// Calculate optimal tick boundaries based on volatility
fn calculate_optimal_boundaries(
    current_lower_tick: i32,
    current_upper_tick: i32,
    volatility: u64,
    max_adjustment_factor: u64,
) -> Result<(i32, i32)> {
    // Calculate the current range width
    let current_width = current_upper_tick - current_lower_tick;
    
    // Calculate adjustment factor based on volatility, capped by maximum
    // Higher volatility = wider range to reduce IL
    let volatility_factor = std::cmp::min(volatility, max_adjustment_factor);
    
    // Calculate range expansion (as a percentage of current width)
    let expansion_percentage = volatility_factor as f64 / 10000.0; // convert basis points to percentage
    let tick_expansion = (current_width as f64 * expansion_percentage) as i32;
    
    // Ensure at least 1 tick expansion if adjustment is needed
    let tick_expansion = std::cmp::max(tick_expansion, 1);
    
    // Calculate new boundaries
    let new_lower_tick = current_lower_tick - tick_expansion;
    let new_upper_tick = current_upper_tick + tick_expansion;
    
    // TODO: Validate ticks are within allowed range
    
    Ok((new_lower_tick, new_upper_tick))
}