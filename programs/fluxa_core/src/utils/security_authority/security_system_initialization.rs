use crate::utils::security_authority::{
    audit_trail::{AuditTrailEntry, AuditTrailHead},
    core_authority::CoreAuthority,
    emergency_contacts::EmergencyContacts,
    multisig_config::MultisigConfig,
    security_coordinator::{SecurityCoordinator, SecurityEventArgs},
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;

/// Comprehensive security system bootstrap context for atomic multi-component initialization.
///
/// This account context orchestrates the creation of an entire security infrastructure in a
/// single atomic transaction, implementing a defense-in-depth security model where multiple
/// independent components collaborate to protect protocol assets and operations.
///
/// ## Atomic Bootstrap Philosophy
/// All security components are initialized together to prevent partial security states that
/// could create vulnerabilities. If any component fails initialization, the entire security
/// system setup is rolled back, ensuring protocols never operate with incomplete protection.
///
/// ## Deterministic PDA Architecture
/// All security accounts use the same pool_core as their seed, creating a hierarchical
/// security model where:
/// - Each pool instance has completely isolated security infrastructure
/// - Account relationships are cryptographically enforced through seed validation
/// - Malicious account substitution is prevented by deterministic address derivation
/// - Security component discovery is predictable without requiring additional storage
///
/// ## Economic Optimization Strategy
/// Single payer covers all account creation costs, optimizing for deployment efficiency
/// while maintaining clear economic responsibility. This approach reduces transaction
/// complexity and gas costs compared to multi-transaction initialization patterns.
///
/// ## Zero-Copy Performance Design
/// All accounts use AccountLoader for direct memory access, ensuring security operations
/// have minimal latency even during high-frequency trading or emergency response scenarios.
/// This is critical because security checks occur on every protocol operation.
#[derive(Accounts)]
pub struct SecuritySystemInitialization<'info> {
    /// Central security orchestrator account with deterministic addressing.
    ///
    /// The coordinator serves as the single entry point for all security operations,
    /// implementing the orchestrator pattern to decouple security component interactions
    /// and enable independent component upgrades without breaking the security model.
    #[account(
        init,
        payer = payer,
        space = 8 + SecurityCoordinator::INIT_SPACE,
        seeds = [b"security_coordinator", pool_core.key().as_ref()],
        bump,
    )]
    pub security_coordinator: AccountLoader<'info, SecurityCoordinator>,

    /// Core authority account managing governance and emergency powers.
    ///
    /// This account implements time-locked authority transitions and emergency controls,
    /// serving as the root of the governance hierarchy while preventing single points
    /// of failure through multisig integration.
    #[account(
        init,
        payer = payer,
        space = 8 + CoreAuthority::INIT_SPACE,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump,
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// Emergency response contact registry for rapid incident response.
    ///
    /// Maintains a curated list of emergency responders who can trigger protocol pauses
    /// during security incidents, operating on a separate authorization model from normal
    /// governance to enable faster response times when protocol safety is at risk.
    #[account(
        init,
        payer = payer,
        space = 8 + EmergencyContacts::INIT_SPACE,
        seeds = [b"emergency_contacts", pool_core.key().as_ref()],
        bump,
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    /// Multi-signature governance configuration for distributed authority.
    ///
    /// Implements M-of-N signature requirements for sensitive operations, preventing
    /// single-key compromise attacks while maintaining operational flexibility through
    /// configurable thresholds and member management.
    #[account(
        init,
        payer = payer,
        space = 8 + MultisigConfig::INIT_SPACE,
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump,
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// Audit trail head managing the cryptographic chain of security events.
    ///
    /// Maintains metadata for the linked list of audit entries, implementing hash-chaining
    /// to create tamper-evident logs of all security-critical operations for compliance
    /// and forensic analysis requirements.
    #[account(
        init,
        payer = payer,
        space = 8 + AuditTrailHead::INIT_SPACE,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump,
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    /// Initial audit trail entry recording security system establishment.
    ///
    /// Creates the genesis entry in the audit trail, establishing the cryptographic
    /// foundation for all subsequent security event logging. The entry index of 1
    /// is hardcoded because this is always the first entry in any pool's audit history.
    #[account(
        init,
        payer = payer,
        space = 8 + AuditTrailEntry::INIT_SPACE,
        seeds = [b"audit_trail_entry", audit_trail_head.key().as_ref(), 1u8.to_le_bytes().as_ref()],
        bump,
    )]
    pub audit_trail_entry: AccountLoader<'info, AuditTrailEntry>,

    /// Pool core account serving as the security scope delimiter.
    ///
    /// UncheckedAccount because we only need its public key for PDA seed derivation,
    /// not to validate its internal structure. All security components are bound to
    /// this pool instance through deterministic address generation.
    pub pool_core: UncheckedAccount<'info>,

    /// Account funding all security component creation and providing rent.
    ///
    /// Mutable access required for lamport deduction during account creation.
    /// Economic responsibility is centralized to simplify deployment while maintaining
    /// separation from operational authority through the initial_authority parameter.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Bootstrap authority for initial security configuration.
    ///
    /// This authority establishes the initial governance structure but does not
    /// automatically receive ongoing operational control, which must be separately
    /// configured through the multisig and authority management systems.
    pub initial_authority: Signer<'info>,

    /// Solana system program for account creation operations.
    pub system_program: Program<'info, System>,
}

