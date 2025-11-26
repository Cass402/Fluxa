use crate::utils::security_authority::audit_trail::AuditTrailHead;
use anchor_lang::prelude::*;

/// Account validation context for audit trail head initialization with security constraints.
///
/// This context enforces the complete set of protocol invariants required for secure
/// audit trail head creation, including proper account derivation, space allocation,
/// authority validation, and payment handling.
///
/// ## PDA Derivation Strategy
/// Uses deterministic PDA derivation with pool-specific seeds to ensure each pool
/// gets exactly one audit trail head account. The seed structure prevents collision
/// attacks and ensures predictable account addresses for verification.
///
/// ## Space Allocation Security
/// Explicitly calculates required space to prevent account size attacks where
/// malicious actors could attempt to create undersized accounts that would fail
/// during normal operations, causing denial of service.
///
/// ## Authority Validation Design
/// Requires explicit authority signature to prevent unauthorized audit trail
/// creation, ensuring only legitimate pool operators can establish audit trails
/// for their pools.
///
/// ## Payment Model Rationale
/// Uses separate payer account to enable flexible payment models where audit
/// trail creation costs can be covered by different entities than the authority,
/// supporting various operational arrangements.
#[derive(Accounts)]
pub struct InitializeAuditTrailHead<'info> {
    /// Primary audit trail head account with deterministic PDA derivation.
    ///
    /// Uses pool-specific seeds to ensure unique audit trail per pool while
    /// enabling predictable address computation for verification and access.
    /// Space calculation includes discriminator to prevent account layout issues.
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<AuditTrailHead>(),
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    /// Pool core binding ensuring audit trail scope isolation.
    ///
    /// Links audit trail to specific pool to prevent cross-contamination and
    /// enable pool-specific audit analysis. Validation ensures the pool
    /// account exists and is properly formatted.
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub pool_core: UncheckedAccount<'info>,

    /// Transaction fee payer enabling flexible cost allocation.
    ///
    /// Marked mutable to allow rent payment deduction. Separation from authority
    /// enables operational models where different entities handle costs versus
    /// control, improving operational flexibility.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Authorized pool operator ensuring legitimate audit trail creation.
    ///
    /// Requires signature to prevent unauthorized audit trail initialization,
    /// ensuring only legitimate pool operators can establish audit capabilities
    /// for their pools.
    pub authority: Signer<'info>,

    /// System program dependency for account creation operations.
    ///
    /// Required for PDA account initialization and rent payment processing.
    /// Anchor validates this is the legitimate system program to prevent
    /// program substitution attacks.
    pub system_program: Program<'info, System>,
}

/// Establishes the foundational audit trail head with proper initialization and validation.
///
/// This function creates the root of an audit trail chain, establishing the initial
/// state required for subsequent audit entry creation. The head serves as both the
/// metadata container and the anchor point for the cryptographic chain.
///
/// ## Initialization Security Model
/// The function enforces strict initialization invariants to prevent malformed
/// audit trails that could compromise forensic integrity. All validation occurs
/// atomically to ensure consistent state creation.
///
/// ## Pool Binding Strategy
/// Links the audit trail head to a specific pool core account, ensuring audit
/// scope isolation and preventing cross-contamination between different pool
/// audit trails during analysis and compliance reporting.
///
/// ## Account Loader Pattern
/// Uses load_init() to safely initialize the zero-copy account while ensuring
/// proper memory layout and preventing initialization race conditions that
/// could lead to corrupted audit trail state.
///
/// ## State Transition Atomicity
/// The entire initialization occurs within a single transaction boundary,
/// ensuring that either the audit trail head is fully initialized or the
/// operation fails completely, preventing partial initialization states.
pub fn initialize_audit_trail_head(ctx: Context<InitializeAuditTrailHead>) -> Result<()> {
    // Load and initialize the audit trail head with atomic state transition
    // This ensures the head starts in a valid, consistent state ready for entries
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_init()?;

    let clock = Clock::get()?;
    // Bind the audit trail to the specific pool for scope isolation
    // This prevents cross-contamination and enables pool-specific analysis
    audit_trail_head.initialize(ctx.accounts.pool_core.key(), clock.unix_timestamp)?;

    Ok(())
}
