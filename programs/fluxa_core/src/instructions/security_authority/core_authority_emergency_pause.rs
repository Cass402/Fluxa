use crate::error::PdaSecurityAuthorityError;
use crate::utils::security_authority::core_authority::{CoreAuthority, EmergencyLevel};
use crate::utils::security_authority::emergency_contacts::EmergencyContacts;
use anchor_lang::prelude::*;

/// Anchor account context for emergency protocol pause activation.
///
/// This context enables rapid emergency response while maintaining authorization
/// controls and audit trails for all emergency actions:
///
/// ## Emergency Authority Validation Pattern
/// The EmergencyContacts account contains the list of authorized emergency
/// responders who can trigger protocol pauses. This separation enables
/// emergency authority management independent of normal governance structures,
/// allowing faster response times during critical incidents.
///
/// ## Read-Only Emergency Contacts Design
/// EmergencyContacts requires only read access because emergency pause activation
/// doesn't modify the emergency responder list. This separation of concerns
/// enables emergency actions without risk of accidentally modifying responder
/// authorization during crisis situations.
///
/// ## Immediate Response Architecture
/// The CoreAuthority account receives mutable access for immediate pause
/// activation without additional validation delays. The emergency responder
/// validation occurs before state changes to ensure only authorized pauses.
#[derive(Accounts)]
pub struct EmergencyPause<'info> {
    /// CoreAuthority account receiving the emergency pause activation.
    ///
    /// Mutable access enables immediate operational status changes and
    /// emergency state updates without additional confirmation delays.
    #[account(
        mut,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// EmergencyContacts for responder authorization validation.
    ///
    /// Read-only access sufficient for emergency authority validation
    /// while preventing accidental modification of responder lists during
    /// crisis response operations.
    #[account(
        seeds = [b"emergency_contacts", pool_core.key().as_ref()],
        bump
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    /// CHECK: Pool core account establishing emergency response context.
    pub pool_core: UncheckedAccount<'info>,

    /// Emergency responder account triggering the pause activation.
    ///
    /// Cryptographic signature proves emergency action intent while
    /// business logic validates emergency authority before activation.
    pub emergency_responder: Signer<'info>,
}

/// Activates emergency protocol pause with severity-appropriate response parameters.
///
/// This handler provides critical incident response capabilities while maintaining
/// authorization controls and ensuring eventual protocol recovery:
///
/// ## Emergency Authority Validation Model
/// The handler validates emergency responder authorization through the EmergencyContacts
/// account rather than using normal governance structures. This separation enables
/// rapid response times during critical incidents when normal governance processes
/// might be too slow or compromised.
///
/// ## Immediate Response Design Philosophy
/// Emergency pause activation takes effect immediately upon successful authorization,
/// providing instant protocol protection against ongoing attacks. The severity level
/// determines pause duration rather than activation delay, balancing rapid response
/// with measured restrictions.
///
/// ## Severity-Calibrated Response Strategy
/// The emergency level parameter enables proportional responses to different threat
/// types, ensuring minor issues don't trigger unnecessarily long protocol freezes
/// while critical threats receive adequate investigation time.
///
/// ## Audit Trail Integration
/// All emergency actions are timestamped and recorded in the CoreAuthority state,
/// providing comprehensive audit trails for post-incident analysis and compliance
/// reporting. This transparency supports forensic analysis and regulatory requirements.
pub fn emergency_pause(
    ctx: Context<EmergencyPause>,
    emergency_level: EmergencyLevel,
) -> Result<()> {
    // Load emergency response and authority accounts
    let core_authority = &mut ctx.accounts.core_authority.load_mut()?;
    let emergency_contacts = &ctx.accounts.emergency_contacts.load()?;

    // Validate emergency responder authorization for immediate response
    if !emergency_contacts.has_emergency_authority(&ctx.accounts.emergency_responder.key()) {
        return Err(PdaSecurityAuthorityError::InsufficientPermissions.into());
    }

    // Establish temporal context for pause duration and audit trails
    let clock = Clock::get()?;

    // Execute immediate emergency pause with severity-appropriate parameters
    core_authority.emergency_pause(emergency_level, clock.unix_timestamp)?;

    Ok(())
}
