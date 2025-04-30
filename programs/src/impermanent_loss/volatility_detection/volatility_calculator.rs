use super::types::{PriceDataPoint, PriceHistory, VolatilityScore};
use std::fmt::Debug;

/// Trait defining the interface for volatility calculation algorithms.
///
/// This trait allows for different volatility calculation strategies to be
/// implemented and used interchangeably within the system. Implementations
/// should be threadsafe and handle edge cases gracefully.
pub trait VolatilityCalculator: Debug {
    /// Calculate volatility from a price history.
    ///
    /// # Arguments
    /// * `price_history` - Historical price data to analyze
    /// * `window_size` - Optional window size in data points (implementation-specific)
    ///
    /// # Returns
    /// * `VolatilityScore` - Calculated volatility with confidence score
    fn calculate_volatility(
        &self,
        price_history: &PriceHistory,
        window_size: Option<usize>,
    ) -> VolatilityScore;

    /// Calculate volatility across multiple timeframes.
    ///
    /// # Arguments
    /// * `price_history` - Historical price data to analyze
    /// * `window_sizes` - Vector of different window sizes to analyze
    ///
    /// # Returns
    /// * `Vec<VolatilityScore>` - Calculated volatilities for each window size
    fn calculate_multi_timeframe_volatility(
        &self,
        price_history: &PriceHistory,
        window_sizes: &[usize],
    ) -> Vec<VolatilityScore> {
        window_sizes
            .iter()
            .map(|&size| self.calculate_volatility(price_history, Some(size)))
            .collect()
    }

    /// Detect significant changes in volatility regime.
    ///
    /// # Arguments
    /// * `price_history` - Historical price data to analyze
    /// * `threshold` - Significance threshold for regime change detection
    ///
    /// # Returns
    /// * `bool` - True if a volatility regime change is detected
    fn detect_volatility_regime_change(
        &self,
        price_history: &PriceHistory,
        threshold: f64,
    ) -> bool {
        if price_history.data.len() < 10 {
            return false; // Not enough data to detect regime change
        }

        let midpoint = price_history.data.len() / 2;
        let recent_history = PriceHistory {
            data: price_history.data.iter().skip(midpoint).cloned().collect(),
            max_size: price_history.max_size,
        };
        let previous_history = PriceHistory {
            data: price_history.data.iter().take(midpoint).cloned().collect(),
            max_size: price_history.max_size,
        };

        let recent_vol = self.calculate_volatility(&recent_history, None);
        let previous_vol = self.calculate_volatility(&previous_history, None);

        // Calculate relative change in volatility
        let relative_change = (recent_vol.value - previous_vol.value).abs() / previous_vol.value;

        relative_change > threshold
    }

    /// Calculate adaptive volatility by combining multiple measures.
    ///
    /// # Arguments
    /// * `price_history` - Historical price data to analyze
    /// * `weights` - Optional weights for different volatility measures (implementation-specific)
    ///
    /// # Returns
    /// * `VolatilityScore` - Calculated adaptive volatility
    fn calculate_adaptive_volatility(
        &self,
        price_history: &PriceHistory,
        weights: Option<Vec<f64>>,
    ) -> VolatilityScore;
}

/// A simple implementation of volatility calculator using standard deviation of returns.
///
/// This implementation calculates volatility as the standard deviation of
/// percentage or logarithmic returns over a specified time window.
#[derive(Debug)]
pub struct StandardVolatilityCalculator {
    pub use_log_returns: bool,
    pub annualization_factor: f64,
}

impl Default for StandardVolatilityCalculator {
    fn default() -> Self {
        Self {
            use_log_returns: true,
            annualization_factor: 252.0, // Default to annualized volatility assuming daily data
        }
    }
}

impl StandardVolatilityCalculator {
    /// Creates a new StandardVolatilityCalculator.
    ///
    /// # Arguments
    /// * `use_log_returns` - Whether to use logarithmic returns (true) or percentage returns (false)
    /// * `annualization_factor` - Factor to convert to annualized volatility
    ///
    /// # Returns
    /// * `StandardVolatilityCalculator` - Configured calculator instance
    pub fn new(use_log_returns: bool, annualization_factor: f64) -> Self {
        Self {
            use_log_returns,
            annualization_factor,
        }
    }
}

