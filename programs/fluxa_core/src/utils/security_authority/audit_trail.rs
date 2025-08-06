use crate::utils::security_authority::utils::AuditUtils;
use anchor_lang::prelude::*;

/// Audit Trail Head - the root of the protocol's on-chain audit log.
///
/// # Why
/// This account anchors the audit trail for a pool, providing a tamper-evident, append-only log of all critical actions.
/// It is designed to be immutable after initialization, ensuring the integrity and trustworthiness of the audit trail.
///
/// # Design Rationale
/// - Zero-copy layout for efficient, deterministic access and auditability.
/// - Tracks the latest entry's hash and index, enabling chain-of-trust verification for all entries.
/// - Reserved space for future upgrades without breaking account layout.
#[account(zero_copy(unsafe))]
#[derive(InitSpace)]
#[repr(C)]
pub struct AuditTrailHead {
    /// Pool reference
    ///
    /// # Why
    /// Associates this audit trail with a specific pool, ensuring all actions are contextually bound and auditable.
    pub pool_core: Pubkey,

    /// Current audit state
    ///
    /// # Why
    /// Tracks the latest entry and total count, enabling efficient verification and append-only guarantees.
    pub current_index: u64,
    pub latest_hash: [u8; 32],
    pub total_entries: u64,

    /// Metadata
    ///
    /// # Why
    /// Provides a full audit trail for the audit log itself, supporting compliance and forensic analysis.
    pub created_at: i64,
    pub last_updated: i64,

    /// Future expansion
    ///
    /// # Why
    /// Reserved space allows for future upgrades or additional fields without breaking account layout, supporting protocol evolution.
    pub reserved: [u8; 32],
}

/// Implementation of the AuditTrailHead account
impl AuditTrailHead {
    /// Initializes the AuditTrailHead account with the given pool core.
    ///
    /// # Why
    /// This method enforces protocol invariants for audit trail creation, ensuring the root is immutable and all state is initialized for append-only operation.
    pub fn initialize(&mut self, pool_core: Pubkey) -> Result<()> {
        // Initialize the pool core
        self.pool_core = pool_core;
        // Initialize the audit state
        self.current_index = 0;
        self.latest_hash = [0u8; 32];
        self.total_entries = 0;

        let clock = Clock::get()?; // Get the current clock time
        self.created_at = clock.unix_timestamp; // Set the creation timestamp
        self.last_updated = clock.unix_timestamp; // Set the last update timestamp

        Ok(())
    }

    /// Adds a new entry to the audit trail.
    ///
    /// # Why
    /// This method updates the audit trail head with the latest entry, maintaining the chain-of-trust and append-only guarantees.
    pub fn add_entry(&mut self, entry: &AuditTrailEntry) -> Result<u64> {
        self.current_index = entry.audit_index; // Increment the current index
        self.latest_hash = entry.current_hash; // Update the latest hash with the new entry hash
        self.total_entries += 1; // Increment the total entries count

        // Update the last updated timestamp
        self.last_updated = entry.timestamp;

        Ok(self.current_index)
    }
}

/// Audit Trail Entry - individual, tamper-evident log entry in the protocol's audit chain.
///
/// # Why
/// Each entry records a single action, with cryptographic linkage to the previous entry, forming an immutable, verifiable chain.
///
/// # Design Rationale
/// - Zero-copy layout for deterministic, efficient access and auditability.
/// - All fields are fixed-size for protocol safety and to avoid dynamic allocation.
/// - Hashes and indices enable chain-of-trust verification and efficient lookups.
/// - Reserved space for future upgrades.
#[account(zero_copy(unsafe))]
#[derive(InitSpace)]
#[repr(C)]
pub struct AuditTrailEntry {
    /// Pool reference
    ///
    /// # Why
    /// Associates this entry with a specific pool, ensuring all actions are contextually bound and auditable.
    pub pool_core: Pubkey,

    /// Entry identification
    ///
    /// # Why
    /// Uniquely identifies the action, actor, and target, supporting full forensic traceability and compliance.
    pub audit_index: u64,
    pub action: [u8; 32],
    pub actor: Pubkey,
    pub target: Pubkey,

    /// Entry data
    ///
    /// # Why
    /// Records the data, time, and block height for the action, supporting tamper-evident, time-stamped auditability.
    pub data_hash: [u8; 32],
    pub timestamp: i64,
    pub block_height: u64,

    /// Chain integrity
    ///
    /// # Why
    /// Cryptographically links this entry to the previous one, forming an immutable, verifiable audit chain.
    pub previous_hash: [u8; 32],
    pub current_hash: [u8; 32],

    /// Future expansion
    ///
    /// # Why
    /// Reserved space allows for future upgrades or additional fields without breaking account layout, supporting protocol evolution.
    pub reserved: [u8; 32],
}

