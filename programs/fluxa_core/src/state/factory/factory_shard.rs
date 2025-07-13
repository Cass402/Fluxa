use crate::error::FactoryError;
use crate::math::core_arithmetic::Q64x64;
use crate::state::factory::factory_account::Factory;
use crate::utils::constants::{MAX_POOLS_PER_SHARD, STATUS_NORMAL};
use crate::utils::security_authority::core_authority::CoreAuthority;
use anchor_lang::prelude::*;

/// FactoryShard account structure for managing shards within the factory.
/// Shards allow for scalable management of pools and resources, enabling efficient distribution and load balancing.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct FactoryShard {
    /// Reference to parent factory
    pub factory: Pubkey,

    /// Shard index for identification
    pub shard_index: u16,

    /// Current number of pools in this shard
    pub pool_count: u16,

    /// Shard status flags
    pub status_flags: u8,

    /// Alignment padding
    pub _padding: [u8; 3],

    /// Last maintenance slot
    pub last_maintenance_slot: u64,

    /// Shard-specific statistics
    /// 'shard_volume' - total volume of trades in this shard
    /// 'shard_fees' - total fees collected in this shard
    pub shard_volume: Q64x64,
    pub shard_fees: Q64x64,

    /// Fixed-size array of pool keys
    pub pool_keys: [Pubkey; MAX_POOLS_PER_SHARD],

    /// Reserved space for future expansion
    pub reserved: [u64; 4], // 32 bytes reserved
}

/// FactoryShard implementation
impl FactoryShard {
    /// Initialize shard with optimized defaults
    /// This function initializes the FactoryShard with the provided factory and shard index,
    /// setting the initial state and preparing it for use.
    /// # Arguments
    /// * `factory` - The public key of the parent factory account.
    /// * `shard_index` - The index of the shard within the factory.
    /// * `current_slot` - The current slot number for timestamping.
    /// # Returns
    /// A `Result` indicating success or failure of the initialization.
    pub fn initialize(
        &mut self,
        factory: Pubkey,
        shard_index: u16,
        current_slot: u64,
    ) -> Result<()> {
        // Set factory reference and shard index
        self.factory = factory;
        self.shard_index = shard_index;

        // set the fields to their initial values
        self.pool_count = 0;
        self.status_flags = STATUS_NORMAL;
        self._padding = [0u8; 3];
        self.last_maintenance_slot = current_slot;
        self.shard_volume = Q64x64::zero();
        self.shard_fees = Q64x64::zero();

        // Initialize pool keys array with default values
        self.pool_keys = [Pubkey::default(); MAX_POOLS_PER_SHARD];
        self.reserved = [0u64; 4];

        Ok(())
    }

    /// Add pool to shard with overflow protection
    /// This function adds a new pool to the shard, ensuring that the maximum number of pools per shard is not exceeded.
    /// # Arguments
    /// * `pool_key` - The public key of the pool to be added.
    /// * `current_slot` - The current slot number for timestamping.
    /// # Returns
    /// A `Result` indicating success or failure of the operation.
    /// # Errors
    /// * `FactoryError::ShardAtCapacity` - If the shard has reached its maximum capacity.
    pub fn add_pool(&mut self, pool_key: Pubkey, current_slot: u64) -> Result<()> {
        let pool_index = self.pool_count as usize;

        require!(
            pool_index < MAX_POOLS_PER_SHARD,
            FactoryError::ShardAtCapacity
        );

        self.pool_keys[pool_index] = pool_key;
        self.pool_count += 1;
        self.last_maintenance_slot = current_slot;

        Ok(())
    }

    /// Get pool key by index with bounds checking
    /// This function retrieves the pool key at the specified index,
    /// ensuring that the index is within the valid range of existing pools.
    /// # Arguments
    /// * `index` - The index of the pool to retrieve.
    /// # Returns
    /// An `Option<Pubkey>` containing the pool key if it exists, or `None` if the index is out of bounds.
    pub fn get_pool_key(&self, index: u16) -> Option<Pubkey> {
        if (index as usize) < (self.pool_count as usize) {
            Some(self.pool_keys[index as usize])
        } else {
            None
        }
    }

