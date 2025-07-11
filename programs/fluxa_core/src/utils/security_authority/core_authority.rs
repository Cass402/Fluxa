use crate::error::PdaSecurityAuthorityError;
use crate::utils::constants::{AUTHORITY_CHANGE_DELAY, EMERGENCY_PAUSE_TIMEOUT};
use crate::utils::security_authority::emergency_contacts::EmergencyContacts;
use crate::utils::security_authority::multisig_config::MultisigConfig;
use crate::utils::security_authority::utils::AuditUtils;
use anchor_lang::prelude::*;

/// The Core Authority account is a critical component of the Fluxa Core security model.
/// It manages the authority transitions for the pool core and maintains the operational status of the core.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct CoreAuthority {
    /// The discriminator for the CoreAuthority account for type safety during deserialization.
    pub discriminator: [u8; 8],

    /// The public key of the pool core associated with this authority.
    pub pool_core: Pubkey,

    /// The public key of the current authority for the pool core.
    pub current_authority: Pubkey,

    /// The public key of the pending authority for the pool core, if any.
    /// This is used for authority transitions and must be confirmed by the current authority.
    pub pending_authority: Pubkey,
    /// Indicates whether there is a pending authority change.
    pub has_pending_authority: bool,
    /// The timestamp when the authority change was requested.
    pub authority_change_requested_at: i64,
    /// The delay period for the authority change
    pub authority_change_delay: i64,
    /// The number of confirmations received for the authority change.
    pub authority_change_confirmations: u8,
    /// The number of confirmations required for the authority change.
    pub required_confirmations: u8,

    /// The hash of the audit trail
    pub audit_trail_hash: [u8; 32],
    /// The index of the audit entry in the audit trail.
    /// This is used to track the order of audit entries and ensure consistency.
    pub audit_index: u64,

    /// The operational status of the Core Authority.
    pub operational_status: OperationalStatus,
    /// Indicates whether the emergency pause is currently active.
    pub emergency_pause_active: bool,
    /// The timestamp when the emergency pause was initiated.
    pub emergency_pause_initiated_at: i64,
    /// The timeout duration for the emergency pause.
    pub emergency_pause_timeout: i64,

    /// Metadata for the Core Authority
    /// The timestamp when the Core Authority was created.
    pub created_at: i64,
    /// The timestamp when the Core Authority was last updated.
    pub last_updated: i64,
    /// The security version of the Core Authority.
    pub security_version: u16,

    /// Reserved space for future use or alignment.
    pub reserved: [u8; 64],
}

impl CoreAuthority {
    pub fn initialize(
        &mut self,
        pool_core: Pubkey,
        initial_authority: Pubkey,
        required_confirmations: u8,
    ) -> Result<()> {
        // Initialize the discriminator
        self.discriminator = Self::discriminator();

        // Initialize the pool core
        self.pool_core = pool_core;

        // Initialize the current authority
        self.current_authority = initial_authority;

        // Initialize the pending authority and related fields
        self.pending_authority = Pubkey::default(); // No pending authority initially
        self.has_pending_authority = false; // No pending authority change initially
        self.authority_change_requested_at = 0; // No authority change requested initially
        self.authority_change_delay = AUTHORITY_CHANGE_DELAY;
        self.authority_change_confirmations = 0; // No confirmations yet
        self.required_confirmations = required_confirmations;

        // Initialize the operational status and emergency pause fields
        self.operational_status = OperationalStatus::Normal;
        self.emergency_pause_active = false;
        self.emergency_pause_initiated_at = 0;
        self.emergency_pause_timeout = 0;

        // Initialize the audit index
        self.audit_index = 0;

        // Initialize the Metadata fields
        let clock = Clock::get()?;
        self.created_at = clock.unix_timestamp;
        self.last_updated = clock.unix_timestamp;
        self.security_version = 1;

        // Initialize the audit trail
        self.audit_trail_hash = AuditUtils::create_audit_hash(
            &[0u8; 32],
            b"authority_initialized",
            initial_authority.as_ref(),
            clock.unix_timestamp,
            0,
        );

        Ok(())
    }

