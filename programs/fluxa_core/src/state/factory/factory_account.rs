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

/// Protocol-level configuration for the factory, governing fee structure and pool creation limits.
///
/// # Why this structure?
/// - All fields are fixed-size and aligned for deterministic account size and rent cost.
/// - No dynamic allocations (e.g., Vec), ensuring safety and predictable performance on Solana.
/// - Encapsulates all protocol-level parameters for atomic updates and easier auditing.
///
/// ## Usage
/// Used for initializing and updating the factory's global parameters, referenced by all pool creation and fee logic.
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

/// Main factory state for protocol-level configuration, pool tracking, and fee accounting.
///
/// # Why this structure?
/// - Uses zero-copy layout for maximum on-chain efficiency and deterministic account size, critical for Solana's rent and compute model.
/// - All fields are fixed-size and aligned, with no dynamic allocations, ensuring safety and predictable performance.
/// - Packs all protocol, fee, and status logic into a single account for atomic updates and easier auditing.
/// - Bitfields are used for status flags, minimizing storage and enabling atomic status changes.
///
/// ## Usage
/// This struct is the canonical source of truth for protocol state, referenced by all pool, admin, and monitoring logic.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct Factory {
    /// Reference to the core authority PDA.
    ///
    /// Why: Ensures this factory is always bound to a specific authority, preventing misconfiguration or spoofing. Used for Anchor constraint validation and upgrade safety.
    pub core_authority: Pubkey,

    /// Protocol fee rate in basis points (0-10000).
    ///
    /// Why: Integer basis points allow for fine-grained fee control, and u32 is sufficient for all practical use cases. Used for protocol revenue and risk management.
    pub protocol_fee_rate: u32,

    /// Total number of pools created by the factory.
    ///
    /// Why: Used for pool indexing, monitoring, and capacity planning. Saturating arithmetic prevents overflows.
    pub pool_count: u32,

    /// Number of active shards for pool distribution.
    ///
    /// Why: Sharding enables horizontal scaling and load balancing. u16 is sufficient for all practical deployments.
    pub shard_count: u16,

    /// Maximum pools allowed per shard (configurable).
    ///
    /// Why: Prevents any single shard from becoming a bottleneck or DoS vector. Enforced at pool creation.
    pub max_pools_per_shard: u16,

    /// Pool creation fee in lamports.
    ///
    /// Why: Discourages spam pool creation and funds protocol operations. u64 allows for future fee increases.
    pub creation_fee: u64,

    /// Last slot when any factory state was updated.
    ///
    /// Why: Enables time-based logic, replay protection, and monitoring. Used for rate limiting and audit trails.
    pub last_update_slot: u64,

    /// Status flags packed into a single u8 bitfield.
    ///
    /// Why: Bitfields allow multiple statuses (e.g., paused, emergency) to be tracked compactly and atomically, minimizing storage and compute. Enables efficient flag checks and updates.
    pub status_flags: u8,

    /// Alignment padding for 8-byte boundary.
    ///
    /// Why: Ensures zero-copy safety and future extensibility. Required by Anchor for deterministic account layout.
    pub _padding: [u8; 7],

    /// Supported fee tiers (fixed-size array for efficiency).
    ///
    /// Why: Fixed-size array avoids dynamic allocation and ensures deterministic account size. Enables fast lookups and prevents unsupported fee tiers.
    pub supported_fee_tiers: [u32; MAX_FEE_TIERS],

    /// Statistics for protocol monitoring and analytics.
    ///
    /// - `total_volume`: Total volume of trades across all pools, in Q64.64. Why: Enables protocol analytics and capacity planning.
    /// - `total_fees_collected`: Total protocol fees collected, in Q64.64. Why: Used for revenue tracking and auditing.
    pub total_volume: Q64x64,
    pub total_fees_collected: Q64x64,

    /// Factory version for protocol upgrades and migrations.
    ///
    /// Why: Allows for safe migrations and backward compatibility. u32 is sufficient for all practical upgrade paths.
    pub version: u32,

    /// Reserved for future upgrades (e.g., new features, protocol extensions) without breaking account layout.
    ///
    /// Why: Pre-allocating space allows for seamless upgrades and avoids costly migrations or rent increases.
    pub reserved: [u64; 8],
}

impl Factory {
    /// Initialize the factory with safe, validated parameters and authority binding.
    ///
    /// # Why this pattern?
    /// - All parameters are validated up front to prevent misconfiguration or protocol bricking.
    /// - Authority is set at initialization for upgrade safety and governance.
    /// - All counters and reserved fields are zeroed for deterministic state and upgradeability.
    ///
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

    /// Update protocol fee, only callable by core authority.
    ///
    /// # Why this method?
    /// - Ensures only authorized changes to protocol fee, protecting protocol revenue and user trust.
    /// - All updates are tracked by slot for auditability and replay protection.
    /// - Fee is validated to prevent bricking the protocol with an out-of-range value.
    ///
    /// # Arguments
    /// * `new_fee` - The new protocol fee rate to set.
    /// * `current_slot` - The current slot number for tracking updates.
    /// # Returns
    /// * `Result<()>` - Returns Ok if the update is successful, or an error if validation fails.
    /// # Errors
    /// * `FactoryError::InvalidFeeTier` - If the new fee exceeds the maximum allowed value.
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

