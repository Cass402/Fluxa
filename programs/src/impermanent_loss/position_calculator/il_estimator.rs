use super::types::{PriceBoundary, SimulationParameters};

/// Impermanent Loss Estimator that calculates expected impermanent loss
/// for liquidity positions with different boundaries
#[derive(Debug, Clone)]
pub struct ImpermanentLossEstimator {
    // Simulation weights for different price movements
    price_movement_weights: Vec<(f64, f64)>, // (price_change_factor, probability_weight)
}

impl ImpermanentLossEstimator {
    /// Create a new IL estimator with custom price movement weights
    pub fn new(price_movement_weights: Vec<(f64, f64)>) -> Self {
        Self {
            price_movement_weights,
        }
    }

    /// Calculate impermanent loss for a specific price change factor
    /// Returns the IL as a fraction (e.g., 0.05 means 5% loss)
    pub fn calculate_il_for_price_change(&self, price_change_factor: f64) -> f64 {
        // Impermanent loss formula:
        // IL = 2 * sqrt(price_change) / (1 + price_change) - 1

        let sqrt_price_change = price_change_factor.sqrt();
        let denominator = 1.0 + price_change_factor;

        let il = 2.0 * sqrt_price_change / denominator - 1.0;

        // IL is always negative or zero, return as positive loss value
        -il
    }

    /// Calculate expected IL based on weighted price movements
    pub fn calculate_expected_il(&self) -> f64 {
        let mut total_weighted_il = 0.0;
        let mut total_weight = 0.0;

        for &(price_factor, weight) in &self.price_movement_weights {
            let il = self.calculate_il_for_price_change(price_factor);
            total_weighted_il += il * weight;
            total_weight += weight;
        }

        // Normalize by total weight
        if total_weight > 0.0 {
            total_weighted_il / total_weight
        } else {
            0.0
        }
    }

    /// Calculate expected IL for a concentrated liquidity position
    /// with specified price boundaries
    pub fn calculate_concentrated_il(&self, boundary: &PriceBoundary, volatility: f64) -> f64 {
        let current_price = boundary.current_price;
        let lower_price = boundary.lower_price;
        let upper_price = boundary.upper_price;

        // For each potential price movement, calculate IL if price remains in range,
        // or calculate IL at boundary if price moves out of range
        let mut total_weighted_il = 0.0;
        let mut total_weight = 0.0;

        for &(price_factor, weight) in &self.price_movement_weights {
            let new_price = current_price * price_factor;

            // Calculate effective price factor for IL calculation
            let effective_price_factor = if new_price < lower_price {
                // Price went below range, use lower bound
                lower_price / current_price
            } else if new_price > upper_price {
                // Price went above range, use upper bound
                upper_price / current_price
            } else {
                // Price is in range
                price_factor
            };

            let il = self.calculate_il_for_price_change(effective_price_factor);

            // If price is outside the range, add penalty for opportunity cost
            let range_penalty = if new_price < lower_price || new_price > upper_price {
                // Apply additional penalty based on how far outside the range
                // and the volatility of the asset
                let distance_factor = if new_price < lower_price {
                    (lower_price / new_price) - 1.0
                } else {
                    (new_price / upper_price) - 1.0
                };

                // Scale penalty with volatility
                0.02 + (distance_factor * volatility * 0.2).min(0.1)
            } else {
                0.0
            };

            let adjusted_il = il + range_penalty;
            total_weighted_il += adjusted_il * weight;
            total_weight += weight;
        }

        // Normalize by total weight
        if total_weight > 0.0 {
            total_weighted_il / total_weight
        } else {
            0.0
        }
    }

    /// Calculate the reduction in IL compared to a full-range position
    pub fn calculate_il_reduction(&self, boundary: &PriceBoundary, volatility: f64) -> f64 {
        // Calculate expected IL for a full range position
        let full_range_il = self.calculate_expected_il();

        // Calculate expected IL for the concentrated position
        let concentrated_il = self.calculate_concentrated_il(boundary, volatility);

        // Calculate reduction as a percentage (can be negative if concentrated IL is higher)
        1.0 - (concentrated_il / full_range_il)
    }

