use crate::error::PoolError;
use crate::state::pool::pool_config::PoolConfig;
use crate::state::pool::pool_core::PoolCore;
use crate::utils::security_authority::core_authority::CoreAuthority;
use crate::utils::security_authority::emergency_contacts::EmergencyContacts;
use crate::utils::security_authority::multisig_config::MultisigConfig;
use anchor_lang::prelude::*;

/// **Phase 1: Security Foundation Bootstrap**
///
/// Implements the first phase of a three-phase enterprise upgrade strategy designed to
/// circumvent Solana's compute unit limitations while maintaining atomic security guarantees.
/// This phase establishes the governance and emergency response foundation (~45k CU).
///
/// ## Phased Upgrade Rationale
/// Enterprise security infrastructure requires extensive initialization that would exceed
/// single-transaction compute limits. By splitting into phases with status flag checkpoints,
/// we ensure each phase can complete successfully while preventing partial upgrades that
/// could leave pools in vulnerable intermediate states.
///
/// ## Security Foundation Components
/// - **Core Authority**: Establishes governance hierarchy and authority delegation patterns
/// - **Multisig Config**: Implements distributed authorization preventing single points of failure  
/// - **Emergency Contacts**: Creates rapid incident response capability separate from governance
///
/// This foundation must be established before audit systems to ensure proper access control
/// for all subsequent security operations.
#[derive(Accounts)]
pub struct InitializeSecurityFoundation<'info> {
    /// Pool undergoing enterprise upgrade - complex validations moved to handler
    /// for better performance and cleaner error handling.
    #[account(mut)]
    pub pool_core: AccountLoader<'info, PoolCore>,

    /// Pool configuration containing the current governance authority.
    /// Required to extract the legitimate core_authority for permission validation
    /// and to establish governance continuity during the enterprise transition.
    #[account(
        seeds = [b"pool_config", pool_core.key().as_ref()],
        bump,
        has_one = pool_core,
    )]
    pub pool_config: AccountLoader<'info, PoolConfig>,

    /// **Core Authority Account**: Foundation of the governance hierarchy.
    /// Uses deterministic PDA to ensure unique authority per pool and prevent
    /// authority confusion attacks. Space allocation optimized for zero-copy access
    /// patterns that avoid runtime memory allocations during authority operations.
    #[account(
        init,
        payer = payer,
        space = 8 + CoreAuthority::INIT_SPACE,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump,
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// **Multi-signature Configuration**: Distributed authorization layer.
    /// Separate from core authority to enable different security models:
    /// core authority for routine governance, multisig for critical operations.
    /// This separation prevents governance bottlenecks while maintaining security.
    #[account(
        init,
        payer = payer,
        space = 8 + MultisigConfig::INIT_SPACE,
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump,
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// **Emergency Contacts Registry**: Rapid incident response capability.
    /// Architecturally separate from multisig to enable faster emergency responses
    /// when consensus-based multisig would be too slow. Critical for DeFi protocols
    /// where minutes can determine exploit containment success.
    #[account(
        init,
        payer = payer,
        space = 8 + EmergencyContacts::INIT_SPACE,
        seeds = [b"emergency_contacts", pool_core.key().as_ref()],
        bump,
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    /// Current governance authority from pool_config - must authorize the enterprise upgrade.
    /// This ensures only legitimate pool operators can enable enterprise features,
    /// preventing unauthorized privilege escalation through enterprise mode activation.
    #[account(
        constraint = core_authority_signer.key() == pool_config.load()?.core_authority @ PoolError::Unauthorized
    )]
    pub core_authority_signer: Signer<'info>,

    /// Account funding the upgrade
    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// **Phase 1 Handler: Security Foundation Bootstrap**
///
/// Establishes the foundational security infrastructure required for enterprise operations.
/// This function implements a carefully sequenced initialization of governance and emergency
/// response systems within Solana's compute unit constraints (~45k CU).
///
/// ## Temporal Consistency Strategy
/// All components are initialized with the same timestamp to establish a consistent
/// baseline for time-based security policies and prevent timing-based attacks that
/// could exploit initialization sequence variations.
///
/// ## Authority Continuity Design
/// The existing pool governance authority is transferred to the new enterprise authority
/// structure, ensuring seamless governance transition without authority gaps that could
/// be exploited during the upgrade process.
///
/// ## Status Flag Checkpoint System
/// The SECURITY_FOUNDATION_INITIALIZED flag serves as a checkpoint that prevents
/// re-entry and enables subsequent phases to verify prerequisite completion.
/// This bitwise flag approach minimizes storage overhead while providing robust
/// state management for the multi-phase upgrade process.
pub fn initialize_security_foundation(
    ctx: Context<InitializeSecurityFoundation>,
    multisig_threshold: u8,
    multisig_members: [Pubkey; 7],
    pause_authority: Pubkey,
) -> Result<()> {
    // Cache pool core load for efficient validation and prevent multiple deserializations
    let pool_core_data = ctx.accounts.pool_core.load()?;

    // Validate enterprise upgrade prerequisites with cached data and constants
    require!(
        !pool_core_data.is_enterprise(),
        PoolError::AlreadyEnterprise
    );
    require!(
        pool_core_data.status_flags & PoolCore::SECURITY_FOUNDATION_INITIALIZED == 0,
        PoolError::SecurityFoundationAlreadyInitialized
    );

    // Explicit drop to release borrow before mutable operations
    drop(pool_core_data);

    // Single timestamp ensures temporal consistency across all security components
    // Critical for time-based security policies that must have consistent baselines
    let clock = Clock::get()?;
    let timestamp = clock.unix_timestamp;

    // Extract current governance authority to maintain continuity during upgrade
    // Prevents authority gaps that could be exploited during transition
    let pool_config = &mut ctx.accounts.pool_config.load_mut()?;
    let current_core_authority = pool_config.core_authority;

    // Core authority initialization establishes governance foundation
    // Must be first as other components may reference authority during setup
    let core_authority = &mut ctx.accounts.core_authority.load_init()?;
    core_authority.initialize(
        ctx.accounts.pool_core.key(),
        current_core_authority,
        timestamp,
    )?;
    pool_config.core_authority = ctx.accounts.core_authority.key();

    // Multisig configuration enables distributed authorization for sensitive operations
    // Separate from core authority to enable different security models per operation type
    let multisig_config = &mut ctx.accounts.multisig_config.load_init()?;
    multisig_config.initialize(
        ctx.accounts.pool_core.key(),
        multisig_threshold,
        multisig_members,
        timestamp,
    )?;

    // Emergency contacts registry enables rapid incident response capabilities
    // Architecturally separate from multisig to allow faster response when consensus would be too slow
    let emergency_contacts = &mut ctx.accounts.emergency_contacts.load_init()?;
    emergency_contacts.initialize(ctx.accounts.pool_core.key(), pause_authority, timestamp)?;

    // Checkpoint flag prevents re-entry and signals completion to subsequent phases
    // Bitwise OR operation is atomic and preserves other status flags
    let mut pool_core = ctx.accounts.pool_core.load_mut()?;
    pool_core.status_flags |= PoolCore::SECURITY_FOUNDATION_INITIALIZED;

    msg!(
        "Security foundation initialized for pool: {}",
        ctx.accounts.pool_core.key()
    );

    Ok(())
}