    /// Proposes a change of authority for the Core Authority.
    /// This function allows the current authority to propose a new authority.
    /// It checks if the proposer is the current authority and if there is no pending authority change.
    /// If successful, it updates the pending authority and records the time of the proposal.
    /// # Arguments
    /// * `new_authority` - The public key of the new authority to be proposed.
    /// * `proposer` - The public key of the current authority proposing the change.
    /// # Returns
    /// A `Result` indicating success or failure of the operation.
    /// # Errors
    /// * `Unauthorized` - If the proposer is not the current authority.
    /// * `AuthorityChangeInProgress` - If there is already a pending authority change.
    pub fn propose_authority_change(
        &mut self,
        new_authority: Pubkey,
        proposer: Pubkey,
    ) -> Result<()> {
        // Check if the proposer is the current authority
        if self.current_authority != proposer {
            return Err(PdaSecurityAuthorityError::Unauthorized.into());
        }

        if self.has_pending_authority {
            return Err(PdaSecurityAuthorityError::AuthorityChangeInProgress.into());
        }

        let clock = Clock::get()?;
        self.has_pending_authority = true;
        self.pending_authority = new_authority;
        self.authority_change_requested_at = clock.unix_timestamp;
        self.authority_change_confirmations = 1; // Start with one confirmation from the proposer

        // Update the audit trail
        self.update_audit_trail(b"authority_change_proposed", &new_authority.as_ref())?;

        Ok(())
    }

    pub fn confirm_authority_change(&mut self) -> Result<()> {
        // Ensure that the authority change has been requested
        if !self.has_pending_authority {
            return Err(PdaSecurityAuthorityError::NoAuthorityChangeRequested.into());
        }

        // Increment the confirmation count for the authority change
        self.authority_change_confirmations += 1;

        // Check if the required number of confirmations has been reached
        // and if the authority change delay has passed
        if self.authority_change_confirmations >= self.required_confirmations {
            // Check if the authority change delay has passed
            let clock = Clock::get()?;
            if clock.unix_timestamp
                >= self.authority_change_requested_at + self.authority_change_delay
            {
                // Execute the authority change
                self.execute_authority_change()?;
            }
        }

        Ok(())
    }

    /// Initiates an emergency pause for the Core Authority.
    /// This function allows the Core Authority to enter an emergency pause state,
    /// which can be triggered by the current authority in response to critical issues.
    /// It sets the operational status to EmergencyPause, records the time of initiation,
    /// and sets the timeout based on the emergency level.
    /// # Arguments
    /// * `reason_hash` - A hash representing the reason for the emergency pause.
    /// * `emergency_level` - The level of emergency being declared (Low, Medium, High, Critical).
    /// # Returns
    /// A `Result` indicating success or failure of the operation.
    pub fn emergency_pause(
        &mut self,
        reason_hash: [u8; 32],
        emergency_level: EmergencyLevel,
    ) -> Result<()> {
        // Get the current clock time
        let clock = Clock::get()?;
        self.emergency_pause_active = true; // Set the emergency pause active flag
        self.emergency_pause_initiated_at = clock.unix_timestamp; // Record the time

        self.emergency_pause_timeout = clock.unix_timestamp
            + match emergency_level {
                EmergencyLevel::Low => 24 * 3600,                // 1 day
                EmergencyLevel::Medium => 3 * 24 * 3600,         // 3 days
                EmergencyLevel::High => EMERGENCY_PAUSE_TIMEOUT, // 7 days
                EmergencyLevel::Critical => 14 * 24 * 3600,      // 14 days
            };

        self.operational_status = OperationalStatus::EmergencyPause; // Set the operational status to EmergencyPause

        // Update the audit trail for the emergency pause
        self.update_audit_trail(b"emergency_pause", &reason_hash)?;

        Ok(())
    }

    /// Executes the authority change for the Core Authority.
    /// This function is called when the required number of confirmations has been reached
    /// and the authority change delay has passed.
    /// It updates the current authority to the pending authority, clears the pending authority,
    /// and resets the authority change requested timestamp and confirmation count.
    /// # Returns
    /// A `Result` indicating success or failure of the operation.
    /// # Errors
    /// * `NoAuthorityChangeRequested` - If there is no pending authority change to execute
    fn execute_authority_change(&mut self) -> Result<()> {
        let pending = self.pending_authority;
        // Execute the authority change
        self.current_authority = pending; // Update the current authority to the pending authority
        self.pending_authority = Pubkey::default(); // Clear the pending authority
        self.has_pending_authority = false; // Reset the pending authority change flag
        self.authority_change_requested_at = 0; // Reset the authority change requested timestamp
        self.authority_change_confirmations = 0; // Reset the confirmation count

        // Update the audit trail for the authority change
        self.update_audit_trail(b"authority_changed", &pending.as_ref())?;

        Ok(())
    }

