use crate::error::FactoryError;
use crate::state::factory::factory_account::Factory;
use crate::utils::security_authority::core_authority::CoreAuthority;
use crate::utils::security_authority::emergency_contacts::EmergencyContacts;
use crate::utils::security_authority::multisig_config::MultisigConfig;
use crate::utils::security_authority::security_coordinator::SecurityCoordinator;
use anchor_lang::prelude::*;

/// **Factory Enterprise Upgrade - Phase 1: Security Foundation**
///
/// ## Factory-Level Enterprise Architecture
/// Factory enterprise upgrades follow the same phased approach as pools but address protocol-wide
/// security concerns rather than individual pool security. This creates a hierarchy where factory
/// enterprise mode can enforce enhanced security policies across all pools it manages.
///
/// ## Global vs Pool Security Coordination  
/// Factory security infrastructure operates at a different scope than pool security:
/// - **Factory Security**: Protocol fees, global emergency pauses, factory configuration changes
/// - **Pool Security**: Individual pool operations, trading controls, pool-specific emergencies
///
/// This separation enables granular security control while maintaining protocol-wide consistency.
///
/// ## Deterministic Addressing Strategy
/// Factory security components use global seeds (without pool-specific context) to create
/// protocol-wide security infrastructure that can coordinate across all pools and factory operations.
#[derive(Accounts)]
pub struct InitializeFactorySecurityFoundation<'info> {
    /// **Factory State**: Target of enterprise security upgrade with prerequisite validation.
    ///
    /// Mutable access required for security foundation status flag updates.
    /// Constraints prevent double-initialization and ensure factory is eligible for upgrade.
    #[account(mut)]
    pub factory: AccountLoader<'info, Factory>,

    /// **Existing Governance Authority**: Validation anchor for legitimate enterprise upgrade authorization.
    ///
    /// Must be the current factory authority to prevent unauthorized enterprise upgrades
    /// that could bypass existing governance structures or create authority confusion.
    #[account(
        seeds = [b"core_authority", factory.key().as_ref()],
        bump,
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// **Factory Security Coordinator**: Protocol-wide security orchestration layer.
    ///
    /// ## Global Security Architecture
    /// Unlike pool coordinators which manage individual pool security, this coordinator
    /// manages factory-wide security: protocol fee updates, global emergency pauses,
    /// and cross-pool security policies. Uses factory-specific seeds for global scope.
    ///
    /// ## Compute Unit Optimization
    /// Initialized in Phase 1 to distribute compute load across upgrade phases,
    /// enabling complex security initialization within Solana's transaction limits.
    #[account(
        init,
        payer = payer,
        space = 8 + SecurityCoordinator::INIT_SPACE,
        seeds = [b"factory_security_coordinator"],
        bump,
    )]
    pub security_coordinator: AccountLoader<'info, SecurityCoordinator>,

    /// **Factory Multisig Configuration**: Protocol-level distributed authorization.
    ///
    /// ## Global Multisig Scope
    /// Governs factory-wide operations requiring consensus: protocol fee changes,
    /// factory emergency controls, global configuration updates. Separate from
    /// individual pool multisig configurations to enable different security models.
    ///
    /// ## Authority Hierarchy Design
    /// Factory multisig sits above pool multisigs in the authority hierarchy,
    /// enabling protocol-wide security policies while preserving pool autonomy.
    #[account(
        init,
        payer = payer,
        space = 8 + MultisigConfig::INIT_SPACE,
        seeds = [b"factory_multisig_config"],
        bump,
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// **Factory Emergency Contacts**: Protocol-wide crisis response capability.
    ///
    /// ## Global Emergency Response Architecture
    /// Provides immediate factory-wide emergency response for protocol-threatening
    /// situations: oracle failures affecting multiple pools, systemic market risks,
    /// or detected protocol-wide exploits requiring immediate global protection.
    ///
    /// ## Response Time Optimization
    /// Separate from multisig governance to enable sub-minute emergency responses
    /// when protocol-wide threats require faster action than consensus allows.
    #[account(
        init,
        payer = payer,
        space = 8 + EmergencyContacts::INIT_SPACE,
        seeds = [b"factory_emergency_contacts"],
        bump,
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    /// **Upgrade Authorization**: Governance signer approving enterprise security activation.
    ///
    /// Must match current factory authority to prevent unauthorized security infrastructure
    /// deployment that could create parallel authority structures or governance confusion.
    pub authority_signer: Signer<'info>,

    /// **Transaction Sponsor**: Account funding the security infrastructure deployment.
    ///
    /// Mutable access required for rent payment during enterprise security account creation.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// **Solana System Program**: Required for security infrastructure account creation.
    pub system_program: Program<'info, System>,
}

