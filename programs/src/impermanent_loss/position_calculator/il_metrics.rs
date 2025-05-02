use super::il_estimator::ImpermanentLossEstimator;
use super::types::{ModelParameters, OptimalPosition, PriceBoundary, SimulationParameters};
use std::collections::HashMap;

/// Represents a historical event in the impermanent loss timeline
#[derive(Debug, Clone)]
pub struct ILEvent {
    /// Timestamp when the event occurred
    pub timestamp: i64,

    /// Type of event (e.g., price change, rebalance)
    pub event_type: ILEventType,

    /// IL value at this point in time
    pub il_value: f64,

    /// IL percentage at this point in time
    pub il_percentage: f64,

    /// Optional metadata related to the event
    pub metadata: Option<HashMap<String, String>>,
}

/// Types of impermanent loss events
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ILEventType {
    /// Initial position creation
    PositionCreated,

    /// Position boundaries adjusted through rebalancing
    Rebalanced,

    /// Significant price movement
    PriceMove,

    /// Position closed
    PositionClosed,
}

/// Comprehensive impermanent loss metrics for a position
#[derive(Debug, Clone)]
pub struct ILMetrics {
    /// Current impermanent loss value in token units
    pub current_il_value: u64,

    /// Current impermanent loss as a percentage
    pub current_il_percentage: f64,

    /// IL that would have occurred without rebalancing
    pub projected_il_without_rebalancing: f64,

    /// Total IL saved through rebalancing
    pub total_il_saved: u64,

    /// IL saved as a percentage of position value
    pub il_saved_percentage: f64,

    /// Fee income earned to offset IL
    pub fee_income: u64,

    /// Net position performance (fees earned minus IL)
    pub net_performance: i64,

    /// Time-weighted average IL
    pub time_weighted_avg_il: f64,

    /// Maximum IL experienced
    pub max_il_percentage: f64,

    /// History of IL events
    pub il_history: Vec<ILEvent>,

    /// Predicted IL range for upcoming period
    pub il_forecast: ILForecast,
}

/// Forecast of potential future IL based on price scenarios
#[derive(Debug, Clone)]
pub struct ILForecast {
    /// Expected IL in baseline scenario
    pub expected_il: f64,

    /// Worst-case IL (e.g., 95th percentile)
    pub worst_case_il: f64,

    /// Best-case IL (e.g., 5th percentile)
    pub best_case_il: f64,

    /// Estimated probability that IL decreases
    pub probability_of_decrease: f64,

    /// Recommended next rebalance time
    pub recommended_rebalance_time: i64,
}

/// Comparative IL metrics to understand relative performance
#[derive(Debug, Clone)]
pub struct ComparativeILMetrics {
    /// IL in current position with Fluxa rebalancing
    pub fluxa_position_il: f64,

    /// IL with traditional AMM (e.g., Uniswap v2)
    pub traditional_amm_il: f64,

    /// IL with concentrated liquidity but no rebalancing (e.g., Uniswap v3)
    pub concentrated_no_rebalance_il: f64,

    /// Percentage reduction vs traditional AMM
    pub reduction_vs_traditional: f64,

    /// Percentage reduction vs concentrated without rebalancing
    pub reduction_vs_concentrated: f64,
}

/// Input parameters for IL metrics calculation
#[derive(Debug, Clone)]
pub struct ILMetricsParams {
    /// Current price of the asset
    pub current_price: f64,

    /// Entry price when position was created
    pub entry_price: f64,

    /// Current price boundaries of the position
    pub current_boundaries: PriceBoundary,

    /// Original price boundaries when position was created
    pub original_boundaries: PriceBoundary,

    /// Current position value in USD
    pub position_value_usd: f64,

    /// Total fees earned by the position
    pub total_fees_earned: u64,

    /// History of rebalancing events
    pub rebalance_history: Vec<ILEvent>,

    /// Current volatility of the asset
    pub volatility: f64,
}

/// IL Metrics Calculator module to generate comprehensive IL statistics
#[derive(Debug, Default)]
pub struct ILMetricsCalculator {
    il_estimator: ImpermanentLossEstimator,
    simulation_parameters: SimulationParameters,
}

impl ILMetricsCalculator {
    /// Create a new IL metrics calculator
    pub fn new(
        il_estimator: ImpermanentLossEstimator,
        simulation_parameters: SimulationParameters,
    ) -> Self {
        Self {
            il_estimator,
            simulation_parameters,
        }
    }

