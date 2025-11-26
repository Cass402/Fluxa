use crate::error::PdaSecurityAuthorityError;
use crate::utils::security_authority::audit_trail::{AuditTrailEntry, AuditTrailHead};
use crate::utils::security_authority::core_authority::CoreAuthority;
use crate::utils::security_authority::emergency_contacts::{EmergencyContacts, EmergencyRole};
use crate::utils::security_authority::security_coordinator::{
    AddEmergencyContactArgs, SecurityCoordinator,
};
use anchor_lang::prelude::*;

/// Account validation context for emergency contact management operations.
///
/// This context governs the addition of new emergency contacts to the protocol's
/// emergency response registry. Emergency contact management is highly sensitive
/// because these contacts have unilateral authority to halt protocol operations
/// during crisis situations, making proper authorization validation critical.
///
/// ## Authority-Only Operations
/// Only the current protocol authority can modify emergency contacts, ensuring
/// that emergency response capabilities remain under legitimate governance control.
/// This prevents insider attacks where rogue actors could install malicious
/// emergency contacts to disrupt operations or extract value.
///
/// ## Trust Boundary Management
/// Emergency contacts operate in a different trust domain from multisig members,
/// with the ability to act unilaterally during emergencies. This makes their
/// management even more critical than normal authorization changes.
#[derive(Accounts)]
#[instruction(next_audit_index: u64)]
pub struct AddEmergencyContact<'info> {
    /// Security coordinator requiring mutation to log the emergency contact addition
    /// Coordinates the audit trail integration for this sensitive operation
    #[account(
        mut,
        seeds = [b"security_coordinator", pool_core.key().as_ref()],
        bump
    )]
    pub security_coordinator: AccountLoader<'info, SecurityCoordinator>,

    /// Core authority account for authorization validation
    /// Read-only access since we only verify the current authority, not modify it
    #[account(
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// Emergency contacts registry requiring mutation to add the new contact
    /// This is where the actual emergency contact data is stored and managed
    #[account(
        mut,
        seeds = [b"emergency_contacts", pool_core.key().as_ref()],
        bump
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    /// Audit trail head requiring mutation to record the contact addition event
    /// Ensures complete transparency and accountability for emergency contact changes
    #[account(
        mut,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    /// New audit trail entry to document the emergency contact addition
    /// Captures who was added, their role, permissions, and authorizing authority
    #[account(
        init,
        payer = authority,
        space = 8 + AuditTrailEntry::INIT_SPACE,
        seeds = [b"audit_trail_entry", audit_trail_head.key().as_ref(), next_audit_index.to_le_bytes().as_ref()],
        bump
    )]
    pub audit_trail_entry: AccountLoader<'info, AuditTrailEntry>,

    /// Pool core account defining the scope of the emergency contact addition
    /// Ensures contact management is properly isolated to the correct pool
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub pool_core: UncheckedAccount<'info>,

    /// The new emergency contact being added to the registry
    /// UncheckedAccount since we only store its public key in the emergency contacts registry
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub new_emergency_contact: UncheckedAccount<'info>,

    /// The protocol authority authorizing the emergency contact addition, who pays for audit entry
    /// Must match the current authority in core_authority for the operation to succeed
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Solana system program required for audit trail entry account creation
    pub system_program: Program<'info, System>,
}

/// Adds new emergency contacts to the protocol's emergency response registry with strict authorization.
///
/// This function manages the addition of emergency contacts who have the authority to trigger
/// protocol-wide emergency pauses. Emergency contact management is highly sensitive because
/// these contacts operate in a separate trust domain from normal multisig operations and can
/// act unilaterally during crisis situations.
///
/// ## Authority-Only Authorization
/// Only the current protocol authority can add emergency contacts, ensuring that emergency
/// response capabilities remain under legitimate governance control. This prevents scenarios
/// where compromised multisig members could install malicious emergency contacts to later
/// disrupt operations or extract value during manufactured crises.
///
/// ## Role and Permission Granularity
/// The function supports granular role and permission assignment, allowing different emergency
/// contacts to have varying levels of authority. This enables a tiered emergency response
/// system where different types of emergencies can be handled by appropriately authorized contacts.
///
/// ## Comprehensive Audit Logging
/// All emergency contact additions are permanently recorded in the audit trail with complete
/// metadata including the contact's role, permissions, and authorizing authority. This ensures
/// full transparency and accountability for changes to the emergency response structure.
///
/// ## Trust Boundary Implications
/// Adding emergency contacts effectively expands the trust boundary of the protocol, as these
/// contacts gain unilateral pause authority. The audit trail provides the necessary transparency
/// to monitor and review these sensitive trust modifications.
pub fn add_emergency_contact(
    ctx: Context<AddEmergencyContact>,
    next_audit_index: u64,
    role: EmergencyRole,
    permissions: u32,
) -> Result<()> {
    // Validate audit index to ensure emergency contact changes are properly sequenced
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_mut()?;
    require!(
        next_audit_index == audit_trail_head.current_index + 1,
        PdaSecurityAuthorityError::InvalidAuditIndex
    );

    // Load all required accounts for emergency contact addition
    let security_coordinator = &mut ctx.accounts.security_coordinator.load_mut()?;
    let core_authority = &ctx.accounts.core_authority.load()?;
    let emergency_contacts = &mut ctx.accounts.emergency_contacts.load_mut()?;
    let audit_trail_entry = &mut ctx.accounts.audit_trail_entry.load_init()?;

    // Execute emergency contact addition through coordinator's orchestration logic
    // This ensures proper authorization validation and consistent audit trail integration
    security_coordinator.coordinate_add_emergency_contact(
        core_authority,
        emergency_contacts,
        audit_trail_head,
        audit_trail_entry,
        AddEmergencyContactArgs {
            authority: ctx.accounts.authority.key(),
            contact: ctx.accounts.new_emergency_contact.key(),
            role,
            permissions,
        },
    )?;

    Ok(())
}
