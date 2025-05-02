use anchor_lang::prelude::*;
use std::collections::HashMap;

/// ILMetricsCalculator provides advanced impermanent loss analytics
/// including real-time IL calculation, historical analysis, and IL savings metrics.
#[derive(Debug)]
pub struct ILMetricsCalculator {
    config: ILMetricsConfig,
}

/// Configuration for IL metrics calculation
#[derive(Debug, Clone)]
pub struct ILMetricsConfig {
    /// Base price point for historical IL calculation
    pub base_price_reference: ILPriceReference,
    /// Historical time windows to track (in seconds)
    pub time_windows: Vec<u64>,
    /// Maximum number of historical price points to store
    pub max_history_points: usize,
}

/// Determines which price point to use as reference for IL calculations
#[derive(Debug, Clone, PartialEq)]
pub enum ILPriceReference {
    /// Use the price at position creation
    EntryPrice,
    /// Use the geometric mean of position boundaries
    GeometricMean,
    /// Use a time-weighted average price over a period
    TWAP(u64), // Time window in seconds
}

/// Represents a comprehensive set of IL metrics for a position
#[derive(Debug, Clone)]
pub struct ILMetricsReport {
    /// Current IL percentage (negative means loss)
    pub current_il_pct: f64,
    /// IL amount in USD terms
    pub il_value_usd: f64,
    /// IL amount relative to position value (%) over different time windows
    pub historical_il: HashMap<u64, f64>,
    /// IL saved by using Fluxa's mitigation strategies
    pub il_saved_pct: f64,
    /// IL saved in USD terms
    pub il_saved_usd: f64,
    /// Fee income as percentage of position value
    pub fee_income_pct: f64,
    /// Fee income in USD
    pub fee_income_usd: f64,
    /// Net position performance (fees minus IL)
    pub net_performance_pct: f64,
    /// Risk metrics (including volatility exposure)
    pub risk_metrics: ILRiskMetrics,
    /// Projections under different market scenarios
    pub projections: HashMap<String, ILProjection>,
    /// Timestamp when this report was generated
    pub timestamp: i64,
}

/// Risk metrics related to impermanent loss
#[derive(Debug, Clone)]
pub struct ILRiskMetrics {
    /// Volatility exposure score (0-100, higher means more exposed)
    pub vol_exposure: u8,
    /// Price deviation threshold that would trigger significant IL
    pub critical_price_deviation: f64,
    /// Risk classification for this position's IL exposure
    pub risk_classification: ILRiskClass,
    /// IL standard deviation (measure of IL volatility)
    pub il_std_dev: f64,
}

/// Risk classification for IL exposure
#[derive(Debug, Clone, PartialEq)]
pub enum ILRiskClass {
    VeryLow,
    Low,
    Medium,
    High,
    VeryHigh,
}

/// Projected IL under different scenarios
#[derive(Debug, Clone)]
pub struct ILProjection {
    /// Projected IL percentage
    pub projected_il_pct: f64,
    /// Projected IL in USD terms
    pub projected_il_usd: f64,
    /// Projected fee income in USD
    pub projected_fees_usd: f64,
    /// Net projected return (fees - IL)
    pub net_projected_return: f64,
}

/// Price history data point
#[derive(Debug, Clone)]
struct PriceDataPoint {
    timestamp: i64,
    price: f64,
}

impl Default for ILMetricsConfig {
    fn default() -> Self {
        Self {
            base_price_reference: ILPriceReference::GeometricMean,
            time_windows: vec![
                3600,    // 1 hour
                86400,   // 1 day
                604800,  // 1 week
                2592000, // 30 days
            ],
            max_history_points: 1000,
        }
    }
}

impl ILMetricsCalculator {
    /// Create a new IL metrics calculator with default configuration
    pub fn new() -> Self {
        Self {
            config: ILMetricsConfig::default(),
        }
    }

    /// Create a new IL metrics calculator with custom configuration
    pub fn new_with_config(config: ILMetricsConfig) -> Self {
        Self { config }
    }

