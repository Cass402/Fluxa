use crate::*;
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<InitializePool>,
    initial_sqrt_price: u128,
    fee_tier: u16,
) -> Result<()> {
    let pool = &mut ctx.accounts.pool;

    // Initialize pool data
    pool.authority = ctx.accounts.payer.key();
    pool.token_a_mint = ctx.accounts.token_a_mint.key();
    pool.token_b_mint = ctx.accounts.token_b_mint.key();
    pool.token_a_vault = ctx.accounts.token_a_vault.key();
    pool.token_b_vault = ctx.accounts.token_b_vault.key();
    pool.sqrt_price = initial_sqrt_price;
    pool.current_tick = 0; // Will be calculated based on initial_sqrt_price
    pool.fee_tier = fee_tier;
    pool.fee_growth_global_a = 0;
    pool.fee_growth_global_b = 0;
    pool.protocol_fee = 500; // 5% of fee goes to protocol (can be adjusted via governance)
    pool.liquidity = 0;
    pool.position_count = 0;

    // TODO: Calculate current_tick from initial_sqrt_price using math module

    Ok(())
}