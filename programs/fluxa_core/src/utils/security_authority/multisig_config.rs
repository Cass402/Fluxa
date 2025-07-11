use crate::error::PdaSecurityAuthorityError;
use anchor_lang::prelude::*;

/// MultisigConfig is a zero-copy account structure that holds the configuration
/// for a multisig authority in the Fluxa Core security model.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct MultisigConfig {
    /// The discriminator is used to identify the account type
    /// and should be unique for each account type.
    pub discriminator: [u8; 8],

    /// The public key of the pool core that this multisig config is associated with
    pub pool_core: Pubkey,

    /// Multisig configuration parameters
    /// - `threshold`: The number of confirmations required to execute a proposal.
    /// - `member_count`: The number of members in the multisig.
    /// - `members`: An array of public keys representing the members of the multisig.
    pub threshold: u8,
    pub member_count: u8,
    pub members: [Pubkey; 7],

    /// Confirmation parameters
    /// - `current_proposal_hash`: The hash of the current proposal being voted on.
    /// - `confirmation_bitmap`: A bitmap representing which members have confirmed the proposal.
    /// - `confirmation_count`: The number of confirmations received for the current proposal.
    pub current_proposal_hash: [u8; 32],
    pub confirmation_bitmap: u8,
    pub confirmation_count: u8,

    /// Metadata for the multisig configuration
    /// - `created_at`: The timestamp when the multisig config was created.
    /// - `last_updated`: The timestamp when the multisig config was last updated.
    pub created_at: i64,
    pub last_updated: i64,

    /// Reserved space for future use or alignment
    pub reserved: [u8; 32],
}

/// Implementation of the MultisigConfig
impl MultisigConfig {
    /// Initializes a new MultisigConfig account with the provided parameters.
    /// # Arguments
    /// * `pool_core`: The public key of the pool core this multisig config is associated with.
    /// * `threshold`: The number of confirmations required to execute a proposal.
    /// * `members`: A vector of public keys representing the members of the multisig.
    /// # Returns
    /// A `Result` indicating success or failure of the initialization.
    /// # Errors
    /// * `InvalidSignatureThreshold` - If the threshold is invalid (0, greater than the number of members, or more than 7 members).
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
        self.discriminator = Self::discriminator();
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

    /// Checks if the provided public key is a member of the multisig.
    /// # Arguments
    /// * `pubkey`: The public key to check for membership.
    /// # Returns
    /// A boolean indicating whether the public key is a member of the multisig.
    pub fn is_member(&self, pubkey: &Pubkey) -> bool {
        // Check if the provided public key is in the members array
        for i in 0..self.member_count {
            if self.members[i as usize] == *pubkey {
                return true;
            }
        }
        false
    }

    /// Gets the index of the provided public key in the members array.
    /// # Arguments
    /// * `pubkey`: The public key to find the index for.
    /// # Returns
    /// An `Option<u8>` containing the index of the public key if found, or `None` if not found.
    pub fn get_member_index(&self, pubkey: &Pubkey) -> Option<u8> {
        // Find the index of the provided public key in the members array
        for i in 0..self.member_count {
            if self.members[i as usize] == *pubkey {
                return Some(i);
            }
        }
        None
    }

    /// Confirms a proposal by the given member.
    /// # Arguments
    /// * `member`: The public key of the member confirming the proposal.
    /// * `proposal_hash`: The hash of the proposal being confirmed.
    /// # Returns
    /// A `Result<bool>` indicating whether the proposal has enough confirmations to be executed.
    /// # Errors
    /// * `InsufficientSignatures` - If the member is not part of the multisig
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

    /// Resets the confirmation parameters for the multisig.
    /// This is typically called when a proposal is executed or discarded.
    /// It clears the current proposal hash, resets the confirmation bitmap,
    /// and resets the confirmation count.
    /// # Returns
    /// None
    pub fn reset_confirmations(&mut self) {
        // Reset the confirmation parameters
        self.current_proposal_hash = [0u8; 32];
        self.confirmation_bitmap = 0;
        self.confirmation_count = 0;

        // Update the last updated timestamp
        let clock = Clock::get().unwrap();
        self.last_updated = clock.unix_timestamp;
    }

    /// Returns the discriminator for the MultisigConfig account.
    /// This is used to identify the account type in the Anchor framework.
    fn discriminator() -> [u8; 8] {
        [0x11, 0x21, 0x31, 0x41, 0x51, 0x61, 0x71, 0x81]
    }
}

/// Initialize Multisig Config Context
/// This context is used to initialize a new MultisigConfig account.
/// It requires the pool core account, the payer account, and the authority
/// that will manage the multisig config.
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

/// Initialize Multisig Config
/// This function initializes a new MultisigConfig account with the provided parameters.
/// It requires the context containing the multisig config account, pool core,
/// payer, authority, and system program.
/// # Arguments
/// * `ctx`: The context containing the accounts required for initialization.
/// * `threshold`: The number of confirmations required to execute a proposal.
/// * `members`: A vector of public keys representing the members of the multisig.
/// # Returns
/// A `Result<()>` indicating success or failure of the initialization.
pub fn initialize_multisig_config(
    ctx: Context<InitializeMultisigConfig>,
    threshold: u8,
    members: Vec<Pubkey>,
) -> Result<()> {
    let multisig_config = &mut ctx.accounts.multisig_config.load_init()?;

    multisig_config.initialize(ctx.accounts.pool_core.key(), threshold, members)?;

    Ok(())
}
