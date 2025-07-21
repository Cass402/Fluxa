use crate::error::FactoryError;
use crate::math::core_arithmetic::Q64x64;
use crate::state::factory::factory_account::Factory;
use crate::utils::constants::{MAX_POOLS_PER_SHARD, STATUS_NORMAL};
use crate::utils::security_authority::core_authority::CoreAuthority;
use anchor_lang::prelude::*;

/// Shard state for scalable pool management within the factory.
///
/// # Why this structure?
/// - Uses zero-copy layout for maximum on-chain efficiency and deterministic account size, critical for Solana's rent and compute model.
/// - All fields are fixed-size and aligned, with no dynamic allocations, ensuring safety and predictable performance.
/// - Sharding enables horizontal scaling, load balancing, and isolation of pool state for performance and risk management.
/// - Bitfields are used for status flags, minimizing storage and enabling atomic status changes.
///
/// ## Usage
/// This struct is the canonical source of truth for shard state, referenced by all pool, admin, and monitoring logic.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct FactoryShard {
    /// Reference to the parent factory account.
    ///
    /// Why: Ensures this shard is always bound to a specific factory, preventing misconfiguration or spoofing. Used for Anchor constraint validation and upgrade safety.
    pub factory: Pubkey,

    /// Shard index for identification within the factory.
    ///
    /// Why: Enables deterministic address derivation and efficient lookup. u16 is sufficient for all practical deployments.
    pub shard_index: u16,

    /// Current number of pools in this shard.
    ///
    /// Why: Used for capacity planning, monitoring, and preventing overflows. Saturating arithmetic prevents overflows.
    pub pool_count: u16,

    /// Status flags packed into a single u8 bitfield.
    ///
    /// Why: Bitfields allow multiple statuses (e.g., paused, maintenance) to be tracked compactly and atomically, minimizing storage and compute. Enables efficient flag checks and updates.
    pub status_flags: u8,

    /// Alignment padding for 8-byte boundary.
    ///
    /// Why: Ensures zero-copy safety and future extensibility. Required by Anchor for deterministic account layout.
    pub _padding: [u8; 3],

    /// Last slot when any shard maintenance was performed.
    ///
    /// Why: Enables time-based logic, replay protection, and monitoring. Used for maintenance scheduling and audit trails.
    pub last_maintenance_slot: u64,

    /// Shard-specific statistics for monitoring and analytics.
    ///
    /// - `shard_volume`: Total volume of trades in this shard, in Q64.64. Why: Enables protocol analytics and capacity planning.
    /// - `shard_fees`: Total protocol fees collected in this shard, in Q64.64. Why: Used for revenue tracking and auditing.
    pub shard_volume: Q64x64,
    pub shard_fees: Q64x64,

    /// Fixed-size array of pool keys managed by this shard.
    ///
    /// Why: Avoids dynamic allocation and ensures deterministic account size. Enables fast lookups and prevents unsupported pool counts.
    pub pool_keys: [Pubkey; MAX_POOLS_PER_SHARD],

    /// Reserved for future upgrades (e.g., new features, protocol extensions) without breaking account layout.
    ///
    /// Why: Pre-allocating space allows for seamless upgrades and avoids costly migrations or rent increases.
    pub reserved: [u64; 4], // 32 bytes reserved
}

/// FactoryShard implementation
impl FactoryShard {
    /// Initialize the shard with safe, validated parameters and authority binding.
    ///
    /// # Why this pattern?
    /// - All parameters are validated up front to prevent misconfiguration or protocol bricking.
    /// - All counters and reserved fields are zeroed for deterministic state and upgradeability.
    ///
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

    /// Add a pool to the shard, with overflow protection.
    ///
    /// # Why this method?
    /// - Ensures no shard exceeds the maximum allowed pools, preventing DoS and performance degradation.
    /// - All updates are atomic and validated for safety and auditability.
    ///
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

    /// Get pool key by index, with bounds checking.
    ///
    /// # Why this method?
    /// - Prevents out-of-bounds access and potential memory corruption.
    /// - Returns None if index is invalid, for safe downstream logic.
    ///
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

    /// Atomically update shard volume and fees.
    ///
    /// # Why this method?
    /// - Ensures protocol analytics are always consistent and up to date.
    /// - All updates are atomic to prevent inconsistencies and race conditions.
    pub fn update_stats(&mut self, volume: Q64x64, fees: Q64x64) -> Result<()> {
        self.shard_volume = self.shard_volume.checked_add(volume)?;
        self.shard_fees = self.shard_fees.checked_add(fees)?;

        Ok(())
    }

    /// Check if the shard has capacity for more pools.
    ///
    /// # Why this method?
    /// - Prevents overflows and ensures safe pool addition.
    /// # Returns
    /// A boolean indicating whether the shard has capacity for more pools.
    pub fn has_capacity(&self) -> bool {
        (self.pool_count as usize) < MAX_POOLS_PER_SHARD
    }

    /// Get utilization percentage (0-100) for the shard.
    ///
    /// # Why this method?
    /// - Used for monitoring, analytics, and capacity planning.
    /// - Prevents division by zero and saturates at 0 if no pools/capacity.
    /// # Returns
    /// An `u8` representing the utilization percentage, where 0 is empty and 100 is full.
    pub fn utilization_percentage(&self) -> u8 {
        ((self.pool_count as u32 * 100) / MAX_POOLS_PER_SHARD as u32) as u8
    }

    /// Check if the shard needs maintenance (e.g., compaction, rebalancing).
    ///
    /// # Why this method?
    /// - Ensures protocol can schedule maintenance efficiently, preventing performance degradation.
    /// - Uses slot-based logic for deterministic, on-chain scheduling.
    /// # Arguments
    /// * `current_slot` - The current slot number to compare against the last maintenance slot.
    /// # Returns
    /// A boolean indicating whether the shard needs maintenance.
    pub fn needs_maintenance(&self, current_slot: u64) -> bool {
        // Maintenance every 10000 slots (~1 hour)
        current_slot.saturating_sub(self.last_maintenance_slot) > 10000
    }
}

/// Anchor context for initializing a new shard within the factory.
///
/// # Why this context?
/// - All accounts are validated and initialized atomically, minimizing risk of partial state.
/// - Seeds and bumps are used for deterministic address derivation and upgrade safety.
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

/// Anchor instruction for initializing a new shard within the factory, with authority validation.
///
/// # Why this function?
/// - Ensures only authorized and operational factories can create new shards, protecting protocol safety.
/// - All updates are atomic and validated for safety and auditability.
/// - Shard index and state are set deterministically for upgrade and monitoring safety.
///
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
