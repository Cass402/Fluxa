use crate::error::PdaSecurityAuthorityError;
use crate::utils::security_authority::core_authority::CoreAuthority;
use crate::utils::security_authority::multisig_config::MultisigConfig;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;

/// Anchor account context for authority transition proposal initiation.
///
/// This context implements the first phase of secure authority handoff, enforcing
/// distributed governance requirements and preventing unauthorized proposals:
///
/// ## Multisig Authorization Pattern
/// Only members of the associated MultisigConfig can propose authority changes,
/// implementing distributed proposal authority rather than single-signer control.
/// This prevents scenarios where a single compromised key could initiate governance
/// takeovers without community consensus.
///
/// ## PDA Validation Architecture
/// All accounts use deterministic seeds tied to the same pool_core, ensuring
/// proposals can only affect the intended pool's governance structure. The
/// seed-based validation prevents cross-pool authority manipulation attacks.
///
/// ## New Authority Pre-Validation
/// The new_authority account uses PDA seeds to establish deterministic addressing,
/// enabling validation of the proposed authority before accepting the proposal.
/// This pattern prevents invalid or malicious authority proposals from entering
/// the governance pipeline.
#[derive(Accounts)]
pub struct ProposeAuthorityChange<'info> {
    /// CoreAuthority account receiving the governance proposal.
    ///
    /// Mutable access enables proposal state updates while seed validation
    /// ensures only the correct pool's authority can be modified.
    #[account(
        mut,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump,
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// MultisigConfig for distributed proposal authorization validation.
    ///
    /// Read-only access sufficient for membership validation while seed
    /// constraints ensure proposals can only come from the pool's designated
    /// multisig governance structure.
    #[account(
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump,
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// Pool core account establishing the governance context.
    ///
    /// UncheckedAccount allows flexible validation while maintaining
    /// deterministic relationships between all governance-related accounts.
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub pool_core: UncheckedAccount<'info>,

    /// Proposer account that must be validated as a multisig member.
    ///
    /// Signer requirement ensures cryptographic proof of proposal intent
    /// while business logic validates multisig membership authorization.
    pub proposer: Signer<'info>,

    /// Proposed new authority account with deterministic addressing.
    ///
    /// PDA constraint enables pre-validation of the proposed authority
    /// while preventing arbitrary or malicious authority specifications.
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(
        seeds = [b"new_authority", pool_core.key().as_ref()],
        bump,
    )]
    pub new_authority: UncheckedAccount<'info>,
}

/// Initiates authority transition proposal with comprehensive validation and commitment.
///
/// This handler implements the first phase of secure governance handoff, enforcing
/// distributed authorization while establishing cryptographic commitment to the proposal:
///
/// ## Multi-Layer Authorization Pattern
/// Validates proposer authorization through multiple independent checks:
/// - **Multisig Membership**: Ensures only governance participants can propose
/// - **Self-Proposal Prevention**: Blocks current authority from proposing itself
/// - **State Consistency**: Verifies no conflicting proposals are in progress
///
/// ## Cryptographic Commitment Strategy
/// The proposal hash combines the new authority and proposer keys, creating an
/// immutable commitment that prevents proposal substitution attacks. This hash
/// serves as the foundation for subsequent multisig validation processes.
///
/// ## Temporal Anchoring Implementation
/// Clock timestamp capture at proposal time establishes the authoritative start
/// of the governance delay period, preventing timing manipulation attacks that
/// could bypass intended security delays.
///
/// ## Error Classification Design
/// Different error types enable calling code to distinguish between authorization
/// failures (wrong proposer) and state conflicts (proposal already in progress),
/// supporting appropriate user interface feedback and retry strategies.
pub fn propose_authority_change(ctx: Context<ProposeAuthorityChange>) -> Result<()> {
    // Load governance accounts for validation and state updates
    let core_authority = &mut ctx.accounts.core_authority.load_mut()?;
    let multisig_config = &ctx.accounts.multisig_config.load()?;

    // Validate proposer membership in multisig governance structure
    if !multisig_config.is_member(&ctx.accounts.proposer.key()) {
        return Err(PdaSecurityAuthorityError::NotAMultisigMember.into());
    }

    // Prevent self-proposal to avoid governance bypass scenarios
    if ctx.accounts.new_authority.key() == core_authority.current_authority {
        return Err(PdaSecurityAuthorityError::Unauthorized.into());
    }

    // Create cryptographic commitment to proposal content
    let data_hash = hashv(&[
        ctx.accounts.new_authority.key().as_ref(),
        ctx.accounts.proposer.key().as_ref(),
    ])
    .to_bytes();

    // Establish temporal anchor for governance delay enforcement
    let clock = Clock::get()?;

    // Initiate authority transition proposal with validated parameters
    core_authority.propose_authority_change(
        ctx.accounts.new_authority.key(),
        data_hash,
        clock.unix_timestamp,
    )?;

    Ok(())
}
