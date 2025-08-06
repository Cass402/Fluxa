use crate::error::PdaSecurityAuthorityError;
use crate::utils::constants::{AUTHORITY_CHANGE_DELAY, EMERGENCY_PAUSE_TIMEOUT};
use crate::utils::security_authority::emergency_contacts::EmergencyContacts;
use crate::utils::security_authority::multisig_config::MultisigConfig;
use crate::utils::security_authority::utils::AuditUtils;
use anchor_lang::prelude::*;

/// Core Authority account: the root of protocol security and governance for a pool.
///
/// # Why
/// This account enforces all authority transitions, operational status, and emergency controls for a pool core.
/// It is the single source of truth for who can control the pool, and how/when that control can change.
///
/// # Design Rationale
/// - Zero-copy layout for deterministic, efficient access and auditability.
/// - All fields are fixed-size and protocol-bounded for safety and upgradeability.
/// - Authority transitions require explicit delay and multi-sig confirmation, deterring governance attacks and rug pulls.
/// - Emergency pause and operational status are tracked on-chain for full transparency and liveness guarantees.
/// - Audit trail hash and index provide a tamper-evident, append-only log of all critical actions.
#[account(zero_copy(unsafe))]
#[derive(InitSpace)]
#[repr(C)]
pub struct CoreAuthority {
    /// Pool core reference
    ///
    /// # Why
    /// Binds this authority to a specific pool, ensuring all actions are contextually bound and auditable.
    pub pool_core: Pubkey,

    /// Current authority
    ///
    /// # Why
    /// The only entity allowed to propose or confirm authority changes, or perform privileged actions.
    pub current_authority: Pubkey,

    /// Pending authority (if any)
    ///
    /// # Why
    /// Used for secure, delayed authority transitions. Ensures that new authorities are not granted control instantly, deterring attacks.
    pub pending_authority: Pubkey,
    /// Authority change request timestamp
    ///
    /// # Why
    /// Used to enforce protocol-mandated delay before authority can be changed, giving users time to react.
    pub authority_change_requested_at: i64,
    /// Authority change delay (seconds)
    ///
    /// # Why
    /// Protocol-mandated minimum delay for authority changes, deterring instant takeovers.
    pub authority_change_delay: i64,
    /// Pending authority change flag
    ///
    /// # Why
    /// Prevents overlapping or conflicting authority transitions, ensuring only one change can be in progress at a time.
    pub has_pending_authority: bool,
    /// Number of confirmations received for authority change
    ///
    /// # Why
    /// Multi-sig: ensures that no single entity can unilaterally change authority.
    pub authority_change_confirmations: u8,
    /// Number of confirmations required for authority change
    ///
    /// # Why
    /// Protocol safety: ensures that a threshold of trusted parties must approve any authority change.
    pub required_confirmations: u8,

    /// Audit trail hash
    ///
    /// # Why
    /// Tamper-evident, append-only log of all critical actions, supporting compliance and forensic analysis.
    pub audit_trail_hash: [u8; 32],
    /// Audit entry index
    ///
    /// # Why
    /// Tracks the order of audit entries, supporting chain-of-trust verification and efficient lookups.
    pub audit_index: u64,

    /// Operational status
    ///
    /// # Why
    /// Tracks the current state of the pool (Normal, Maintenance, EmergencyPause, etc.), supporting liveness and safety guarantees.
    pub operational_status: OperationalStatus,
    /// Emergency pause active flag
    ///
    /// # Why
    /// Protocol safety: allows for rapid response to critical issues, pausing all privileged actions.
    pub emergency_pause_active: bool,
    /// Emergency pause initiation timestamp
    ///
    /// # Why
    /// Used to enforce protocol-mandated pause duration and for auditability.
    pub emergency_pause_initiated_at: i64,
    /// Emergency pause timeout (seconds)
    ///
    /// # Why
    /// Protocol-mandated maximum duration for emergency pause, ensuring liveness.
    pub emergency_pause_timeout: i64,

    /// Metadata: creation timestamp
    ///
    /// # Why
    /// Full audit trail for the authority account itself, supporting compliance and forensic analysis.
    pub created_at: i64,
    /// Metadata: last updated timestamp
    ///
    /// # Why
    /// Tracks the last time any privileged action or state change occurred.
    pub last_updated: i64,
    /// Security version
    ///
    /// # Why
    /// Enables protocol upgrades and migration logic, supporting future-proofing.
    pub security_version: u16,

    /// Reserved space for future use or alignment
    ///
    /// # Why
    /// Allows for future upgrades or additional fields without breaking account layout.
    pub reserved: [u8; 64],
}

