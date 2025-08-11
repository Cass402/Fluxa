use crate::error::PoolError;
use crate::state::pool::pool_config::PoolConfig;
use crate::state::pool::pool_core::PoolCore;
use crate::state::pool::pool_security::PoolSecurity;
use crate::utils::security_authority::{
    audit_trail::{AuditTrailEntry, AuditTrailHead, InitArgs},
    core_authority::CoreAuthority,
    emergency_contacts::EmergencyContacts,
    multisig_config::MultisigConfig,
    security_coordinator::{SecurityCoordinator, SecurityEventArgs},
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;

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

/// **Phase 2: Audit Trail Infrastructure**
///
/// Establishes cryptographic audit capabilities after security foundation is in place.
/// This phase creates immutable event logging infrastructure essential for enterprise
/// compliance and security monitoring (~35k CU).
///
/// ## Architectural Sequencing Logic
/// Audit systems depend on having established authority structures to properly
/// attribute and authorize logging operations. By placing this after Phase 1,
/// we ensure audit entries have legitimate authority attribution from the start.
///
/// ## Cryptographic Integrity Foundation
/// The audit trail head establishes a hash-chained event log that provides
/// tamper-evident security event tracking. This is critical for enterprise
/// environments requiring compliance audit capabilities and forensic analysis.
#[derive(Accounts)]
pub struct InitializeAuditSystem<'info> {
    /// Pool with completed security foundation - complex validations moved to handler
    /// for better performance and cached load access.
    #[account(mut)]
    pub pool_core: AccountLoader<'info, PoolCore>,

    /// Pool configuration required for authority validation during audit system setup.
    /// Ensures only legitimate authorities can establish audit infrastructure,
    /// preventing unauthorized creation of audit systems that could mask malicious activity.
    #[account(
        seeds = [b"pool_config", pool_core.key().as_ref()],
        bump,
        has_one = pool_core,
    )]
    pub pool_config: AccountLoader<'info, PoolConfig>,

    /// **Audit Trail Head**: Cryptographic foundation for tamper-evident event logging.
    /// Manages the hash chain linkage and event sequence numbers that provide
    /// cryptographic guarantees of log integrity. Critical for enterprise compliance
    /// where audit trail authenticity must be verifiable.
    #[account(
        init,
        payer = payer,
        space = 8 + AuditTrailHead::INIT_SPACE,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump,
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    /// **Genesis Audit Entry**: First cryptographic commitment in the audit chain.
    /// Deterministic PDA using audit_index ensures proper ordering and prevents
    /// audit entry collision attacks. The genesis entry establishes the baseline
    /// hash for all subsequent entries in the cryptographic chain.
    #[account(
        init,
        payer = payer,
        space = 8 + AuditTrailEntry::INIT_SPACE,
        seeds = [b"audit_trail_entry", audit_trail_head.key().as_ref(), 1u8.to_le_bytes().as_ref()],
        bump,
    )]
    pub audit_trail_entry: AccountLoader<'info, AuditTrailEntry>,

    /// Governance authority authorization required for audit system establishment.
    /// Prevents unauthorized audit system creation that could be used to obfuscate
    /// malicious activity or create false security assurances.
    #[account(
        constraint = core_authority_signer.key() == pool_config.load()?.core_authority @ PoolError::Unauthorized
    )]
    pub core_authority_signer: Signer<'info>,

    /// Account funding the upgrade
    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// **Phase 3: Enterprise Activation & Security Orchestration**
