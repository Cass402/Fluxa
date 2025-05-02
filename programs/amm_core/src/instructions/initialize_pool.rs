/// Initialize Pool Instruction Module
///
/// This module implements the instruction for creating a new liquidity pool within the Fluxa AMM.
/// A liquidity pool represents a specific market for a token pair with a defined fee tier,
/// enabling concentrated liquidity provision and swaps between the paired tokens.
///
/// The initialization process creates a new pool account, sets up token vaults, and
/// establishes the initial price and tick for the market.
use crate::constants;
use crate::errors::ErrorCode;
use crate::math;
use crate::InitializePool;
use anchor_lang::prelude::*;

/// Handler function for initializing a new liquidity pool
///
/// This function initializes a new pool with specified parameters, creating
/// the core market infrastructure for a specific token pair and fee tier.
/// The pool starts with zero liquidity until positions are created.
///
/// This operation includes:
/// - Validating all parameters for correctness
/// - Aligning the initial price with the appropriate tick grid
/// - Configuring the pool with the specified fee tier
/// - Registering the pool with the token pair for discoverability
///
/// # Parameters
/// * `ctx` - The context containing all accounts involved in the operation
/// * `initial_sqrt_price` - The initial square root price in Q64.64 fixed-point format
/// * `fee_tier` - The fee tier for the pool in basis points (e.g., 3000 for 0.3%)
///
/// # Returns
/// * `Result<()>` - Result indicating success or failure
///
/// # Errors
/// * `ErrorCode::InvalidTickSpacing` - If the fee tier is not a recognized value
/// * `ErrorCode::MintsMustDiffer` - If both token mints are the same
/// * `ErrorCode::InvalidInitialPrice` - If the initial price is outside valid range
/// * `ErrorCode::VaultSetupFailed` - If token vaults cannot be properly initialized
pub fn handler(
    ctx: Context<InitializePool>,
    initial_sqrt_price: u128,
    fee_tier: u16,
) -> Result<()> {
    // === Parameter Validation ===

    // Ensure the two token mints are different
    require!(
        ctx.accounts.token_a_mint.key() != ctx.accounts.token_b_mint.key(),
        ErrorCode::MintsMustDiffer
    );

    // Validate the initial price is within acceptable bounds
    // Only check that it's greater than zero since the type system enforces upper bound
    require!(initial_sqrt_price > 0, ErrorCode::InvalidInitialPrice);

    // Validate fee tier against constants
    require!(
        fee_tier == constants::FEE_TIER_LOW
            || fee_tier == constants::FEE_TIER_MEDIUM
            || fee_tier == constants::FEE_TIER_HIGH,
        ErrorCode::InvalidTickSpacing
    );

    // === Vault Validation ===
    // Moving this validation before any mutable borrowing to avoid ownership conflicts
    validate_token_vaults(&ctx)?;

    // Calculate tick spacing based on fee tier
    let tick_spacing = match fee_tier {
        constants::FEE_TIER_LOW => constants::TICK_SPACING_LOW,
        constants::FEE_TIER_MEDIUM => constants::TICK_SPACING_MEDIUM,
        constants::FEE_TIER_HIGH => constants::TICK_SPACING_HIGH,
        _ => unreachable!(), // We already validated fee_tier above
    };

    // === Pool Initialization ===

    let pool = &mut ctx.accounts.pool;
    let token_pair = &mut ctx.accounts.token_pair;

    // Initialize core pool data
    pool.authority = ctx.accounts.payer.key();
    pool.token_a_mint = ctx.accounts.token_a_mint.key();
    pool.token_b_mint = ctx.accounts.token_b_mint.key();
    pool.token_a_vault = ctx.accounts.token_a_vault.key();
    pool.token_b_vault = ctx.accounts.token_b_vault.key();
    pool.sqrt_price = initial_sqrt_price;

    // Calculate current_tick from initial_sqrt_price using math module
    // and ensure it's aligned with the chosen tick spacing
    let raw_tick = math::sqrt_price_to_tick(initial_sqrt_price)?;
    pool.current_tick = math::nearest_usable_tick(raw_tick, tick_spacing);

    // Adjust sqrt_price to match the aligned tick for consistency
    let adjusted_sqrt_price = math::tick_to_sqrt_price(pool.current_tick)?;
    if adjusted_sqrt_price != pool.sqrt_price {
        msg!(
            "Adjusted sqrt_price from {} to {} to align with tick spacing",
            pool.sqrt_price,
            adjusted_sqrt_price
        );
        pool.sqrt_price = adjusted_sqrt_price;
    }

    // Initialize fee and liquidity settings
    pool.fee_tier = fee_tier;
    pool.fee_growth_global_a = 0;
    pool.fee_growth_global_b = 0;

    // Default protocol fee to 5% of trading fees (configurable by governance)
    pool.protocol_fee = 500;

    // Pool starts with zero liquidity until positions are created
    pool.liquidity = 0;
    pool.position_count = 0;

    // === Token Pair Integration ===

    // Register this pool with the token pair for discoverability
    token_pair.add_pool(pool.key(), fee_tier)?;

    // === Event Emission ===

    // Emit event for pool initialization
    emit!(PoolInitializedEvent {
        pool_id: pool.key(),
        token_a_mint: pool.token_a_mint,
        token_b_mint: pool.token_b_mint,
        fee_tier,
        initial_price: initial_sqrt_price,
        initial_tick: pool.current_tick,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "Pool initialized: {} <> {} with fee tier: {}",
        ctx.accounts.token_a_mint.key().to_string(),
        ctx.accounts.token_b_mint.key().to_string(),
        fee_tier
    );

    msg!(
        "Initial sqrt price: {}, initial tick: {}",
        pool.sqrt_price,
        pool.current_tick
    );

    Ok(())
}

/// Validates that token vaults are properly configured for the pool
///
/// Ensures the token vaults are:
/// - Owned by the token program
/// - Associated with the correct mint
/// - Have proper authority settings
///
/// # Arguments
/// * `ctx` - The context containing all accounts involved in the operation
///
/// # Returns
/// * `Result<()>` - Result indicating success or failure
fn validate_token_vaults(ctx: &Context<InitializePool>) -> Result<()> {
    // Validate token A vault
    require!(
        ctx.accounts.token_a_vault.owner == ctx.accounts.token_program.key(),
        ErrorCode::InvalidVault
    );

    require!(
        ctx.accounts.token_a_vault.mint == ctx.accounts.token_a_mint.key(),
        ErrorCode::InvalidVault
    );

    // Validate token B vault
    require!(
        ctx.accounts.token_b_vault.owner == ctx.accounts.token_program.key(),
        ErrorCode::InvalidVault
    );

    require!(
        ctx.accounts.token_b_vault.mint == ctx.accounts.token_b_mint.key(),
        ErrorCode::InvalidVault
    );

    Ok(())
}

/// Event emitted when a new liquidity pool is initialized
///
/// This event provides data for indexers and UIs to track pool creation
/// and display relevant information to users.
#[event]
pub struct PoolInitializedEvent {
    /// The address of the newly created pool
    pub pool_id: Pubkey,

    /// Token A mint address
    pub token_a_mint: Pubkey,

    /// Token B mint address
    pub token_b_mint: Pubkey,

    /// Fee tier in basis points
    pub fee_tier: u16,

    /// Initial square root price
    pub initial_price: u128,

    /// Initial tick index
    pub initial_tick: i32,

    /// Timestamp of pool creation
    pub timestamp: i64,
}