    /// Calculate comprehensive IL metrics for a position
    pub fn calculate_metrics(&self, params: &ILMetricsParams) -> ILMetrics {
        // Calculate current IL
        let current_il_percentage = self.calculate_current_il(
            params.current_price,
            params.entry_price,
            &params.current_boundaries,
        );

        // Calculate IL value in token units
        let current_il_value =
            self.to_token_amount(current_il_percentage, params.position_value_usd);

        // Calculate IL that would have occurred without rebalancing
        let projected_il = self.calculate_projected_il(
            params.current_price,
            params.entry_price,
            &params.original_boundaries,
        );

        // Calculate IL saved through rebalancing
        let il_saved_percentage = projected_il - current_il_percentage;
        let total_il_saved = self.to_token_amount(il_saved_percentage, params.position_value_usd);

        // Net performance calculation
        let net_performance = params.total_fees_earned as i64 - current_il_value as i64;

        // Calculate time-weighted average IL
        let time_weighted_avg_il = self.calculate_time_weighted_il(&params.rebalance_history);

        // Find maximum IL experienced
        let max_il_percentage = self.find_max_il(&params.rebalance_history);

        // Generate IL forecast
        let il_forecast = self.generate_il_forecast(
            params.current_price,
            &params.current_boundaries,
            params.volatility,
        );

        ILMetrics {
            current_il_value,
            current_il_percentage,
            projected_il_without_rebalancing: projected_il,
            total_il_saved,
            il_saved_percentage,
            fee_income: params.total_fees_earned,
            net_performance,
            time_weighted_avg_il,
            max_il_percentage,
            il_history: params.rebalance_history.clone(),
            il_forecast,
        }
    }

