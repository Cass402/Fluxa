use crate::error::PdaSecurityAuthorityError;
use crate::utils::security_authority::audit_trail::{AuditTrailEntry, AuditTrailHead};
use crate::utils::security_authority::core_authority::CoreAuthority;
use crate::utils::security_authority::multisig_config::MultisigConfig;
use crate::utils::security_authority::security_coordinator::SecurityCoordinator;
use anchor_lang::prelude::*;

/// Account validation context for multisig confirmation operations in the authority change protocol.
///
/// This context implements the confirmation phase of the two-phase authority change protocol,
/// where authorized multisig members provide their confirmations for a pending authority change.
/// The design ensures that only valid multisig members can participate and that all confirmations
/// are properly recorded in the audit trail for transparency and verification.
///
/// ## Sequential Confirmation Security
/// Each confirmation creates a new audit trail entry, maintaining a complete record of who
/// confirmed what and when. The audit index serves as a nonce to prevent confirmation replay
/// attacks and ensures strict ordering of confirmation events.
///
/// ## Atomic State Management
/// The context provides mutable access to both the coordinator and multisig configuration
/// to enable atomic updates of confirmation state and threshold checking, preventing race
/// conditions that could allow partial confirmations or threshold bypassing.
#[derive(Accounts)]
#[instruction(next_audit_index: u64)]
pub struct MultisigConfirmation<'info> {
    /// Security coordinator requiring mutation to update confirmation state and security status
    /// Tracks the overall state of the authority change process across multiple confirmations
    #[account(
        mut,
        seeds = [b"security_coordinator", pool_core.key().as_ref()],
        bump
    )]
    pub security_coordinator: AccountLoader<'info, SecurityCoordinator>,

    /// Core authority account requiring mutation to process confirmation and potentially execute authority change
    /// Maintains the pending authority state and executes the change when threshold is reached
    #[account(
        mut,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// Multisig configuration requiring mutation to record confirmations and check threshold
    /// Tracks which members have confirmed and whether the required threshold has been met
    #[account(
        mut,
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// Audit trail head requiring mutation to append the confirmation audit entry
    /// Maintains the chronological chain of all security events including confirmations
    #[account(
        mut,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    /// New audit trail entry to record this specific confirmation event
    /// Creates an immutable record of who confirmed the authority change and when
    #[account(
        init,
        payer = confirmer,
        space = 8 + AuditTrailEntry::INIT_SPACE,
        seeds = [b"audit_trail_entry", audit_trail_head.key().as_ref(), next_audit_index.to_le_bytes().as_ref()],
        bump
    )]
    pub audit_trail_entry: AccountLoader<'info, AuditTrailEntry>,

    /// Pool core account defining the operational scope for this confirmation
    /// Ensures confirmations are properly scoped to the correct pool instance
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub pool_core: UncheckedAccount<'info>,

    /// The multisig member providing confirmation, who pays for audit entry creation
    /// Must be validated as an authorized multisig member with confirmation rights
    #[account(mut)]
    pub confirmer: Signer<'info>,

    /// Solana system program required for audit trail entry account creation
    pub system_program: Program<'info, System>,
}

/// Processes multisig confirmations for pending authority changes with threshold enforcement.
///
/// This function implements the confirmation and potential execution phase of the two-phase
/// authority change protocol. It collects confirmations from authorized multisig members
/// and automatically executes the authority change when the required threshold is reached.
///
/// ## Atomic Confirmation Processing
/// The function performs confirmation recording and threshold checking atomically to prevent
/// race conditions where multiple confirmations could be processed simultaneously, potentially
/// leading to inconsistent state or threshold bypassing.
///
/// ## Return Value Semantics
/// Returns true if the confirmation caused the threshold to be reached and authority change
/// executed, false if more confirmations are still needed. This allows callers to take
/// appropriate follow-up actions without additional state queries.
///
/// ## Audit Trail Completeness
/// Every confirmation is logged regardless of whether it triggers execution, ensuring
/// complete transparency and accountability for all participants in the authority change process.
pub fn confirm_authority_change(
    ctx: Context<MultisigConfirmation>,
    next_audit_index: u64,
) -> Result<bool> {
    // Validate audit index to maintain audit trail integrity and prevent manipulation
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_mut()?;
    require!(
        next_audit_index == audit_trail_head.current_index + 1,
        PdaSecurityAuthorityError::InvalidAuditIndex
    );

    // Load all required accounts with appropriate access levels for confirmation processing
    let security_coordinator = &mut ctx.accounts.security_coordinator.load_mut()?;
    let core_authority = &mut ctx.accounts.core_authority.load_mut()?;
    let multisig_config = &mut ctx.accounts.multisig_config.load_mut()?;
    let audit_trail_entry = &mut ctx.accounts.audit_trail_entry.load_init()?;

    // Process confirmation and determine if threshold reached for execution
    // Returns boolean indicating whether authority change was executed
    let threshold_reached = security_coordinator.coordinate_multisig_confirmation(
        core_authority,
        multisig_config,
        audit_trail_head,
        audit_trail_entry,
        ctx.accounts.confirmer.key(),
    )?;

    Ok(threshold_reached)
}
