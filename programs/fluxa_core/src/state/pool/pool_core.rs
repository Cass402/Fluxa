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

/// Core state for a concentrated liquidity pool.
///
/// # Why this structure?
/// - Uses zero-copy layout for maximum on-chain efficiency, avoiding serialization overhead and enabling direct memory access in Anchor.
/// - All fields are fixed-size and aligned for deterministic account size and predictable rent costs.
/// - No dynamic allocations (e.g., Vec), ensuring safety and performance on Solana's constrained runtime.
///
/// ## Usage
/// This struct is the canonical source of truth for pool state, referenced by all swap, liquidity, and admin instructions.
#[account(zero_copy(unsafe))]
#[derive(InitSpace)]
#[repr(C)]
pub struct PoolCore {
    /// Token mint addresses for the pool.
    ///
    /// Why: Immutable after initialization, these fields define the asset pair and are used for all downstream validation and accounting. Storing as Pubkey ensures compatibility with SPL tokens and Anchor constraints.
    pub token_0: Pubkey,
    pub token_1: Pubkey,

    /// Core price and liquidity state, all in fixed-point for precision and safety.
    ///
    /// - `sqrt_price`: Square root of the current price, in Q64.64. Why: Using sqrt(P) enables efficient tick math and price movement calculations, as in Uniswap v3.
    /// - `liquidity`: Current active liquidity, in Q64.64. Why: Fixed-point avoids rounding errors and overflows in swap math.
    /// - `tick_current`: Current tick index. Why: Ticks are the fundamental unit for range orders and liquidity management.
    /// - `tick_spacing`: Minimum tick step. Why: Enforces granularity and prevents excessive tick bloat.
    /// - `fee`: Fee in basis points (0-10000). Why: Basis points allow for fine-grained fee control, and u16 is sufficient for all practical use cases.
    pub sqrt_price: Q64x64,
    pub liquidity: Q64x64,
    pub tick_current: i32,
    pub tick_spacing: u16,
    pub fee: u16,

    /// Global fee growth accumulators for each token, in Q64.64.
    ///
    /// Why: Tracks total fees earned by all liquidity providers, enabling precise, gas-efficient fee accounting. Q64.64 ensures no loss of precision over long periods.
    pub fee_growth_global_0: Q64x64,
    pub fee_growth_global_1: Q64x64,

    /// Operational metadata for protocol upgrades and status management.
    ///
    /// - `last_update_slot`: Last slot when price/liquidity was updated. Why: Enables time-based logic (e.g., TWAP, rate limits) and replay protection.
    /// - `protocol_version`: Version for future upgrades. Why: Allows for safe migrations and backward compatibility.
    /// - `status_flags`: Bitfield for operational status (e.g., paused, emergency). Why: Bitwise flags allow multiple statuses to be tracked compactly and atomically, minimizing storage and compute.
    /// - `_padding`: Ensures 8-byte alignment for Anchor zero-copy safety and future extensibility.
    pub last_update_slot: u64,
    pub protocol_version: u16,
    pub status_flags: u16,
    pub bump_core: u8,    // Cache bump for efficiency
    pub bump_vault_0: u8, // Cache bump for vault 0
    pub bump_vault_1: u8, // Cache bump for vault 1
    pub _padding: [u8; 1],

    /// Reserved for future upgrades (e.g., new features, protocol extensions) without breaking account layout.
    ///
    /// Why: Pre-allocating space allows for seamless upgrades and avoids costly migrations or rent increases.
    pub reserved: [u64; 8],
}

/// Returns the expected tick spacing for a given fee tier.
///
/// # Why this function?
/// - Enforces protocol invariants by mapping fee tiers to their canonical tick spacing.
/// - Prevents invalid pool configurations and ensures consistency across deployments.
fn expected_spacing(fee: u32) -> Option<u16> {
    TICK_SPACING_PER_FEE
        .into_iter()
        .find_map(|(f, s)| (f == fee).then_some(s))
}

/// Checks if a tick index is properly aligned to the given tick spacing.
///
/// # Why this function?
/// - Prevents misaligned initial ticks, which could break swap math and liquidity accounting.
/// - Enforces protocol safety by ensuring all ticks are valid for the pool's spacing.
#[inline(always)]
fn is_tick_aligned(tick: i32, spacing: u16) -> bool {
    tick.rem_euclid(spacing as i32) == 0
}

