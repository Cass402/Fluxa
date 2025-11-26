use crate::error::PoolError;
use crate::state::pool::pool_config::PoolConfig;
use crate::state::pool::pool_core::PoolCore;
use crate::utils::security_authority::audit_trail::{AuditTrailEntry, AuditTrailHead, InitArgs};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;

/// **Phase 2: Audit Trail Infrastructure**
///
/// Establishes cryptographic audit capabilities after security foundation is in place.
/// This phase creates immutable event logging infrastructure essential for enterprise
/// compliance and security monitoring (~35k CU).
///
/// ## Architectural Sequencing Logic
/// Audit systems depend on having established authority structures to properly
/// attribute and authorize logging operations. By placing this after Phase 1,
/// we ensure audit entries have legitimate authority attribution from the start.
///
/// ## Cryptographic Integrity Foundation
/// The audit trail head establishes a hash-chained event log that provides
/// tamper-evident security event tracking. This is critical for enterprise
/// environments requiring compliance audit capabilities and forensic analysis.
#[derive(Accounts)]
pub struct InitializeAuditSystem<'info> {
    /// Pool with completed security foundation - complex validations moved to handler
    /// for better performance and cached load access.
    #[account(mut)]
    pub pool_core: AccountLoader<'info, PoolCore>,

    /// Pool configuration required for authority validation during audit system setup.
    /// Ensures only legitimate authorities can establish audit infrastructure,
    /// preventing unauthorized creation of audit systems that could mask malicious activity.
    #[account(
        seeds = [b"pool_config", pool_core.key().as_ref()],
        bump,
        has_one = pool_core,
    )]
    pub pool_config: AccountLoader<'info, PoolConfig>,

    /// **Audit Trail Head**: Cryptographic foundation for tamper-evident event logging.
    /// Manages the hash chain linkage and event sequence numbers that provide
    /// cryptographic guarantees of log integrity. Critical for enterprise compliance
    /// where audit trail authenticity must be verifiable.
    #[account(
        init,
        payer = payer,
        space = 8 + AuditTrailHead::INIT_SPACE,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump,
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    /// **Genesis Audit Entry**: First cryptographic commitment in the audit chain.
    /// Deterministic PDA using audit_index ensures proper ordering and prevents
    /// audit entry collision attacks. The genesis entry establishes the baseline
    /// hash for all subsequent entries in the cryptographic chain.
    #[account(
        init,
        payer = payer,
        space = 8 + AuditTrailEntry::INIT_SPACE,
        seeds = [b"audit_trail_entry", audit_trail_head.key().as_ref(), 1u8.to_le_bytes().as_ref()],
        bump,
    )]
    pub audit_trail_entry: AccountLoader<'info, AuditTrailEntry>,

    /// Governance authority authorization required for audit system establishment.
    /// Prevents unauthorized audit system creation that could be used to obfuscate
    /// malicious activity or create false security assurances.
    #[account(
        constraint = core_authority_signer.key() == pool_config.load()?.core_authority @ PoolError::Unauthorized
    )]
    pub core_authority_signer: Signer<'info>,

    /// Account funding the upgrade
    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// **Phase 2 Handler: Cryptographic Audit Infrastructure**
///
/// Establishes tamper-evident audit trail capabilities essential for enterprise compliance
/// and security monitoring. This phase creates the cryptographic foundation for immutable
/// event logging with hash-chained integrity guarantees (~35k CU).
///
/// ## Genesis Entry Strategy
/// The first audit entry serves as the cryptographic genesis of the audit chain,
/// establishing the baseline hash that all subsequent entries will reference.
/// This genesis entry is simplified compared to normal audit entries since the
/// full security coordinator isn't available yet.
///
/// ## Audit Chain Architecture
/// The audit system uses a hash-chained structure where each entry cryptographically
/// links to the previous entry, creating tamper-evident logging. The audit trail head
/// manages this chain and maintains sequence numbers for integrity verification.
///
/// ## Authority Attribution Pattern
/// Even in this phase, audit entries are properly attributed to legitimate authorities
/// to establish consistent audit patterns and prevent attribution confusion in
/// later forensic analysis.
pub fn initialize_audit_system(ctx: Context<InitializeAuditSystem>) -> Result<()> {
    // Cache pool core load for efficient validation and prevent multiple deserializations
    let pool_core_data = ctx.accounts.pool_core.load()?;

    // Validate audit system prerequisites with cached data and constants
    require!(
        pool_core_data.has_security_foundation(),
        PoolError::SecurityFoundationNotInitialized
    );
    require!(
        pool_core_data.status_flags & PoolCore::AUDIT_SYSTEM_INITIALIZED == 0,
        PoolError::AuditSystemAlreadyInitialized
    );
    require!(
        !pool_core_data.is_enterprise(),
        PoolError::AlreadyEnterprise
    );

    // Explicit drop to release borrow before mutable operations
    drop(pool_core_data);

    // Consistent temporal reference for audit chain baseline establishment
    let clock = Clock::get()?;
    let timestamp = clock.unix_timestamp;
    let slot = clock.slot;

    // Audit trail head initialization establishes the cryptographic chain foundation
    // Must precede any audit entries to provide proper hash chain management
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_init()?;
    audit_trail_head.initialize(ctx.accounts.pool_core.key(), timestamp)?;

    // Genesis audit entry preparation - first link in the cryptographic chain
    let audit_trail_entry = &mut ctx.accounts.audit_trail_entry.load_init()?;

    // Extract governance authority for proper audit attribution
    // Ensures audit entries have legitimate authority attribution from genesis
    let pool_config = ctx.accounts.pool_config.load()?;
    let current_core_authority = pool_config.core_authority;

    // Standardized 32-byte action identifier for consistent audit parsing
    // Fixed-width formatting ensures consistent audit log structure
    let action_bytes = b"AUDIT_SYSTEM_INIT               "; // 32 bytes
    let mut action = [0u8; 32];
    action.copy_from_slice(action_bytes);

    // Genesis audit entry initialization with simplified security context
    // Previous hash is zero for genesis entry as it's the chain foundation
    audit_trail_entry.initialize(InitArgs {
        pool_core: ctx.accounts.pool_core.key(),
        audit_index: 1,
        action,
        actor: current_core_authority,
        target: ctx.accounts.pool_core.key(),
        data_hash: hashv(&[
            ctx.accounts.pool_core.key().as_ref(),
            ctx.accounts.audit_trail_head.key().as_ref(),
            b"audit_system_initialized",
        ])
        .to_bytes(),
        previous_hash: [0; 32], // Genesis entry has no predecessor
        timestamp,
        block_height: slot,
    })?;

    // Link genesis entry to audit trail head to establish chain management
    // This completes the cryptographic audit foundation setup
    audit_trail_head.add_entry(audit_trail_entry)?;

    // Checkpoint flag signals audit system completion and enables phase 3
    // Bitwise OR preserves existing flags while adding audit system status
    let mut pool_core = ctx.accounts.pool_core.load_mut()?;
    pool_core.status_flags |= PoolCore::AUDIT_SYSTEM_INITIALIZED;

    msg!(
        "Audit system initialized for pool: {}",
        ctx.accounts.pool_core.key()
    );

    Ok(())
}
