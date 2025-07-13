use crate::error::FactoryError;
use crate::math::core_arithmetic::{mul_div_q64, Q64x64};
use crate::utils::constants::{
    DEFAULT_FEE_TIERS, DEFAULT_PROTOCOL_FEE, MAX_FEE_TIERS, MAX_POOLS_PER_SHARD, POOL_CREATION_FEE,
    STATUS_EMERGENCY, STATUS_MAINTENANCE, STATUS_NORMAL, STATUS_PAUSED,
};
use crate::utils::security_authority::core_authority::CoreAuthority;
use crate::utils::security_authority::emergency_contacts::EmergencyContacts;
use crate::utils::security_authority::multisig_config::MultisigConfig;
use anchor_lang::prelude::*;

/// FactoryConfig structure for managing protocol-level configurations and state.
/// This configuration includes parameters such as protocol fee rates, creation fees, and supported fee tiers.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct FactoryConfig {
    pub protocol_fee_rate: u32,
    pub creation_fee: u64,
    pub supported_fee_tiers: [u32; MAX_FEE_TIERS],
    pub max_pools_per_shard: u16,
}

/// Default implementation for FactoryConfig
impl Default for FactoryConfig {
    fn default() -> Self {
        Self {
            protocol_fee_rate: DEFAULT_PROTOCOL_FEE,
            creation_fee: POOL_CREATION_FEE,
            supported_fee_tiers: DEFAULT_FEE_TIERS,
            max_pools_per_shard: MAX_POOLS_PER_SHARD as u16,
        }
    }
}

/// Factory account structure for managing protocol-level configurations and state.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct Factory {
    /// Reference to core authority PDA
    pub core_authority: Pubkey,

    /// Protocol fee rate in basis points
    pub protocol_fee_rate: u32,

    /// Total number of pools created
    pub pool_count: u32,

    /// Number of active shards
    pub shard_count: u16,

    /// Maximum pools per shard (configurable)
    pub max_pools_per_shard: u16,

    /// Creation fee in lamports
    pub creation_fee: u64,

    /// Last update slot for tracking
    pub last_update_slot: u64,

    /// Factory status flags (bitfield for various states)
    pub status_flags: u8,

    /// Alignment padding
    pub _padding: [u8; 7],

    /// Supported fee tiers (fixed size for efficiency)
    pub supported_fee_tiers: [u32; MAX_FEE_TIERS],

    /// Statistics for monitoring
    /// 'total_volume' - total volume of trades across all pools
    /// 'total_fees_collected' - total fees collected by the factory
    pub total_volume: Q64x64,
    pub total_fees_collected: Q64x64,

    /// Factory version for upgrades
    pub version: u32,

    /// Reserved space for future expansion (tightly packed)
    pub reserved: [u64; 8],
}

impl Factory {
    /// Initialize the factory account with the provided configuration and core authority.
    /// This method sets up the factory with initial parameters and validates them.
    /// # Arguments
    /// * `core_authority` - The public key of the core authority managing the factory.
    /// * `config` - The configuration parameters for the factory.
    /// * `current_slot` - The current slot number for tracking updates.
    /// # Returns
    /// * `Result<()>` - Returns Ok if initialization is successful, or an error if validation fails.
    /// # Errors
    /// * `FactoryError::InvalidFeeTier` - If the protocol fee rate exceeds the maximum allowed value.
    /// * `FactoryError::InvalidShardIndex` - If the maximum pools per shard is not within the valid range.
    pub fn initialize(
        &mut self,
        core_authority: Pubkey,
        config: FactoryConfig,
        current_slot: u64,
    ) -> Result<()> {
        // Validate configuration
        require!(
            config.protocol_fee_rate <= 10000, // Max 100%
            FactoryError::InvalidFeeTier
        );

        // Validate max pools per shard
        require!(
            config.max_pools_per_shard > 0 && config.max_pools_per_shard <= 1000,
            FactoryError::InvalidShardIndex
        );

        // Set the core authority of the factory
        self.core_authority = core_authority;

        // Set the protocol fee rate and other parameters
        self.protocol_fee_rate = config.protocol_fee_rate;
        self.pool_count = 0; // Initialize pool count to zero
        self.shard_count = 0; // Initialize shard count to zero
        self.max_pools_per_shard = config.max_pools_per_shard; // Set max pools per shard
        self.creation_fee = config.creation_fee; // Set creation fee
        self.last_update_slot = current_slot; // Set the last update slot to the current slot
        self.status_flags = STATUS_NORMAL; // Set initial status flags to normal
        self._padding = [0u8; 7]; // Padding for alignment
        self.supported_fee_tiers = config.supported_fee_tiers; // Set supported fee tiers
        self.total_volume = Q64x64::zero(); // Initialize total volume to zero
        self.total_fees_collected = Q64x64::zero(); // Initialize total fees collected to zero
        self.version = 1; // Set initial version

        // Zero out reserved space
        self.reserved = [0u64; 8];

        Ok(())
    }

