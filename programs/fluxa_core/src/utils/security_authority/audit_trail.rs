use crate::utils::security_authority::utils::AuditUtils;
use anchor_lang::prelude::*;

/// Audit Trail Head - the main entry point for the audit trail system
/// This account maintains the state of the audit trail, including the latest entry and metadata.
/// It is designed to be immutable once initialized, ensuring the integrity of the audit trail.
/// The head of the audit trail is a zero-copy account, allowing efficient access to its data
/// without the need for deserialization.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct AuditTrailHead {
    /// Account discriminator
    pub discriminator: [u8; 8],

    /// Pool reference
    pub pool_core: Pubkey,

    /// Current audit state
    /// - `current_index`: The index of the latest audit entry.
    /// - `latest_hash`: The hash of the latest audit entry.
    /// - `total_entries`: The total number of audit entries recorded.
    pub current_index: u64,
    pub latest_hash: [u8; 32],
    pub total_entries: u64,

    /// Metadata
    /// - `created_at`: The timestamp when the audit trail was created.
    /// - `last_updated`: The timestamp when the audit trail was last updated.
    pub created_at: i64,
    pub last_updated: i64,

    /// Future expansion
    pub reserved: [u8; 32],
}

/// Implementation of the AuditTrailHead account
impl AuditTrailHead {
    /// Initializes the AuditTrailHead account with the given pool core.
    /// # Arguments
    /// * `pool_core` - The public key of the pool core this audit trail is associated with.
    /// # Returns
    /// A `Result` indicating success or failure of the initialization.
    pub fn initialize(&mut self, pool_core: Pubkey) -> Result<()> {
        // Initialize the account discriminator
        self.discriminator = Self::discriminator();
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
    /// # Arguments
    /// * `entry_hash` - The hash of the new audit entry to be added.
    /// # Returns
    /// A `Result` containing the index of the newly added entry
    pub fn add_entry(&mut self, entry_hash: [u8; 32]) -> Result<u64> {
        self.current_index = self.current_index.wrapping_add(1); // Increment the current index
        self.latest_hash = entry_hash; // Update the latest hash with the new entry hash
        self.total_entries += 1; // Increment the total entries count

        // Update the last updated timestamp
        let clock = Clock::get()?; // Get the current clock time
        self.last_updated = clock.unix_timestamp;

        Ok(self.current_index)
    }

    /// Returns the discriminator for the AuditTrailHead account.
    /// This is used to identify the account type in the Anchor framework.
    fn discriminator() -> [u8; 8] {
        [0x13, 0x23, 0x33, 0x43, 0x53, 0x63, 0x73, 0x83]
    }
}

/// Audit Trail Entry - individual audit log entry
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct AuditTrailEntry {
    /// Account discriminator
    pub discriminator: [u8; 8],

    /// Pool reference
    pub pool_core: Pubkey,

    /// Entry identification
    /// - `audit_index`: The index of this entry in the audit trail.
    /// - `action`: A unique identifier for the action being audited.
    /// - `actor`: The public key of the actor performing the action.
    /// - `target`: The public key of the target account, if applicable.
    pub audit_index: u64,
    pub action: [u8; 32],
    pub actor: Pubkey,
    pub target: Pubkey,

    /// Entry data
    /// - `data_hash`: The hash of the data associated with this entry.
    /// - `timestamp`: The timestamp when the action was performed.
    /// - `block_height`: The block height at which the action was recorded.
    pub data_hash: [u8; 32],
    pub timestamp: i64,
    pub block_height: u64,

    /// Chain integrity
    /// - `previous_hash`: The hash of the previous audit entry, ensuring the integrity of the audit trail.
    /// - `current_hash`: The hash of the current audit entry, calculated based on the previous entry and the current data.
    pub previous_hash: [u8; 32],
    pub current_hash: [u8; 32],

    /// Future expansion
    pub reserved: [u8; 32],
}

/// Implementation of the AuditTrailEntry account
impl AuditTrailEntry {
    /// Initializes the AuditTrailEntry with the given parameters.
    /// # Arguments
    /// * `pool_core` - The public key of the pool core this entry is associated with.
    /// * `audit_index` - The index of this entry in the audit trail.
    /// * `action` - A unique identifier for the action being audited.
    /// * `actor` - The public key of the actor performing the action.
    /// * `target` - The public key of the target account, if applicable.
    /// * `data_hash` - The hash of the data associated with this entry.
    /// * `previous_hash` - The hash of the previous audit entry, ensuring the integrity of the audit trail.
    /// # Returns
    /// A `Result` indicating success or failure of the initialization.
    pub fn initialize(
        &mut self,
        pool_core: Pubkey,
        audit_index: u64,
        action: [u8; 32],
        actor: Pubkey,
        target: Pubkey,
        data_hash: [u8; 32],
        previous_hash: [u8; 32],
    ) -> Result<()> {
        // Initialize the account discriminator
        self.discriminator = Self::discriminator();

        // Initialize the pool core
        self.pool_core = pool_core;

        // Initialize the entry data
        self.audit_index = audit_index;
        self.action = action;
        self.actor = actor;
        self.target = target;

        // Initialize the data and chain integrity
        self.data_hash = data_hash;
        self.previous_hash = previous_hash;

        // Update the timestamp and block height
        let clock = Clock::get()?;
        self.timestamp = clock.unix_timestamp;
        self.block_height = clock.slot;

        // Calculate current hash
        self.current_hash = AuditUtils::create_audit_hash(
            &previous_hash,
            &action,
            &data_hash,
            self.timestamp,
            audit_index,
        );

        Ok(())
    }

