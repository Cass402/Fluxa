use crate::error::PoolError;
use crate::math::core_arithmetic::Q64x64;
use crate::math::price_math::sqrt_price_to_tick;
use crate::state::factory::factory_shard::FactoryShard;
use crate::state::pool::pool_config::PoolConfig;
use crate::state::pool::pool_core::PoolCore;
use crate::state::pool::pool_security::PoolSecurity;
use crate::utils::constants::{
    DEFAULT_FEE_TIERS, MAX_SQRT_X64, MIN_SQRT_X64, TICK_SPACING_PER_FEE,
};
use crate::utils::event::PoolCreatedEvent;
use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

/// Validates fee tier against protocol-defined canonical mappings for security and consistency.
///
/// This function implements a critical safety check by ensuring only pre-approved fee tiers
/// can be used in pool creation. The design prevents several attack vectors:
/// - Malicious actors creating pools with excessive fees to trap users
/// - Fee tier manipulation to exploit pricing algorithms expecting specific values
/// - Inconsistent fee structures that could break automated market maker assumptions
///
/// ## Linear Search Trade-off Analysis
/// Uses linear search through TICK_SPACING_PER_FEE because:
/// - The array is very small (typically 3-5 elements), making linear search faster than
///   hash table overhead for this use case
/// - Avoids dynamic allocation required for HashMap, critical in constrained Solana runtime
/// - Provides deterministic gas costs regardless of fee tier position
/// - Enables compile-time verification of supported fee tiers
///
/// The Option return type enables safe handling of unsupported fee tiers without panics,
/// essential for user-facing pool creation functions that must gracefully handle invalid input.
fn expected_spacing(fee: u32) -> Option<u16> {
    TICK_SPACING_PER_FEE
        .into_iter()
        .find_map(|(f, s)| (f == fee).then_some(s))
}

/// Validates tick alignment to prevent liquidity fragmentation and maintain pricing integrity.
///
/// This seemingly simple check prevents a critical class of attacks and protocol violations:
/// - Misaligned initial ticks could create gaps in liquidity that break swap calculations
/// - Non-standard tick positions could exploit rounding errors in fixed-point math
/// - Invalid alignment could cause positions to be unreachable by normal trading activity
///
/// ## Mathematical Foundation
/// Uses rem_euclid instead of % operator to handle negative ticks correctly. The standard
/// modulo operator has different behavior for negative numbers across languages, but
/// rem_euclid provides consistent mathematical behavior essential for tick calculations
/// that can span both positive and negative price ranges.
///
/// ## Performance Optimization
/// Marked #[inline(always)] because this function is called frequently during pool creation
/// and position management. The function is trivial (single arithmetic operation) and
/// inlining eliminates function call overhead. However, this optimization must be used
/// judiciously as excessive inlining can increase code size and harm instruction cache performance.
#[inline(always)]
fn is_tick_aligned(tick: i32, spacing: u16) -> bool {
    tick.rem_euclid(spacing as i32) == 0
}