    /// Updates the audit trail for the Core Authority.
    /// This function creates a new audit hash based on the current state of the Core Authority,
    /// the action performed, and the associated data.
    /// # Arguments
    /// * `action` - A byte slice representing the action taken (e.g., "create", "update", "delete").
    /// * `data` - A byte slice containing the data associated with the action.
    /// # Returns
    /// A `Result` indicating success or failure of the operation.
    fn update_audit_trail(&mut self, action: &[u8], data: &[u8]) -> Result<()> {
        // Get the current clock time
        let clock = Clock::get()?;
        // Ensure the audit index is incremented
        self.audit_index = self.audit_index.wrapping_add(1);

        // Create a new audit hash based on the current state
        self.audit_trail_hash = AuditUtils::create_audit_hash(
            &self.audit_trail_hash,
            action,
            data,
            clock.unix_timestamp,
            self.audit_index,
        );

        // Update the last updated timestamp for the Core Authority
        self.last_updated = clock.unix_timestamp;

        Ok(())
    }

    /// Returns the discriminator for the CoreAuthority account. (This is used for type safety during deserialization.)
    /// The discriminator is a unique identifier for the CoreAuthority account type.
    /// This function is used to ensure that the account being deserialized is indeed a CoreAuthority account.
    fn discriminator() -> [u8; 8] {
        [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80]
    }
}

/// The operational status of the Core Authority.
/// This enum represents the different operational states of the Core Authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum OperationalStatus {
    Normal = 0,
    Maintenance = 1,
    EmergencyPause = 2,
    Deprecated = 3,
    Upgrading = 4,
}

/// The emergency level for the Core Authority.
/// This enum represents the different levels of emergency that can be declared by the Core Authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum EmergencyLevel {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

/// Initialize the Core Authority.
/// This instruction initializes the Core Authority account with the provided pool core and initial authority.
/// It sets the initial authority, required confirmations, and initializes the operational status.
#[derive(Accounts)]
pub struct InitializeCoreAuthority<'info> {
    /// Initializes the Core Authority account
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<CoreAuthority>(),
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump,
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// The pool core account
    pub pool_core: UncheckedAccount<'info>,

    /// The payer account that will pay for the initialization
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The initial authority for the Core Authority
    pub initial_authority: Signer<'info>,

    /// The system program account
    pub system_program: Program<'info, System>,
}

/// Propose a change of authority for the Core Authority.
/// This instruction allows the current authority to propose a new authority.
#[derive(Accounts)]
pub struct ProposeAuthorityChange<'info> {
    /// The Core Authority account
    #[account(
        mut,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump,
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// The multisig configuration account
    #[account(
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump,
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// The pool core account
    pub pool_core: UncheckedAccount<'info>,

    /// The proposer account, which must be the current authority
    pub proposer: Signer<'info>,

    /// The new authority account, which will replace the current authority
    #[account(
        seeds = [b"new_authority", pool_core.key().as_ref()],
        bump,
    )]
    pub new_authority: UncheckedAccount<'info>,
}

/// Confirm a proposed change of authority for the Core Authority.
/// This instruction allows a multisig member to confirm a proposed authority change.
/// It checks if the confirmer is a member of the multisig and confirms the proposal.
#[derive(Accounts)]
pub struct ConfirmAuthorityChange<'info> {
    /// The Core Authority account
    #[account(
        mut,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// The multisig configuration account
    #[account(
        mut,
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// The pool core account
    pub pool_core: UncheckedAccount<'info>,

    /// The confirmer account, which must be a member of the multisig
    pub confirmer: Signer<'info>,
}

/// Emergency Pause Context
/// This context is used to execute an emergency pause for the Core Authority.
/// It requires the Core Authority account, Emergency Contacts account, pool core,
/// and the emergency responder account.
/// The emergency responder must have emergency authority to execute the pause.
#[derive(Accounts)]
pub struct EmergencyPause<'info> {
    /// The Core Authority account
    #[account(
        mut,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// The emergency contacts account
    #[account(
        seeds = [b"emergency_contacts", pool_core.key().as_ref()],
        bump
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    /// The pool core account associated with the Core Authority and Emergency Contacts
    pub pool_core: UncheckedAccount<'info>,

    /// The emergency responder account, which must have emergency authority
    pub emergency_responder: Signer<'info>,
}