    /// Verifies the integrity of the audit entry.
    /// This checks that the current hash matches the expected hash based on the previous entry and the current data.
    /// # Returns
    /// A `Result` indicating success or failure of the verification.
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

    /// Returns the discriminator for the AuditTrailEntry account.
    fn discriminator() -> [u8; 8] {
        [0x14, 0x24, 0x34, 0x44, 0x54, 0x64, 0x74, 0x84]
    }
}

/// Initialize Audit Trail Head Context
/// This context is used to initialize the Audit Trail Head account.
/// It sets up the account with the necessary parameters and ensures that it is ready to record audit
/// entries for the associated pool core.
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
/// This context is used to create a new audit trail entry.
/// It initializes a new entry with the provided parameters and links it to the audit trail head.
/// The entry is created with a unique index and hash, ensuring the integrity of the audit trail
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
/// This function initializes the Audit Trail Head account.
/// It sets up the account with the necessary parameters and ensures that it is ready to record audit
/// entries for the associated pool core.
/// # Arguments
/// * `ctx` - The context containing the accounts and parameters for initializing the audit trail head
/// # Returns
/// A `Result` indicating success or failure of the initialization.
pub fn initialize_audit_trail_head(ctx: Context<InitializeAuditTrailHead>) -> Result<()> {
    // Load the Audit Trail Head account
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_init()?;

    // Initialize the Audit Trail Head with the pool core
    audit_trail_head.initialize(ctx.accounts.pool_core.key())?;

    Ok(())
}

/// Create Audit Trail Entry
/// This function creates a new audit trail entry.
/// It initializes a new entry with the provided parameters and links it to the audit trail head.
/// The entry is created with a unique index and hash, ensuring the integrity of the audit trail.
/// # Arguments
/// * `ctx` - The context containing the accounts and parameters for creating the audit entry.
/// * `audit_index` - The index of the audit entry being created.
/// * `action` - A unique identifier for the action being audited.
/// * `target` - The public key of the target account, if applicable.
/// * `data_hash` - The hash of the data associated with this entry.
/// # Returns
/// A `Result` indicating success or failure of the entry creation.
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

    // Initialize entry
    audit_trail_entry.initialize(
        ctx.accounts.pool_core.key(),
        audit_index,
        action,
        ctx.accounts.actor.key(),
        target,
        data_hash,
        previous_hash,
    )?;

    // Update head
    audit_trail_head.add_entry(audit_trail_entry.current_hash)?;

    Ok(())
}
