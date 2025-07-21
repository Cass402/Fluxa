use crate::math::core_arithmetic::Q64x64;
use crate::state::factory::factory_account::Factory;
use crate::state::factory::factory_shard::FactoryShard;
use crate::utils::constants::MAX_POOLS_PER_SHARD;
use anchor_lang::prelude::*;

/// Calculate the minimum balance required for rent exemption for a Factory account.
///
/// # Why this function?
/// - Ensures all factory accounts are rent-exempt, preventing accidental deletion and protocol bricking.
/// - Uses Anchor's rent API for deterministic, up-to-date calculations.
/// - Always includes 8-byte discriminator for Anchor account safety.
///
/// # Returns
/// The minimum balance required for rent exemption.
pub fn get_factory_rent_exemption() -> u64 {
    Rent::get()
        .unwrap()
        .minimum_balance(8 + std::mem::size_of::<Factory>())
}

/// Calculate the minimum balance required for rent exemption for a FactoryShard account.
///
/// # Why this function?
/// - Ensures all shard accounts are rent-exempt, preventing accidental deletion and protocol bricking.
/// - Uses Anchor's rent API for deterministic, up-to-date calculations.
/// - Always includes 8-byte discriminator for Anchor account safety.
///
/// # Returns
/// The minimum balance required for rent exemption.
pub fn get_shard_rent_exemption() -> u64 {
    Rent::get()
        .unwrap()
        .minimum_balance(8 + std::mem::size_of::<FactoryShard>())
}

/// Calculate total storage cost (in lamports) for a factory and its shards.
///
/// # Why this function?
/// - Ensures protocol deployers can pre-fund all required accounts, preventing partial or failed deployments.
/// - Storage cost is always deterministic, as all accounts are fixed-size and rent-exempt.
///
/// # Arguments
/// * `num_shards` - The number of shards associated with the factory.
/// # Returns
/// The total storage cost in lamports.
pub fn calculate_total_storage_cost(num_shards: u16) -> u64 {
    let factory_rent = get_factory_rent_exemption();
    let shard_rent = get_shard_rent_exemption();

    factory_rent + (shard_rent * num_shards as u64)
}

/// Calculate the optimal number of shards for a given expected pool count.
///
/// # Why this function?
/// - Ensures no shard exceeds the maximum allowed pools, preventing DoS and performance degradation.
/// - Uses integer division with ceiling to guarantee all pools fit.
///
/// # Arguments
/// * `expected_pools` - The expected number of pools to be managed.
/// # Returns
/// The optimal number of shards required.
pub fn calculate_optimal_shards(expected_pools: u32) -> u16 {
    let pools_per_shard = MAX_POOLS_PER_SHARD as u32;
    expected_pools.div_ceil(pools_per_shard) as u16
}

/// Conservative estimate of compute units (CU) required for factory initialization.
///
/// # Why this function?
/// - Used for transaction simulation and fee estimation, preventing failed transactions due to underestimation.
/// - Estimates are intentionally conservative and should be tuned with real-world data.
pub fn estimate_factory_init_cu() -> u32 {
    5000 // Conservative estimate
}

/// Conservative estimate of compute units (CU) required for shard initialization.
///
/// # Why this function?
/// - Used for transaction simulation and fee estimation, preventing failed transactions due to underestimation.
/// - Estimates are intentionally conservative and should be tuned with real-world data.
pub fn estimate_shard_init_cu() -> u32 {
    3000 // Conservative estimate
}

/// Conservative estimate of compute units (CU) required for pool creation.
///
/// # Why this function?
/// - Used for transaction simulation and fee estimation, preventing failed transactions due to underestimation.
/// - Estimates are intentionally conservative and should be tuned with real-world data.
pub fn estimate_pool_creation_cu() -> u32 {
    2000 // Conservative estimate
}

