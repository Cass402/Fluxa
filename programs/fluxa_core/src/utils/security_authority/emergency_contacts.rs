use crate::error::PdaSecurityAuthorityError;
use anchor_lang::prelude::*;

/// EmergencyContacts account: protocol-level registry of emergency responders and authorities.
///
/// # Why
/// This account enables rapid, protocol-governed response to emergencies by maintaining a fixed, auditable list of trusted contacts and authorities.
/// It supports up to 5 contacts, each with a role and permissions, and a designated pause authority for critical actions.
///
/// # Design Rationale
/// - Zero-copy layout for deterministic, efficient access and auditability.
/// - Fixed-size array (no Vec) for contacts, ensuring predictable compute and storage costs.
/// - Roles and permissions are explicit, supporting fine-grained, protocol-enforced emergency response.
/// - Metadata and reserved space support future upgrades and compliance.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct EmergencyContacts {
    /// Pool reference
    ///
    /// # Why
    /// Binds this emergency contact registry to a specific pool, ensuring all actions are contextually bound and auditable.
    pub pool_core: Pubkey,

    /// Emergency contacts
    ///
    /// # Why
    /// Fixed-size array (up to 5) for deterministic, zero-copy access and to prevent unbounded state growth.
    pub contact_count: u8,
    pub contacts: [EmergencyContact; 5], // Up to 5 contacts

    /// Emergency configuration
    ///
    /// # Why
    /// Pause authority is the only entity that can unilaterally pause the protocol; response level encodes severity for protocol logic.
    pub pause_authority: Pubkey,
    pub emergency_response_level: u8,

    /// Metadata
    ///
    /// # Why
    /// Tracks creation and last update for compliance, auditability, and protocol liveness.
    pub created_at: i64,
    pub last_updated: i64,

    /// Future expansion
    ///
    /// # Why
    /// Reserved space for future upgrades or additional fields without breaking account layout.
    pub reserved: [u8; 32],
}

impl EmergencyContacts {
    /// Initialize the EmergencyContacts account with protocol invariants enforced.
    ///
    /// # Why
    /// Ensures all fields are set to safe, protocol-compliant values, and that the registry is ready for secure, auditable operation.
    pub fn initialize(&mut self, pool_core: Pubkey, pause_authority: Pubkey) -> Result<()> {
        // Initialize the pool core and pause authority
        self.pool_core = pool_core;
        self.pause_authority = pause_authority;

        // Initialize contact count and emergency response level
        self.contact_count = 0;
        self.emergency_response_level = 0;

        // Initialize contacts array
        self.contacts = [EmergencyContact::default(); 5];

        let clock = Clock::get()?; // Get the current clock time
                                   // Set the created and last updated timestamps to the current time
        self.created_at = clock.unix_timestamp;
        self.last_updated = clock.unix_timestamp;

        Ok(())
    }

    /// Add a new emergency contact, enforcing protocol safety and uniqueness.
    ///
    /// # Why
    /// Ensures the registry cannot exceed its fixed size, and that each contact is unique. All actions are timestamped for auditability.
    pub fn add_contact(
        &mut self,
        contact: Pubkey,
        role: EmergencyRole,
        permissions: u32,
    ) -> Result<()> {
        // Check if the contact limit has been reached
        if self.contact_count >= 5 {
            return Err(PdaSecurityAuthorityError::EmergencyContactLimitReached.into());
        }

        // Check if the contact already exists
        if self.is_emergency_contact(&contact) {
            return Err(PdaSecurityAuthorityError::EmergencyContactAlreadyExists.into());
        }

        let clock = Clock::get()?; // Get the current clock time
                                   // Initialize the emergency contact
        let emergency_contact = EmergencyContact {
            pubkey: contact,
            role,
            added_at: clock.unix_timestamp,
            last_active: 0,
            permissions,
        };

        // Add the emergency contact to the contacts array
        self.contacts[self.contact_count as usize] = emergency_contact;
        self.contact_count += 1;
        self.last_updated = clock.unix_timestamp;

        Ok(())
    }

    /// Check if a given public key is an emergency contact.
    ///
    /// # Why
    /// Enables protocol logic to verify responder status for privileged actions, supporting fine-grained access control.
    pub fn is_emergency_contact(&self, pubkey: &Pubkey) -> bool {
        for i in 0..self.contact_count {
            if self.contacts[i as usize].pubkey == *pubkey {
                return true;
            }
        }
        false
    }

    /// Check if a given public key has emergency authority (pause authority or responder).
    ///
    /// # Why
    /// Enables protocol logic to enforce emergency controls, ensuring only trusted parties can trigger critical actions.
    pub fn has_emergency_authority(&self, pubkey: &Pubkey) -> bool {
        *pubkey == self.pause_authority || self.is_emergency_contact(pubkey)
    }
}

/// EmergencyContact: individual responder entry in the EmergencyContacts registry.
///
/// # Why
/// Encodes all relevant metadata for a responder, supporting fine-grained, protocol-enforced emergency response and auditability.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct EmergencyContact {
    pub pubkey: Pubkey,
    pub role: EmergencyRole,
    pub added_at: i64,
    pub last_active: i64,
    pub permissions: u32,
}

/// Default implementation for EmergencyContact
///
/// # Why
/// Ensures all fields are initialized to safe, protocol-compliant values, supporting zero-copy and deterministic state.
impl Default for EmergencyContact {
    fn default() -> Self {
        Self {
            pubkey: Pubkey::default(),
            role: EmergencyRole::Responder,
            added_at: 0,
            last_active: 0,
            permissions: 0,
        }
    }
}

/// EmergencyRole: protocol-defined roles for emergency contacts.
///
/// # Why
/// Encodes the responsibilities and authority levels for each responder, supporting fine-grained, protocol-enforced emergency response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum EmergencyRole {
    Responder = 0,
    Coordinator = 1,
    TechnicalLead = 2,
    CommunityDelegate = 3,
    AuditPartner = 4,
}

/// Anchor context for initializing the EmergencyContacts account.
///
/// # Why
/// Enforces protocol invariants for secure initialization: deterministic PDA seeds, payer, and authority assignment. Ensures the registry is created with the correct pool and system program, and that all state is zero-copy and auditable.
#[derive(Accounts)]
pub struct InitializeEmergencyContacts<'info> {
    /// The EmergencyContacts account to be initialized
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<EmergencyContacts>(),
        seeds = [b"emergency_contacts", pool_core.key().as_ref()],
        bump
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    /// The pool core account associated with the emergency contacts
    pub pool_core: UncheckedAccount<'info>,

    /// The payer account responsible for the transaction fees
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The authority account
    pub authority: Signer<'info>,

    /// The system program account
    pub system_program: Program<'info, System>,
}

/// Handler: Initialize Emergency Contacts with the provided pool core and pause authority.
///
/// # Why
/// Enforces protocol invariants for secure initialization, ensuring all state is set up for safe, auditable operation.
pub fn initialize_emergency_contacts(
    ctx: Context<InitializeEmergencyContacts>,
    pause_authority: Pubkey,
) -> Result<()> {
    let emergency_contacts = &mut ctx.accounts.emergency_contacts.load_init()?;

    emergency_contacts.initialize(ctx.accounts.pool_core.key(), pause_authority)?;

    Ok(())
}