    // Keep the old method signature for backward compatibility, but delegate to the new implementation
    #[deprecated(
        since = "1.1.0",
        note = "Use calculate_metrics with ILMetricsParams instead"
    )]
    #[allow(clippy::too_many_arguments)]
    pub fn calculate_metrics_old(
        &self,
        current_price: f64,
        entry_price: f64,
        current_boundaries: &PriceBoundary,
        original_boundaries: &PriceBoundary,
        position_value_usd: f64,
        total_fees_earned: u64,
        rebalance_history: &[ILEvent],
        volatility: f64,
    ) -> ILMetrics {
        let params = ILMetricsParams {
            current_price,
            entry_price,
            current_boundaries: current_boundaries.clone(),
            original_boundaries: original_boundaries.clone(),
            position_value_usd,
            total_fees_earned,
            rebalance_history: rebalance_history.to_vec(),
            volatility,
        };

        self.calculate_metrics(&params)
    }

    /// Calculate comparative IL metrics against other AMM strategies
    pub fn calculate_comparative_metrics(
        &self,
        current_price: f64,
        entry_price: f64,
        current_boundaries: &PriceBoundary,
    ) -> ComparativeILMetrics {
        // Calculate IL for this position with Fluxa rebalancing
        let fluxa_il = self.calculate_current_il(current_price, entry_price, current_boundaries);

        // Calculate IL for traditional AMM (constant product)
        let traditional_il = self.calculate_traditional_amm_il(current_price, entry_price);

        // Calculate IL for concentrated liquidity without rebalancing
        let concentrated_il = self.calculate_concentrated_il_no_rebalance(
            current_price,
            entry_price,
            current_boundaries,
        );

        // Calculate reduction percentages
        let reduction_vs_traditional = if traditional_il != 0.0 {
            (traditional_il - fluxa_il) / traditional_il.abs()
        } else {
            0.0
        };

        let reduction_vs_concentrated = if concentrated_il != 0.0 {
            (concentrated_il - fluxa_il) / concentrated_il.abs()
        } else {
            0.0
        };

        ComparativeILMetrics {
            fluxa_position_il: fluxa_il,
            traditional_amm_il: traditional_il,
            concentrated_no_rebalance_il: concentrated_il,
            reduction_vs_traditional,
            reduction_vs_concentrated,
        }
    }

    /// Calculate current impermanent loss for a position
    fn calculate_current_il(
        &self,
        current_price: f64,
        entry_price: f64,
        boundaries: &PriceBoundary,
    ) -> f64 {
        // Price change factor from entry to current
        let price_change_factor = current_price / entry_price;

        if current_price < boundaries.lower_price || current_price > boundaries.upper_price {
            // Price is outside the range
            // IL is more complex when price is outside the range
            let effective_price_factor = if current_price < boundaries.lower_price {
                boundaries.lower_price / entry_price
            } else {
                boundaries.upper_price / entry_price
            };

            self.il_estimator
                .calculate_il_for_price_change(effective_price_factor)
        } else {
            // Price is within the range, use the standard IL formula
            self.il_estimator
                .calculate_il_for_price_change(price_change_factor)
        }
    }

    /// Calculate projected IL without rebalancing
    fn calculate_projected_il(
        &self,
        current_price: f64,
        entry_price: f64,
        original_boundaries: &PriceBoundary,
    ) -> f64 {
        // Similar to current IL, but using original boundaries
        let price_change_factor = current_price / entry_price;

        if current_price < original_boundaries.lower_price
            || current_price > original_boundaries.upper_price
        {
            // Price moved outside the original range
            let effective_price_factor = if current_price < original_boundaries.lower_price {
                original_boundaries.lower_price / entry_price
            } else {
                original_boundaries.upper_price / entry_price
            };

            self.il_estimator
                .calculate_il_for_price_change(effective_price_factor)
        } else {
            // Price is within the original range
            self.il_estimator
                .calculate_il_for_price_change(price_change_factor)
        }
    }

    /// Calculate IL for traditional AMM (constant product)
    fn calculate_traditional_amm_il(&self, current_price: f64, entry_price: f64) -> f64 {
        let price_change_factor = current_price / entry_price;
        self.il_estimator
            .calculate_il_for_price_change(price_change_factor)
    }

    /// Calculate IL for concentrated liquidity without rebalancing
    fn calculate_concentrated_il_no_rebalance(
        &self,
        current_price: f64,
        entry_price: f64,
        original_boundaries: &PriceBoundary,
    ) -> f64 {
        // Create a concentrated liquidity scenario with the original boundaries
        self.calculate_projected_il(current_price, entry_price, original_boundaries)
    }

    /// Generate a forecast of future IL based on simulations
    fn generate_il_forecast(
        &self,
        current_price: f64,
        current_boundaries: &PriceBoundary,
        volatility: f64,
    ) -> ILForecast {
        // Updated simulation parameters with current volatility
        let mut sim_params = self.simulation_parameters;
        sim_params.volatility = volatility;

        // Regenerate price movement weights based on simulation parameters
        let mut estimator = self.il_estimator.clone();
        estimator.generate_weights_from_simulation(&sim_params);

        // Calculate expected IL
        let expected_il = estimator.calculate_concentrated_il(current_boundaries, volatility);

        // Calculate worst and best case scenarios (simplified)
        let worst_case_volatility = volatility * 1.5;
        let best_case_volatility = volatility * 0.5;

        let worst_case_il = expected_il * 1.5;
        let best_case_il = expected_il * 0.5;

        // Estimate probability of IL decrease
        // This is a simplified model - in reality would be based on much more sophisticated forecasting
        let probability_of_decrease = if volatility > 0.3 {
            // High volatility leads to higher probability of IL decrease
            0.7
        } else {
            // Lower volatility means more stable IL
            0.4
        };

        // Recommend rebalance timing
        // Higher volatility = more frequent rebalancing
        let rebalance_interval = if volatility > 0.3 {
            60 * 60 * 24 * 2 // 2 days
        } else if volatility > 0.15 {
            60 * 60 * 24 * 5 // 5 days
        } else {
            60 * 60 * 24 * 10 // 10 days
        };

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let recommended_rebalance_time = current_time + rebalance_interval;

        ILForecast {
            expected_il,
            worst_case_il,
            best_case_il,
            probability_of_decrease,
            recommended_rebalance_time,
        }
    }

    /// Calculate time-weighted average IL from historical events
    fn calculate_time_weighted_il(&self, il_history: &[ILEvent]) -> f64 {
        if il_history.is_empty() {
            return 0.0;
        }

        // Calculate time-weighted average
        let mut total_weighted_il = 0.0;
        let mut total_time = 0;

        for i in 0..il_history.len() - 1 {
            let current = &il_history[i];
            let next = &il_history[i + 1];

            let time_period = (next.timestamp - current.timestamp) as u64;
            total_weighted_il += current.il_percentage * time_period as f64;
            total_time += time_period;
        }

        // Add the most recent period
        if let Some(last) = il_history.last() {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let time_period = (current_time - last.timestamp) as u64;
            total_weighted_il += last.il_percentage * time_period as f64;
            total_time += time_period;
        }

        if total_time > 0 {
            total_weighted_il / total_time as f64
        } else {
            0.0
        }
    }

    /// Find maximum IL experienced
    fn find_max_il(&self, il_history: &[ILEvent]) -> f64 {
        il_history
            .iter()
            .map(|event| event.il_percentage)
            .fold(0.0, f64::max)
    }

    /// Convert an IL percentage to a token amount
    fn to_token_amount(&self, il_percentage: f64, position_value_usd: f64) -> u64 {
        (il_percentage * position_value_usd) as u64
    }
}
