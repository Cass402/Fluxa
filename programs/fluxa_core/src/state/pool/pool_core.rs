use crate::error::PoolError;
use crate::math::core_arithmetic::Q64x64;
use crate::math::price_math::sqrt_price_to_tick;
use crate::state::pool::pool_config::PoolConfig;
use crate::state::pool::pool_security::PoolSecurity;
use crate::state::pool::volatility_tracker::EwmaVolatilityTracker;
use crate::utils::constants::{
    DEFAULT_FEE_TIERS, DEFAULT_PROTOCOL_FEE, MAX_SQRT_X64, MIN_SQRT_X64, SECURITY_FLAG_DEFAULT,
    TICK_SPACING_PER_FEE,
};
use crate::utils::event::PoolCreatedEvent;
use crate::utils::security_authority::audit_trail::AuditTrailHead;
use crate::utils::security_authority::core_authority::CoreAuthority;
use crate::utils::security_authority::emergency_contacts::EmergencyContacts;
use crate::utils::security_authority::multisig_config::MultisigConfig;
use crate::utils::security_authority::security_coordinator::SecurityCoordinator;
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
#[derive(InitSpace)]
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

/// Comprehensive account validation context for atomic pool creation with security guarantees.
///
/// This context structure implements defense-in-depth for pool creation by enforcing multiple
/// layers of validation and security constraints. The design reflects lessons learned from
/// DeFi exploits where insufficient validation during pool creation led to protocol vulnerabilities.
///
/// ## Atomic Initialization Strategy
/// All related accounts (core, security, config, vaults, authority structures) are created
/// in a single transaction to prevent partial initialization states that could be exploited.
/// This approach eliminates race conditions where attackers could interfere with pool
/// setup between multiple transactions.
///
/// ## Deterministic Address Derivation
/// Uses consistent seed patterns across all accounts to ensure predictable addresses that
/// can be computed off-chain. This enables efficient indexing and prevents address collision
/// attacks where malicious actors might try to claim pool-related account addresses.
///
/// ## Constraint-Based Security Model
/// Anchor constraints are used extensively to enforce protocol invariants at the instruction
/// level, preventing invalid pool configurations from being created. This approach moves
/// security validation to the framework level, reducing the attack surface and ensuring
/// consistent enforcement across all pool creation attempts.
///
/// ## Authority Separation Principle
/// Distinguishes between different authority roles (payer, initial authority, emergency responder)
/// to enable flexible operational models while maintaining security. This separation allows
/// for scenarios where operational costs, management authority, and emergency response are
/// handled by different entities.
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

    /// Security coordination layer orchestrating multi-layered protection mechanisms.
    ///
    /// Acts as the central coordinator for all security-related functionality including
    /// authority validation, emergency response, and audit trail management. This
    /// centralized approach ensures consistent security policy enforcement across
    /// all pool operations while maintaining clear separation of concerns.
    #[account(
        init,
        payer = payer,
        seeds = [b"security_coordinator", pool_core.key().as_ref()],
        bump,
        space = 8 + SecurityCoordinator::INIT_SPACE,
    )]
    pub security_coordinator: AccountLoader<'info, SecurityCoordinator>,

    /// Core authority management for administrative operations and access control.
    ///
    /// Implements a separate authority structure to enable sophisticated access
    /// control patterns including time-locked operations, multi-signature requirements,
    /// and emergency response procedures. Separation from pool core enables independent
    /// evolution of governance models without affecting trading functionality.
    #[account(
        init,
        payer = payer,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump,
        space = 8 + CoreAuthority::INIT_SPACE,
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// Multi-signature configuration for distributed authority and enhanced security.
    ///
    /// Enables sophisticated governance models where critical operations require
    /// consensus from multiple parties. This structure supports various multisig
    /// patterns from simple M-of-N signatures to complex governance workflows
    /// with different approval requirements for different operation types.
    #[account(
        init,
        payer = payer,
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump,
        space = 8 + MultisigConfig::INIT_SPACE,
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// Audit trail head for tamper-evident logging of all critical operations.
    ///
    /// Provides cryptographic proof of all pool operations for regulatory compliance
    /// and forensic analysis. The audit trail enables reconstruction of complete
    /// pool history and detection of any unauthorized modifications to pool state.
    #[account(
        init,
        payer = payer,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump,
        space = 8 + AuditTrailHead::INIT_SPACE,
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    /// Emergency contact registry for rapid incident response coordination.
    ///
    /// Maintains verified contact information for emergency responders who can
    /// take immediate action during security incidents. This system enables
    /// faster response times than traditional governance processes when pools
    /// face active exploitation or other critical threats.
    #[account(
        init,
        payer = payer,
        seeds = [b"emergency_contacts", pool_core.key().as_ref()],
        bump,
        space = 8 + EmergencyContacts::INIT_SPACE,
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

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
    multisig_threshold: u8,
    multisig_members: [Pubkey; 7],
    required_confirmations: u8,
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

    // Initialize security infrastructure in dependency order to ensure proper linking
    // Core authority must be established first as other components depend on it
    let mut core_authority = ctx.accounts.core_authority.load_init()?;
    core_authority.initialize(
        ctx.accounts.pool_core.key(),
        ctx.accounts.initial_authority.key(),
        required_confirmations,
    )?;

    // Multi-signature configuration enables distributed governance and enhanced security
    // Initialized early to provide authority validation for subsequent security components
    let mut multisig_config = ctx.accounts.multisig_config.load_init()?;
    multisig_config.initialize(
        ctx.accounts.pool_core.key(),
        multisig_threshold,
        multisig_members,
    )?;

    // Audit trail head establishes the foundation for tamper-evident logging
    // Must be initialized before security coordinator to provide audit capabilities
    let mut audit_trail_head = ctx.accounts.audit_trail_head.load_init()?;
    audit_trail_head.initialize(ctx.accounts.pool_core.key())?;

    // Emergency contacts registry enables rapid incident response coordination
    // Provides pre-authorized channels for critical security communications
    let mut emergency_contacts = ctx.accounts.emergency_contacts.load_init()?;
    emergency_contacts.initialize(
        ctx.accounts.pool_core.key(),
        ctx.accounts.emergency_responder.key(),
    )?;

    // Security coordinator orchestrates all security components into cohesive protection
    // Initialized last as it requires references to all other security infrastructure
    let mut security_coordinator = ctx.accounts.security_coordinator.load_init()?;
    security_coordinator.initialize(
        ctx.accounts.pool_core.key(),
        ctx.accounts.core_authority.key(),
        ctx.accounts.multisig_config.key(),
        ctx.accounts.audit_trail_head.key(),
        ctx.accounts.emergency_contacts.key(),
    )?;

    // Initialize core trading state with validated parameters and zero-copy efficiency
    // Uses direct struct assignment for atomic initialization preventing partial state
    let mut pool_core = ctx.accounts.pool_core.load_init()?;
    *pool_core = PoolCore {
        // Immutable token pair identity established during creation
        token_0: ctx.accounts.token_0.key(),
        token_1: ctx.accounts.token_1.key(),

        // Market state initialized to provided starting conditions
        sqrt_price: init_sqrt_price,
        liquidity: Q64x64::zero(), // No liquidity until first provision
        tick_current: init_tick,
        tick_spacing,
        fee: fee_tier as u16,

        // Fee tracking accumulators start at zero for clean accounting
        fee_growth_global_0: Q64x64::zero(),
        fee_growth_global_1: Q64x64::zero(),

        // Temporal anchoring for protocol operations
        last_update_slot: slot,
        protocol_version: 1, // Initial version enabling future migrations
        status_flags: 0,     // Normal operational status

        // Cached PDA bumps for gas optimization
        bump_core,
        bump_vault_0,
        bump_vault_1,

        // Alignment and future expansion space
        _padding: [0; 1],
        reserved: [0; 8],
    };

    // Initialize security monitoring and protection systems with defensive defaults
    // Security settings prioritize protection over convenience for new pools
    let mut pool_security = ctx.accounts.pool_security.load_init()?;
    *pool_security = PoolSecurity {
        // Core pool binding for security scope isolation
        pool_core: ctx.accounts.pool_core.key(),

        // Security status tracking with conservative defaults
        security_flags: SECURITY_FLAG_DEFAULT,
        total_swap_volume_0: Q64x64::zero(), // Volume tracking for anomaly detection
        total_swap_volume_1: Q64x64::zero(),
        active_positions_count: 0,
        last_security_check: slot,
        suspicious_activity_score: 0,

        // MEV protection enabled by default due to high exploitation risk in new pools
        mev_protection_enabled: true,

        // Security infrastructure references for coordinated response
        emergency_contacts: ctx.accounts.emergency_contacts.key(),
        security_coordinator: ctx.accounts.security_coordinator.key(),

        // Circuit breaker configuration for automatic protection
        circuit_breaker_triggered_at: 0,
        circuit_breaker_threshold: 800, // Conservative threshold for new pools

        // Account metadata and expansion space
        bump_security,
        _padding: [0; 3],
        reserved: [0; 4],
    };

    // Initialize operational configuration with protocol defaults and governance hooks
    // Separates mutable config from immutable core state for flexible governance
    let mut pool_config = ctx.accounts.pool_config.load_init()?;
    *pool_config = PoolConfig {
        // Core pool binding for configuration scope
        pool_core: ctx.accounts.pool_core.key(),

        // Protocol fee structure initialized to network defaults
        protocol_fee_0: DEFAULT_PROTOCOL_FEE,
        protocol_fee_1: DEFAULT_PROTOCOL_FEE,
        protocol_fees_token_0: Q64x64::zero(), // Accumulated fees start at zero
        protocol_fees_token_1: Q64x64::zero(),

        // Volatility tracking for risk management and dynamic parameters
        volatility_tracker: EwmaVolatilityTracker::new(60), // 60-slot smoothing window

        // Operational limits disabled initially, can be configured by governance
        max_swap_limit: 0,     // No single-swap limit
        daily_volume_limit: 0, // No daily volume cap
        last_volume_reset: slot,

        // Governance binding for configuration management
        core_authority: ctx.accounts.core_authority.key(),

        // Account metadata and expansion space
        bump_config,
        _padding: [0; 7],
        reserved: [0; 8],
    };

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

        // Security infrastructure for monitoring and governance tracking
        secureity_coordinator: ctx.accounts.security_coordinator.key(),
        multisig_config: ctx.accounts.multisig_config.key(),
        core_authority: ctx.accounts.core_authority.key(),
        audit_trail_head: ctx.accounts.audit_trail_head.key(),
        emergency_contacts: ctx.accounts.emergency_contacts.key(),

        // Governance configuration for policy analysis
        multisig_threshold,
        required_confirmations,
    });

    Ok(())
}