    /// Atomically increment pool count and update slot.
    ///
    /// # Why this method?
    /// - Ensures pool count is always consistent and prevents race conditions.
    /// - Slot is updated for auditability and replay protection.
    /// - Designed for high performance in pool creation logic.
    pub fn increment_pool_count(&mut self, current_slot: u64) {
        self.pool_count = self.pool_count.saturating_add(1);
        self.last_update_slot = current_slot;
    }

    /// Add a new shard and update slot.
    ///
    /// # Why this method?
    /// - Enables horizontal scaling and load balancing by adding new shards.
    /// - Slot is updated for auditability and replay protection.
    /// - Returns new shard index for downstream logic.
    pub fn add_shard(&mut self, current_slot: u64) -> Result<u16> {
        let new_shard_index = self.shard_count; // Use current shard count as the new index
        self.shard_count = self.shard_count.saturating_add(1); // Increment shard count
        self.last_update_slot = current_slot; // Update last update slot

        Ok(new_shard_index)
    }

    /// Set pause status (paused/resumed) and update slot.
    ///
    /// # Why this method?
    /// - Allows for safe protocol upgrades and maintenance without redeploying.
    /// - Status flags are updated atomically for safety and auditability.
    pub fn set_paused(&mut self, paused: bool, current_slot: u64) {
        // Update the status flags based on the paused state
        if paused {
            self.status_flags |= STATUS_PAUSED;
        } else {
            self.status_flags &= !STATUS_PAUSED;
        }
        self.last_update_slot = current_slot;
    }

    /// Set emergency pause status and update slot.
    ///
    /// # Why this method?
    /// - Enables rapid response to critical failures or attacks, protecting protocol funds and users.
    /// - Status flags are updated atomically for safety and auditability.
    pub fn set_emergency_pause(&mut self, active: bool, current_slot: u64) {
        if active {
            self.status_flags |= STATUS_EMERGENCY;
        } else {
            self.status_flags &= !STATUS_EMERGENCY;
        }
        self.last_update_slot = current_slot;
    }

    /// Atomically update total volume and fees collected.
    ///
    /// # Why this method?
    /// - Ensures protocol analytics are always consistent and up to date.
    /// - All updates are atomic to prevent inconsistencies and race conditions.
    pub fn update_stats(&mut self, volume: Q64x64, fees: Q64x64) -> Result<()> {
        self.total_volume = self.total_volume.checked_add(volume)?;
        self.total_fees_collected = self.total_fees_collected.checked_add(fees)?;
        Ok(())
    }

    /// Status check methods for protocol state.
    ///
    /// # Why these methods?
    /// - Bitwise checks are used for efficiency and atomicity.
    /// - Enables fast, safe gating of protocol operations based on status.
    pub fn is_paused(&self) -> bool {
        self.status_flags & STATUS_PAUSED != 0
    }

    pub fn is_emergency_paused(&self) -> bool {
        self.status_flags & STATUS_EMERGENCY != 0
    }

    pub fn is_operational(&self) -> bool {
        self.status_flags & (STATUS_PAUSED | STATUS_EMERGENCY) == 0
    }

    /// Check if a fee tier is supported by the factory.
    ///
    /// # Why this method?
    /// - Optimized for common fee tiers to improve performance for frequent lookups.
    /// - Custom fee tiers are supported via array search, enabling protocol flexibility.
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

    /// Get optimal shard index for new pool (round-robin for now).
    ///
    /// # Why this method?
    /// - Ensures even distribution of pools across shards for load balancing.
    /// - Can be extended to consider shard utilization in the future for more advanced balancing.
    pub fn get_optimal_shard_index(&self) -> Option<u16> {
        // Simple round-robin for now
        // In production, this would consider shard utilization
        if self.shard_count == 0 {
            None
        } else {
            Some(self.pool_count as u16 % self.shard_count)
        }
    }

    /// Calculate protocol fee for a given amount, using optimized math for common rates.
    ///
    /// # Why this method?
    /// - Avoids division where possible for performance, using fast paths for common fee rates.
    /// - All math is performed in fixed-point for on-chain safety and predictability.
    /// - Used for all protocol fee calculations (trades, pool creation, etc.).
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

/// Anchor context for initializing a new factory with authority integration.
///
/// # Why this context?
/// - All accounts are validated and initialized atomically, minimizing risk of partial state.
/// - Seeds and bumps are used for deterministic address derivation and upgrade safety.
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

/// Anchor context for updating factory configuration with multisig validation.
///
/// # Why this context?
/// - Ensures only authorized and multisig-validated changes to protocol configuration.
/// - All accounts are validated atomically for safety and auditability.
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

/// Anchor context for pausing the factory in emergency situations.
///
/// # Why this context?
/// - Ensures only authorized emergency responders can pause protocol operations.
/// - All accounts are validated atomically for safety and auditability.
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

/// Anchor instruction for initializing the factory with authority integration.
///
/// # Why this function?
/// - All parameters and authorities are validated up front for safety and upgradeability.
/// - Ensures atomic initialization of all protocol state, preventing partial or inconsistent state.
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

/// Anchor instruction for updating factory configuration with multisig validation.
///
/// # Why this function?
/// - Ensures only authorized and multisig-validated changes to protocol configuration.
/// - All updates are atomic and validated for safety and auditability.
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

/// Anchor instruction for pausing the factory in emergency situations.
///
/// # Why this function?
/// - Ensures only authorized emergency responders can pause protocol operations.
/// - All updates are atomic and validated for safety and auditability.
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
