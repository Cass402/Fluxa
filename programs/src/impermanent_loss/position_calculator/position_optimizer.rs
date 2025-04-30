use super::boundary_calculator::BoundaryCalculator;
use super::types::{ModelParameters, OptimalPosition};
use crate::adaptive_threshold::threshold_manager::ThresholdManager;
use crate::adaptive_threshold::types::PoolCharacteristics;
use crate::volatility_detection::types::PriceHistory;
use crate::volatility_detection::volatility_calculator::{
    StandardVolatilityCalculator, VolatilityCalculator,
};

/// Position Optimizer combines all IL mitigation components to calculate
/// optimal liquidity positions that balance impermanent loss risk and fee generation
#[derive(Debug)]
pub struct PositionOptimizer {
    volatility_calculator: Box<dyn VolatilityCalculator>,
    threshold_manager: ThresholdManager,
    boundary_calculator: BoundaryCalculator,
}

impl PositionOptimizer {
    /// Create a new position optimizer with custom components
    pub fn new(
        volatility_calculator: Box<dyn VolatilityCalculator>,
        threshold_manager: ThresholdManager,
        boundary_calculator: BoundaryCalculator,
    ) -> Self {
        Self {
            volatility_calculator,
            threshold_manager,
            boundary_calculator,
        }
    }

    /// Calculate optimal position for a liquidity provider based on current market conditions
    /// This is the main entry point for IL mitigation optimization
    pub fn calculate_optimal_position(
        &mut self,
        current_price: f64,
        pool_characteristics: &PoolCharacteristics,
        position_value_usd: f64,
    ) -> OptimalPosition {
        // Step 1: Calculate volatility using the volatility detection module
        let price_history = &PriceHistory::new(100); // Create empty price history or get from somewhere
        let volatility_score = self
            .volatility_calculator
            .calculate_volatility(price_history, None);
        let volatility = volatility_score.value;

        // Step 2: Calculate adaptive threshold using the threshold manager
        let adaptive_threshold = self
            .threshold_manager
            .calculate_adaptive_threshold(pool_characteristics, volatility);

        // Step 3: Calculate optimal position boundaries using boundary calculator
        let boundaries = self.boundary_calculator.calculate_boundaries(
            current_price,
            volatility,
            &adaptive_threshold,
        );

        // Step 4: Calculate estimated IL reduction
        let il_reduction = self
            .boundary_calculator
            .il_estimator()
            .calculate_il_reduction(&boundaries, volatility);

        // Step 5: Calculate estimated fee APY
        let fee_apy = self.boundary_calculator.il_estimator().estimate_fee_apy(
            &boundaries,
            self.boundary_calculator.model_parameters().fee_tier,
            pool_characteristics.trading_volume_24h,
            pool_characteristics.liquidity_depth,
            position_value_usd,
        );

        // Step 6: Calculate net estimated APY (fees minus IL)
        // Assuming 10% annualized IL for a wide position as baseline
        let base_il_cost = 0.10;
        let mitigated_il_cost = base_il_cost * (1.0 - il_reduction);
        let net_estimated_apy = fee_apy - mitigated_il_cost;

        // Step 7: Determine optimal rebalance frequency
        let rebalance_frequency = self
            .boundary_calculator
            .calculate_rebalance_frequency(volatility, &adaptive_threshold);

        OptimalPosition {
            boundaries,
            estimated_il_reduction: il_reduction,
            estimated_fee_apy: fee_apy,
            net_estimated_apy,
            recommended_rebalance_frequency: rebalance_frequency,
        }
    }

