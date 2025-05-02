use crate::adaptive_threshold::types::PoolCharacteristics;
use crate::impermanent_loss::position_calculator::{
    boundary_calculator::BoundaryCalculator,
    position_optimizer::PositionOptimizer,
    types::{OptimalPosition, RebalanceResult, RebalanceStrategy},
};
use crate::ErrorCode;
use anchor_lang::prelude::*;

/// RebalanceExecutor handles the execution of position rebalancing operations
/// based on optimal boundaries calculated by the PositionOptimizer.
#[derive(Debug)]
pub struct RebalanceExecutor {
    /// Optimizer used to determine optimal position boundaries
    optimizer: PositionOptimizer,
    /// Configuration for rebalancing operations
    config: RebalanceConfig,
}

/// Configuration parameters for rebalancing operations
#[derive(Debug, Clone)]
pub struct RebalanceConfig {
    /// Minimum IL threshold to trigger rebalancing (as a decimal, e.g., 0.01 = 1%)
    pub min_il_threshold: f64,
    /// Minimum price change percentage to consider rebalancing
    pub min_price_change_pct: f64,
    /// Cooldown period between rebalances (in seconds)
    pub cooldown_period: i64,
    /// Maximum gas cost willing to pay for rebalance (in SOL)
    pub max_gas_cost: u64,
    /// Minimum benefit-to-cost ratio to execute rebalance
    pub min_benefit_cost_ratio: f64,
    /// Preferred rebalance strategy (can be overridden by market conditions)
    pub default_strategy: RebalanceStrategy,
}

impl Default for RebalanceConfig {
    fn default() -> Self {
        Self {
            min_il_threshold: 0.01,      // 1%
            min_price_change_pct: 0.05,  // 5%
            cooldown_period: 86400,      // 24 hours
            max_gas_cost: 10_000_000,    // 0.01 SOL (in lamports)
            min_benefit_cost_ratio: 2.0, // Benefit should be at least 2x the cost
            default_strategy: RebalanceStrategy::Standard,
        }
    }
}

impl RebalanceExecutor {
    /// Create a new RebalanceExecutor with default configuration
    pub fn new() -> Self {
        Self {
            optimizer: PositionOptimizer::default(),
            config: RebalanceConfig::default(),
        }
    }

    /// Create a new RebalanceExecutor with custom optimizer and configuration
    pub fn new_with_config(optimizer: PositionOptimizer, config: RebalanceConfig) -> Self {
        Self { optimizer, config }
    }

    /// Calculate whether a position should be rebalanced based on current market conditions
    /// Returns a RebalanceResult with details about the decision
    pub fn should_rebalance(
        &mut self,
        current_price: f64,
        previous_price: f64,
        position_lower_price: f64,
        position_upper_price: f64,
        last_rebalance_time: i64,
        pool_characteristics: &PoolCharacteristics,
        position_value_usd: f64,
    ) -> Result<RebalanceResult> {
        // Check cooldown period
        let current_time = Clock::get()?.unix_timestamp;
        let time_since_last_rebalance = current_time - last_rebalance_time;

        if time_since_last_rebalance < self.config.cooldown_period {
            return Ok(RebalanceResult {
                should_rebalance: false,
                reason: "Cooldown period not met".to_string(),
                optimal_position: None,
                estimated_il_saved: 0.0,
                strategy: RebalanceStrategy::None,
            });
        }

        // Check price movement threshold
        let price_change_pct = (current_price - previous_price).abs() / previous_price;
        if price_change_pct < self.config.min_price_change_pct {
            return Ok(RebalanceResult {
                should_rebalance: false,
                reason: "Price movement below threshold".to_string(),
                optimal_position: None,
                estimated_il_saved: 0.0,
                strategy: RebalanceStrategy::None,
            });
        }

        // Check if current price is near boundaries
        let range_width = position_upper_price - position_lower_price;
        let upper_proximity = (position_upper_price - current_price) / range_width;
        let lower_proximity = (current_price - position_lower_price) / range_width;

        let is_near_boundary = upper_proximity < 0.1 || lower_proximity < 0.1;

        // Calculate optimal position
        let optimal_position = self.optimizer.optimize_position(
            current_price,
            pool_characteristics,
            position_value_usd,
        );

        // Calculate current IL and potential IL reduction
        let current_boundaries = (position_lower_price, position_upper_price);
        let optimal_boundaries = optimal_position.boundaries.clone();

        let il_reduction = calculate_il_improvement(
            current_boundaries,
            optimal_boundaries.as_tuple(),
            current_price,
            pool_characteristics,
            position_value_usd,
        );

        // Determine rebalance strategy based on conditions
        let strategy = self.select_rebalance_strategy(
            current_price,
            price_change_pct,
            is_near_boundary,
            il_reduction,
            pool_characteristics,
        );

        // Calculate gas costs and compare with expected benefits
        let estimated_gas_cost = self.estimate_gas_cost(strategy);
        let estimated_benefit = il_reduction * position_value_usd;
        let benefit_cost_ratio = estimated_benefit / estimated_gas_cost as f64;

        let should_rebalance = il_reduction > self.config.min_il_threshold
            && benefit_cost_ratio > self.config.min_benefit_cost_ratio;

        let reason = if should_rebalance {
            format!(
                "IL reduction: {:.2}%, benefit/cost: {:.2}",
                il_reduction * 100.0,
                benefit_cost_ratio
            )
        } else if il_reduction <= self.config.min_il_threshold {
            "IL reduction below threshold".to_string()
        } else {
            "Benefit does not justify cost".to_string()
        };

        Ok(RebalanceResult {
            should_rebalance,
            reason,
            optimal_position: Some(optimal_position),
            estimated_il_saved: il_reduction,
            strategy: if should_rebalance {
                strategy
            } else {
                RebalanceStrategy::None
            },
        })
    }