/// Context for pool creation, enforcing all protocol invariants and account constraints.
///
/// # Why this struct?
/// - Ensures all pool accounts are initialized atomically and with deterministic seeds.
/// - Constraints enforce protocol safety (fee tier, token order, rent payer, etc.) and prevent invalid pool deployments.
/// - Vaults are created with pool_core as authority, ensuring secure custody of assets.
#[derive(Accounts)]
#[instruction(fee_tier: u32, tick_spacing: u16, initial_sqrt_price: Q64x64)]
pub struct CreatePool<'info> {
    #[account(
        init,
        payer = payer,
        seeds = [b"pool_core", token_0.key().as_ref(), token_1.key().as_ref(), &fee_tier.to_le_bytes()],
        bump,
        space = 8 + PoolCore::INIT_SPACE,
        constraint = fee_tier <= 10_000 @ PoolError::InvalidFeeTier, // Prevents excessive fees and protocol abuse
        constraint = token_0.key() < token_1.key() @ PoolError::InvalidTokenOrder, // Canonical ordering for deterministic pool addresses
    )]
    pub pool_core: AccountLoader<'info, PoolCore>,

    #[account(
        init,
        payer = payer,
        seeds = [b"pool_security", pool_core.key().as_ref()],
        bump,
        space = 8 + PoolSecurity::INIT_SPACE,
    )]
    pub pool_security: AccountLoader<'info, PoolSecurity>,

    #[account(
        init,
        payer = payer,
        seeds = [b"pool_config", pool_core.key().as_ref()],
        bump,
        space = 8 + PoolConfig::INIT_SPACE,
    )]
    pub pool_config: AccountLoader<'info, PoolConfig>,

    pub token_0: Account<'info, Mint>,
    pub token_1: Account<'info, Mint>,

    #[account(
        init,
        payer = payer,
        seeds = [b"vault", pool_core.key().as_ref(), token_0.key().as_ref()],
        bump,
        token::mint = token_0,
        token::authority = pool_core, // Ensures only pool_core can move assets
    )]
    pub vault_0: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = payer,
        seeds = [b"vault", pool_core.key().as_ref(), token_1.key().as_ref()],
        bump,
        token::mint = token_1,
        token::authority = pool_core, // Ensures only pool_core can move assets
    )]
    pub vault_1: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = payer,
        seeds = [b"security_coordinator", pool_core.key().as_ref()],
        bump,
        space = 8 + SecurityCoordinator::INIT_SPACE,
    )]
    pub security_coordinator: AccountLoader<'info, SecurityCoordinator>,

    #[account(
        init,
        payer = payer,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump,
        space = 8 + CoreAuthority::INIT_SPACE,
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    #[account(
        init,
        payer = payer,
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump,
        space = 8 + MultisigConfig::INIT_SPACE,
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    #[account(
        init,
        payer = payer,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump,
        space = 8 + AuditTrailHead::INIT_SPACE,
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    #[account(
        init,
        payer = payer,
        seeds = [b"emergency_contacts", pool_core.key().as_ref()],
        bump,
        space = 8 + EmergencyContacts::INIT_SPACE, // Space for emergency contacts
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    #[account(mut)]
    pub payer: Signer<'info>, // Rent payer and initial authority

    pub initial_authority: Signer<'info>, // Initial authority for pool setup

    pub emergency_responder: Signer<'info>, // Emergency response authority

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