///
/// Final phase that creates the security coordinator and activates enterprise mode.
/// This orchestration layer unifies all security components into a cohesive system
/// with centralized event coordination and emergency response capabilities (~25k CU).
///
/// ## Orchestration Architecture Rationale
/// The security coordinator acts as the integration layer between all security
/// components, providing unified interfaces for complex security operations that
/// span multiple components. This design prevents tight coupling while enabling
/// sophisticated security workflows.
///
/// ## Enterprise Mode Activation Strategy
/// Enterprise mode is only activated after all security infrastructure is verified
/// and operational. This final phase ensures atomic transition from basic to
/// enterprise security model, preventing partial activation vulnerabilities.
#[derive(Accounts)]
pub struct FinalizeEnterpriseUpgrade<'info> {
    /// Pool with complete security infrastructure - final validation moved to handler
    /// for better performance with cached load access.
    #[account(mut)]
    pub pool_core: AccountLoader<'info, PoolCore>,

    /// Pool configuration required for final authority validation and governance continuity.
    /// Used to verify legitimate enterprise activation and maintain authority chain integrity.
    #[account(
        seeds = [b"pool_config", pool_core.key().as_ref()],
        bump,
        has_one = pool_core,
    )]
    pub pool_config: AccountLoader<'info, PoolConfig>,

    /// Pool security account that will be linked to the enterprise security infrastructure.
    /// This connection enables enterprise-grade security policies to be enforced
    /// across all pool operations, not just governance functions.
    #[account(
        mut,
        seeds = [b"pool_security", pool_core.key().as_ref()],
        bump,
        has_one = pool_core,
    )]
    pub pool_security: AccountLoader<'info, PoolSecurity>,

    /// **Security Coordinator**: Central orchestration layer for multi-component security operations.
    /// This coordinator provides unified interfaces for complex security workflows that
    /// span multiple security components, enabling sophisticated enterprise security patterns
    /// while maintaining loose coupling between individual components.
    #[account(
        init,
        payer = payer,
        space = 8 + SecurityCoordinator::INIT_SPACE,
        seeds = [b"security_coordinator", pool_core.key().as_ref()],
        bump,
    )]
    pub security_coordinator: AccountLoader<'info, SecurityCoordinator>,

    /// **Security Foundation References**: All security components created in previous phases.
    /// These references enable the security coordinator to establish integration with
    /// existing security infrastructure and provide unified orchestration capabilities.

    /// Core authority reference for governance integration and authority delegation patterns.
    #[account(
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump,
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// Multisig configuration reference for distributed authorization workflows.
    #[account(
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump,
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// Emergency contacts reference for rapid incident response coordination.
    #[account(
        seeds = [b"emergency_contacts", pool_core.key().as_ref()],
        bump,
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    /// Audit trail head reference (mutable) for security event logging integration.
    /// Mutable access required as the coordinator will immediately log the enterprise
    /// activation event to establish the first coordinated security event.
    #[account(
        mut,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump,
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    #[account(
        init,
        payer = payer,
        seeds = [b"audit_trail_entry", audit_trail_head.key().as_ref(), 2u8.to_le_bytes().as_ref()],
        space = 8 + AuditTrailEntry::INIT_SPACE,
        bump,
    )]
    pub audit_trail_entry: AccountLoader<'info, AuditTrailEntry>,

    /// Final governance authorization for enterprise mode activation.
    /// This signature represents the governance decision to transition to enterprise
    /// security model with its associated operational complexities and capabilities.
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

/// **Phase 2 Handler: Cryptographic Audit Infrastructure**
///
/// Establishes tamper-evident audit trail capabilities essential for enterprise compliance
/// and security monitoring. This phase creates the cryptographic foundation for immutable
/// event logging with hash-chained integrity guarantees (~35k CU).
///
/// ## Genesis Entry Strategy
/// The first audit entry serves as the cryptographic genesis of the audit chain,
/// establishing the baseline hash that all subsequent entries will reference.
/// This genesis entry is simplified compared to normal audit entries since the
/// full security coordinator isn't available yet.
///
/// ## Audit Chain Architecture
/// The audit system uses a hash-chained structure where each entry cryptographically
/// links to the previous entry, creating tamper-evident logging. The audit trail head
/// manages this chain and maintains sequence numbers for integrity verification.
///
/// ## Authority Attribution Pattern
/// Even in this phase, audit entries are properly attributed to legitimate authorities
/// to establish consistent audit patterns and prevent attribution confusion in
/// later forensic analysis.
pub fn initialize_audit_system(ctx: Context<InitializeAuditSystem>) -> Result<()> {
    // Cache pool core load for efficient validation and prevent multiple deserializations
    let pool_core_data = ctx.accounts.pool_core.load()?;

    // Validate audit system prerequisites with cached data and constants
    require!(
        pool_core_data.has_security_foundation(),
        PoolError::SecurityFoundationNotInitialized
    );
    require!(
        pool_core_data.status_flags & PoolCore::AUDIT_SYSTEM_INITIALIZED == 0,
        PoolError::AuditSystemAlreadyInitialized
    );
    require!(
        !pool_core_data.is_enterprise(),
        PoolError::AlreadyEnterprise
    );

    // Explicit drop to release borrow before mutable operations
    drop(pool_core_data);

    // Consistent temporal reference for audit chain baseline establishment
    let clock = Clock::get()?;
    let timestamp = clock.unix_timestamp;
    let slot = clock.slot;

    // Audit trail head initialization establishes the cryptographic chain foundation
    // Must precede any audit entries to provide proper hash chain management
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_init()?;
    audit_trail_head.initialize(ctx.accounts.pool_core.key(), timestamp)?;

    // Genesis audit entry preparation - first link in the cryptographic chain
    let audit_trail_entry = &mut ctx.accounts.audit_trail_entry.load_init()?;

    // Extract governance authority for proper audit attribution
    // Ensures audit entries have legitimate authority attribution from genesis
    let pool_config = ctx.accounts.pool_config.load()?;
    let current_core_authority = pool_config.core_authority;

    // Standardized 32-byte action identifier for consistent audit parsing
    // Fixed-width formatting ensures consistent audit log structure
    let action_bytes = b"AUDIT_SYSTEM_INIT               "; // 32 bytes
    let mut action = [0u8; 32];
    action.copy_from_slice(action_bytes);

    // Genesis audit entry initialization with simplified security context
    // Previous hash is zero for genesis entry as it's the chain foundation
    audit_trail_entry.initialize(InitArgs {
        pool_core: ctx.accounts.pool_core.key(),
        audit_index: 1,
        action,
        actor: current_core_authority,
        target: ctx.accounts.pool_core.key(),
        data_hash: hashv(&[
            ctx.accounts.pool_core.key().as_ref(),
            ctx.accounts.audit_trail_head.key().as_ref(),
            b"audit_system_initialized",
        ])
        .to_bytes(),
        previous_hash: [0; 32], // Genesis entry has no predecessor
        timestamp,
        block_height: slot,
    })?;

    // Link genesis entry to audit trail head to establish chain management
    // This completes the cryptographic audit foundation setup
    audit_trail_head.add_entry(audit_trail_entry)?;

    // Checkpoint flag signals audit system completion and enables phase 3
    // Bitwise OR preserves existing flags while adding audit system status
    let mut pool_core = ctx.accounts.pool_core.load_mut()?;
    pool_core.status_flags |= PoolCore::AUDIT_SYSTEM_INITIALIZED;

    msg!(
        "Audit system initialized for pool: {}",
        ctx.accounts.pool_core.key()
    );

    Ok(())
}