    /// Find the optimal position by testing multiple strategies and selecting the best
    pub fn optimize_position(
        &mut self,
        current_price: f64,
        pool_characteristics: &PoolCharacteristics,
        position_value_usd: f64,
    ) -> OptimalPosition {
        // Calculate volatility
        let price_history = &PriceHistory::new(100); // Create empty price history or get from somewhere
        let volatility_score = self
            .volatility_calculator
            .calculate_volatility(price_history, None);
        let volatility = volatility_score.value;

        // Calculate optimal boundaries with different strategies, then pick the best one
        let boundaries = self.boundary_calculator.find_optimal_boundary(
            current_price,
            volatility,
            pool_characteristics.trading_volume_24h,
            pool_characteristics.liquidity_depth,
            position_value_usd,
        );

        // Calculate metrics for the optimal boundaries
        let il_reduction = self
            .boundary_calculator
            .il_estimator()
            .calculate_il_reduction(&boundaries, volatility);

        let fee_apy = self.boundary_calculator.il_estimator().estimate_fee_apy(
            &boundaries,
            self.boundary_calculator.model_parameters().fee_tier,
            pool_characteristics.trading_volume_24h,
            pool_characteristics.liquidity_depth,
            position_value_usd,
        );

        // Calculate net estimated APY
        let base_il_cost = 0.10;
        let mitigated_il_cost = base_il_cost * (1.0 - il_reduction);
        let net_estimated_apy = fee_apy - mitigated_il_cost;

        // Calculate adaptive threshold for rebalance frequency calculation
        let adaptive_threshold = self
            .threshold_manager
            .calculate_adaptive_threshold(pool_characteristics, volatility);

        // Calculate rebalance frequency
        let rebalance_frequency = self
            .boundary_calculator
            .calculate_rebalance_frequency(volatility, &adaptive_threshold);

        OptimalPosition {
            boundaries,
            estimated_il_reduction: il_reduction,
            estimated_fee_apy: fee_apy,
            net_estimated_apy,
            recommended_rebalance_frequency: rebalance_frequency,
        }
    }

    /// Check if a position should be rebalanced based on current price and adaptive threshold
    pub fn should_rebalance(
        &mut self,
        current_price: f64,
        previous_price: f64,
        pool_characteristics: &PoolCharacteristics,
    ) -> bool {
        // Calculate volatility
        let price_history = &PriceHistory::new(100); // Create empty price history or get from somewhere
        let volatility_score = self
            .volatility_calculator
            .calculate_volatility(price_history, None);
        let volatility = volatility_score.value;

        // Calculate adaptive threshold
        let adaptive_threshold = self
            .threshold_manager
            .calculate_adaptive_threshold(pool_characteristics, volatility);

        // Calculate price movement as a percentage
        let price_change_pct = (current_price - previous_price).abs() / previous_price;

        // Check if price movement exceeds rebalance threshold
        price_change_pct >= adaptive_threshold.rebalance_threshold
    }

    /// Access the volatility calculator
    pub fn volatility_calculator(&self) -> &dyn VolatilityCalculator {
        self.volatility_calculator.as_ref()
    }

    /// Access the volatility calculator mutably
    pub fn volatility_calculator_mut(&mut self) -> &mut dyn VolatilityCalculator {
        self.volatility_calculator.as_mut()
    }

    /// Access the threshold manager
    pub fn threshold_manager(&self) -> &ThresholdManager {
        &self.threshold_manager
    }

    /// Access the threshold manager mutably
    pub fn threshold_manager_mut(&mut self) -> &mut ThresholdManager {
        &mut self.threshold_manager
    }

    /// Access the boundary calculator
    pub fn boundary_calculator(&self) -> &BoundaryCalculator {
        &self.boundary_calculator
    }

    /// Access the boundary calculator mutably
    pub fn boundary_calculator_mut(&mut self) -> &mut BoundaryCalculator {
        &mut self.boundary_calculator
    }

    /// Update model parameters for the boundary calculator
    pub fn update_model_parameters(&mut self, params: ModelParameters) {
        self.boundary_calculator.update_model_parameters(params);
    }
}

// Implement the Default trait for PositionOptimizer
impl Default for PositionOptimizer {
    /// Create a default position optimizer
    fn default() -> Self {
        Self {
            volatility_calculator: Box::new(StandardVolatilityCalculator::default())
                as Box<dyn VolatilityCalculator>,
            threshold_manager: ThresholdManager::default(),
            boundary_calculator: BoundaryCalculator::default(),
        }
    }
}