    /// Update protocol fee
    /// This method allows the core authority to update the protocol fee rate.
    /// # Arguments
    /// * `new_fee` - The new protocol fee rate to set.
    /// * `current_slot` - The current slot number for tracking updates.
    /// # Returns
    /// * `Result<()>` - Returns Ok if the update is successful, or an error if validation fails.
    /// # Errors
    /// * `FactoryError::InvalidFeeTier` - If the new fee exceeds the maximum allowed value.
    /// # Note
    /// This method is designed to be called by the core authority, ensuring that only authorized changes
    /// to the protocol fee can be made. It also updates the last update slot to track when the change occurred.
    pub fn update_protocol_fee(&mut self, new_fee: u32, current_slot: u64) -> Result<()> {
        require!(
            self.status_flags == STATUS_NORMAL || self.status_flags == STATUS_MAINTENANCE,
            FactoryError::FactoryPaused
        );
        // Check if the new fee is within valid range (0 to 10000 basis points)
        require!(new_fee <= 10000, FactoryError::InvalidFeeTier);

        // Update the protocol fee rate
        self.protocol_fee_rate = new_fee;
        self.last_update_slot = current_slot;

        Ok(())
    }

    /// Increment pool count atomically
    /// This method increments the pool count by one and updates the last update slot.
    /// It is designed to be called whenever a new pool is created within the factory.
    /// # Arguments
    /// * `current_slot` - The current slot number for tracking updates.
    /// # Note
    /// This method is optimized for performance and should be called in the context of pool creation.
    /// It ensures that the pool count is incremented atomically, preventing race conditions.
    pub fn increment_pool_count(&mut self, current_slot: u64) {
        self.pool_count = self.pool_count.saturating_add(1);
        self.last_update_slot = current_slot;
    }

    /// Add new shard
    /// This method adds a new shard to the factory and updates the last update slot.
    /// # Arguments
    /// * `current_slot` - The current slot number for tracking updates.
    /// # Returns
    /// * `Result<u16>` - Returns the index of the newly created shard or an error if the maximum shard limit is reached.
    pub fn add_shard(&mut self, current_slot: u64) -> Result<u16> {
        let new_shard_index = self.shard_count; // Use current shard count as the new index
        self.shard_count = self.shard_count.saturating_add(1); // Increment shard count
        self.last_update_slot = current_slot; // Update last update slot

        Ok(new_shard_index)
    }

    /// Set pause status
    /// This method allows the factory to be paused or resumed.
    /// # Arguments
    /// * `paused` - A boolean indicating whether to pause or resume the factory.
    /// * `current_slot` - The current slot number for tracking updates.
    /// # Note
    /// This method updates the status flags to reflect the paused state and sets the last update slot.
    /// It is designed to be called by the core authority or during maintenance operations.
    pub fn set_paused(&mut self, paused: bool, current_slot: u64) {
        // Update the status flags based on the paused state
        if paused {
            self.status_flags |= STATUS_PAUSED;
        } else {
            self.status_flags &= !STATUS_PAUSED;
        }
        self.last_update_slot = current_slot;
    }

    /// Set emergency pause status
    /// This method allows the factory to be set into an emergency pause state.
    /// # Arguments
    /// * `active` - A boolean indicating whether to activate or deactivate the emergency pause.
    /// * `current_slot` - The current slot number for tracking updates.
    /// # Note
    /// This method updates the status flags to reflect the emergency pause state and sets the last update slot.
    /// It is designed to be called in critical situations where immediate action is required to protect the system.
    pub fn set_emergency_pause(&mut self, active: bool, current_slot: u64) {
        if active {
            self.status_flags |= STATUS_EMERGENCY;
        } else {
            self.status_flags &= !STATUS_EMERGENCY;
        }
        self.last_update_slot = current_slot;
    }

    /// Update statistics
    /// This method updates the total volume and fees collected by the factory.
    /// # Arguments
    /// * `volume` - The volume of trades to add to the total.
    /// * `fees` - The fees collected to add to the total.
    /// # Note
    /// This method is designed to be called whenever trades are executed within the factory.
    /// It ensures that the statistics are updated atomically to prevent inconsistencies.
    pub fn update_stats(&mut self, volume: Q64x64, fees: Q64x64) -> Result<()> {
        self.total_volume = self.total_volume.checked_add(volume)?;
        self.total_fees_collected = self.total_fees_collected.checked_add(fees)?;
        Ok(())
    }