/// **Phase 3 Handler: Enterprise Activation & Security Orchestration**
///
/// Completes the enterprise upgrade by creating the security coordinator and atomically
/// activating enterprise mode. This final orchestration layer unifies all security
/// components into a cohesive system with centralized coordination capabilities (~25k CU).
///
/// ## Security Coordinator Integration Pattern
/// The coordinator is initialized with references to all existing security components,
/// enabling it to orchestrate complex multi-component security operations while
/// maintaining loose coupling between individual security services.
///
/// ## Atomic Enterprise Activation Strategy
/// Enterprise mode flags are set atomically across both pool_core and pool_security
/// to ensure consistent enterprise state. This prevents partial activation scenarios
/// that could lead to security policy inconsistencies.
///
/// ## Pool Security Integration Logic
/// The pool_security account is explicitly linked to the security coordinator to
/// enable enterprise security policies to be enforced across all pool operations,
/// not just governance functions. This creates comprehensive security coverage.
pub fn finalize_enterprise_upgrade(ctx: Context<FinalizeEnterpriseUpgrade>) -> Result<()> {
    // Cache pool core load for efficient validation and prevent multiple deserializations
    let pool_core_data = ctx.accounts.pool_core.load()?;

    // Validate enterprise finalization prerequisites with cached data and constants
    require!(
        pool_core_data.ready_for_enterprise_finalization(),
        PoolError::NotReadyForEnterpriseFinalization
    );

    // Explicit drop to release borrow before mutable operations
    drop(pool_core_data);

    // Consistent timestamp for coordinated enterprise activation across all components
    let clock = Clock::get()?;
    let timestamp = clock.unix_timestamp;

    // Security coordinator initialization with complete security infrastructure references
    // This creates the orchestration layer that unifies all security components
    let security_coordinator = &mut ctx.accounts.security_coordinator.load_init()?;
    security_coordinator.initialize(
        ctx.accounts.pool_core.key(),
        ctx.accounts.core_authority.key(),
        ctx.accounts.multisig_config.key(),
        ctx.accounts.audit_trail_head.key(),
        ctx.accounts.emergency_contacts.key(),
        timestamp,
    )?;

    // Atomic enterprise mode activation in pool core state
    // This flag transition represents the complete enterprise upgrade completion
    let mut pool_core = ctx.accounts.pool_core.load_mut()?;
    pool_core.status_flags |= PoolCore::ENTERPRISE_MODE_ACTIVE;

    // Pool security integration with enterprise infrastructure
    // Links operational security policies to the enterprise security coordinator
    let mut pool_security = ctx.accounts.pool_security.load_mut()?;
    pool_security.enterprise_mode = true;
    pool_security.security_coordinator = ctx.accounts.security_coordinator.key();
    pool_security.emergency_contacts = ctx.accounts.emergency_contacts.key();

    // This would create the first fully-coordinated security event in the audit trail
    // demonstrating the complete security infrastructure operational capability
    let pool_config = &ctx.accounts.pool_config.load()?;
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_mut()?;
    let audit_trail_entry = &mut ctx.accounts.audit_trail_entry.load_init()?;
    let data_hash = hashv(&[
        ctx.accounts.pool_core.key().as_ref(),
        pool_config.core_authority.as_ref(),
        ctx.accounts.core_authority.key().as_ref(),
        ctx.accounts.multisig_config.key().as_ref(),
        ctx.accounts.emergency_contacts.key().as_ref(),
        ctx.accounts.audit_trail_head.key().as_ref(),
        ctx.accounts.security_coordinator.key().as_ref(),
    ])
    .to_bytes();

    security_coordinator.log_security_event(
        audit_trail_head,
        audit_trail_entry,
        SecurityEventArgs {
            actor: ctx.accounts.payer.key(),
            target: ctx.accounts.security_coordinator.key(),
            action: b"security_system_initialized",
            data_hash,
            timestamp,
            block_height: clock.slot,
        },
    )?;

    msg!(
        "Enterprise upgrade finalized for pool: {}",
        ctx.accounts.pool_core.key()
    );
    msg!(
        "Security coordinator: {}",
        ctx.accounts.security_coordinator.key()
    );

    Ok(())
}
