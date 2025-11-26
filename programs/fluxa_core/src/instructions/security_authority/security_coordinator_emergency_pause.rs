use crate::error::PdaSecurityAuthorityError;
use crate::utils::security_authority::audit_trail::{AuditTrailEntry, AuditTrailHead};
use crate::utils::security_authority::core_authority::CoreAuthority;
use crate::utils::security_authority::core_authority::EmergencyLevel;
use crate::utils::security_authority::emergency_contacts::EmergencyContacts;
use crate::utils::security_authority::security_coordinator::{
    EmergencyPauseArgs, SecurityCoordinator,
};
use anchor_lang::prelude::*;

/// Account validation context for emergency pause operations with specialized authorization.
///
/// Emergency pauses represent one of the most critical security operations in the protocol,
/// designed to rapidly halt potentially dangerous operations when threats are detected.
/// This context enforces a separate authorization model from normal multisig operations
/// to enable faster response times during genuine emergencies.
///
/// ## Emergency Authorization Model
/// Unlike normal operations that require multisig consensus, emergency pauses can be
/// triggered by individual emergency contacts to enable rapid response. This trades
/// some security for speed when immediate action is needed to protect user funds.
///
/// ## Audit Trail Criticality
/// Emergency pause operations must be extensively logged since they represent extraordinary
/// circumstances that require post-incident analysis and accountability. The audit trail
/// captures not just who triggered the pause, but the justification and severity level.
#[derive(Accounts)]
#[instruction(next_audit_index: u64)]
pub struct EmergencyPause<'info> {
    /// Security coordinator requiring mutation to transition to emergency pause state
    /// Updates security status and flags to reflect the emergency condition
    #[account(
        mut,
        seeds = [b"security_coordinator", pool_core.key().as_ref()],
        bump
    )]
    pub security_coordinator: AccountLoader<'info, SecurityCoordinator>,

    /// Core authority requiring mutation to execute the emergency pause mechanics
    /// Handles the actual implementation of operational restrictions during pause
    #[account(
        mut,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// Emergency contacts registry for validating responder authorization
    /// Read-only access since we only verify emergency response permissions
    #[account(
        seeds = [b"emergency_contacts", pool_core.key().as_ref()],
        bump
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    /// Audit trail head requiring mutation to record the emergency pause event
    /// Critical for post-incident analysis and regulatory compliance
    #[account(
        mut,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    /// New audit trail entry to document the emergency pause activation
    /// Captures the responder, justification hash, and severity level for investigation
    #[account(
        init,
        payer = emergency_responder,
        space = 8 + AuditTrailEntry::INIT_SPACE,
        seeds = [b"audit_trail_entry", audit_trail_head.key().as_ref(), next_audit_index.to_le_bytes().as_ref()],
        bump
    )]
    pub audit_trail_entry: AccountLoader<'info, AuditTrailEntry>,

    /// Pool core account defining the scope of the emergency pause operation
    /// Ensures emergency actions are properly isolated to the affected pool
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub pool_core: UncheckedAccount<'info>,

    /// The authorized emergency responder triggering the pause, who pays for audit entry
    /// Must be validated against the emergency contacts registry before execution
    #[account(mut)]
    pub emergency_responder: Signer<'info>,

    /// Solana system program required for audit trail entry account creation
    pub system_program: Program<'info, System>,
}

/// Executes emergency pause operations with rapid response capabilities and comprehensive logging.
///
/// This function implements the protocol's emergency circuit breaker mechanism, designed to
/// rapidly halt potentially dangerous operations when threats are detected. Unlike normal
/// multisig operations, emergency pauses can be triggered by individual emergency contacts
/// to enable faster response during genuine crises.
///
/// ## Emergency Response Trade-offs
/// The emergency pause mechanism trades some security for speed, allowing individual emergency
/// contacts to halt operations unilaterally. This is justified because:
/// - Genuine emergencies require immediate response to protect user funds
/// - Emergency actions are heavily audited and logged for post-incident analysis
/// - The cost of false positives (temporary service disruption) is less than false negatives (fund loss)
///
/// ## Justification and Accountability
/// The `reason_hash` parameter ensures that emergency responders must provide justification
/// for their actions, even if the specific details are hashed for privacy. This enables
/// post-incident review and accountability while maintaining operational security.
///
/// ## Severity-Based Response
/// The `emergency_level` parameter allows for graduated responses where higher severity
/// emergencies may trigger more restrictive pause modes, enabling proportional response
/// to different types of threats.
pub fn emergency_pause(
    ctx: Context<EmergencyPause>,
    next_audit_index: u64,
    reason_hash: [u8; 32],
    emergency_level: EmergencyLevel,
) -> Result<()> {
    // Validate audit index to ensure emergency actions are properly sequenced in audit trail
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_mut()?;
    require!(
        next_audit_index == audit_trail_head.current_index + 1,
        PdaSecurityAuthorityError::InvalidAuditIndex
    );

    // Load required accounts for emergency pause execution
    let security_coordinator = &mut ctx.accounts.security_coordinator.load_mut()?;
    let core_authority = &mut ctx.accounts.core_authority.load_mut()?;
    let emergency_contacts = &ctx.accounts.emergency_contacts.load()?;
    let audit_trail_entry = &mut ctx.accounts.audit_trail_entry.load_init()?;

    // Execute emergency pause through coordinator's orchestration logic
    // This ensures consistent state management and audit trail integration
    security_coordinator.coordinate_emergency_pause(
        core_authority,
        emergency_contacts,
        audit_trail_head,
        audit_trail_entry,
        EmergencyPauseArgs {
            responder: ctx.accounts.emergency_responder.key(),
            reason_hash,
            emergency_level,
        },
    )?;

    Ok(())
}