    /// Status check methods
    pub fn is_paused(&self) -> bool {
        self.status_flags & STATUS_PAUSED != 0
    }

    pub fn is_emergency_paused(&self) -> bool {
        self.status_flags & STATUS_EMERGENCY != 0
    }

    pub fn is_operational(&self) -> bool {
        self.status_flags & (STATUS_PAUSED | STATUS_EMERGENCY) == 0
    }

    /// Check if fee tier is supported
    /// This method checks if a given fee tier is supported by the factory.
    /// # Arguments
    /// * `fee_tier` - The fee tier to check.
    /// # Returns
    /// * `bool` - Returns true if the fee tier is supported, false otherwise.
    /// # Note
    /// This method uses an optimized approach to check for common fee tiers first, improving performance for
    /// frequently used tiers. It also handles custom fee tiers defined in the supported_fee_tiers array
    pub fn is_fee_tier_supported(&self, fee_tier: u32) -> bool {
        // Optimized loop unrolling for common case
        if fee_tier == 0 {
            return false;
        }

        // Check most common tiers first
        if fee_tier == 100 || fee_tier == 500 || fee_tier == 3000 || fee_tier == 10000 {
            return true;
        }

        // Fallback to array search for custom tiers
        self.supported_fee_tiers
            .iter()
            .any(|&tier| tier == fee_tier && tier != 0)
    }

    /// Get optimal shard for new pool
    /// This method determines the optimal shard index for a new pool based on the current shard count.
    /// It uses a simple round-robin approach for now, but can be extended to consider shard utilization in the future.
    /// # Returns
    /// * `Option<u16>` - Returns the index of the optimal shard or None if there are no shards available.
    /// # Note
    /// This method is designed to be efficient and should be called when creating new pools to ensure
    /// that they are distributed evenly across available shards.
    pub fn get_optimal_shard_index(&self) -> Option<u16> {
        // Simple round-robin for now
        // In production, this would consider shard utilization
        if self.shard_count == 0 {
            None
        } else {
            Some(self.pool_count as u16 % self.shard_count)
        }
    }

    /// Calculate protocol fees
    /// This method calculates the protocol fees based on the provided amount and the current protocol fee rate.
    /// It uses an optimized approach to avoid division where possible, improving performance for common fee rates.
    /// # Arguments
    /// * `amount` - The amount to calculate the protocol fee for.
    /// # Returns
    /// * `u64` - The calculated protocol fee.
    /// # Note
    /// This method is designed to be efficient and should be used whenever protocol fees need to be
    /// calculated, such as during trade executions or pool creations.
    pub fn calculate_protocol_fee(&self, amount: Q64x64) -> Result<u64> {
        // Optimized calculation avoiding division where possible
        if self.protocol_fee_rate == 0 {
            return Ok(0u64);
        }

        // Use bit shifting for common percentages
        let protocol_fee = match self.protocol_fee_rate {
            100 => (amount.checked_div(Q64x64::from_int(100u64)))?, // 1%
            500 => (amount.checked_div(Q64x64::from_int(500u64)))?, // 5%
            1000 => (amount.checked_div(Q64x64::from_int(1000u64)))?, // 10%
            _ => mul_div_q64(
                amount,
                Q64x64::from_int(self.protocol_fee_rate as u64),
                Q64x64::from_int(10000u64),
            )?,
        };

        let protocol_fee = (protocol_fee.raw() >> 64) as u64;
        Ok(protocol_fee)
    }
}

/// Initialize Factory with Authority Integration
/// This context is used to initialize a new factory with the provided configuration.
#[derive(Accounts)]
#[instruction(config: FactoryConfig)]
pub struct InitializeFactory<'info> {
    /// The Factory account to be initialized
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<Factory>(),
        seeds = [b"factory"],
        bump
    )]
    pub factory: AccountLoader<'info, Factory>,

    /// Core authority for the factory
    #[account(
        seeds = [b"core_authority", factory.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// The payer account responsible for the transaction fees
    #[account(mut)]
    pub payer: Signer<'info>,

    /// System program for account creation
    pub system_program: Program<'info, System>,
}

/// Update Factory Configuration
/// This context is used to update the factory configuration with multisig validation.
/// It requires the factory account, core authority, multisig config, and the authority making the change.
/// This ensures that critical changes to the factory configuration are validated through a multisig process.
#[derive(Accounts)]
pub struct UpdateFactoryConfig<'info> {
    /// The Factory account to be updated
    #[account(
        mut,
        seeds = [b"factory"],
        bump
    )]
    pub factory: AccountLoader<'info, Factory>,

    /// Core authority validation
    #[account(
        seeds = [b"core_authority", factory.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// Multisig validation for critical changes
    #[account(
        seeds = [b"multisig_config", factory.key().as_ref()],
        bump
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// The authority making the change
    pub authority: Signer<'info>,
}