    /// Calculate current IL for a position
    ///
    /// # Arguments
    /// * `entry_price` - Price when position was opened or last rebalanced
    /// * `current_price` - Current asset price
    /// * `lower_price` - Lower bound of position price range
    /// * `upper_price` - Upper bound of position price range
    /// * `position_value` - Total value of position in USD
    pub fn calculate_current_il(
        &self,
        entry_price: f64,
        current_price: f64,
        lower_price: f64,
        upper_price: f64,
        position_value: f64,
    ) -> (f64, f64) {
        // Calculate base price depending on reference type
        let base_price = match self.config.base_price_reference {
            ILPriceReference::EntryPrice => entry_price,
            ILPriceReference::GeometricMean => (lower_price * upper_price).sqrt(),
            // For TWAP, we would need historical data, but we'll use entry_price for simplicity
            ILPriceReference::TWAP(_) => entry_price,
        };

        // Calculate price ratio
        let price_ratio = current_price / base_price;

        // IL formula: 2 * sqrt(price_ratio) / (1 + price_ratio) - 1
        let il_pct = 2.0 * price_ratio.sqrt() / (1.0 + price_ratio) - 1.0;

        // IL in USD
        let il_usd = position_value * il_pct;

        (il_pct, il_usd)
    }

    /// Generate a comprehensive IL metrics report for a position
    pub fn generate_il_report(
        &self,
        entry_price: f64,
        current_price: f64,
        price_history: &[PriceDataPoint],
        lower_price: f64,
        upper_price: f64,
        position_value: f64,
        fee_earnings: f64,
        rebalance_history: &[(i64, f64, f64)], // (timestamp, old_il, new_il)
    ) -> Result<ILMetricsReport> {
        // Calculate current IL
        let (il_pct, il_usd) = self.calculate_current_il(
            entry_price,
            current_price,
            lower_price,
            upper_price,
            position_value,
        );

        // Calculate historical IL at different time windows
        let mut historical_il = HashMap::new();
        for window in &self.config.time_windows {
            if let Some(history_il) = self.calculate_historical_il(price_history, *window) {
                historical_il.insert(*window, history_il);
            }
        }

        // Calculate IL saved from rebalancing
        let (il_saved_pct, il_saved_usd) =
            self.calculate_il_savings(rebalance_history, position_value);

        // Calculate fee metrics
        let fee_income_pct = fee_earnings / position_value;

        // Calculate net performance
        let net_performance_pct = fee_income_pct + il_pct; // IL is negative, so we add here

        // Calculate risk metrics
        let risk_metrics = self.calculate_risk_metrics(
            lower_price,
            upper_price,
            current_price,
            price_history,
            il_pct,
        );

        // Generate projections
        let projections = self.generate_il_projections(
            current_price,
            lower_price,
            upper_price,
            position_value,
            fee_earnings,
        );

        // Get current timestamp
        let timestamp = Clock::get()?.unix_timestamp;

        Ok(ILMetricsReport {
            current_il_pct: il_pct,
            il_value_usd: il_usd,
            historical_il,
            il_saved_pct,
            il_saved_usd,
            fee_income_pct,
            fee_income_usd: fee_earnings,
            net_performance_pct,
            risk_metrics,
            projections,
            timestamp,
        })
    }

    /// Calculate IL over a historical time window
    fn calculate_historical_il(
        &self,
        price_history: &[PriceDataPoint],
        time_window: u64,
    ) -> Option<f64> {
        if price_history.is_empty() {
            return None;
        }

        // Get current timestamp
        let now = match Clock::get() {
            Ok(clock) => clock.unix_timestamp,
            Err(_) => return None,
        };

        // Determine window start time
        let window_start = now - time_window as i64;

        // Find the closest price point after window start
        let mut window_start_price = None;
        for point in price_history {
            if point.timestamp >= window_start {
                window_start_price = Some(point.price);
                break;
            }
        }

        let start_price = window_start_price?;
        let end_price = price_history.last()?.price;

        // Calculate IL for this window
        let price_ratio = end_price / start_price;
        let il = 2.0 * price_ratio.sqrt() / (1.0 + price_ratio) - 1.0;

        Some(il)
    }