#[derive(Accounts)]
#[instruction(fee_tier: u32, tick_spacing: u16, initial_sqrt_price: Q64x64, factory_key: Pubkey, shard_index: u16)]
pub struct CreatePool<'info> {
    /// Primary pool state account with deterministic derivation and comprehensive validation.
    ///
    /// The seed structure [pool_core, token_0, token_1, fee_tier] ensures each unique
    /// trading pair and fee combination gets exactly one pool, preventing fragmentation
    /// and enabling predictable address computation for indexing and integration.
    ///
    /// Constraint validations serve critical security functions:
    /// - fee_tier <= 10_000 prevents exploitation through excessive fees that could trap users
    /// - token_0 < token_1 enforces canonical ordering that eliminates duplicate pools and
    ///   simplifies address computation (prevents both A/B and B/A pools for same assets)
    ///
    /// Space calculation includes 8-byte discriminator required by Anchor's account
    /// initialization, preventing subtle account size bugs that could cause runtime failures.
    #[account(
        init,
        payer = payer,
        seeds = [b"pool_core", token_0.key().as_ref(), token_1.key().as_ref(), &fee_tier.to_le_bytes()],
        bump,
        space = 8 + PoolCore::INIT_SPACE,
        constraint = fee_tier <= 10_000 @ PoolError::InvalidFeeTier,
        constraint = token_0.key() < token_1.key() @ PoolError::InvalidTokenOrder,
    )]
    pub pool_core: AccountLoader<'info, PoolCore>,

    /// Security state account enabling MEV protection and anomaly detection.
    ///
    /// Separated from core pool state to allow independent evolution of security features
    /// without affecting core trading logic. This architectural choice enables security
    /// upgrades and emergency responses without disrupting normal pool operations.
    #[account(
        init,
        payer = payer,
        seeds = [b"pool_security", pool_core.key().as_ref()],
        bump,
        space = 8 + PoolSecurity::INIT_SPACE,
    )]
    pub pool_security: AccountLoader<'info, PoolSecurity>,

    /// Configuration state account for protocol fees and operational parameters.
    ///
    /// Isolated from core state to enable governance updates to fees and limits
    /// without requiring pool state migrations. This separation supports dynamic
    /// protocol optimization based on market conditions and governance decisions.
    #[account(
        init,
        payer = payer,
        seeds = [b"pool_config", pool_core.key().as_ref()],
        bump,
        space = 8 + PoolConfig::INIT_SPACE,
    )]
    pub pool_config: AccountLoader<'info, PoolConfig>,

    /// Factory shard that will manage this pool.
    ///
    /// Validates that the shard exists and is properly initialized.
    /// Capacity validation is done in the instruction handler.
    #[account(
        mut,
        seeds = [b"shard", factory_key.as_ref(), &shard_index.to_le_bytes()],
        bump,
    )]
    pub factory_shard: AccountLoader<'info, FactoryShard>,

    /// Token mint accounts defining the trading pair with immutable references.
    ///
    /// Using Account<'info, Mint> instead of UncheckedAccount provides automatic
    /// validation that these are legitimate SPL token mints, preventing pools
    /// from being created with invalid or malicious token addresses.
    pub token_0: Account<'info, Mint>,
    pub token_1: Account<'info, Mint>,

    /// Token vault accounts with pool-controlled authority for secure asset custody.
    ///
    /// These accounts hold all pool assets and use the pool_core account as authority,
    /// ensuring only the pool program can move funds. This design prevents external
    /// manipulation of pool balances while enabling efficient swap operations.
    ///
    /// The authority model (token::authority = pool_core) creates a program-controlled
    /// account structure where only pool instructions can authorize token transfers,
    /// eliminating the risk of unauthorized asset movement that could drain pools.
    ///
    /// Deterministic derivation using pool and token keys ensures each pool gets
    /// unique vault addresses while enabling efficient vault discovery for indexing
    /// and integration purposes.
    #[account(
        init,
        payer = payer,
        seeds = [b"vault", pool_core.key().as_ref(), token_0.key().as_ref()],
        bump,
        token::mint = token_0,
        token::authority = pool_core,
    )]
    pub vault_0: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = payer,
        seeds = [b"vault", pool_core.key().as_ref(), token_1.key().as_ref()],
        bump,
        token::mint = token_1,
        token::authority = pool_core,
    )]
    pub vault_1: Account<'info, TokenAccount>,

    /// Transaction cost payer with independent authority for flexible operational models.
    ///
    /// Separated from other authority roles to enable scenarios where pool creation
    /// costs are covered by different entities than those who will manage the pool.
    /// This flexibility supports various deployment models including subsidized
    /// pool creation and third-party pool deployment services.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Initial administrative authority for pool management and configuration.
    ///
    /// This account becomes the first authorized manager of the pool, responsible
    /// for initial configuration and ongoing governance decisions. The separation
    /// from payer enables clean authority handoffs after pool creation.
    pub initial_authority: Signer<'info>,

    /// Designated emergency response authority for critical incident handling.
    ///
    /// Pre-authorized to take immediate protective actions during security incidents
    /// without waiting for normal governance processes. This role is essential for
    /// rapid response to active exploits or other time-sensitive threats.
    pub emergency_responder: Signer<'info>,

    /// Required Anchor program dependencies for account creation and token operations.
    ///
    /// These programs are validated by Anchor to ensure only legitimate system
    /// programs are used, preventing program substitution attacks that could
    /// compromise pool security or functionality.
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

