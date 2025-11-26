use crate::utils::security_authority::audit_trail::{AuditTrailEntry, AuditTrailHead, InitArgs};
use anchor_lang::prelude::*;

/// Account validation context for audit trail entry creation with sequential integrity.
///
/// This context enforces the complete security model for audit entry creation,
/// including sequential ordering validation, cryptographic chain linking,
/// proper account derivation, and authority verification.
///
/// ## Sequential Index Integration
/// Uses the audit_index parameter in PDA derivation to ensure each entry gets
/// a unique, predictable address while maintaining sequential ordering constraints.
/// This prevents index collision attacks and enables efficient entry lookup.
///
/// ## Head Account Linkage
/// Requires mutable access to the audit trail head to update the entry count
/// and last hash, ensuring the head maintains accurate chain state while
/// enabling atomic entry creation and head updates.
///
/// ## Actor Authentication Model
/// Requires actor signature to ensure accountability and prevent unauthorized
/// audit entry creation. The actor becomes permanently recorded in the audit
/// trail for forensic analysis and compliance reporting.
///
/// ## Space Optimization Strategy
/// Explicitly calculates required space including discriminator to prevent
/// account size attacks and ensure consistent memory layout across different
/// entry types and data payload sizes.
#[derive(Accounts)]
#[instruction(audit_index: u64)]
pub struct CreateAuditTrailEntry<'info> {
    /// Sequential audit entry account with index-based PDA derivation.
    ///
    /// Uses audit_index in seeds to create unique, predictable addresses for
    /// each entry while maintaining sequential ordering. Space calculation
    /// includes discriminator to prevent layout issues.
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<AuditTrailEntry>(),
        seeds = [b"audit_trail_entry", pool_core.key().as_ref(), &audit_index.to_le_bytes()],
        bump
    )]
    pub audit_trail_entry: AccountLoader<'info, AuditTrailEntry>,

    /// Mutable audit trail head for atomic chain state updates.
    ///
    /// Requires mutable access to update entry count and maintain the current
    /// chain head hash, ensuring the head accurately reflects the latest
    /// audit chain state after entry creation.
    #[account(
        mut,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    /// Pool core binding ensuring audit entry scope isolation.
    ///
    /// Links entry to specific pool to prevent cross-contamination and enable
    /// pool-specific audit analysis. Must match the head's pool binding to
    /// maintain audit trail integrity.
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub pool_core: UncheckedAccount<'info>,

    /// Transaction fee payer enabling flexible operational cost models.
    ///
    /// Marked mutable for rent payment deduction. Separation from actor
    /// enables scenarios where audit costs are covered by different entities
    /// than those performing the audited actions.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Authenticated actor responsible for the audited action.
    ///
    /// Requires signature to ensure accountability and prevent unauthorized
    /// audit entry creation. This signature permanently links the actor to
    /// the audit record for forensic and compliance purposes.
    pub actor: Signer<'info>,

    /// System program dependency for account creation operations.
    ///
    /// Required for PDA account initialization and rent payment processing.
    /// Anchor validates this is the legitimate system program to prevent
    /// program substitution attacks.
    pub system_program: Program<'info, System>,
}

/// Creates a cryptographically-linked audit entry with comprehensive validation and chain updates.
///
/// This function performs the critical operation of adding a new entry to the audit trail
/// while maintaining cryptographic chain integrity, sequential ordering constraints,
/// and atomic state updates across both the entry and head accounts.
///
/// ## Cryptographic Chain Maintenance
/// Each new entry incorporates the hash of the previous entry, creating an unbreakable
/// cryptographic link that makes tampering detectable. The chain continues by updating
/// the head's latest_hash to point to the new entry's computed hash.
///
/// ## Sequential Ordering Enforcement
/// The audit_index parameter ensures strict sequential ordering of entries, preventing
/// out-of-order insertion attacks that could compromise the audit trail's temporal
/// integrity and forensic value.
///
/// ## Atomic State Updates
/// Both the new entry initialization and head state updates occur within a single
/// transaction boundary, ensuring either complete success or complete failure
/// without leaving the audit trail in an inconsistent state.
///
/// ## Temporal Anchoring Strategy
/// Records both Solana's current slot and Unix timestamp to provide dual temporal
/// anchoring that makes backdating attacks detectable and provides multiple
/// reference points for chronological verification.
///
/// ## Zero-Copy Performance Optimization
/// Uses load_mut() and load_init() to work directly with account memory without
/// deserialization overhead, crucial for maintaining high transaction throughput
/// in busy DeFi environments with frequent audit requirements.
pub fn create_audit_trail_entry(
    ctx: Context<CreateAuditTrailEntry>,
    audit_index: u64,
    action: [u8; 32],
    target: Pubkey,
    data_hash: [u8; 32],
) -> Result<()> {
    // Load accounts with zero-copy access for performance optimization
    // Mutable access to head enables atomic chain state updates
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_mut()?;
    let audit_trail_entry = &mut ctx.accounts.audit_trail_entry.load_init()?;

    // Extract previous hash from head to maintain cryptographic chain linkage
    // This hash becomes the cryptographic anchor for the new entry
    let previous_hash = audit_trail_head.latest_hash;

    // Capture current temporal anchors for dual time reference validation
    // Provides both blockchain-native and calendar time for forensic analysis
    let clock = Clock::get()?;

    // Initialize the new audit entry with comprehensive parameter validation
    // All fields are validated and cryptographic chain linkage is established
    audit_trail_entry.initialize(InitArgs {
        pool_core: ctx.accounts.pool_core.key(),
        audit_index,
        action,
        actor: ctx.accounts.actor.key(),
        target,
        data_hash,
        previous_hash,
        timestamp: clock.unix_timestamp,
        block_height: clock.slot,
    })?;

    // Update the audit trail head to reflect the new chain state
    // This maintains the head's role as the current chain pointer and entry counter
    audit_trail_head.add_entry(audit_trail_entry)?;

    Ok(())
}
