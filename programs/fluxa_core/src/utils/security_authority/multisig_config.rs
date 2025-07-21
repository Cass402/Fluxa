use crate::error::PdaSecurityAuthorityError;
use anchor_lang::prelude::*;

/// MultisigConfig: protocol-level multi-signature authority configuration for a pool.
///
/// # Why
/// This account enforces multi-party control over privileged actions, deterring single-point-of-failure and governance attacks.
///
/// # Design Rationale
/// - Zero-copy layout for deterministic, efficient access and auditability.
/// - Fixed-size array (no Vec) for members, ensuring predictable compute and storage costs.
/// - Bitfield confirmation tracking for atomic, efficient multi-sig logic.
/// - All state transitions are timestamped for compliance and forensic analysis.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct MultisigConfig {
    /// Pool core reference
    ///
    /// # Why
    /// Binds this multisig config to a specific pool, ensuring all actions are contextually bound and auditable.
    pub pool_core: Pubkey,

    /// Multisig configuration parameters
    ///
    /// # Why
    /// Threshold and member count enforce protocol safety: no single entity can unilaterally execute privileged actions. Fixed-size array (up to 7) for deterministic, zero-copy access.
    pub threshold: u8,
    pub member_count: u8,
    pub members: [Pubkey; 7],

    /// Confirmation parameters
    ///
    /// # Why
    /// Bitfield and hash enable atomic, efficient multi-sig confirmation tracking. All state is protocol-bounded for safety and auditability.
    pub current_proposal_hash: [u8; 32],
    pub confirmation_bitmap: u8,
    pub confirmation_count: u8,

    /// Metadata for the multisig configuration
    ///
    /// # Why
    /// Tracks creation and last update for compliance, auditability, and protocol liveness.
    pub created_at: i64,
    pub last_updated: i64,

    /// Reserved space for future use or alignment
    ///
    /// # Why
    /// Reserved for future upgrades or additional fields without breaking account layout.
    pub reserved: [u8; 32],
}

/// Implementation of the MultisigConfig
impl MultisigConfig {
    /// Initialize the MultisigConfig account with protocol invariants enforced.
    ///
    /// # Why
    /// Ensures all fields are set to safe, protocol-compliant values, and that the config is ready for secure, auditable operation. Threshold and member count are strictly validated to prevent misconfiguration or attacks.
    pub fn initialize(
        &mut self,
        pool_core: Pubkey,
        threshold: u8,
        members: Vec<Pubkey>,
    ) -> Result<()> {
        // Validate the number of members and threshold
        if threshold == 0 || threshold > members.len() as u8 || members.len() > 7 {
            return Err(PdaSecurityAuthorityError::InvalidSignatureThreshold.into());
        }

        // Initialize the multisig config
        self.pool_core = pool_core;
        self.threshold = threshold;
        self.member_count = members.len() as u8;

        // Initialize members array
        self.members = [Pubkey::default(); 7];
        for (i, member) in members.iter().enumerate() {
            self.members[i] = *member;
        }

        // Initialize metadata
        let clock = Clock::get()?;
        self.created_at = clock.unix_timestamp;
        self.last_updated = clock.unix_timestamp;

        // Initialize confirmation parameters
        self.current_proposal_hash = [0u8; 32];
        self.confirmation_bitmap = 0;
        self.confirmation_count = 0;

        Ok(())
    }

    /// Check if the provided public key is a member of the multisig.
    ///
    /// # Why
    /// Enables protocol logic to enforce multi-party control for privileged actions, supporting fine-grained access control.
    pub fn is_member(&self, pubkey: &Pubkey) -> bool {
        // Check if the provided public key is in the members array
        for i in 0..self.member_count {
            if self.members[i as usize] == *pubkey {
                return true;
            }
        }
        false
    }

    /// Get the index of the provided public key in the members array.
    ///
    /// # Why
    /// Used for bitfield confirmation tracking, enabling atomic, efficient multi-sig logic.
    pub fn get_member_index(&self, pubkey: &Pubkey) -> Option<u8> {
        // Find the index of the provided public key in the members array
        (0..self.member_count).find(|&i| self.members[i as usize] == *pubkey)
    }

    /// Confirm a proposal by the given member, enforcing protocol safety and atomicity.
    ///
    /// # Why
    /// Ensures only multisig members can confirm, and that each member can only confirm once per proposal. Bitfield logic enables atomic, efficient confirmation tracking. All actions are timestamped for auditability.
    pub fn confirm_proposal(&mut self, member: &Pubkey, proposal_hash: [u8; 32]) -> Result<bool> {
        // Check if the member is part of the multisig
        if !self.is_member(member) {
            return Err(PdaSecurityAuthorityError::InsufficientSignatures.into());
        }

        // If new proposal, reset confirmations
        if self.current_proposal_hash != proposal_hash {
            self.current_proposal_hash = proposal_hash;
            self.confirmation_bitmap = 0;
            self.confirmation_count = 0;
        }

        // Get the index of the member
        let member_index = self.get_member_index(member).unwrap();
        // Create a bit mask for the member
        let member_bit = 1u8 << member_index;

        // Check if the member has already confirmed
        if (self.confirmation_bitmap & member_bit) != 0 {
            return Ok(self.confirmation_count >= self.threshold);
        }

        // Add confirmation
        self.confirmation_bitmap |= member_bit;
        self.confirmation_count += 1;

        // Update the last updated timestamp
        // This is necessary to track when the last confirmation was made
        let clock = Clock::get()?;
        self.last_updated = clock.unix_timestamp;

        Ok(self.confirmation_count >= self.threshold)
    }

    /// Reset the confirmation parameters for the multisig.
    ///
    /// # Why
    /// Ensures the config is ready for the next proposal, and that no stale confirmations persist. All actions are timestamped for auditability.
    pub fn reset_confirmations(&mut self) {
        // Reset the confirmation parameters
        self.current_proposal_hash = [0u8; 32];
        self.confirmation_bitmap = 0;
        self.confirmation_count = 0;

        // Update the last updated timestamp
        let clock = Clock::get().unwrap();
        self.last_updated = clock.unix_timestamp;
    }
}

/// Anchor context for initializing a new MultisigConfig account.
///
/// # Why
/// Enforces protocol invariants for secure initialization: deterministic PDA seeds, payer, and authority assignment. Ensures the config is created with the correct pool and system program, and that all state is zero-copy and auditable.
#[derive(Accounts)]
pub struct InitializeMultisigConfig<'info> {
    /// The MultisigConfig account to be initialized
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<MultisigConfig>(),
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// The pool core account that this multisig config is associated with
    pub pool_core: UncheckedAccount<'info>,

    /// The payer account that will pay for the initialization
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The authority that can manage the multisig config
    pub authority: Signer<'info>,

    /// The system program used for account creation
    pub system_program: Program<'info, System>,
}

/// Handler: Initialize Multisig Config with the provided pool core, threshold, and members.
///
/// # Why
/// Enforces protocol invariants for secure initialization, ensuring all state is set up for safe, auditable operation.
pub fn initialize_multisig_config(
    ctx: Context<InitializeMultisigConfig>,
    threshold: u8,
    members: Vec<Pubkey>,
) -> Result<()> {
    let multisig_config = &mut ctx.accounts.multisig_config.load_init()?;

    multisig_config.initialize(ctx.accounts.pool_core.key(), threshold, members)?;

    Ok(())
}