/// Atomically creates a concentrated liquidity pool with comprehensive security infrastructure.
///
/// This function orchestrates the complex initialization of a complete pool ecosystem,
/// including trading state, security mechanisms, governance structures, and audit capabilities.
/// The atomic design ensures either complete success or complete failure, preventing
/// partially-initialized pools that could be exploited or cause operational issues.
///
/// ## Security-First Initialization Strategy
/// Every validation check is performed upfront before any state modification to prevent
/// pools from being created in invalid states that could be exploited. This fail-fast
/// approach protects both the protocol and users from malformed pool configurations.
///
/// ## Multi-Layer State Architecture
/// Creates interconnected but separable state accounts that enable independent evolution
/// of different protocol concerns (trading, security, governance) without requiring
/// monolithic account migrations that would impact all pools simultaneously.
///
/// ## Zero-Copy Performance Optimization
/// Uses load_init() throughout to initialize accounts with zero-copy access patterns,
/// essential for maintaining reasonable compute unit consumption during the complex
/// initialization process. Traditional deserialization would likely exceed Solana's
/// compute limits for this comprehensive setup.
///
/// ## Deterministic Event Emission
/// Emits comprehensive creation events to enable off-chain indexing and monitoring
/// systems to track pool deployment and initial configuration. This auditability
/// is crucial for both operational monitoring and regulatory compliance.
pub fn create_pool(
    ctx: Context<CreatePool>,
    fee_tier: u32,
    tick_spacing: u16,
    init_sqrt_price: Q64x64,
) -> Result<()> {
    // Capture blockchain state for temporal anchoring and audit trail initialization
    let clock = Clock::get()?;
    let slot = clock.slot;
    let unix = clock.unix_timestamp;

    // Validate that the factory shard has capacity for new pools
    let factory_shard = ctx.accounts.factory_shard.load()?;
    require!(factory_shard.has_capacity(), PoolError::ShardAtCapacity);
    drop(factory_shard); // Release the borrow before mut borrow below

    // Validate fee tier against protocol whitelist using binary search for efficiency
    // Binary search is O(log N) and works because DEFAULT_FEE_TIERS is sorted
    // This prevents malicious pools with non-standard fees that could exploit pricing assumptions
    require!(
        DEFAULT_FEE_TIERS.binary_search(&fee_tier).is_ok(),
        PoolError::UnsupportedFeeTier
    );

    // Enforce canonical tick spacing for the specified fee tier
    // This relationship is critical for maintaining consistent liquidity density expectations
    // across pools and preventing fragmentation attacks through non-standard spacing
    require!(
        expected_spacing(fee_tier) == Some(tick_spacing),
        PoolError::InvalidTickSpacingForFeeTier
    );

    // Validate initial price falls within protocol-safe bounds
    // These bounds prevent overflow in price calculations and ensure the price can be
    // represented accurately in our fixed-point arithmetic without precision loss
    require!(
        init_sqrt_price >= Q64x64::from_raw(MIN_SQRT_X64)
            && init_sqrt_price <= Q64x64::from_raw(MAX_SQRT_X64),
        PoolError::InvalidInitialPrice
    );

    // Convert sqrt price to tick and validate alignment with spacing requirements
    // Misaligned initial ticks could create unreachable price levels or break the
    // mathematical relationship between price and tick that swap algorithms depend on
    let init_tick = sqrt_price_to_tick(init_sqrt_price)?;
    require!(
        is_tick_aligned(init_tick, tick_spacing),
        PoolError::InitialTickSpacingMismatch
    );

    // Cache all PDA bumps to avoid recomputation during future operations
    // This optimization reduces compute unit consumption for subsequent transactions
    // that need to derive these same account addresses
    let bump_core = ctx.bumps.pool_core;
    let bump_security = ctx.bumps.pool_security;
    let bump_config = ctx.bumps.pool_config;
    let bump_vault_0 = ctx.bumps.vault_0;
    let bump_vault_1 = ctx.bumps.vault_1;

    // Initialize core trading state with validated parameters and zero-copy efficiency
    // Uses direct struct assignment for atomic initialization preventing partial state
    let mut pool_core = ctx.accounts.pool_core.load_init()?;
    *pool_core = PoolCore::default();
    pool_core.token_0 = ctx.accounts.token_0.key();
    pool_core.token_1 = ctx.accounts.token_1.key();
    pool_core.sqrt_price = init_sqrt_price;
    pool_core.tick_current = init_tick;
    pool_core.tick_spacing = tick_spacing;
    pool_core.fee = fee_tier as u16;
    pool_core.last_update_slot = slot;
    pool_core.bump_core = bump_core;
    pool_core.bump_vault_0 = bump_vault_0;
    pool_core.bump_vault_1 = bump_vault_1;

    // Add pool to the factory shard and update shard state
    let mut factory_shard = ctx.accounts.factory_shard.load_mut()?;
    factory_shard.add_pool(ctx.accounts.pool_core.key(), slot)?;

    // Initialize security monitoring and protection systems with defensive defaults
    // Security settings prioritize protection over convenience for new pools
    let mut pool_security = ctx.accounts.pool_security.load_init()?;
    *pool_security = PoolSecurity::default();
    pool_security.pool_core = ctx.accounts.pool_core.key();
    pool_security.last_security_check = slot;
    pool_security.emergency_contacts = ctx.accounts.emergency_responder.key();
    pool_security.bump_security = bump_security;

    // Initialize operational configuration with protocol defaults and governance hooks
    // Separates mutable config from immutable core state for flexible governance
    let mut pool_config = ctx.accounts.pool_config.load_init()?;
    *pool_config = PoolConfig::default();
    pool_config.pool_core = ctx.accounts.pool_core.key();
    pool_config.factory_shard = ctx.accounts.factory_shard.key();
    pool_config.last_volume_reset = slot;
    pool_config.core_authority = ctx.accounts.initial_authority.key();
    pool_config.bump_config = bump_config;

    // Emit comprehensive pool creation event for monitoring and indexing systems
    // This event provides all necessary information for off-chain services to track
    // pool deployment, analyze configuration, and monitor initial state
    emit!(PoolCreatedEvent {
        // Core pool identity and account addresses
        pool_core: ctx.accounts.pool_core.key(),
        pool_security: ctx.accounts.pool_security.key(),
        pool_config: ctx.accounts.pool_config.key(),
        factory_shard: ctx.accounts.factory_shard.key(),
        token_0: ctx.accounts.token_0.key(),
        token_1: ctx.accounts.token_1.key(),
        vault_0: ctx.accounts.vault_0.key(),
        vault_1: ctx.accounts.vault_1.key(),

        // Trading configuration for market analysis
        fee: fee_tier as u16,
        tick_spacing,
        init_sqrt_price: init_sqrt_price.raw(),
        init_tick,

        // PDA bumps for efficient address derivation by indexers
        bump_core,
        bump_security,
        bump_config,
        bump_vault_0,
        bump_vault_1,

        // Operational metadata for tracking and compliance
        creator: ctx.accounts.payer.key(),
        timestamp: unix,
        slot,
    });

    Ok(())
}