    /// Update shard statistics
    pub fn update_stats(&mut self, volume: Q64x64, fees: Q64x64) -> Result<()> {
        self.shard_volume = self.shard_volume.checked_add(volume)?;
        self.shard_fees = self.shard_fees.checked_add(fees)?;

        Ok(())
    }

    /// Check if shard has capacity
    /// This function checks if the shard can accommodate more pools,
    /// ensuring that the number of pools does not exceed the maximum allowed per shard.
    /// # Returns
    /// A boolean indicating whether the shard has capacity for more pools.
    pub fn has_capacity(&self) -> bool {
        (self.pool_count as usize) < MAX_POOLS_PER_SHARD
    }

    /// Get utilization percentage (0-100)
    /// This function calculates the utilization percentage of the shard based on the number of pools it currently holds.
    /// # Returns
    /// An `u8` representing the utilization percentage, where 0 is empty and 100 is full.
    pub fn utilization_percentage(&self) -> u8 {
        ((self.pool_count as u32 * 100) / MAX_POOLS_PER_SHARD as u32) as u8
    }

    /// Check if shard needs maintenance
    /// This function checks if the shard requires maintenance based on the last maintenance slot.
    /// Maintenance is required if the last maintenance slot is more than 10,000 slots ago.
    /// # Arguments
    /// * `current_slot` - The current slot number to compare against the last maintenance slot.
    /// # Returns
    /// A boolean indicating whether the shard needs maintenance.
    pub fn needs_maintenance(&self, current_slot: u64) -> bool {
        // Maintenance every 10000 slots (~1 hour)
        current_slot.saturating_sub(self.last_maintenance_slot) > 10000
    }
}

/// InitializeShard context for shard initialization
/// This context is used to initialize a new shard within the factory.
/// It includes the necessary accounts and authority validation.
#[derive(Accounts)]
#[instruction(shard_index: u16)]
pub struct InitializeShard<'info> {
    /// Shard account to be initialized
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<FactoryShard>(),
        seeds = [b"shard", factory.key().as_ref(), &shard_index.to_le_bytes()],
        bump
    )]
    pub shard: AccountLoader<'info, FactoryShard>,

    /// Factory account to which this shard belongs
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

    /// Authority that is initializing the shard
    pub authority: Signer<'info>,

    /// Payer for the transaction
    #[account(mut)]
    pub payer: Signer<'info>,

    /// System program for account creation
    pub system_program: Program<'info, System>,
}

/// Initialize shard with authority validation
/// This function initializes a new shard within the factory,
/// ensuring that the authority is valid and the factory is operational.
/// It sets the shard index, initializes the shard state, and updates the factory's shard count.
/// # Arguments
/// * `ctx` - The context containing the accounts required for initialization.
/// * `shard_index` - The index of the shard to be initialized.
/// # Returns
/// A `Result<()>` indicating success or failure of the initialization.
pub fn initialize_shard(ctx: Context<InitializeShard>, shard_index: u16) -> Result<()> {
    // Load accounts
    let factory = &mut ctx.accounts.factory.load_mut()?;
    let shard = &mut ctx.accounts.shard.load_init()?;
    let clock = Clock::get()?;

    // Validate authority
    let core_authority = &ctx.accounts.core_authority.load()?;
    require!(
        // Ensure the authority is the current core authority
        core_authority.current_authority == ctx.accounts.authority.key(),
        FactoryError::InvalidAuthority
    );

    // Validate factory is operational
    require!(factory.is_operational(), FactoryError::FactoryPaused);

    shard.initialize(ctx.accounts.factory.key(), shard_index, clock.slot)?;

    // Update factory shard count
    factory.add_shard(clock.slot)?;

    msg!("Shard {} initialized for factory", shard_index);
    Ok(())
}
