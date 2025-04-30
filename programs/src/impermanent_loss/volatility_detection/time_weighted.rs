use super::types::{PriceHistory, VolatilityScore};

/// Time-Weighted Volatility model
/// This model applies exponential weighting to price movements,
/// giving more significance to recent price changes while still
/// considering historical data.
#[derive(Debug)]
pub struct TimeWeightedVolatility {
    // Configuration parameters
    lambda: f64,            // Decay factor (0 < λ < 1)
    min_window_size: usize, // Minimum data points required

    // Analysis windows in time units (e.g., minutes, hours)
    short_window: usize,  // Short-term window (e.g., 15 minutes)
    medium_window: usize, // Medium-term window (e.g., 4 hours)
    long_window: usize,   // Long-term window (e.g., 24 hours)
}

// Implement Default trait instead of custom default() method
impl Default for TimeWeightedVolatility {
    /// Create a default TimeWeightedVolatility model
    /// Uses standard parameters suitable for typical AMM environments
    fn default() -> Self {
        // Default settings:
        // - Lambda: 0.94 (moderate decay)
        // - Min window: 10 data points
        // - Short window: 60 (1 hour with minute data)
        // - Medium window: 240 (4 hours with minute data)
        // - Long window: 1440 (24 hours with minute data)
        Self::new(0.94, 10, 60, 240, 1440)
    }
}

impl TimeWeightedVolatility {
    /// Create a new TimeWeightedVolatility model with custom parameters
    pub fn new(
        lambda: f64,
        min_window_size: usize,
        short_window: usize,
        medium_window: usize,
        long_window: usize,
    ) -> Self {
        assert!(
            lambda > 0.0 && lambda < 1.0,
            "Lambda must be between 0 and 1"
        );
        assert!(
            short_window < medium_window && medium_window < long_window,
            "Windows must be in ascending order: short < medium < long"
        );

        Self {
            lambda,
            min_window_size,
            short_window,
            medium_window,
            long_window,
        }
    }

    /// Calculate volatility based on price history using time-weighted approach
    pub fn calculate_volatility(&self, price_history: &PriceHistory) -> VolatilityScore {
        let data_points = price_history.data.len();

        if data_points < self.min_window_size {
            // Not enough data for reliable estimation
            return VolatilityScore {
                value: 0.0,
                confidence: 0.0,
                window_size: data_points,
            };
        }

        // Get log returns
        let log_returns = price_history.get_log_returns();

        // Determine effective window size based on available data
        let window_size = std::cmp::min(data_points - 1, self.medium_window);

        // Calculate exponentially weighted variance
        let ewv = self.calculate_exp_weighted_variance(&log_returns, window_size);

        // Calculate volatility as square root of variance, annualized
        // Assuming time units are in minutes
        let annualization_factor = (525600.0 / window_size as f64).sqrt(); // 525600 minutes in a year
        let volatility = ewv.sqrt() * annualization_factor;

        // Calculate blended volatility using multiple time windows if enough data
        let volatility = if data_points > self.long_window {
            // We have enough data for all time windows, so blend them
            self.calculate_blended_volatility(&log_returns)
        } else {
            volatility
        };

        // Calculate confidence based on data quantity
        let confidence = Self::calculate_confidence(data_points);

        VolatilityScore {
            value: volatility,
            confidence,
            window_size,
        }
    }

    /// Calculate exponentially weighted variance for a given window
    fn calculate_exp_weighted_variance(&self, returns: &[f64], window_size: usize) -> f64 {
        let window = if returns.len() < window_size {
            returns
        } else {
            &returns[returns.len() - window_size..]
        };

        let n = window.len();
        if n < 2 {
            return 0.0;
        }

        // Calculate weights based on lambda
        let mut weights = vec![0.0; n];
        let mut weight_sum = 0.0;

        // Using enumerate() instead of range-based loop
        for (i, weight) in weights.iter_mut().enumerate() {
            *weight = self.lambda.powi((n - i - 1) as i32);
            weight_sum += *weight;
        }

        // Normalize weights
        for w in &mut weights {
            *w /= weight_sum;
        }

        // Calculate weighted mean
        let weighted_mean = window
            .iter()
            .zip(weights.iter())
            .map(|(r, w)| r * w)
            .sum::<f64>();

        // Calculate weighted variance
        let weighted_variance = window
            .iter()
            .zip(weights.iter())
            .map(|(r, w)| w * (r - weighted_mean).powi(2))
            .sum::<f64>();

        weighted_variance
    }

    /// Calculate blended volatility using multiple time horizons
    /// This creates a more robust volatility measure that accounts for
    /// both short-term spikes and longer-term trends
    fn calculate_blended_volatility(&self, returns: &[f64]) -> f64 {
        let n = returns.len();

        // Calculate volatility for each window
        let short_window_size = std::cmp::min(self.short_window, n);
        let medium_window_size = std::cmp::min(self.medium_window, n);
        let long_window_size = std::cmp::min(self.long_window, n);

        let short_var = self.calculate_exp_weighted_variance(returns, short_window_size);
        let medium_var = self.calculate_exp_weighted_variance(returns, medium_window_size);
        let long_var = self.calculate_exp_weighted_variance(returns, long_window_size);

        // Annualization factors (assuming minute data)
        let short_factor = (525600.0 / short_window_size as f64).sqrt();
        let medium_factor = (525600.0 / medium_window_size as f64).sqrt();
        let long_factor = (525600.0 / long_window_size as f64).sqrt();

        // Calculate annualized volatilities
        let short_vol = short_var.sqrt() * short_factor;
        let medium_vol = medium_var.sqrt() * medium_factor;
        let long_vol = long_var.sqrt() * long_factor;

        // Blend volatilities with more weight on medium-term and return directly
        0.25 * short_vol + 0.5 * medium_vol + 0.25 * long_vol
    }

    /// Calculate confidence score based on data quantity
    fn calculate_confidence(data_points: usize) -> f64 {
        // Simple confidence measure based on data quantity
        // Increases with more data points, maxes out at 0.95
        let base_confidence = (data_points as f64 / 200.0).min(0.95);

        // Minimum confidence of 0.2 with any data
        0.2f64.max(base_confidence)
    }
}