/// Arguments for initializing an AuditTrailEntry.
pub struct InitArgs {
    pub pool_core: Pubkey,
    pub audit_index: u64,
    pub action: [u8; 32],
    pub actor: Pubkey,
    pub target: Pubkey,
    pub data_hash: [u8; 32],
    pub previous_hash: [u8; 32],
    pub timestamp: i64,
    pub block_height: u64,
}

/// Implementation of the AuditTrailEntry account
impl AuditTrailEntry {
    /// Initializes the AuditTrailEntry with the given parameters.
    ///
    /// # Why
    /// This method enforces protocol invariants for audit entry creation, ensuring all fields are set and the cryptographic chain is maintained.
    pub fn initialize(&mut self, args: InitArgs) -> Result<()> {
        // Initialize the pool core
        self.pool_core = args.pool_core;

        // Initialize the entry data
        self.audit_index = args.audit_index;
        self.action = args.action;
        self.actor = args.actor;
        self.target = args.target;

        // Initialize the data and chain integrity
        self.data_hash = args.data_hash;
        self.previous_hash = args.previous_hash;

        // Update the timestamp and block height
        self.timestamp = args.timestamp;
        self.block_height = args.block_height;

        // Calculate current hash
        self.current_hash = AuditUtils::create_audit_hash(
            &args.previous_hash,
            &args.action,
            &args.data_hash,
            self.timestamp,
            args.audit_index,
        );

        Ok(())
    }

    /// Verifies the integrity of the audit entry.
    ///
    /// # Why
    /// This method enables on-chain or off-chain verification of the audit chain, ensuring that no entry has been tampered with or omitted.
    pub fn verify_integrity(&self) -> Result<()> {
        AuditUtils::verify_audit_chain(
            &self.current_hash,
            &self.previous_hash,
            &self.action,
            &self.data_hash,
            self.timestamp,
            self.audit_index,
        )
    }
}

/// Initialize Audit Trail Head Context
///
/// # Why
/// This context enforces all protocol invariants for audit trail creation, ensuring that the head is initialized with the correct pool and payer, and that all account layouts are deterministic and auditable.
#[derive(Accounts)]
pub struct InitializeAuditTrailHead<'info> {
    /// Audit trail head account
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<AuditTrailHead>(),
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    /// Pool core account that this audit trail is associated with
    pub pool_core: UncheckedAccount<'info>,

    /// Payer account that is responsible for paying the transaction fees
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Authority account that is initializing the audit trail head
    pub authority: Signer<'info>,

    /// System program
    pub system_program: Program<'info, System>,
}

/// Create Audit Trail Entry Context
///
/// # Why
/// This context enforces all protocol invariants for audit entry creation, ensuring that each entry is linked to the correct head, pool, and payer, and that all account layouts are deterministic and auditable.
#[derive(Accounts)]
#[instruction(audit_index: u64)]
pub struct CreateAuditTrailEntry<'info> {
    /// Audit trail entry account
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<AuditTrailEntry>(),
        seeds = [b"audit_trail_entry", pool_core.key().as_ref(), &audit_index.to_le_bytes()],
        bump
    )]
    pub audit_trail_entry: AccountLoader<'info, AuditTrailEntry>,

    /// Audit trail head account that this entry is associated with
    #[account(
        mut,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    /// Pool core account that this entry is associated with
    pub pool_core: UncheckedAccount<'info>,

    /// Payer account that is responsible for paying the transaction fees
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Actor account that is creating the audit trail entry
    pub actor: Signer<'info>,

    /// System program
    pub system_program: Program<'info, System>,
}

/// Initialize Audit Trail Head
///
/// # Why
/// This function enforces protocol invariants for audit trail creation, ensuring that the head is initialized in a valid, immutable state and ready for append-only operation.
pub fn initialize_audit_trail_head(ctx: Context<InitializeAuditTrailHead>) -> Result<()> {
    // Load the Audit Trail Head account
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_init()?;

    // Initialize the Audit Trail Head with the pool core
    audit_trail_head.initialize(ctx.accounts.pool_core.key())?;

    Ok(())
}

/// Create Audit Trail Entry
///
/// # Why
/// This function enforces protocol invariants for audit entry creation, ensuring that each entry is cryptographically linked to the previous one, forming an immutable, tamper-evident chain.
pub fn create_audit_trail_entry(
    ctx: Context<CreateAuditTrailEntry>,
    audit_index: u64,
    action: [u8; 32],
    target: Pubkey,
    data_hash: [u8; 32],
) -> Result<()> {
    // Load the Audit Trail Head and Audit Trail Entry accounts
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_mut()?;
    let audit_trail_entry = &mut ctx.accounts.audit_trail_entry.load_init()?;

    // Get previous hash from head
    let previous_hash = audit_trail_head.latest_hash;
    // Get current clock time
    let clock = Clock::get()?;
    // Initialize entry
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

    audit_trail_head.add_entry(audit_trail_entry)?;

    Ok(())
}