    /// Select the appropriate rebalancing strategy based on market conditions
    fn select_rebalance_strategy(
        &self,
        current_price: f64,
        price_change_pct: f64,
        is_near_boundary: bool,
        il_reduction: f64,
        pool_characteristics: &PoolCharacteristics,
    ) -> RebalanceStrategy {
        // For high volatility or large price movements, use a wider range
        if pool_characteristics.volatility_24h > 0.5 || price_change_pct > 0.15 {
            return RebalanceStrategy::WidenRange;
        }

        // If price is near boundary but not highly volatile, shift the range
        if is_near_boundary && pool_characteristics.volatility_24h < 0.3 {
            return RebalanceStrategy::ShiftRange;
        }

        // For substantial IL but moderate conditions, use a balanced approach
        if il_reduction > 0.05 {
            return RebalanceStrategy::Balanced;
        }

        // Default strategy from config
        self.config.default_strategy.clone()
    }

    /// Estimate gas cost for executing a rebalance operation
    fn estimate_gas_cost(&self, strategy: RebalanceStrategy) -> u64 {
        // Gas costs will vary by strategy
        match strategy {
            RebalanceStrategy::WidenRange => 8_000_000, // Higher cost due to potential liquidity movement
            RebalanceStrategy::ShiftRange => 9_000_000, // Higher cost due to more complex operations
            RebalanceStrategy::Balanced => 7_500_000,   // Medium cost
            RebalanceStrategy::Standard => 6_000_000,   // Standard cost
            RebalanceStrategy::None => 0,               // No cost if not rebalancing
        }
    }

    /// Execute a rebalance operation with given strategy
    /// Returns the estimated IL saved and other execution data
    pub fn execute_rebalance(
        &self,
        rebalance_result: &RebalanceResult,
        position_id: Pubkey,
        // Additional parameters needed for CPI calls would be added here
    ) -> Result<()> {
        // Validate that rebalancing is actually needed
        if !rebalance_result.should_rebalance
            || rebalance_result.strategy == RebalanceStrategy::None
        {
            return Err(ErrorCode::NoRebalanceNeeded.into());
        }

        // Extract optimal boundaries from result
        let optimal_position = rebalance_result
            .optimal_position
            .as_ref()
            .ok_or(ErrorCode::OptimizationFailure)?;

        // The actual rebalance would involve:
        // 1. Calling into the AMM Core to modify position boundaries
        // 2. Collecting any accrued fees
        // 3. Withdrawing liquidity from current position
        // 4. Creating new position with optimal boundaries
        // 5. Tracking the IL saved

        // For this implementation, we're focusing on the framework
        // Actual CPI calls would be implemented based on the AMM Core interface

        // Example placeholder for integrating with execute_rebalance instruction
        // execute_rebalance_cpi(
        //     position_id,
        //     optimal_boundaries.lower_tick_index,
        //     optimal_boundaries.upper_tick_index,
        //     rebalance_result.strategy
        // )?;

        Ok(())
    }

    /// Update the rebalance configuration
    pub fn update_config(&mut self, new_config: RebalanceConfig) {
        self.config = new_config;
    }

    /// Get the current rebalance configuration
    pub fn config(&self) -> &RebalanceConfig {
        &self.config
    }

    /// Access the position optimizer
    pub fn optimizer(&self) -> &PositionOptimizer {
        &self.optimizer
    }

    /// Access the position optimizer mutably
    pub fn optimizer_mut(&mut self) -> &mut PositionOptimizer {
        &mut self.optimizer
    }
}

/// Calculate the improvement in impermanent loss between current and optimal boundaries
fn calculate_il_improvement(
    current_boundaries: (f64, f64),
    optimal_boundaries: (f64, f64),
    current_price: f64,
    pool_characteristics: &PoolCharacteristics,
    position_value_usd: f64,
) -> f64 {
    // This is a simplified model for the implementation
    // A more sophisticated implementation would calculate exact IL differences

    // Wider ranges generally reduce IL
    let current_width = current_boundaries.1 / current_boundaries.0;
    let optimal_width = optimal_boundaries.1 / optimal_boundaries.0;

    // Calculate how centered each range is on the current price
    let current_center = (current_boundaries.0 * current_boundaries.1).sqrt();
    let optimal_center = (optimal_boundaries.0 * optimal_boundaries.1).sqrt();

    let current_price_ratio = current_price / current_center;
    let optimal_price_ratio = current_price / optimal_center;

    // Deviation from geometric center increases IL
    let current_deviation = (current_price_ratio - 1.0).abs();
    let optimal_deviation = (optimal_price_ratio - 1.0).abs();

    // Calculate estimated IL improvement
    // Base formula: IL reduction from width increase and better centering
    let width_improvement = (optimal_width / current_width) - 1.0;
    let centering_improvement = current_deviation - optimal_deviation;

    // Weight factors (can be tuned for accuracy)
    let width_weight = 0.6;
    let centering_weight = 0.4;

    // Combined improvement factor
    let improvement_factor =
        width_weight * width_improvement + centering_weight * centering_improvement;

    // Scale by volatility (higher volatility means more IL can be saved)
    let volatility_scale = 0.5 + pool_characteristics.volatility_24h;

    // Final estimated IL reduction (capped between 0-0.5 or 0-50%)
    (improvement_factor * volatility_scale).max(0.0).min(0.5)
}