/// Initialize the Core Authority with the provided parameters.
pub fn initialize_core_authority(
    ctx: Context<InitializeCoreAuthority>,
    required_confirmations: u8,
) -> Result<()> {
    // Load the Core Authority account
    let core_authority = &mut ctx.accounts.core_authority.load_init()?;

    // Initialize the Core Authority with the provided parameters
    core_authority.initialize(
        ctx.accounts.pool_core.key(),
        ctx.accounts.initial_authority.key(),
        required_confirmations,
    )?;

    Ok(())
}

/// Propose a change of authority for the Core Authority.
/// This function allows the current authority to propose a new authority.
/// It checks if the proposer is the current authority and if there is no pending authority change.
/// If successful, it updates the pending authority and records the time of the proposal.
/// # Arguments
/// * `ctx` - The context containing the accounts required for proposing the authority change.
/// * `new_authority` - The public key of the new authority to be proposed.
/// * `proposer` - The public key of the current authority proposing the change.
/// # Returns
/// A `Result` indicating success or failure of the operation.
pub fn propose_authority_change(ctx: Context<ProposeAuthorityChange>) -> Result<()> {
    // Load the Core Authority and Multisig Config accounts
    let core_authority = &mut ctx.accounts.core_authority.load_mut()?;
    let multisig_config = &ctx.accounts.multisig_config.load()?;

    // Validate proposer is multisig member
    if !multisig_config.is_member(&ctx.accounts.proposer.key()) {
        return Err(PdaSecurityAuthorityError::NotAMultisigMember.into());
    }

    // Propose authority change
    core_authority.propose_authority_change(
        ctx.accounts.new_authority.key(),
        ctx.accounts.proposer.key(),
    )?;

    Ok(())
}

/// Confirm a proposed change of authority for the Core Authority.
/// This function allows a multisig member to confirm a proposed authority change.
/// It checks if the confirmer is a member of the multisig and confirms the proposal.
/// If the required number of confirmations is reached, it executes the authority change.
/// # Arguments
/// * `ctx` - The context containing the accounts required for confirming the authority change.
/// # Returns
/// A `Result` indicating success or failure of the operation.
pub fn confirm_authority_change(ctx: Context<ConfirmAuthorityChange>) -> Result<()> {
    // Load the Core Authority and Multisig Config accounts
    let core_authority = &mut ctx.accounts.core_authority.load_mut()?;
    let multisig_config = &mut ctx.accounts.multisig_config.load_mut()?;

    // Validate confirmer is multisig member
    if !multisig_config.is_member(&ctx.accounts.confirmer.key()) {
        return Err(PdaSecurityAuthorityError::NotAMultisigMember.into());
    }

    // Create proposal hash for multisig tracking
    let pending_authority = core_authority.pending_authority;
    let proposal_hash = anchor_lang::solana_program::hash::hashv(&[
        b"authority_change",
        pending_authority.as_ref(),
        &core_authority.authority_change_requested_at.to_le_bytes(),
    ])
    .to_bytes();

    // Confirm in multisig
    let threshold_reached =
        multisig_config.confirm_proposal(&ctx.accounts.confirmer.key(), proposal_hash)?;

    // If threshold reached, confirm in core authority
    if threshold_reached {
        core_authority.confirm_authority_change()?;
    }

    Ok(())
}

/// Emergency Pause
/// This function allows the Core Authority to enter an emergency pause state,
/// which can be triggered by the current authority in response to critical issues.
/// It sets the operational status to EmergencyPause, records the time of initiation,
/// and sets the timeout based on the emergency level.
/// # Arguments
/// * `ctx` - The context containing the accounts required for the emergency pause.
/// * `reason_hash` - A hash representing the reason for the emergency pause.
/// * `emergency_level` - The level of emergency being declared (Low, Medium, High, Critical).
/// # Returns
/// A `Result` indicating success or failure of the operation.
pub fn emergency_pause(
    ctx: Context<EmergencyPause>,
    reason_hash: [u8; 32],
    emergency_level: EmergencyLevel,
) -> Result<()> {
    // Load the Core Authority and Emergency Contacts accounts
    let core_authority = &mut ctx.accounts.core_authority.load_mut()?;
    let emergency_contacts = &ctx.accounts.emergency_contacts.load()?;

    // Validate emergency authority
    if !emergency_contacts.has_emergency_authority(&ctx.accounts.emergency_responder.key()) {
        return Err(PdaSecurityAuthorityError::InsufficientPermissions.into());
    }

    // Execute emergency pause
    core_authority.emergency_pause(reason_hash, emergency_level)?;

    Ok(())
}
