use crate::error::PdaSecurityAuthorityError;
use crate::utils::security_authority::audit_trail::{AuditTrailEntry, AuditTrailHead};
use crate::utils::security_authority::core_authority::CoreAuthority;
use crate::utils::security_authority::multisig_config::MultisigConfig;
use crate::utils::security_authority::security_coordinator::SecurityCoordinator;
use anchor_lang::prelude::*;

/// Account validation context for authority change proposals with comprehensive audit infrastructure.
///
/// This context implements the first phase of the two-phase authority change protocol,
/// ensuring that all necessary security checks and audit trail components are properly
/// validated and initialized before any authority modifications can proceed.
///
/// ## Audit Index Security
/// The `next_audit_index` instruction parameter serves as a nonce to prevent audit trail
/// manipulation attacks. By requiring callers to specify the expected audit index,
/// we ensure that audit entries are created in strict sequential order and prevent
/// race conditions that could allow audit trail gaps or duplicates.
///
/// ## Account Relationship Validation
/// All accounts must reference the same pool_core to ensure operation scope integrity
/// and prevent cross-pool authority manipulation attacks. The PDA derivation constraints
/// cryptographically enforce these relationships at the protocol level.
#[derive(Accounts)]
#[instruction(next_audit_index: u64)]
pub struct AuthorityChangeProposal<'info> {
    /// Security coordinator account requiring mutation for state updates during proposal process
    /// The coordinator tracks proposal state and manages audit trail integration
    #[account(
        mut,
        seeds = [b"security_coordinator", pool_core.key().as_ref()],
        bump
    )]
    pub security_coordinator: AccountLoader<'info, SecurityCoordinator>,

    /// Core authority account requiring mutation to store the pending authority proposal
    /// This account maintains the two-phase commit state for authority changes
    #[account(
        mut,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// Multisig configuration for validating proposer authorization and threshold requirements
    /// Read-only access since we only validate membership, not modify configuration
    #[account(
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// Audit trail head requiring mutation to link the new audit entry to the trail
    /// Maintains the cryptographic chain of audit events for integrity verification
    #[account(
        mut,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    /// New audit trail entry initialized to record the authority change proposal
    /// Uses deterministic PDA derivation based on audit index for sequential ordering
    #[account(
        init,
        payer = proposer,
        space = 8 + AuditTrailEntry::INIT_SPACE,
        seeds = [b"audit_trail_entry", audit_trail_head.key().as_ref(), next_audit_index.to_le_bytes().as_ref()],
        bump
    )]
    pub audit_trail_entry: AccountLoader<'info, AuditTrailEntry>,

    /// Pool core account serving as the scope delimiter for this authority change operation
    /// UncheckedAccount since we only need its public key for PDA seed validation
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub pool_core: UncheckedAccount<'info>,

    /// The multisig member proposing the authority change, who pays for audit entry creation
    /// Must be validated as an authorized multisig member before proposal acceptance
    #[account(mut)]
    pub proposer: Signer<'info>,

    /// The proposed new authority that would replace the current authority if confirmed
    /// UncheckedAccount since we only store its public key, not validate its structure
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub new_authority: UncheckedAccount<'info>,

    /// Solana system program required for creating the new audit trail entry account
    pub system_program: Program<'info, System>,
}

/// Initiates the authority change proposal phase with comprehensive validation and audit logging.
///
/// This function implements the first phase of the secure two-phase authority change protocol,
/// establishing a pending authority change that requires subsequent multisig confirmations.
/// The design prevents atomic authority takeover attacks by separating proposal from execution.
///
/// ## Audit Index Validation
/// The `next_audit_index` parameter serves as a critical security nonce that prevents audit
/// trail manipulation. By requiring callers to specify the expected next audit index, we
/// ensure strict sequential ordering and prevent race conditions or audit gaps.
///
/// ## Proposal State Management
/// The function immediately transitions the security coordinator to AuthorityTransition status,
/// acting as a distributed lock that prevents concurrent authority change attempts and signals
/// to other protocol components that sensitive operations should be restricted.
///
/// ## Error Handling
/// All validation failures result in transaction rollback with no state changes, ensuring
/// the protocol cannot be left in an inconsistent state due to partial execution.
pub fn propose_authority_change(
    ctx: Context<AuthorityChangeProposal>,
    next_audit_index: u64,
) -> Result<()> {
    // Validate audit index to prevent audit trail manipulation attacks
    // This ensures strict sequential ordering of all security events
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_mut()?;
    require!(
        next_audit_index == audit_trail_head.current_index + 1,
        PdaSecurityAuthorityError::InvalidAuditIndex
    );

    // Load all required accounts with appropriate mutability levels
    // Coordinator and core authority need mutation, others are read-only for this operation
    let security_coordinator = &mut ctx.accounts.security_coordinator.load_mut()?;
    let core_authority = &mut ctx.accounts.core_authority.load_mut()?;
    let multisig_config = &ctx.accounts.multisig_config.load()?;
    let audit_trail_entry = &mut ctx.accounts.audit_trail_entry.load_init()?;

    // Delegate to coordinator's orchestration logic with all necessary context
    // This maintains separation of concerns between instruction handlers and business logic
    security_coordinator.coordinate_authority_change_proposal(
        core_authority,
        multisig_config,
        audit_trail_head,
        audit_trail_entry,
        ctx.accounts.new_authority.key(),
        ctx.accounts.proposer.key(),
    )?;

    Ok(())
}