/// Orchestrates atomic initialization of the complete security infrastructure for a pool.
///
/// This function implements a carefully sequenced bootstrap process that establishes a
/// multi-layered security architecture in a single transaction. The ordering is critical
/// to ensure proper dependency resolution and consistent state across all components.
///
/// ## Initialization Sequence Rationale
/// The component initialization follows a dependency hierarchy:
/// 1. **Core Authority**: Foundation layer providing governance and emergency powers
/// 2. **Multisig Config**: Distributed authorization layer for sensitive operations
/// 3. **Audit Trail Head**: Logging infrastructure foundation for event tracking
/// 4. **Emergency Contacts**: Rapid response capability for security incidents
/// 5. **Security Coordinator**: Orchestration layer that references all other components
/// 6. **Initial Audit Entry**: Genesis log entry establishing audit trail baseline
///
/// This ordering ensures each component can safely reference dependencies during initialization.
///
/// ## Temporal Consistency Strategy
/// All components are initialized with the same timestamp to establish a consistent
/// temporal baseline for time-based security policies and prevent subtle timing attacks
/// that could exploit initialization sequence delays.
///
/// ## Atomic Failure Handling
/// Any initialization failure causes the entire transaction to revert, preventing
/// partial security infrastructure that could create vulnerabilities. This all-or-nothing
/// approach ensures protocols never operate with incomplete protection.
///
/// ## Cryptographic Genesis Entry
/// The initial audit entry creates a cryptographic commitment to the complete security
/// configuration, enabling future verification of the security system's integrity and
/// preventing retroactive tampering with the security establishment process.
pub fn initialize_security_system(
    ctx: Context<SecuritySystemInitialization>,
    multisig_threshold: u8,
    multisig_members: [Pubkey; 7],
    pause_authority: Pubkey,
) -> Result<()> {
    // Single timestamp ensures temporal consistency across all security components
    // Prevents timing-based attacks that could exploit initialization sequence delays
    let clock = Clock::get()?;
    let clock_timestamp = clock.unix_timestamp;

    // Core Authority initialization establishes governance foundation
    // Must be first as other components may reference authority state during setup
    let core_authority = &mut ctx.accounts.core_authority.load_init()?;
    core_authority.initialize(
        ctx.accounts.pool_core.key(),
        ctx.accounts.initial_authority.key(),
        clock_timestamp,
    )?;

    // Multisig configuration enables distributed authorization for sensitive operations
    // Initialized after core authority to prevent circular dependencies during setup
    let multisig_config = &mut ctx.accounts.multisig_config.load_init()?;
    multisig_config.initialize(
        ctx.accounts.pool_core.key(),
        multisig_threshold,
        multisig_members,
        clock_timestamp,
    )?;

    // Audit trail head establishes logging infrastructure before any events are logged
    // Must precede security coordinator which will immediately log initialization event
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_init()?;
    audit_trail_head.initialize(ctx.accounts.pool_core.key(), clock_timestamp)?;

    // Emergency contacts registry enables rapid incident response capabilities
    // Separate from multisig to allow faster emergency response times when needed
    let emergency_contacts = &mut ctx.accounts.emergency_contacts.load_init()?;
    emergency_contacts.initialize(
        ctx.accounts.pool_core.key(),
        pause_authority,
        clock_timestamp,
    )?;

    // Security coordinator initialization creates orchestration layer
    // Initialized last as it requires references to all other security components
    let security_coordinator = &mut ctx.accounts.security_coordinator.load_init()?;
    security_coordinator.initialize(
        ctx.accounts.pool_core.key(),
        ctx.accounts.core_authority.key(),
        ctx.accounts.multisig_config.key(),
        ctx.accounts.audit_trail_head.key(),
        ctx.accounts.emergency_contacts.key(),
        clock_timestamp,
    )?;

    // Genesis audit entry preparation for immutable security establishment record
    let audit_trail_entry = &mut ctx.accounts.audit_trail_entry.load_init()?;

    // Comprehensive hash of all security component addresses for verification
    // Enables future validation that security system was properly established
    let data_hash = hashv(&[
        ctx.accounts.pool_core.key().as_ref(),
        ctx.accounts.initial_authority.key().as_ref(),
        ctx.accounts.core_authority.key().as_ref(),
        ctx.accounts.multisig_config.key().as_ref(),
        ctx.accounts.emergency_contacts.key().as_ref(),
        ctx.accounts.audit_trail_head.key().as_ref(),
        ctx.accounts.security_coordinator.key().as_ref(),
    ])
    .to_bytes();

    // Genesis audit entry creates immutable record of security system establishment
    // First entry in audit trail provides cryptographic foundation for all future events
    security_coordinator.log_security_event(
        audit_trail_head,
        audit_trail_entry,
        SecurityEventArgs {
            actor: ctx.accounts.payer.key(),
            target: ctx.accounts.security_coordinator.key(),
            action: b"security_system_initialized",
            data_hash,
            timestamp: clock_timestamp,
            block_height: clock.slot,
        },
    )?;

    Ok(())
}
