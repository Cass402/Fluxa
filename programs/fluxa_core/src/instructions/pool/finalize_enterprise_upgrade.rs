use crate::error::PoolError;
use crate::state::pool::pool_config::PoolConfig;
use crate::state::pool::pool_core::PoolCore;
use crate::state::pool::pool_security::PoolSecurity;
use crate::utils::security_authority::audit_trail::{AuditTrailEntry, AuditTrailHead};
use crate::utils::security_authority::core_authority::CoreAuthority;
use crate::utils::security_authority::emergency_contacts::EmergencyContacts;
use crate::utils::security_authority::multisig_config::MultisigConfig;
use crate::utils::security_authority::security_coordinator::{
    SecurityCoordinator, SecurityEventArgs,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;

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
    pool_security.enterprise_mode = 1;
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
