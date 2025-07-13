use crate::math::core_arithmetic::Q64x64;
use crate::state::factory::factory_account::Factory;
use crate::state::factory::factory_shard::FactoryShard;
use crate::utils::constants::MAX_POOLS_PER_SHARD;
use anchor_lang::prelude::*;

/// Get minimum balance for rent exemption
/// This function calculates the minimum balance required for rent exemption
/// for a Factory account based on the current rent parameters.
/// # Returns
/// The minimum balance required for rent exemption.
pub fn get_factory_rent_exemption() -> u64 {
    Rent::get()
        .unwrap()
        .minimum_balance(8 + std::mem::size_of::<Factory>())
}

/// Get minimum balance for rent exemption for a FactoryShard account
/// This function calculates the minimum balance required for rent exemption
/// for a FactoryShard account based on the current rent parameters.
/// # Returns
/// The minimum balance required for rent exemption.
pub fn get_shard_rent_exemption() -> u64 {
    Rent::get()
        .unwrap()
        .minimum_balance(8 + std::mem::size_of::<FactoryShard>())
}

/// Calculate total storage costs for factory and shards
/// This function calculates the total storage costs for a Factory account
/// and its associated shards based on the number of shards.
/// # Arguments
/// * `num_shards` - The number of shards associated with the factory.
/// # Returns
/// The total storage cost in lamports.
pub fn calculate_total_storage_cost(num_shards: u16) -> u64 {
    let factory_rent = get_factory_rent_exemption();
    let shard_rent = get_shard_rent_exemption();

    factory_rent + (shard_rent * num_shards as u64)
}

/// Calculate optimal number of shards based on expected pools
/// This function calculates the optimal number of shards needed
/// based on the expected number of pools. It ensures that the number of pools
/// does not exceed the maximum allowed per shard.
/// # Arguments
/// * `expected_pools` - The expected number of pools to be managed.
/// # Returns
/// The optimal number of shards required.
pub fn calculate_optimal_shards(expected_pools: u32) -> u16 {
    let pools_per_shard = MAX_POOLS_PER_SHARD as u32;
    ((expected_pools + pools_per_shard - 1) / pools_per_shard) as u16
}

/// Estimate gas costs for operations
/// This function provides conservative estimates for the computational units
/// required for various factory operations. These estimates can be adjusted
/// based on actual performance metrics and network conditions.
pub fn estimate_factory_init_cu() -> u32 {
    5000 // Conservative estimate
}

/// Estimate gas costs for shard initialization
/// This function provides a conservative estimate for the computational units
/// required to initialize a shard within the factory.
/// This can be adjusted based on actual performance metrics and network conditions.
pub fn estimate_shard_init_cu() -> u32 {
    3000 // Conservative estimate
}

/// Estimate gas costs for pool creation
/// This function provides a conservative estimate for the computational units
/// required to create a new pool within the factory.
/// This can be adjusted based on actual performance metrics and network conditions.
pub fn estimate_pool_creation_cu() -> u32 {
    2000 // Conservative estimate
}

/// Advanced factory management traits for extensibility
/// This trait defines methods for managing factory operations,
pub trait FactoryManager {
    fn validate_fee_tier(&self, fee_tier: u32) -> bool;
    fn get_optimal_shard(&self) -> Option<u16>;
    fn calculate_fees(&self, amount: Q64x64) -> Result<u64>;
    fn is_operational(&self) -> bool;
    fn get_utilization(&self) -> u8;
}

/// Factory management implementation
impl FactoryManager for Factory {
    /// Validate fee tier against supported tiers
    fn validate_fee_tier(&self, fee_tier: u32) -> bool {
        self.is_fee_tier_supported(fee_tier)
    }

    /// Get optimal shard index for new pool
    fn get_optimal_shard(&self) -> Option<u16> {
        self.get_optimal_shard_index()
    }

    /// Calculate fees based on the provided amount
    fn calculate_fees(&self, amount: Q64x64) -> Result<u64> {
        self.calculate_protocol_fee(amount)
    }

    /// Check if factory is operational
    fn is_operational(&self) -> bool {
        self.is_operational()
    }

    /// Get utilization percentage of the factory
    fn get_utilization(&self) -> u8 {
        if self.shard_count == 0 || self.max_pools_per_shard == 0 {
            return 0;
        }

        let total_capacity = self.shard_count as u32 * self.max_pools_per_shard as u32;
        ((self.pool_count * 100) / total_capacity) as u8
    }
}

/// Shard management traits for extensibility
/// This trait defines methods for managing shard operations,
/// including efficiency scoring, rebalancing, and health status.
pub trait ShardManager {
    fn get_efficiency_score(&self) -> u8;
    fn should_rebalance(&self) -> bool;
    fn get_health_status(&self) -> ShardHealth;
}

/// Shard health status enumeration
/// This enumeration defines the health status of a shard,
/// including healthy, degraded, and critical states.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShardHealth {
    Healthy,
    Degraded,
    Critical,
}

/// Shard management implementation
impl ShardManager for FactoryShard {
    /// Calculate efficiency score based on utilization
    /// This function calculates the efficiency score of the shard based on its utilization percentage.
    /// The score is determined by the utilization range:
    /// - 70-90%: 100 points (optimal)
    /// - 60-69% or 91-95%: 80 points (good)
    /// - 50-59% or 96-98%: 60 points (fair)
    /// - Below 50% or above 98%: 40 points (poor)
    /// # Returns
    /// The efficiency score as a u8 value.
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

    /// Determine if the shard should be rebalanced
    /// This function checks if the shard's utilization percentage exceeds 95%.
    /// If it does, the shard is considered for rebalancing to ensure optimal performance.
    /// # Returns
    /// A boolean indicating whether the shard should be rebalanced.
    fn should_rebalance(&self) -> bool {
        self.utilization_percentage() > 95
    }

    /// Get the health status of the shard based on its utilization percentage
    /// This function categorizes the shard's health status into three levels:
    /// - Healthy: Utilization is 0-95%
    /// - Degraded: Utilization is 96-99%
    /// - Critical: Utilization is 100% or above
    /// # Returns
    /// The health status as a ShardHealth enum value.
    fn get_health_status(&self) -> ShardHealth {
        match self.utilization_percentage() {
            0..=95 => ShardHealth::Healthy,
            96..=99 => ShardHealth::Degraded,
            100 => ShardHealth::Critical,
            _ => ShardHealth::Critical,
        }
    }
}