    /// Calculate IL saved from rebalancing operations
    fn calculate_il_savings(
        &self,
        rebalance_history: &[(i64, f64, f64)], // (timestamp, old_il, new_il)
        position_value: f64,
    ) -> (f64, f64) {
        // If no rebalancing history, no savings
        if rebalance_history.is_empty() {
            return (0.0, 0.0);
        }

        // Sum up the improvements from each rebalance
        let mut total_il_saved_pct = 0.0;

        for (_timestamp, old_il, new_il) in rebalance_history {
            // Improvement is the difference between old and new IL
            // Negative IL values mean losses, so we subtract new from old
            let improvement = old_il - new_il;
            total_il_saved_pct += improvement;
        }

        // Calculate dollar value of savings
        let il_saved_usd = total_il_saved_pct * position_value;

        (total_il_saved_pct, il_saved_usd)
    }

    /// Calculate IL risk metrics
    fn calculate_risk_metrics(
        &self,
        lower_price: f64,
        upper_price: f64,
        current_price: f64,
        price_history: &[PriceDataPoint],
        current_il: f64,
    ) -> ILRiskMetrics {
        // Calculate range width
        let range_width = upper_price / lower_price;

        // Calculate position centering (1.0 is perfectly centered)
        let geometric_center = (lower_price * upper_price).sqrt();
        let center_ratio = current_price / geometric_center;
        let centering = (center_ratio - 1.0).abs();

        // Calculate price volatility
        let volatility = self.calculate_price_volatility(price_history);

        // Calculate critical price deviation
        // This is the price change that would cause severe IL (e.g. > 5%)
        let critical_deviation = calculate_critical_price_deviation(range_width);

        // Determine vol exposure score (0-100)
        // Formula: Narrower range + high volatility + poor centering = higher score
        let range_factor = (1.0 / range_width.ln().max(0.1)) * 50.0;
        let volatility_factor = volatility * 100.0;
        let centering_factor = centering * 50.0;

        let vol_exposure_raw = range_factor + volatility_factor + centering_factor;
        let vol_exposure = vol_exposure_raw.min(100.0).max(0.0) as u8;

        // Determine risk classification
        let risk_classification = match vol_exposure {
            0..=20 => ILRiskClass::VeryLow,
            21..=40 => ILRiskClass::Low,
            41..=60 => ILRiskClass::Medium,
            61..=80 => ILRiskClass::High,
            _ => ILRiskClass::VeryHigh,
        };

        // Calculate IL standard deviation
        let il_std_dev =
            self.calculate_il_standard_deviation(price_history, lower_price, upper_price);

        ILRiskMetrics {
            vol_exposure,
            critical_price_deviation: critical_deviation,
            risk_classification,
            il_std_dev,
        }
    }

    /// Calculate standard deviation of price returns
    fn calculate_price_volatility(&self, price_history: &[PriceDataPoint]) -> f64 {
        if price_history.len() < 2 {
            return 0.0;
        }

        // Calculate daily returns
        let mut returns = Vec::new();
        let mut prev_price = price_history[0].price;

        for point in &price_history[1..] {
            let ret = (point.price / prev_price) - 1.0;
            returns.push(ret);
            prev_price = point.price;
        }

        // Calculate standard deviation
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance =
            returns.iter().map(|x| (x - mean).powf(2.0)).sum::<f64>() / returns.len() as f64;

        variance.sqrt()
    }

