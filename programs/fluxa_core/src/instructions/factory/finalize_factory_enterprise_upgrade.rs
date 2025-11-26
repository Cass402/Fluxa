use crate::error::FactoryError;
use crate::state::factory::factory_account::Factory;
use crate::utils::security_authority::core_authority::CoreAuthority;
use crate::utils::security_authority::security_coordinator::SecurityCoordinator;
use anchor_lang::prelude::*;

/// **Factory Enterprise Upgrade - Phase 3: Enterprise Activation**
///
/// ## Final Phase Strategy
/// Phase 3 represents the atomic transition from basic to enterprise mode operation.
/// No Phase 2 audit system for factories yet - factory audit requirements are typically
/// less complex than individual pool audit needs, focusing on protocol-level events
/// rather than detailed trading activity.
///
/// ## Enterprise Mode Activation Logic
/// This phase links the security coordinator to the factory state and activates
/// enterprise mode flags, ensuring all factory operations now route through
/// enhanced security infrastructure for appropriate operation types.
///
/// ## Simplified Architecture Rationale
/// Factory enterprise upgrades intentionally skip detailed audit trail infrastructure
/// (Phase 2) initially, as factory-level events are less frequent and can leverage
/// pool-level audit trails when detailed forensics are required.
#[derive(Accounts)]
pub struct FinalizeFactoryEnterpriseUpgrade<'info> {
    /// **Factory State**: Target of final enterprise activation with readiness validation.
    ///
    /// Must have security foundation already established (Phase 1 complete) to ensure
    /// enterprise mode activation occurs with complete security infrastructure.
    #[account(mut)]
    pub factory: AccountLoader<'info, Factory>,

    /// **Governance Authority**: Validation anchor ensuring legitimate enterprise activation.
    ///
    /// Authority continuity check prevents unauthorized enterprise mode activation
    /// that could bypass governance approval or create authority confusion.
    #[account(
        seeds = [b"core_authority", factory.key().as_ref()],
        bump,
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// **Factory Security Coordinator**: Previously initialized security infrastructure.
    ///
    /// ## Mutable Access Rationale
    /// While coordinator was initialized in Phase 1, it may require final configuration
    /// updates during enterprise activation, such as operational status flags or
    /// final integration parameters with the factory state.
    ///
    /// ## Global Addressing Consistency
    /// Uses same deterministic addressing as Phase 1 to ensure coordinator continuity
    /// and prevent creation of duplicate or conflicting security infrastructure.
    #[account(
        mut,
        seeds = [b"factory_security_coordinator"],
        bump,
    )]
    pub security_coordinator: AccountLoader<'info, SecurityCoordinator>,

    /// **Enterprise Activation Authorization**: Governance approval for final enterprise transition.
    ///
    /// Must match current factory authority to ensure legitimate enterprise activation
    /// rather than unauthorized security model changes.
    pub authority_signer: Signer<'info>,
}

/// **Factory Enterprise Upgrade Handler - Phase 3 (Final Activation)**
///
/// ## Atomic Enterprise Transition Strategy
/// This handler performs the final atomic transition from basic to enterprise mode
/// operation. All security infrastructure must be in place before this activation
/// to ensure no factory operations occur in an inconsistent security state.
///
/// ## Security Coordinator Linkage
/// Links the previously initialized security coordinator to the factory state,
/// enabling dynamic authority resolution for factory operations based on
/// operation type and risk level.
///
/// ## Enterprise Mode Benefits
/// Once activated, factory enterprise mode provides:
/// - Enhanced authorization for protocol fee changes
/// - Distributed approval requirements for critical factory configuration
/// - Coordinated emergency response across factory and pool operations
/// - Audit trail integration for compliance and forensic analysis
///
/// ## Operational Continuity Design
/// Existing basic operations continue to function while enterprise-classified
/// operations gain enhanced security requirements. This gradual transition
/// prevents operational disruption during security upgrades.
pub fn finalize_factory_enterprise_upgrade(
    ctx: Context<FinalizeFactoryEnterpriseUpgrade>,
) -> Result<()> {
    // Pre-validation: cache factory state for readiness verification
    let factory_data = ctx.accounts.factory.load()?;

    // Phase sequence validation: ensure security foundation established before activation
    require!(
        factory_data.security_foundation_initialized == 1,
        FactoryError::InvalidAuthority
    );

    // Double-activation prevention: ensure factory not already in enterprise mode
    require!(
        factory_data.enterprise_mode == 0,
        FactoryError::InvalidAuthority
    );

    // Authority continuity validation: ensure same governance approving final activation
    let core_authority = ctx.accounts.core_authority.load()?;
    require!(
        core_authority.current_authority == ctx.accounts.authority_signer.key(),
        FactoryError::InvalidAuthority
    );

    // Release factory reference for mutable access
    drop(factory_data);

    // Temporal tracking for enterprise activation audit trail
    let clock = Clock::get()?;

    // Atomic enterprise mode activation: link security coordinator and enable enterprise flags
    let mut factory = ctx.accounts.factory.load_mut()?;
    factory.security_coordinator = ctx.accounts.security_coordinator.key();
    factory.enterprise_mode = 1;
    factory.last_update_slot = clock.slot;

    msg!(
        "Factory enterprise upgrade finalized with security coordinator: {}",
        factory.security_coordinator
    );

    Ok(())
}
