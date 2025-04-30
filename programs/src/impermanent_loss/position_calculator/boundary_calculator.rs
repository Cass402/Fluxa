use super::il_estimator::ImpermanentLossEstimator;
use super::stochastic_model::StochasticModel;
use super::types::{ModelParameters, PriceBoundary};
use crate::adaptive_threshold::types::AdaptiveThreshold;

/// BoundaryCalculator computes optimal entry and exit boundaries for LP positions
/// to minimize impermanent loss exposure while maximizing fee revenue
#[derive(Debug)]
pub struct BoundaryCalculator {
    stochastic_model: StochasticModel,
    il_estimator: ImpermanentLossEstimator,
    model_parameters: ModelParameters,
}

impl BoundaryCalculator {
    /// Create a new boundary calculator with custom components
    pub fn new(
        stochastic_model: StochasticModel,
        il_estimator: ImpermanentLossEstimator,
        model_parameters: ModelParameters,
    ) -> Self {
        Self {
            stochastic_model,
            il_estimator,
            model_parameters,
        }
    }

    /// Calculate optimal price boundaries based on current price,
    /// volatility, and adaptive threshold parameters
    pub fn calculate_boundaries(
        &mut self,
        current_price: f64,
        volatility: f64,
        adaptive_threshold: &AdaptiveThreshold,
    ) -> PriceBoundary {
        // Calculate baseline price range using stochastic model
        let base_boundary = self.stochastic_model.calculate_price_range(
            current_price,
            volatility,
            adaptive_threshold.confidence,
            None, // Use default time horizon
        );

        // Apply position width factor from adaptive threshold to adjust range
        // Wider ranges reduce IL, but may earn fewer fees
        let position_width_factor = adaptive_threshold.position_width_factor;

        // Calculate range width as percentage of current price
        let base_width_percentage =
            (base_boundary.upper_price - base_boundary.lower_price) / current_price;

        // Apply adaptive width factor
        let adjusted_width_percentage = base_width_percentage * position_width_factor;

        // Calculate new boundaries based on adjusted width
        let half_width = adjusted_width_percentage * current_price / 2.0;
        let lower_price = current_price - half_width;
        let upper_price = current_price + half_width;

        // Ensure lower price is positive
        let lower_price = lower_price.max(current_price * 0.01);

        PriceBoundary {
            lower_price,
            upper_price,
            current_price,
            confidence: base_boundary.confidence,
        }
    }

    /// Find optimal boundary by testing multiple width factors and selecting
    /// the one with the best balance of IL reduction and fee generation
    pub fn find_optimal_boundary(
        &mut self,
        current_price: f64,
        volatility: f64,
        daily_volume_usd: f64,
        pool_tvl_usd: f64,
        position_value_usd: f64,
    ) -> PriceBoundary {
        // Test different width factors
        let width_factors = [0.2, 0.4, 0.6, 0.8, 1.0, 1.2, 1.5, 2.0];
        let mut best_boundary = None;
        let mut best_score = f64::NEG_INFINITY;

        // Get base price range for 90% confidence interval
        let base_boundary = self.stochastic_model.calculate_price_range(
            current_price,
            volatility,
            0.9,  // 90% confidence
            None, // Use default time horizon
        );

        // Test each width factor and calculate score
        for width_factor in width_factors.iter() {
            let boundary = self.apply_width_factor(&base_boundary, *width_factor);

            // Calculate IL reduction for this boundary
            let il_reduction = self
                .il_estimator
                .calculate_il_reduction(&boundary, volatility);

            // Calculate estimated fee APY for this boundary
            let fee_apy = self.il_estimator.estimate_fee_apy(
                &boundary,
                self.model_parameters.fee_tier,
                daily_volume_usd,
                pool_tvl_usd,
                position_value_usd,
            );

            // Calculate net APY (fees minus IL)
            let net_apy = fee_apy - (1.0 - il_reduction) * 0.10; // Assuming 10% annualized IL for a wide position

            // Calculate overall score
            // Higher score is better (balanced between IL reduction and fee generation)
            let score = net_apy + il_reduction * 0.05; // Add small bonus for IL reduction

            if score > best_score {
                best_score = score;
                best_boundary = Some(boundary);
            }
        }

        best_boundary.unwrap_or(base_boundary)
    }

    /// Apply width factor to a base boundary
    fn apply_width_factor(&self, base: &PriceBoundary, factor: f64) -> PriceBoundary {
        let current_price = base.current_price;

        // Calculate range width as percentage of current price
        let base_width_percentage = (base.upper_price - base.lower_price) / current_price;

        // Apply width factor
        let adjusted_width_percentage = base_width_percentage * factor;

        // Calculate new boundaries based on adjusted width
        let half_width = adjusted_width_percentage * current_price / 2.0;
        let lower_price = current_price - half_width;
        let upper_price = current_price + half_width;

        // Ensure lower price is positive
        let lower_price = lower_price.max(current_price * 0.01);

        PriceBoundary {
            lower_price,
            upper_price,
            current_price,
            confidence: base.confidence,
        }
    }

    /// Calculate rebalancing frequency based on volatility and threshold
    pub fn calculate_rebalance_frequency(
        &self,
        volatility: f64,
        adaptive_threshold: &AdaptiveThreshold,
    ) -> u64 {
        // Base rebalancing frequency in minutes
        let base_frequency = 1440; // Once per day (1440 minutes)

        // Scale based on volatility and rebalance threshold
        let frequency_factor = volatility / adaptive_threshold.rebalance_threshold;

        // Calculate minutes between rebalances
        let minutes = (base_frequency as f64 / frequency_factor).round() as u64;

        // Constrain to reasonable limits
        let min_minutes = 60; // At least once per hour
        let max_minutes = 10080; // At most once per week (10080 minutes)

        minutes.max(min_minutes).min(max_minutes)
    }

    /// Access the stochastic model
    pub fn stochastic_model(&self) -> &StochasticModel {
        &self.stochastic_model
    }

    /// Access the stochastic model mutably
    pub fn stochastic_model_mut(&mut self) -> &mut StochasticModel {
        &mut self.stochastic_model
    }

    /// Access the IL estimator
    pub fn il_estimator(&self) -> &ImpermanentLossEstimator {
        &self.il_estimator
    }

    /// Access the IL estimator mutably
    pub fn il_estimator_mut(&mut self) -> &mut ImpermanentLossEstimator {
        &mut self.il_estimator
    }

    /// Get model parameters
    pub fn model_parameters(&self) -> &ModelParameters {
        &self.model_parameters
    }

    /// Update model parameters
    pub fn update_model_parameters(&mut self, params: ModelParameters) {
        self.model_parameters = params;
    }
}

// Implement the Default trait for BoundaryCalculator
impl Default for BoundaryCalculator {
    /// Create a default boundary calculator
    fn default() -> Self {
        Self {
            stochastic_model: StochasticModel::default(),
            il_estimator: ImpermanentLossEstimator::default(),
            model_parameters: ModelParameters::default(),
        }
    }
}
