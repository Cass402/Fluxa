use crate::error::PdaSecurityAuthorityError;
use anchor_lang::prelude::*;

/// EmergencyContacts account maintains a list of emergency contacts for the Fluxa protocol.
/// It allows for up to 5 emergency contacts, each with a specific role and permissions.
/// The account also includes metadata such as creation and last updated timestamps,
/// and a pause authority that can pause emergency operations.
/// The account is designed to be zero-copy for efficient access and manipulation.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct EmergencyContacts {
    /// Account discriminator
    pub discriminator: [u8; 8],

    /// Pool reference
    pub pool_core: Pubkey,

    /// Emergency contacts
    pub contact_count: u8,
    pub contacts: [EmergencyContact; 5], // Up to 5 contacts

    /// Emergency configuration
    /// Pause authority is the main authority that can pause emergency operations
    /// Emergency response level determines the severity of the emergency
    pub pause_authority: Pubkey,
    pub emergency_response_level: u8,

    /// Metadata
    /// These timestamps track when the emergency contacts were created and last updated
    pub created_at: i64,
    pub last_updated: i64,

    /// Future expansion
    pub reserved: [u8; 32],
}

impl EmergencyContacts {
    /// Initializes the EmergencyContacts account with the given pool core and pause authority.
    /// This function sets the account discriminator, initializes the pool core and pause authority,
    /// sets the contact count to zero, initializes the emergency response level, and sets the created
    /// and last updated timestamps to the current time.
    /// # Arguments
    /// * `pool_core` - The public key of the pool core account.
    /// * `pause_authority` - The public key of the authority that can pause emergency operations.
    /// # Returns
    /// A `Result` indicating success or failure of the initialization.
    pub fn initialize(&mut self, pool_core: Pubkey, pause_authority: Pubkey) -> Result<()> {
        // Initialize the account discriminator which is used to identify the account type
        self.discriminator = Self::discriminator();

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

    /// Adds a new emergency contact to the contacts array.
    /// This function checks if the contact limit has been reached, verifies if the contact already exists
    /// in the contacts array, and if not, initializes a new `EmergencyContact` with the provided
    /// public key, role, and permissions. It then adds the contact to the contacts array,
    /// increments the contact count, and updates the last updated timestamp.
    /// # Arguments
    /// * `contact` - The public key of the emergency contact to be added.
    /// * `role` - The role of the emergency contact (e.g., Responder   , Coordinator, etc.).
    /// * `permissions` - The permissions associated with the emergency contact.
    /// # Returns
    /// A `Result` indicating success or failure of the operation.
    /// # Errors
    /// * `EmergencyContactLimitReached` - If the maximum number of emergency contacts (5) has been reached.
    /// * `EmergencyContactAlreadyExists` - If the contact already exists in the contacts array.
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

    /// Checks if a given public key is an emergency contact.
    /// This function iterates through the contacts array and compares each contact's public key
    /// with the provided public key. If a match is found, it returns true; otherwise, it returns false.
    /// # Arguments
    /// * `pubkey` - The public key to check against the emergency contacts.
    /// # Returns
    /// A boolean indicating whether the provided public key is an emergency contact.
    pub fn is_emergency_contact(&self, pubkey: &Pubkey) -> bool {
        for i in 0..self.contact_count {
            if self.contacts[i as usize].pubkey == *pubkey {
                return true;
            }
        }
        false
    }

    /// Checks if a given public key has emergency authority.
    /// This function checks if the provided public key matches the pause authority or if it is listed
    /// as an emergency contact. If either condition is true, it returns true; otherwise,
    /// it returns false.
    /// # Arguments
    /// * `pubkey` - The public key to check for emergency authority.
    /// # Returns
    /// A boolean indicating whether the provided public key has emergency authority.
    pub fn has_emergency_authority(&self, pubkey: &Pubkey) -> bool {
        *pubkey == self.pause_authority || self.is_emergency_contact(pubkey)
    }

    /// Returns the account discriminator for the EmergencyContacts account.
    fn discriminator() -> [u8; 8] {
        [0x12, 0x22, 0x32, 0x42, 0x52, 0x62, 0x72, 0x82]
    }
}

/// EmergencyContact represents an individual emergency contact within the EmergencyContacts account.
/// It contains the public key of the contact, their role, timestamps for when they were added
/// and last active, and their permissions.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct EmergencyContact {
    pub pubkey: Pubkey,
    pub role: EmergencyRole,
    pub added_at: i64,
    pub last_active: i64,
    pub permissions: u32,
}

//// Default implementation for EmergencyContact
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

/// EmergencyRole defines the roles that emergency contacts can have within the Fluxa protocol.
/// Each role has a specific responsibility and level of authority during emergency situations.
/// The roles include Responder, Coordinator, Technical Lead, Community Delegate, and Audit Partner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum EmergencyRole {
    Responder = 0,
    Coordinator = 1,
    TechnicalLead = 2,
    CommunityDelegate = 3,
    AuditPartner = 4,
}

/// InitializeEmergencyContacts is the context for initializing the EmergencyContacts account.
/// It includes the EmergencyContacts account to be initialized, the pool core account,
/// the payer account responsible for the transaction fees, the authority account,
/// and the system program account.
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

/// Initialize Emergency Contacts
/// This function initializes the EmergencyContacts account with the provided pool core and pause authority.
/// It sets the account discriminator, initializes the pool core and pause authority,
/// sets the contact count to zero, initializes the emergency response level, and sets the created
/// and last updated timestamps to the current time.
/// # Arguments
/// * `ctx` - The context containing the accounts required for initialization.
/// * `pause_authority` - The public key of the authority that can pause emergency operations
pub fn initialize_emergency_contacts(
    ctx: Context<InitializeEmergencyContacts>,
    pause_authority: Pubkey,
) -> Result<()> {
    let emergency_contacts = &mut ctx.accounts.emergency_contacts.load_init()?;

    emergency_contacts.initialize(ctx.accounts.pool_core.key(), pause_authority)?;

    Ok(())
}