    /// Calculate standard deviation of IL over historical price points
    fn calculate_il_standard_deviation(
        &self,
        price_history: &[PriceDataPoint],
        lower_price: f64,
        upper_price: f64,
    ) -> f64 {
        if price_history.is_empty() {
            return 0.0;
        }

        // Calculate reference price (geometric mean of range)
        let reference_price = (lower_price * upper_price).sqrt();

        // Calculate IL at each price point
        let mut il_values = Vec::new();
        for point in price_history {
            let price_ratio = point.price / reference_price;
            let il = 2.0 * price_ratio.sqrt() / (1.0 + price_ratio) - 1.0;
            il_values.push(il);
        }

        // Calculate standard deviation
        let mean = il_values.iter().sum::<f64>() / il_values.len() as f64;
        let variance =
            il_values.iter().map(|x| (x - mean).powf(2.0)).sum::<f64>() / il_values.len() as f64;

        variance.sqrt()
    }

    /// Generate IL projections under different market scenarios
    fn generate_il_projections(
        &self,
        current_price: f64,
        lower_price: f64,
        upper_price: f64,
        position_value: f64,
        fee_earnings: f64,
    ) -> HashMap<String, ILProjection> {
        let mut projections = HashMap::new();

        // Reference values for calculations
        let range_width = upper_price / lower_price;
        let daily_fee_rate = fee_earnings / position_value / 30.0; // Assuming monthly fees

        // Scenario 1: Price increases by 20%
        let upside_price = current_price * 1.2;
        let (upside_il_pct, _) = self.calculate_current_il(
            current_price,
            upside_price,
            lower_price,
            upper_price,
            position_value,
        );
        let upside_fee_estimate = daily_fee_rate * position_value * 30.0; // 30 days projection

        projections.insert(
            "upside_20pct".to_string(),
            ILProjection {
                projected_il_pct: upside_il_pct,
                projected_il_usd: upside_il_pct * position_value,
                projected_fees_usd: upside_fee_estimate,
                net_projected_return: upside_fee_estimate + (upside_il_pct * position_value),
            },
        );

        // Scenario 2: Price decreases by 20%
        let downside_price = current_price * 0.8;
        let (downside_il_pct, _) = self.calculate_current_il(
            current_price,
            downside_price,
            lower_price,
            upper_price,
            position_value,
        );
        let downside_fee_estimate = daily_fee_rate * position_value * 30.0; // 30 days projection

        projections.insert(
            "downside_20pct".to_string(),
            ILProjection {
                projected_il_pct: downside_il_pct,
                projected_il_usd: downside_il_pct * position_value,
                projected_fees_usd: downside_fee_estimate,
                net_projected_return: downside_fee_estimate + (downside_il_pct * position_value),
            },
        );

        // Scenario 3: High volatility (price movement within range but high variance)
        // For high volatility, we estimate higher fees but also higher IL
        let high_vol_il_pct = -0.03; // Estimated IL under high volatility
        let high_vol_fee_multiplier = 2.0; // Higher trading volume means more fees
        let high_vol_fee_estimate =
            daily_fee_rate * position_value * 30.0 * high_vol_fee_multiplier;

        projections.insert(
            "high_volatility".to_string(),
            ILProjection {
                projected_il_pct: high_vol_il_pct,
                projected_il_usd: high_vol_il_pct * position_value,
                projected_fees_usd: high_vol_fee_estimate,
                net_projected_return: high_vol_fee_estimate + (high_vol_il_pct * position_value),
            },
        );

        projections
    }

    /// Update the metrics configuration
    pub fn update_config(&mut self, new_config: ILMetricsConfig) {
        self.config = new_config;
    }

    /// Access the current configuration
    pub fn config(&self) -> &ILMetricsConfig {
        &self.config
    }
}

/// Calculate the price deviation that would cause significant IL (> 5%)
fn calculate_critical_price_deviation(range_width: f64) -> f64 {
    // For a range with width w, calculate price movement that would cause 5% IL
    // This is based on the IL formula and solving for price ratio

    // For concentrated liquidity, the critical deviation depends on range width
    // Wider ranges can tolerate more deviation

    // A simplified approximation:
    let il_threshold = 0.05; // 5% IL threshold
    let k = 0.5; // Adjustment factor based on concentrated liquidity

    (il_threshold / k) * range_width.sqrt()
}