/// Trait for factory management, enabling extensibility and abstraction.
///
/// # Why this trait?
/// - Allows for protocol upgrades and custom logic without breaking interface.
/// - Enables testing, simulation, and alternative implementations.
pub trait FactoryManager {
    fn validate_fee_tier(&self, fee_tier: u32) -> bool;
    fn get_optimal_shard(&self) -> Option<u16>;
    fn calculate_fees(&self, amount: Q64x64) -> Result<u64>;
    fn is_operational(&self) -> bool;
    fn get_utilization(&self) -> u8;
}

/// Factory management implementation
impl FactoryManager for Factory {
    /// Validate fee tier against supported tiers.
    ///
    /// # Why this method?
    /// - Ensures only supported fee tiers are used, preventing protocol bricking or user confusion.
    fn validate_fee_tier(&self, fee_tier: u32) -> bool {
        self.is_fee_tier_supported(fee_tier)
    }

    /// Get optimal shard index for new pool.
    ///
    /// # Why this method?
    /// - Ensures even distribution of pools across shards for load balancing.
    fn get_optimal_shard(&self) -> Option<u16> {
        self.get_optimal_shard_index()
    }

    /// Calculate protocol fees for a given amount.
    ///
    /// # Why this method?
    /// - All math is performed in fixed-point for on-chain safety and predictability.
    fn calculate_fees(&self, amount: Q64x64) -> Result<u64> {
        self.calculate_protocol_fee(amount)
    }

    /// Check if factory is operational (not paused or in emergency).
    ///
    /// # Why this method?
    /// - Enables fast, safe gating of protocol operations based on status.
    fn is_operational(&self) -> bool {
        self.is_operational()
    }

    /// Get utilization percentage of the factory (0-100).
    ///
    /// # Why this method?
    /// - Used for monitoring, analytics, and capacity planning.
    /// - Prevents division by zero and saturates at 0 if no shards/capacity.
    fn get_utilization(&self) -> u8 {
        if self.shard_count == 0 || self.max_pools_per_shard == 0 {
            return 0;
        }

        let total_capacity = self.shard_count as u32 * self.max_pools_per_shard as u32;
        ((self.pool_count * 100) / total_capacity) as u8
    }
}

/// Trait for shard management, enabling extensibility and abstraction.
///
/// # Why this trait?
/// - Allows for protocol upgrades and custom logic without breaking interface.
/// - Enables testing, simulation, and alternative implementations.
pub trait ShardManager {
    fn get_efficiency_score(&self) -> u8;
    fn should_rebalance(&self) -> bool;
    fn get_health_status(&self) -> ShardHealth;
}

/// Health status for a shard, used for monitoring and automated rebalancing.
///
/// # Why this enum?
/// - Enables protocol to react to degraded or critical states, e.g., by triggering rebalancing or alerts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShardHealth {
    Healthy,
    Degraded,
    Critical,
}

/// Shard management implementation
impl ShardManager for FactoryShard {
    /// Calculate efficiency score for the shard based on utilization.
    ///
    /// # Why this method?
    /// - Provides a simple, protocol-tunable scoring system for automated rebalancing and monitoring.
    /// - Optimal range is 70-90% to maximize utilization without risking overload.
    fn get_efficiency_score(&self) -> u8 {
        let utilization = self.utilization_percentage();

        // Optimal utilization is 70-90%
        match utilization {
            70..=90 => 100,
            60..=69 | 91..=95 => 80,
            50..=59 | 96..=98 => 60,
            _ => 40,
        }
    }

    /// Determine if the shard should be rebalanced (utilization > 95%).
    ///
    /// # Why this method?
    /// - Ensures protocol can react to overloaded shards, maintaining performance and safety.
    fn should_rebalance(&self) -> bool {
        self.utilization_percentage() > 95
    }

    /// Get the health status of the shard based on utilization.
    ///
    /// # Why this method?
    /// - Enables protocol to react to degraded or critical states, e.g., by triggering rebalancing or alerts.
    fn get_health_status(&self) -> ShardHealth {
        match self.utilization_percentage() {
            0..=95 => ShardHealth::Healthy,
            96..=99 => ShardHealth::Degraded,
            100 => ShardHealth::Critical,
            _ => ShardHealth::Critical,
        }
    }
}
