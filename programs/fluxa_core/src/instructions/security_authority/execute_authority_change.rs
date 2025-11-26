use crate::error::PdaSecurityAuthorityError;
use crate::utils::security_authority::core_authority::CoreAuthority;
use crate::utils::security_authority::multisig_config::MultisigConfig;
use anchor_lang::prelude::*;

/// Anchor account context for authority transition execution and confirmation.
///
/// This context implements the final phase of secure authority handoff, combining
/// time-lock validation with multisig consensus requirements:
///
/// ## Dual-Authority Pattern for Execution
/// Both CoreAuthority and MultisigConfig require mutable access because execution
/// involves updating both the authority state and resetting multisig confirmations.
/// This atomic update pattern ensures consistent state across both accounts.
///
/// ## Distributed Confirmation Authority
/// Only multisig members can trigger execution attempts, preventing unauthorized
/// parties from repeatedly testing execution conditions. The confirmer validation
/// occurs within the handler logic rather than account constraints to enable
/// detailed error reporting.
///
/// ## Deterministic Account Relationships
/// All accounts derive from the same pool_core seed, ensuring execution can only
/// affect the intended pool's governance structure. This prevents execution
/// attempts from being misdirected to wrong pools or authority structures.
#[derive(Accounts)]
pub struct ExecuteAuthorityChange<'info> {
    /// CoreAuthority account where the transition will be executed.
    ///
    /// Mutable access required for authority state updates and proposal cleanup
    /// after successful execution or failed validation attempts.
    #[account(
        mut,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// MultisigConfig for confirmation tracking and threshold validation.
    ///
    /// Mutable access needed for confirmation state updates and potential
    /// reset operations during the execution process.
    #[account(
        mut,
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// CHECK: Pool core account establishing execution context and account relationships.
    pub pool_core: UncheckedAccount<'info>,

    /// Confirmer account that must be validated as a multisig member.
    ///
    /// Cryptographic signature proves intent while business logic validates
    /// authorization to participate in the execution process.
    pub confirmer: Signer<'info>,
}

/// Processes authority transition confirmation and executes validated changes.
///
/// This handler orchestrates the final phase of secure governance handoff, combining
/// multisig consensus tracking with time-lock validation and atomic execution:
///
/// ## Confirmation Tracking Architecture
/// The handler creates a deterministic proposal hash that uniquely identifies the
/// specific authority transition being confirmed. This hash combines multiple
/// elements to prevent confirmation replay attacks:
/// - **Authority Identity**: The specific new authority being proposed
/// - **Temporal Component**: The exact timestamp when the proposal was initiated
/// - **Operation Context**: A prefix distinguishing authority changes from other proposals
///
/// ## Progressive Validation Strategy
/// Confirmation processing follows a staged approach:
/// 1. **Authorization Check**: Validates confirmer is a multisig member
/// 2. **Consensus Tracking**: Records confirmation and checks threshold
/// 3. **Execution Attempt**: Only proceeds if multisig threshold is reached
/// 4. **Final Validation**: Time-lock and approval checks within execution method
///
/// ## Atomic Execution Design
/// The execution attempt only occurs when multisig threshold is reached, but the
/// actual execution method performs final validation (time-lock completion, approval
/// status). This layered approach ensures all security requirements are satisfied
/// before irreversible state changes.
///
/// ## State Synchronization Pattern
/// Both CoreAuthority and MultisigConfig are updated atomically during successful
/// execution, ensuring consistent governance state across both account structures.
/// Partial updates are prevented by the atomic execution model.
pub fn execute_authority_change(ctx: Context<ExecuteAuthorityChange>) -> Result<()> {
    // Load governance accounts for confirmation processing and potential execution
    let core_authority = &mut ctx.accounts.core_authority.load_mut()?;
    let multisig_config = &mut ctx.accounts.multisig_config.load_mut()?;

    // Validate confirmer authorization for governance participation
    if !multisig_config.is_member(&ctx.accounts.confirmer.key()) {
        return Err(PdaSecurityAuthorityError::NotAMultisigMember.into());
    }

    // Create deterministic proposal hash for confirmation tracking
    let pending_authority = core_authority.pending_authority;
    let proposal_hash = anchor_lang::solana_program::hash::hashv(&[
        b"authority_change",
        pending_authority.as_ref(),
        &core_authority.authority_change_requested_at.to_le_bytes(),
    ])
    .to_bytes();

    // Establish temporal context for confirmation processing
    let clock = Clock::get()?;

    // Process confirmation and check if threshold reached
    let threshold_reached = multisig_config.confirm_proposal(
        &ctx.accounts.confirmer.key(),
        proposal_hash,
        clock.unix_timestamp,
    )?;

    // Execute authority transition if multisig consensus achieved
    if threshold_reached {
        core_authority.execute_authority_change(clock.unix_timestamp)?;
    }

    Ok(())
}