/// Creates a new concentrated liquidity pool, enforcing all protocol invariants and initializing all state accounts.
///
/// # Why this function?
/// - Ensures atomic, deterministic pool creation with all safety checks and protocol constraints enforced up front.
/// - Initializes all pool state (core, security, config) with zero-copy logic for maximum on-chain efficiency.
/// - Emits a creation event for auditability and downstream indexing.
pub fn create_pool(
    ctx: Context<CreatePool>,
    fee_tier: u32,
    tick_spacing: u16,
    init_sqrt_price: Q64x64,
    multisig_threshold: u8,
    multisig_members: [Pubkey; 7],
    required_confirmations: u8,
) -> Result<()> {
    let clock = Clock::get()?;
    let slot = clock.slot;
    let unix = clock.unix_timestamp;

    // Enforce supported fee tiers using binary search for O(log N) validation.
    require!(
        DEFAULT_FEE_TIERS.binary_search(&fee_tier).is_ok(),
        PoolError::UnsupportedFeeTier
    );

    // Enforce tick spacing matches canonical value for fee tier.
    require!(
        expected_spacing(fee_tier) == Some(tick_spacing),
        PoolError::InvalidTickSpacingForFeeTier
    );

    // Enforce initial price is within protocol bounds.
    require!(
        init_sqrt_price >= Q64x64::from_raw(MIN_SQRT_X64)
            && init_sqrt_price <= Q64x64::from_raw(MAX_SQRT_X64),
        PoolError::InvalidInitialPrice
    );

    // Convert initial sqrt price to tick and enforce tick alignment.
    let init_tick = sqrt_price_to_tick(init_sqrt_price)?;
    require!(
        is_tick_aligned(init_tick, tick_spacing),
        PoolError::InitialTickSpacingMismatch
    );

    // Cache bump seeds for all pool accounts for deterministic address derivation and future upgrades.
    let bump_core = ctx.bumps.pool_core;
    let bump_security = ctx.bumps.pool_security;
    let bump_config = ctx.bumps.pool_config;
    let bump_vault_0 = ctx.bumps.vault_0;
    let bump_vault_1 = ctx.bumps.vault_1;

    let mut core_authority = ctx.accounts.core_authority.load_init()?;
    core_authority.initialize(
        ctx.accounts.pool_core.key(),
        ctx.accounts.initial_authority.key(),
        required_confirmations,
    )?;

    let mut multisig_config = ctx.accounts.multisig_config.load_init()?;
    multisig_config.initialize(
        ctx.accounts.pool_core.key(),
        multisig_threshold,
        multisig_members,
    )?;

    let mut audit_trail_head = ctx.accounts.audit_trail_head.load_init()?;
    audit_trail_head.initialize(ctx.accounts.pool_core.key())?;

    let mut emergency_contacts = ctx.accounts.emergency_contacts.load_init()?;
    emergency_contacts.initialize(
        ctx.accounts.pool_core.key(),
        ctx.accounts.emergency_responder.key(),
    )?;

    let mut security_coordinator = ctx.accounts.security_coordinator.load_init()?;
    security_coordinator.initialize(
        ctx.accounts.pool_core.key(),
        ctx.accounts.core_authority.key(),
        ctx.accounts.multisig_config.key(),
        ctx.accounts.audit_trail_head.key(),
        ctx.accounts.emergency_contacts.key(),
    )?;

    // Initialize core pool state with zero-copy logic for maximum efficiency and deterministic layout.
    let mut pool_core = ctx.accounts.pool_core.load_init()?;
    *pool_core = PoolCore {
        token_0: ctx.accounts.token_0.key(),
        token_1: ctx.accounts.token_1.key(),
        sqrt_price: init_sqrt_price,
        liquidity: Q64x64::zero(),
        tick_current: init_tick,
        tick_spacing,
        fee: fee_tier as u16,
        fee_growth_global_0: Q64x64::zero(),
        fee_growth_global_1: Q64x64::zero(),
        last_update_slot: slot,
        protocol_version: 1, // Initial version for future upgrades
        status_flags: 0,     // Normal status; bitfield for future protocol states
        bump_core,
        bump_vault_0,
        bump_vault_1,
        _padding: [0; 1],
        reserved: [0; 8],
    };

    // Initialize pool security state, including MEV protection and emergency contacts.
    let mut pool_security = ctx.accounts.pool_security.load_init()?;
    *pool_security = PoolSecurity {
        pool_core: ctx.accounts.pool_core.key(),
        security_flags: SECURITY_FLAG_DEFAULT, // Default status; bitfield for future upgrades
        total_swap_volume_0: Q64x64::zero(),
        total_swap_volume_1: Q64x64::zero(),
        active_positions_count: 0,
        last_security_check: slot,
        suspicious_activity_score: 0,
        mev_protection_enabled: true, // MEV protection enabled by default for protocol safety
        emergency_contacts: ctx.accounts.emergency_contacts.key(), // Initial emergency contact is the payer for rapid response
        security_coordinator: ctx.accounts.security_coordinator.key(),
        circuit_breaker_triggered_at: 0,
        circuit_breaker_threshold: 800, // Default threshold for circuit breaker logic
        bump_security,
        _padding: [0; 3],
        reserved: [0; 4],
    };

    // Initialize pool config state, including protocol fees and volatility tracker.
    let mut pool_config = ctx.accounts.pool_config.load_init()?;
    *pool_config = PoolConfig {
        pool_core: ctx.accounts.pool_core.key(),
        protocol_fee_0: DEFAULT_PROTOCOL_FEE,
        protocol_fee_1: DEFAULT_PROTOCOL_FEE,
        protocol_fees_token_0: Q64x64::zero(),
        protocol_fees_token_1: Q64x64::zero(),
        volatility_tracker: EwmaVolatilityTracker::new(60), // 60-slot min update interval for volatility tracking
        max_swap_limit: 0,     // No swap limit by default; can be set by governance
        daily_volume_limit: 0, // No daily volume limit by default
        last_volume_reset: slot,
        core_authority: ctx.accounts.core_authority.key(), // Initial authority is the payer for bootstrap
        bump_config,
        _padding: [0; 7],
        reserved: [0; 8],
    };

    // Emit pool creation event for auditability and downstream indexing.
    emit!(PoolCreatedEvent {
        pool_core: ctx.accounts.pool_core.key(),
        pool_security: ctx.accounts.pool_security.key(),
        pool_config: ctx.accounts.pool_config.key(),
        token_0: ctx.accounts.token_0.key(),
        token_1: ctx.accounts.token_1.key(),
        vault_0: ctx.accounts.vault_0.key(),
        vault_1: ctx.accounts.vault_1.key(),
        fee: fee_tier as u16,
        tick_spacing,
        init_sqrt_price: init_sqrt_price.raw(),
        init_tick,
        bump_core,
        bump_security,
        bump_config,
        bump_vault_0,
        bump_vault_1,
        creator: ctx.accounts.payer.key(),
        timestamp: unix,
        slot,
        secureity_coordinator: ctx.accounts.security_coordinator.key(),
        multisig_config: ctx.accounts.multisig_config.key(),
        core_authority: ctx.accounts.core_authority.key(),
        audit_trail_head: ctx.accounts.audit_trail_head.key(),
        emergency_contacts: ctx.accounts.emergency_contacts.key(),
        multisig_threshold,
        required_confirmations,
    });

    Ok(())
}