impl CoreAuthority {
    /// Initialize the Core Authority account with all protocol invariants enforced.
    ///
    /// # Why
    /// This method ensures that all fields are set to safe, protocol-compliant values, and that the audit trail is initialized for tamper-evident logging.
    ///
    /// # Design Rationale
    /// - All state is initialized up front to prevent uninitialized or invalid state.
    /// - Audit trail hash is seeded with the initial authority and timestamp for full traceability.
    pub fn initialize(
        &mut self,
        pool_core: Pubkey,
        initial_authority: Pubkey,
        required_confirmations: u8,
    ) -> Result<()> {
        self.pool_core = pool_core;
        self.current_authority = initial_authority;
        self.pending_authority = Pubkey::default();
        self.has_pending_authority = false;
        self.authority_change_requested_at = 0;
        self.authority_change_delay = AUTHORITY_CHANGE_DELAY;
        self.authority_change_confirmations = 0;
        self.required_confirmations = required_confirmations;
        self.operational_status = OperationalStatus::Normal;
        self.emergency_pause_active = false;
        self.emergency_pause_initiated_at = 0;
        self.emergency_pause_timeout = 0;
        self.audit_index = 0;
        let clock = Clock::get()?;
        self.created_at = clock.unix_timestamp;
        self.last_updated = clock.unix_timestamp;
        self.security_version = 1;
        self.audit_trail_hash = AuditUtils::create_audit_hash(
            &[0u8; 32],
            b"authority_initialized",
            initial_authority.as_ref(),
            clock.unix_timestamp,
            0,
        );
        Ok(())
    }

    /// Propose a new authority for the pool, enforcing protocol safety and multi-sig requirements.
    ///
    /// # Why
    /// This method ensures that only the current authority can propose a change, and that only one change can be in progress at a time.
    /// It records the proposal in the audit trail for full traceability.
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
        self.update_audit_trail(b"authority_change_proposed", new_authority.as_ref())?;

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

    /// Initiate an emergency pause, enforcing protocol safety and liveness guarantees.
    ///
    /// # Why
    /// This method allows the protocol to rapidly pause privileged actions in response to critical issues, with a timeout based on severity.
    /// All actions are recorded in the audit trail for compliance and forensic analysis.
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

    /// Execute the authority change, enforcing all protocol invariants and auditability.
    ///
    /// # Why
    /// This method is only called after all confirmations and delays have been satisfied, ensuring protocol safety and traceability.
    fn execute_authority_change(&mut self) -> Result<()> {
        let pending = self.pending_authority;
        // Execute the authority change
        self.current_authority = pending; // Update the current authority to the pending authority
        self.pending_authority = Pubkey::default(); // Clear the pending authority
        self.has_pending_authority = false; // Reset the pending authority change flag
        self.authority_change_requested_at = 0; // Reset the authority change requested timestamp
        self.authority_change_confirmations = 0; // Reset the confirmation count

        // Update the audit trail for the authority change
        self.update_audit_trail(b"authority_changed", pending.as_ref())?;

        Ok(())
    }

    /// Update the audit trail for the Core Authority, creating a tamper-evident log of all critical actions.
    ///
    /// # Why
    /// This method ensures that every privileged action is recorded in a cryptographically linked audit trail, supporting compliance and forensic analysis.
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
}

/// Operational status of the Core Authority.
///
/// # Why
/// Encodes the current state of the pool, supporting liveness, maintenance, and emergency controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, InitSpace)]
#[repr(u8)]
pub enum OperationalStatus {
    Normal = 0,
    Maintenance = 1,
    EmergencyPause = 2,
    Deprecated = 3,
    Upgrading = 4,
}

/// Emergency level for the Core Authority.
///
/// # Why
/// Encodes the severity of an emergency, determining the allowed pause duration and response requirements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum EmergencyLevel {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

/// Anchor context for initializing the Core Authority account.
///
/// # Why
/// This context enforces all protocol invariants for secure initialization: deterministic PDA seeds, payer, and authority assignment.
/// Ensures the account is created with the correct pool, authority, and system program, and that all state is zero-copy and auditable.
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

/// Anchor context for proposing a new authority for the Core Authority.
///
/// # Why
/// This context enforces protocol safety for authority transitions: only the current authority (and a multisig member) can propose a change, and all accounts are validated with deterministic seeds.
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

/// Anchor context for confirming a proposed authority change.
///
/// # Why
/// This context enforces protocol safety for multi-sig confirmation: only a multisig member can confirm, and all accounts are validated with deterministic seeds.
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

/// Anchor context for executing an emergency pause on the Core Authority.
///
/// # Why
/// This context enforces protocol safety for emergency controls: only an authorized emergency responder can trigger a pause, and all accounts are validated with deterministic seeds.
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

/// Handler: Initialize the Core Authority with the provided parameters.
///
/// # Why
/// This handler enforces protocol invariants for secure initialization, ensuring all state is set up for safe, auditable operation.
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

/// Handler: Propose a new authority for the Core Authority.
///
/// # Why
/// This handler enforces protocol safety for authority transitions: only a multisig member can propose, and only if no change is in progress. All actions are recorded for auditability.
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

/// Handler: Confirm a proposed authority change for the Core Authority.
///
/// # Why
/// This handler enforces protocol safety for multi-sig confirmation: only a multisig member can confirm, and the change only executes after threshold and delay. All actions are recorded for auditability.
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

/// Handler: Emergency Pause for the Core Authority.
///
/// # Why
/// This handler enforces protocol safety for emergency controls: only an authorized responder can pause, and all actions are recorded for compliance and forensic analysis.
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