impl VolatilityCalculator for StandardVolatilityCalculator {
    fn calculate_volatility(
        &self,
        price_history: &PriceHistory,
        window_size: Option<usize>,
    ) -> VolatilityScore {
        let data_len = price_history.data.len();
        if data_len < 2 {
            return VolatilityScore {
                value: 0.0,
                confidence: 0.0,
                window_size: 0,
            };
        }

        let effective_window = window_size.unwrap_or(data_len);
        let effective_window = std::cmp::min(effective_window, data_len);

        let returns = if self.use_log_returns {
            let mut recent_data = PriceHistory {
                data: price_history
                    .data
                    .iter()
                    .rev()
                    .take(effective_window)
                    .cloned()
                    .collect(),
                max_size: effective_window,
            };
            recent_data.data = recent_data.data.iter().rev().cloned().collect();
            recent_data.get_log_returns()
        } else {
            let mut recent_data = PriceHistory {
                data: price_history
                    .data
                    .iter()
                    .rev()
                    .take(effective_window)
                    .cloned()
                    .collect(),
                max_size: effective_window,
            };
            recent_data.data = recent_data.data.iter().rev().cloned().collect();
            recent_data.get_returns()
        };

        if returns.is_empty() {
            return VolatilityScore {
                value: 0.0,
                confidence: 0.0,
                window_size: effective_window,
            };
        }

        let n = returns.len() as f64;
        let mean = returns.iter().sum::<f64>() / n;
        let variance = returns.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
        let std_dev = variance.sqrt();

        // Annualize the volatility
        let annualized_vol = std_dev * self.annualization_factor.sqrt();

        // Calculate confidence based on sample size
        let confidence = 1.0 - (1.0 / n.sqrt()).min(0.9);

        VolatilityScore {
            value: annualized_vol,
            confidence,
            window_size: effective_window,
        }
    }

    fn calculate_adaptive_volatility(
        &self,
        price_history: &PriceHistory,
        weights: Option<Vec<f64>>,
    ) -> VolatilityScore {
        // Default window sizes for short, medium, and long-term
        let window_sizes = vec![5, 22, 66]; // ~1 week, ~1 month, ~3 months (assuming daily data)
        let default_weights = vec![0.5, 0.3, 0.2]; // Higher weight to recent volatility

        let weights = weights.unwrap_or(default_weights);
        assert!(
            weights.len() >= window_sizes.len(),
            "Not enough weights provided"
        );

        let vols = self.calculate_multi_timeframe_volatility(price_history, &window_sizes);
        if vols.is_empty() {
            return VolatilityScore {
                value: 0.0,
                confidence: 0.0,
                window_size: 0,
            };
        }

        // Calculate weighted average
        let mut weighted_vol = 0.0;
        let mut weighted_conf = 0.0;
        let mut total_weight = 0.0;

        for (i, vol) in vols.iter().enumerate() {
            let weight = weights[i];
            weighted_vol += vol.value * weight;
            weighted_conf += vol.confidence * weight;
            total_weight += weight;
        }

        if total_weight > 0.0 {
            weighted_vol /= total_weight;
            weighted_conf /= total_weight;
        }

        VolatilityScore {
            value: weighted_vol,
            confidence: weighted_conf,
            window_size: price_history.data.len(), // Use full history size
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_price_history() -> PriceHistory {
        let mut history = PriceHistory::new(100);

        // Add some test data points - price increasing steadily then more volatility
        for i in 0..50 {
            history.add_price_point(PriceDataPoint {
                timestamp: i,
                price: 100.0 + (i as f64) * 0.1, // Small steady increase
            });
        }

        for i in 50..75 {
            history.add_price_point(PriceDataPoint {
                timestamp: i,
                price: 105.0 + ((i - 50) as f64) * 0.5, // Steeper increase
            });
        }

        for i in 75..100 {
            // Add some volatility
            let volatility = if i % 2 == 0 { 1.0 } else { -0.8 };
            history.add_price_point(PriceDataPoint {
                timestamp: i,
                price: 117.5 + volatility * ((i - 75) as f64) * 0.2,
            });
        }

        history
    }

    #[test]
    fn test_calculate_volatility() {
        let calculator = StandardVolatilityCalculator::default();
        let history = create_test_price_history();

        let vol_score = calculator.calculate_volatility(&history, None);

        // We expect a non-zero volatility
        assert!(vol_score.value > 0.0);
        // Confidence should be high with our sample size
        assert!(vol_score.confidence > 0.8);
    }

    #[test]
    fn test_multi_timeframe_volatility() {
        let calculator = StandardVolatilityCalculator::default();
        let history = create_test_price_history();

        let window_sizes = vec![10, 25, 50];
        let vols = calculator.calculate_multi_timeframe_volatility(&history, &window_sizes);

        // We should get one volatility score per window size
        assert_eq!(vols.len(), window_sizes.len());

        // Short-term window should have higher volatility due to recent price movements
        assert!(vols[0].value >= vols[2].value);
    }

    #[test]
    fn test_detect_volatility_regime_change() {
        let calculator = StandardVolatilityCalculator::default();
        let history = create_test_price_history();

        // With our test data, we should detect a regime change with a reasonable threshold
        assert!(calculator.detect_volatility_regime_change(&history, 0.3));
        // With a very high threshold, we shouldn't detect a change
        assert!(!calculator.detect_volatility_regime_change(&history, 10.0));
    }

    #[test]
    fn test_adaptive_volatility() {
        let calculator = StandardVolatilityCalculator::default();
        let history = create_test_price_history();

        let adaptive_vol = calculator.calculate_adaptive_volatility(&history, None);

        // Adaptive volatility should be positive
        assert!(adaptive_vol.value > 0.0);
        // Confidence should be high with our sample size
        assert!(adaptive_vol.confidence > 0.8);
    }
}