    /// Estimate the annualized fees earned for a position with given boundaries
    /// based on fee tier and estimated trading volume
    pub fn estimate_fee_apy(
        &self,
        boundary: &PriceBoundary,
        fee_tier_bps: u16,
        daily_volume_usd: f64,
        pool_tvl_usd: f64,
        position_value_usd: f64,
    ) -> f64 {
        // Convert fee tier from basis points to percentage
        let fee_percentage = fee_tier_bps as f64 / 10000.0;

        // Calculate price range width as a percentage of full range
        let full_range_width = boundary.current_price * 10.0; // Assuming 10x is "full range"
        let position_range_width = boundary.upper_price - boundary.lower_price;
        let range_concentration = full_range_width / position_range_width;

        // Estimate daily fees for the entire pool
        let daily_fees_pool = daily_volume_usd * fee_percentage;

        // Calculate position's share of the pool
        let position_share = position_value_usd / pool_tvl_usd;

        // Adjust share based on range concentration (narrower ranges get more fees per unit of liquidity)
        let adjusted_position_share = position_share * range_concentration;

        // Calculate estimated daily fees for the position
        let daily_fees_position = daily_fees_pool * adjusted_position_share;

        // Calculate APY (Annual Percentage Yield)
        let annual_fees = daily_fees_position * 365.0;
        let apy = annual_fees / position_value_usd;

        // Apply liquidity utilization adjustment
        // Not all liquidity is utilized all the time, especially at range edges
        let in_range_probability = self.calculate_in_range_probability(boundary);

        apy * in_range_probability
    }

    /// Calculate the probability that the price stays within the position's range
    fn calculate_in_range_probability(&self, boundary: &PriceBoundary) -> f64 {
        let mut in_range_weight = 0.0;
        let mut total_weight = 0.0;

        for &(price_factor, weight) in &self.price_movement_weights {
            let new_price = boundary.current_price * price_factor;

            if new_price >= boundary.lower_price && new_price <= boundary.upper_price {
                in_range_weight += weight;
            }

            total_weight += weight;
        }

        if total_weight > 0.0 {
            in_range_weight / total_weight
        } else {
            0.0
        }
    }

    /// Update the price movement weights
    pub fn update_price_movement_weights(&mut self, weights: Vec<(f64, f64)>) {
        self.price_movement_weights = weights;
    }

    /// Generate price movement weights based on simulation parameters
    pub fn generate_weights_from_simulation(&mut self, params: &SimulationParameters) {
        // Generate weights based on a normal distribution around 1.0
        // with standard deviation derived from volatility
        let mut weights = Vec::new();

        // Convert annual volatility to the simulation timeframe
        let time_factor = params.time_horizon / 365.0;
        let adjusted_volatility = params.volatility * time_factor.sqrt();

        // Create price points at various standard deviations
        let std_points = [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0];

        for &std_dev in &std_points {
            // Calculate price factor (log-normal distribution)
            let price_factor = (std_dev * adjusted_volatility + (params.drift * time_factor)
                - (0.5 * adjusted_volatility * adjusted_volatility))
                .exp();

            // Calculate weight based on normal PDF
            let weight = (-0.5 * std_dev * std_dev).exp();

            weights.push((price_factor, weight));
        }

        // Normalize weights
        let total_weight: f64 = weights.iter().map(|(_, w)| w).sum();

        if total_weight > 0.0 {
            for (_, weight) in &mut weights {
                *weight /= total_weight;
            }
        }

        self.price_movement_weights = weights;
    }
}

// Implement the Default trait for ImpermanentLossEstimator
impl Default for ImpermanentLossEstimator {
    fn default() -> Self {
        // Default weights for different price movements
        // (price_change_factor, probability_weight)
        // These weights form a discretized probability distribution
        let price_movement_weights = vec![
            (0.5, 0.05), // 50% price decrease: 5% probability
            (0.7, 0.10), // 30% price decrease: 10% probability
            (0.8, 0.15), // 20% price decrease: 15% probability
            (0.9, 0.20), // 10% price decrease: 20% probability
            (1.0, 0.25), // No price change: 25% probability
            (1.1, 0.10), // 10% price increase: 10% probability
            (1.2, 0.08), // 20% price increase: 8% probability
            (1.5, 0.05), // 50% price increase: 5% probability
            (2.0, 0.02), // 100% price increase: 2% probability
        ];

        Self::new(price_movement_weights)
    }
}
