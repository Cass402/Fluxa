use crate::error::PoolError;
use crate::math::core_arithmetic::Q64x64;
use crate::math::price_math::sqrt_price_to_tick;
use crate::state::pool::pool_config::PoolConfig;
use crate::state::pool::pool_security::PoolSecurity;
use crate::utils::constants::{
    DEFAULT_FEE_TIERS, MAX_SQRT_X64, MIN_SQRT_X64, TICK_SPACING_PER_FEE,
};
use crate::utils::event::PoolCreatedEvent;
use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

/// Concentrated liquidity pool core state with zero-copy optimization for high-frequency operations.
///
/// This structure serves as the single source of truth for all pool state, designed specifically
/// for Solana's constrained runtime environment where memory allocation and deserialization
/// overhead can severely impact transaction throughput and computational limits.
///
/// ## Zero-Copy Architecture Rationale
/// Uses `#[account(zero_copy(unsafe))]` because concentrated liquidity pools require frequent
/// state access during swaps, position management, and price updates. Traditional Anchor
/// deserialization would create significant overhead for every transaction, potentially
/// causing CU limit violations in complex operations. Zero-copy enables direct memory
/// manipulation without serialization costs, crucial for maintaining sub-millisecond
/// transaction processing in high-frequency DeFi environments.
///
/// ## Fixed-Size Field Strategy  
/// Every field uses fixed-size types (no Vec, String, or variable-length data) to achieve:
/// - Deterministic account size for predictable rent calculations and storage costs
/// - Cache-friendly memory layout enabling efficient CPU access patterns
/// - Prevention of memory fragmentation attacks where malicious users could create
///   oversized accounts to exhaust validator memory
/// - Elimination of dynamic allocation failures during critical operations
///
/// ## Memory Alignment Considerations
/// Uses `#[repr(C)]` to ensure consistent memory layout across different compilation
/// targets and prevent subtle bugs from memory padding differences. This is essential
/// for zero-copy operations where direct memory access assumes specific byte offsets.
///
/// ## Protocol State Isolation
/// Serves as the coordination point for all pool operations while maintaining clear
/// separation from security, configuration, and audit concerns through linked accounts.
/// This separation enables independent evolution of different protocol aspects without
/// requiring costly state migrations.
#[account(zero_copy(unsafe))]
#[derive(InitSpace)] // Expecting 224 bytes
#[repr(C)]
pub struct PoolCore {
    /// Immutable token pair defining the pool's trading assets with canonical ordering.
    ///
    /// These fields are set once during initialization and never modified, serving as the
    /// fundamental identity of the pool. The ordering (token_0 < token_1) is enforced
    /// during creation to ensure deterministic pool addresses and prevent duplicate
    /// pools for the same asset pair. This canonical ordering also simplifies swap
    /// logic by eliminating conditional branches for token direction handling.
    pub token_0: Pubkey,
    pub token_1: Pubkey,

    /// Current market state using fixed-point arithmetic for precision and safety.
    ///
    /// sqrt_price: Square root of current price in Q64.64 fixed-point format.
    /// This representation choice stems from Uniswap V3's insight that using √P instead
    /// of P enables more efficient tick calculations and reduces precision loss during
    /// price movements. Q64.64 provides sufficient precision for even micro-cent assets
    /// while avoiding floating-point arithmetic issues that could lead to exploitation.
    ///
    /// liquidity: Active liquidity available at current tick in Q64.64 format.
    /// Represents the amount of virtual tokens available for trading at the current
    /// price. Fixed-point arithmetic prevents rounding errors that could be exploited
    /// to drain pools through precision manipulation attacks.
    ///
    /// tick_current: Current tick index representing the price point.
    /// Ticks provide discrete price levels for range orders, with each tick representing
    /// a 0.01% price change. This discretization enables efficient liquidity management
    /// and prevents dust positions that could bloat state storage.
    ///
    /// tick_spacing: Minimum tick increment enforced by fee tier.
    /// Higher fees require wider tick spacing to ensure adequate compensation for
    /// liquidity providers relative to the granularity of positions. This prevents
    /// excessive fragmentation of liquidity that could degrade trading efficiency.
    ///
    /// fee: Trading fee in basis points (0-10000).
    /// u16 is sufficient for all practical fee structures while preventing overflow
    /// in fee calculations. Basis points provide fine-grained control needed for
    /// competitive fee optimization.
    pub sqrt_price: Q64x64,
    pub liquidity: Q64x64,
    pub tick_current: i32,
    pub tick_spacing: u16,
    pub fee: u16,

    /// Global fee accumulation tracking for precise liquidity provider compensation.
    ///
    /// These accumulators implement a gas-efficient mechanism for tracking fees earned
    /// by all liquidity providers without requiring iteration over individual positions.
    /// Each accumulator represents the cumulative fees per unit of liquidity since pool
    /// inception, enabling O(1) fee calculation for any position regardless of pool size.
    ///
    /// The Q64.64 precision ensures that even tiny fees accumulate accurately over
    /// millions of transactions, preventing value leakage that could be exploited
    /// or cause accounting discrepancies in long-running pools.
    ///
    /// This design choice trades a small amount of storage (2 * 128 bits) for massive
    /// computational savings compared to maintaining individual fee balances, crucial
    /// for maintaining transaction throughput as pool usage scales.
    pub fee_growth_global_0: Q64x64,
    pub fee_growth_global_1: Q64x64,