/// **Factory Enterprise Upgrade Handler - Phase 1**
///
/// ## Factory-Level Security Foundation Strategy
/// Establishes protocol-wide security infrastructure that governs factory operations
/// and can enforce enterprise policies across all pools managed by the factory.
/// This creates a security hierarchy where factory enterprise mode enables
/// enhanced coordination across the entire protocol ecosystem.
///
/// ## Authority Validation Approach
/// Comprehensive validation ensures only legitimate factory authorities can initiate
/// enterprise upgrades, preventing unauthorized security infrastructure deployment
/// that could create parallel governance structures or bypass existing controls.
///
/// ## Infrastructure Initialization Sequence
/// Components are initialized in dependency order: security coordinator first (as the
/// orchestrator), then multisig configuration (distributed authority), then emergency
/// contacts (rapid response capability). This ordering ensures each component can
/// safely reference dependencies during initialization.
///
/// ## Status Flag Checkpoint System
/// The security_foundation_initialized flag serves as a checkpoint for Phase 3,
/// preventing enterprise activation without complete security infrastructure.
pub fn initialize_factory_security_foundation(
    ctx: Context<InitializeFactorySecurityFoundation>,
    multisig_threshold: u8,
    multisig_members: [Pubkey; 7],
    pause_authority: Pubkey,
) -> Result<()> {
    // Pre-validation: cache factory state for comprehensive prerequisite checks
    let factory_data = ctx.accounts.factory.load()?;

    // Enterprise mode collision prevention: ensure factory not already enterprise
    require!(
        factory_data.enterprise_mode == 0,
        FactoryError::InvalidAuthority
    );

    // Double-initialization prevention: ensure security foundation not already established
    require!(
        factory_data.security_foundation_initialized == 0,
        FactoryError::InvalidAuthority
    );

    // Authority continuity validation: ensure legitimate governance authorization
    let core_authority = ctx.accounts.core_authority.load()?;
    require!(
        core_authority.current_authority == ctx.accounts.authority_signer.key(),
        FactoryError::InvalidAuthority
    );

    // Release factory reference to enable mutable access later
    drop(factory_data);

    // Temporal consistency: single timestamp for all security component initialization
    let clock = Clock::get()?;
    let timestamp = clock.unix_timestamp;

    // Security coordinator initialization: central orchestration layer for factory security
    // No audit trail head reference yet as factory audit system not implemented in this phase
    let security_coordinator = &mut ctx.accounts.security_coordinator.load_init()?;
    security_coordinator.initialize(
        ctx.accounts.factory.key(),
        ctx.accounts.core_authority.key(),
        ctx.accounts.multisig_config.key(),
        Pubkey::default(), // No factory audit trail head in current implementation
        ctx.accounts.emergency_contacts.key(),
        timestamp,
    )?;

    // Factory multisig configuration: distributed authorization for protocol-wide decisions
    // Separate from individual pool multisigs to enable different governance models
    let multisig_config = &mut ctx.accounts.multisig_config.load_init()?;
    multisig_config.initialize(
        ctx.accounts.factory.key(),
        multisig_threshold,
        multisig_members,
        timestamp,
    )?;

    // Emergency contacts registry: rapid response capability for protocol-wide threats
    // Independent from multisig to enable faster emergency responses when consensus delays could be fatal
    let emergency_contacts = &mut ctx.accounts.emergency_contacts.load_init()?;
    emergency_contacts.initialize(ctx.accounts.factory.key(), pause_authority, timestamp)?;

    // Phase completion checkpoint: mark security foundation as established
    let mut factory = ctx.accounts.factory.load_mut()?;
    factory.security_foundation_initialized = 1;
    factory.last_update_slot = clock.slot;

    msg!(
        "Factory security foundation initialized with coordinator: {}",
        ctx.accounts.security_coordinator.key()
    );

    Ok(())
}