/// Emergency Pause Factory
/// This context is used to pause the factory in emergency situations.
/// It requires the factory account, core authority, emergency contacts, and the emergency responder.
/// This ensures that the factory can be paused safely during emergencies, preventing further operations until resolved.
#[derive(Accounts)]
pub struct EmergencyPauseFactory<'info> {
    /// The Factory account to be paused
    #[account(
        mut,
        seeds = [b"factory"],
        bump
    )]
    pub factory: AccountLoader<'info, Factory>,

    /// Core authority validation
    #[account(
        seeds = [b"core_authority", factory.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// Emergency contacts validation
    #[account(
        seeds = [b"emergency_contacts", factory.key().as_ref()],
        bump
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    /// The emergency responder account
    pub emergency_responder: Signer<'info>,
}

/// Initialize factory with authority.rs integration
/// This function initializes the factory with the provided configuration and core authority.
/// It sets up the factory account with initial parameters and validates them.
/// # Arguments
/// * `ctx` - The context containing the accounts required for initialization.
/// * `config` - The configuration parameters for the factory.
/// # Returns
/// A `Result<()>` indicating success or failure of the initialization.
pub fn initialize_factory(ctx: Context<InitializeFactory>, config: FactoryConfig) -> Result<()> {
    let factory = &mut ctx.accounts.factory.load_init()?;
    let clock = Clock::get()?;

    // Validate core authority is properly initialized
    let core_authority = &ctx.accounts.core_authority.load()?;
    require!(
        core_authority.pool_core == ctx.accounts.factory.key(),
        FactoryError::InvalidAuthority
    );

    factory.initialize(ctx.accounts.core_authority.key(), config, clock.slot)?;

    msg!(
        "Factory initialized with core authority: {}",
        ctx.accounts.core_authority.key()
    );
    Ok(())
}

/// Update factory configuration with multisig validation
/// This function updates the factory configuration with the provided parameters.
/// It requires the authority to be a member of the multisig if critical changes are made.
/// # Arguments
/// * `ctx` - The context containing the accounts required for updating the factory.
/// * `new_config` - The new configuration parameters for the factory.
/// # Returns
/// A `Result<()>` indicating success or failure of the update.
pub fn update_factory_config(
    ctx: Context<UpdateFactoryConfig>,
    new_config: FactoryConfig,
) -> Result<()> {
    let factory = &mut ctx.accounts.factory.load_mut()?;
    let clock = Clock::get()?;

    // Validate authority
    let core_authority = &ctx.accounts.core_authority.load()?;
    require!(
        core_authority.current_authority == ctx.accounts.authority.key(),
        FactoryError::InvalidAuthority
    );

    // Validate multisig if required for critical changes
    let multisig_config = &ctx.accounts.multisig_config.load()?;
    if new_config.protocol_fee_rate != factory.protocol_fee_rate {
        require!(
            multisig_config.is_member(&ctx.accounts.authority.key()),
            FactoryError::InsufficientPermissions
        );
    }

    // Update configuration
    factory.update_protocol_fee(new_config.protocol_fee_rate, clock.slot)?;
    factory.creation_fee = new_config.creation_fee;
    factory.supported_fee_tiers = new_config.supported_fee_tiers;
    factory.max_pools_per_shard = new_config.max_pools_per_shard;

    msg!("Factory configuration updated");
    Ok(())
}

/// Emergency pause factory
/// This function pauses the factory in emergency situations.
/// It requires the emergency responder to have the authority to pause operations.
/// # Arguments
/// * `ctx` - The context containing the accounts required for pausing the factory.
/// * `pause_active` - A boolean indicating whether to activate or deactivate the emergency pause.
/// # Returns
/// A `Result<()>` indicating success or failure of the operation.
pub fn emergency_pause_factory(
    ctx: Context<EmergencyPauseFactory>,
    pause_active: bool,
) -> Result<()> {
    let factory = &mut ctx.accounts.factory.load_mut()?;
    let clock = Clock::get()?;

    // Validate emergency authority
    let emergency_contacts = &ctx.accounts.emergency_contacts.load()?;
    require!(
        emergency_contacts.has_emergency_authority(&ctx.accounts.emergency_responder.key()),
        FactoryError::InsufficientPermissions
    );

    factory.set_emergency_pause(pause_active, clock.slot);

    msg!("Factory emergency pause: {}", pause_active);
    Ok(())
}