    /// Operational metadata enabling temporal logic and protocol evolution.
    ///
    /// last_update_slot: Solana slot when pool state was last modified.
    /// Essential for implementing time-based logic like TWAP oracles, rate limiting,
    /// and MEV protection. Slot-based timing is preferred over Unix timestamps because
    /// slots provide guaranteed monotonic progression and are harder to manipulate,
    /// crucial for preventing timestamp-based attacks on time-sensitive features.
    ///
    /// protocol_version: Semantic version for handling protocol upgrades.
    /// Enables safe migration strategies and backward compatibility checks during
    /// protocol evolution. Critical for long-lived pools that must survive multiple
    /// protocol iterations without requiring costly state migrations or user action.
    ///
    /// status_flags: Bitfield encoding multiple operational states efficiently.
    /// Each bit represents a different status (paused, emergency, deprecated, etc.)
    /// allowing atomic status checks and updates. Bitwise operations are extremely
    /// efficient on Solana's BPF runtime, and compact encoding minimizes storage costs
    /// while enabling complex state combinations (e.g., "paused but emergency-withdrawable").
    ///
    /// Bump caching: Pre-computed PDA bumps for gas optimization.
    /// Storing bump seeds eliminates the need to recompute them during every transaction
    /// that accesses pool vaults or derives related accounts. This seemingly small
    /// optimization can save significant compute units in complex operations involving
    /// multiple account derivations.
    ///
    /// _padding: Ensures proper struct alignment for zero-copy safety.
    /// Memory alignment is critical for zero-copy operations to prevent undefined
    /// behavior from misaligned memory access, which could cause transaction failures
    /// or subtle corruption bugs.
    pub last_update_slot: u64,
    pub protocol_version: u16,
    pub status_flags: u16,
    pub bump_core: u8,
    pub bump_vault_0: u8,
    pub bump_vault_1: u8,
    pub _padding: [u8; 1],

    /// Future-proofing space preventing costly account migrations during protocol evolution.
    ///
    /// Pre-allocating 64 bytes (8 * u64) provides substantial expansion room for new
    /// features without requiring users to migrate to new account layouts. This design
    /// choice reflects the lessons learned from early DeFi protocols where adding new
    /// features required complex and expensive migration processes that often left
    /// legacy pools stranded with outdated functionality.
    ///
    /// The reserved space enables seamless protocol upgrades for features like:
    /// - New fee structures or rebate mechanisms
    /// - Advanced MEV protection parameters
    /// - Cross-chain bridge integration metadata
    /// - Enhanced oracle price feeds
    /// - Regulatory compliance fields
    ///
    /// Using u64 array ensures proper alignment while providing flexibility for
    /// various future field types. The space cost (64 bytes per pool) is minimal
    /// compared to the value of upgrade flexibility for long-lived DeFi infrastructure.
    pub reserved: [u64; 8],
}

impl Default for PoolCore {
    fn default() -> Self {
        Self {
            token_0: Pubkey::default(),
            token_1: Pubkey::default(),
            sqrt_price: Q64x64::zero(),
            liquidity: Q64x64::zero(),
            tick_current: 0,
            tick_spacing: 0,
            fee: 0,
            fee_growth_global_0: Q64x64::zero(),
            fee_growth_global_1: Q64x64::zero(),
            last_update_slot: 0,
            protocol_version: 1,
            status_flags: 0,
            bump_core: 0,
            bump_vault_0: 0,
            bump_vault_1: 0,
            _padding: [0; 1],
            reserved: [0; 8],
        }
    }
}

impl PoolCore {
    /// Enterprise upgrade status flags for tracking multi-phase initialization
    pub const SECURITY_FOUNDATION_INITIALIZED: u16 = 0x01;
    pub const AUDIT_SYSTEM_INITIALIZED: u16 = 0x02;
    pub const ENTERPRISE_MODE_ACTIVE: u16 = 0x04;

    /// Check if pool has enterprise capabilities enabled
    pub fn is_enterprise(&self) -> bool {
        self.status_flags & Self::ENTERPRISE_MODE_ACTIVE != 0
    }

    /// Check if security foundation is initialized
    pub fn has_security_foundation(&self) -> bool {
        self.status_flags & Self::SECURITY_FOUNDATION_INITIALIZED != 0
    }

    /// Check if audit system is initialized
    pub fn has_audit_system(&self) -> bool {
        self.status_flags & Self::AUDIT_SYSTEM_INITIALIZED != 0
    }

    /// Check if pool is ready for enterprise upgrade finalization
    pub fn ready_for_enterprise_finalization(&self) -> bool {
        self.has_security_foundation() && self.has_audit_system() && !self.is_enterprise()
    }
}

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
#[instruction(fee_tier: u32, tick_spacing: u16, initial_sqrt_price: Q64x64)]
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
