use crate::constants;
use crate::errors::ErrorCode;
/// Initialize Pool Instruction Module
///
/// This module implements the instruction for creating a new liquidity pool within the Fluxa AMM.
/// A liquidity pool represents a specific market for a token pair with a defined fee tier,
/// enabling concentrated liquidity provision and swaps between the paired tokens.
///
/// The initialization process creates a new pool account, sets up token vaults, and
/// establishes the initial price and tick for the market.
use crate::math;
use crate::InitializePool;
use anchor_lang::prelude::*;
/// Handler function for initializing a new liquidity pool
///
/// This function initializes a new pool with specified parameters, creating
/// the core market infrastructure for a specific token pair and fee tier.
/// The pool starts with zero liquidity until positions are created.
///
/// # Arguments
/// * `ctx` - The context containing all accounts involved in the operation
/// * `initial_sqrt_price` - The initial square root price in Q64.64 fixed-point format
/// * `fee_tier` - The fee tier for the pool in basis points (e.g., 3000 for 0.3%)
///
/// # Returns
/// * `Result<()>` - Result indicating success or failure
///
/// # Errors
/// * `ErrorCode::InvalidTickSpacing` - If the fee tier is not a recognized value
pub fn handler(
    ctx: Context<InitializePool>,
    initial_sqrt_price: u128,
    fee_tier: u16,
) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    let token_pair = &mut ctx.accounts.token_pair;

    // Validate fee tier against constants
    require!(
        fee_tier == constants::FEE_TIER_LOW
            || fee_tier == constants::FEE_TIER_MEDIUM
            || fee_tier == constants::FEE_TIER_HIGH,
        ErrorCode::InvalidTickSpacing
    );

    // Initialize pool data
    pool.authority = ctx.accounts.payer.key();
    pool.token_a_mint = ctx.accounts.token_a_mint.key();
    pool.token_b_mint = ctx.accounts.token_b_mint.key();
    pool.token_a_vault = ctx.accounts.token_a_vault.key();
    pool.token_b_vault = ctx.accounts.token_b_vault.key();
    pool.sqrt_price = initial_sqrt_price;

    // Calculate current_tick from initial_sqrt_price using math module
    pool.current_tick = math::sqrt_price_to_tick(initial_sqrt_price)?;

    pool.fee_tier = fee_tier;
    pool.fee_growth_global_a = 0;
    pool.fee_growth_global_b = 0;
    pool.protocol_fee = 500; // 5% of fee goes to protocol (can be adjusted via governance)
    pool.liquidity = 0;
    pool.position_count = 0;

    // Register the pool with the token pair
    token_pair.add_pool(pool.key(), fee_tier)?;

    msg!(
        "Pool initialized: {} <> {} with fee tier: {}",
        ctx.accounts.token_a_mint.key().to_string(),
        ctx.accounts.token_b_mint.key().to_string(),
        fee_tier
    );

    msg!(
        "Initial sqrt price: {}, initial tick: {}",
        initial_sqrt_price,
        pool.current_tick
    );

    Ok(())
}
