use crate::*;
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<InitializeILMitigation>,
    pool_id: Pubkey,
    volatility_window: u64,
    adjustment_threshold: u64,
    max_adjustment_factor: u64,
    rebalance_cooldown: u64,
    reserve_imbalance_threshold: u64,
) -> Result<()> {
    let il_params = &mut ctx.accounts.il_params;
    let volatility_state = &mut ctx.accounts.volatility_state;
    let price_history = &mut ctx.accounts.price_history;

    // Initialize IL mitigation parameters
    il_params.pool_id = pool_id;
    il_params.volatility_window = volatility_window;
    il_params.adjustment_threshold = adjustment_threshold;
    il_params.max_adjustment_factor = max_adjustment_factor;
    il_params.rebalance_cooldown = rebalance_cooldown;
    il_params.reserve_imbalance_threshold = reserve_imbalance_threshold;

    // Initialize volatility state
    volatility_state.short_term_volatility = 0;
    volatility_state.medium_term_volatility = 0;
    volatility_state.long_term_volatility = 0;
    volatility_state.volatility_acceleration = 0;
    volatility_state.last_calculation = Clock::get()?.unix_timestamp;

    // Initialize price history
    price_history.current_index = 0;
    price_history.data_count = 0;

    // Initialize arrays to zero
    for i in 0..price_history.prices.len() {
        price_history.prices[i] = 0;
        price_history.timestamps[i] = 0;
    }

    Ok(())
}
