use super::types::{PriceDataPoint, PriceHistory, VolatilityScore};
use super::volatility_calculator::VolatilityCalculator;

// Define the LN_2PI constant that's missing from std::f64::consts
const LN_2PI: f64 = 1.8378770664093453; // ln(2π)

/// Implementation of a simplified GARCH (Generalized Autoregressive Conditional Heteroskedasticity) model
/// for volatility forecasting.
///
/// This model captures volatility clustering effects, where high volatility periods
/// tend to be followed by high volatility, and low volatility by low volatility.
#[derive(Debug)]
pub struct GarchModel {
    /// GARCH(1,1) parameter: weight for previous conditional variance (persistence)
    pub alpha: f64,

    /// GARCH(1,1) parameter: weight for previous squared return (responsiveness)
    pub beta: f64,

    /// GARCH(1,1) parameter: long-run average variance
    pub omega: f64,

    /// Factor to convert to annualized volatility
    pub annualization_factor: f64,

    /// Whether to use log returns (true) or percentage returns (false)
    pub use_log_returns: bool,

    /// Maximum iterations for parameter estimation
    pub max_iterations: usize,

    /// Convergence threshold for parameter estimation
    pub convergence_threshold: f64,
}

impl Default for GarchModel {
    fn default() -> Self {
        Self {
            alpha: 0.1,                  // Weight for previous squared return
            beta: 0.85,                  // Weight for previous conditional variance
            omega: 0.000001,             // Long-run average variance component
            annualization_factor: 252.0, // Assuming daily data
            use_log_returns: true,
            max_iterations: 100,
            convergence_threshold: 0.0001,
        }
    }
}

impl GarchModel {
    /// Creates a new GARCH model with custom parameters.
    ///
    /// # Arguments
    /// * `alpha` - GARCH parameter for squared return weight (responsiveness)
    /// * `beta` - GARCH parameter for previous variance weight (persistence)
    /// * `omega` - GARCH parameter for long-run average variance
    /// * `annualization_factor` - Factor to convert to annualized volatility
    /// * `use_log_returns` - Whether to use log returns
    ///
    /// # Returns
    /// * `GarchModel` instance
    pub fn new(
        alpha: f64,
        beta: f64,
        omega: f64,
        annualization_factor: f64,
        use_log_returns: bool,
    ) -> Self {
        Self {
            alpha,
            beta,
            omega,
            annualization_factor,
            use_log_returns,
            max_iterations: 100,
            convergence_threshold: 0.0001,
        }
    }

    /// Estimate GARCH parameters from historical returns
    ///
    /// This is a simplified parameter estimation that uses a quasi-maximum likelihood approach
    /// For production use, consider a more robust numerical optimization library
    ///
    /// # Arguments
    /// * `returns` - Historical returns data
    ///
    /// # Returns
    /// * Tuple of (omega, alpha, beta) parameters
    fn estimate_parameters(&self, returns: &[f64]) -> (f64, f64, f64) {
        if returns.len() < 30 {
            // Not enough data for reliable estimation, return default parameters
            return (self.omega, self.alpha, self.beta);
        }

        // Initial parameter estimates
        let mut omega = self.omega;
        let mut alpha = self.alpha;
        let mut beta = self.beta;

        // Calculate unconditional variance as starting point
        let unconditional_variance =
            returns.iter().map(|&r| r * r).sum::<f64>() / returns.len() as f64;

        // Initial log-likelihood
        let mut prev_log_likelihood = self.calculate_log_likelihood(returns, omega, alpha, beta);

        // Iterative optimization
        for _ in 0..self.max_iterations {
            // Update parameters using gradient descent
            // (this is simplified; a proper implementation would use numerical optimization)
            let gradient = self.calculate_gradient(returns, omega, alpha, beta);

            omega += 0.01 * gradient.0;
            alpha += 0.01 * gradient.1;
            beta += 0.01 * gradient.2;

            // Ensure constraints are satisfied
            omega = omega.max(0.00000001);
            alpha = alpha.clamp(0.01, 0.3);
            beta = beta.clamp(0.6, 0.99);

            // Ensure persistence < 1 for stationarity
            if alpha + beta >= 0.999 {
                let sum = alpha + beta;
                alpha = alpha * 0.998 / sum;
                beta = beta * 0.998 / sum;
            }

            // Calculate new log-likelihood
            let new_log_likelihood = self.calculate_log_likelihood(returns, omega, alpha, beta);

            // Check convergence
            if (new_log_likelihood - prev_log_likelihood).abs() < self.convergence_threshold {
                break;
            }

            prev_log_likelihood = new_log_likelihood;
        }

        (omega, alpha, beta)
    }

    /// Calculate GARCH log-likelihood for a set of parameters
    ///
    /// # Arguments
    /// * `returns` - Historical returns
    /// * `omega`, `alpha`, `beta` - GARCH parameters
    ///
    /// # Returns
    /// * Log-likelihood value
    fn calculate_log_likelihood(&self, returns: &[f64], omega: f64, alpha: f64, beta: f64) -> f64 {
        let n = returns.len();
        if n < 2 {
            return 0.0;
        }

        // Initialize with unconditional variance
        let unconditional_variance = returns.iter().map(|&r| r * r).sum::<f64>() / n as f64;

        let mut h_t = unconditional_variance;
        let mut log_likelihood = 0.0;

        // Calculate log-likelihood
        for &r_t in returns {
            // Update conditional variance
            h_t = omega + alpha * r_t * r_t + beta * h_t;

            // Update log-likelihood (assuming normal distribution)
            log_likelihood += -0.5 * (LN_2PI + h_t.ln() + r_t * r_t / h_t);
        }

        log_likelihood
    }

    /// Calculate gradient of log-likelihood for parameter updates
    /// This is a simplified numerical approximation
    ///
    /// # Arguments
    /// * `returns` - Historical returns
    /// * `omega`, `alpha`, `beta` - Current GARCH parameters
    ///
    /// # Returns
    /// * Gradient tuple (d_omega, d_alpha, d_beta)
    fn calculate_gradient(
        &self,
        returns: &[f64],
        omega: f64,
        alpha: f64,
        beta: f64,
    ) -> (f64, f64, f64) {
        let epsilon = 0.00001;

        let base_ll = self.calculate_log_likelihood(returns, omega, alpha, beta);

        let d_omega = (self.calculate_log_likelihood(returns, omega + epsilon, alpha, beta)
            - base_ll)
            / epsilon;
        let d_alpha = (self.calculate_log_likelihood(returns, omega, alpha + epsilon, beta)
            - base_ll)
            / epsilon;
        let d_beta = (self.calculate_log_likelihood(returns, omega, alpha, beta + epsilon)
            - base_ll)
            / epsilon;

        (d_omega, d_alpha, d_beta)
    }

    /// Calculate GARCH volatility forecast for a specific horizon
    ///
    /// # Arguments
    /// * `returns` - Historical returns
    /// * `horizon` - Number of periods ahead to forecast
    ///
    /// # Returns
    /// * Forecasted volatility
    pub fn forecast_volatility(&self, returns: &[f64], horizon: usize) -> f64 {
        if returns.is_empty() {
            return 0.0;
        }

        let (omega, alpha, beta) = self.estimate_parameters(returns);

        // Calculate current conditional variance
        let current_variance = self.calculate_current_variance(returns, omega, alpha, beta);

        // Calculate long-run variance
        let long_run_variance = omega / (1.0 - alpha - beta);

        // Calculate multi-step forecast
        let mut forecast_variance = current_variance;
        for _ in 0..horizon {
            forecast_variance = omega + (alpha + beta) * forecast_variance;
        }

        // For longer horizons, approach the long-run variance
        if horizon > 10 {
            let weight_long_run = (horizon as f64 - 10.0) / horizon as f64;
            let weight_forecast = 1.0 - weight_long_run;
            forecast_variance =
                weight_forecast * forecast_variance + weight_long_run * long_run_variance;
        }

        // Return standard deviation (volatility)
        forecast_variance.sqrt()
    }

    /// Calculate current conditional variance based on historical returns
    ///
    /// # Arguments
    /// * `returns` - Historical returns
    /// * `omega`, `alpha`, `beta` - GARCH parameters
    ///
    /// # Returns
    /// * Current conditional variance
    fn calculate_current_variance(
        &self,
        returns: &[f64],
        omega: f64,
        alpha: f64,
        beta: f64,
    ) -> f64 {
        if returns.is_empty() {
            return 0.0;
        }

        // Initialize with unconditional variance
        let unconditional_variance =
            returns.iter().map(|&r| r * r).sum::<f64>() / returns.len() as f64;

        let mut h_t = unconditional_variance;

        // Update through all historical returns
        for &r_t in returns {
            h_t = omega + alpha * r_t * r_t + beta * h_t;
        }

        h_t
    }
}

impl VolatilityCalculator for GarchModel {
    fn calculate_volatility(
        &self,
        price_history: &PriceHistory,
        window_size: Option<usize>,
    ) -> VolatilityScore {
        let data_len = price_history.data.len();
        if data_len < 10 {
            // Need sufficient data for GARCH
            return VolatilityScore {
                value: 0.0,
                confidence: 0.0,
                window_size: 0,
            };
        }

        // Use the requested window size or full history
        let effective_window = window_size.unwrap_or(data_len);
        let effective_window = std::cmp::min(effective_window, data_len);

        // Get most recent data for the window
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
        // Reverse back to chronological order
        recent_data.data = recent_data.data.iter().rev().cloned().collect();

        // Calculate returns
        let returns = if self.use_log_returns {
            recent_data.get_log_returns()
        } else {
            recent_data.get_returns()
        };

        if returns.len() < 5 {
            return VolatilityScore {
                value: 0.0,
                confidence: 0.0,
                window_size: effective_window,
            };
        }

        // Estimate GARCH parameters
        let (omega, alpha, beta) = self.estimate_parameters(&returns);

        // Calculate current volatility
        let current_variance = self.calculate_current_variance(&returns, omega, alpha, beta);
        let volatility = current_variance.sqrt();

        // Annualize the volatility
        let annualized_vol = volatility * self.annualization_factor.sqrt();

        // Calculate confidence based on data size and parameter stability
        let data_confidence = 1.0 - (5.0 / returns.len() as f64).min(0.9);
        let param_stability = (1.0 - (alpha + beta)).abs(); // Higher stability when closer to non-unit root
        let confidence = data_confidence * param_stability.min(1.0);

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
        // For GARCH, adaptive volatility combines short and long-term forecasts

        // Calculate current volatility
        let current_vol = self.calculate_volatility(price_history, None);

        if price_history.data.len() < 20 {
            return current_vol; // Not enough data for reliable forecasting
        }

        // Get returns
        let returns = if self.use_log_returns {
            price_history.get_log_returns()
        } else {
            price_history.get_returns()
        };

        if returns.is_empty() {
            return current_vol;
        }

        // Forecast volatilities at different horizons
        let horizons = [1, 5, 22]; // 1-day, 1-week, 1-month ahead
        let default_weights = vec![0.6, 0.3, 0.1]; // Higher weight to short-term

        let weights = weights.unwrap_or(default_weights);
        assert!(
            weights.len() >= horizons.len(),
            "Not enough weights provided"
        );

        let mut weighted_vol = 0.0;
        let mut total_weight = 0.0;

        for (i, &horizon) in horizons.iter().enumerate() {
            let weight = weights[i];
            if weight > 0.0 {
                let forecast = self.forecast_volatility(&returns, horizon);
                let annualized_forecast = forecast * self.annualization_factor.sqrt();
                weighted_vol += annualized_forecast * weight;
                total_weight += weight;
            }
        }

        if total_weight > 0.0 {
            weighted_vol /= total_weight;
        } else {
            weighted_vol = current_vol.value;
        }

        VolatilityScore {
            value: weighted_vol,
            confidence: current_vol.confidence * 0.8, // Slightly lower confidence for forecasts
            window_size: price_history.data.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn create_test_price_history() -> PriceHistory {
        let mut history = PriceHistory::new(200);

        // Generate synthetic price data with GARCH-like volatility clustering
        let mut price = 100.0;
        let mut volatility;

        for i in 0..200 {
            // Update volatility - create clusters of volatility
            let cycle = (i as f64 / 50.0 * PI).sin(); // Oscillating factor
            volatility = 0.005 + 0.02 * cycle.abs(); // Base + oscillating component

            // Random return with current volatility
            let z = rand_normal();
            let return_pct = z * volatility;

            // Update price
            price *= 1.0 + return_pct;

            history.add_price_point(PriceDataPoint {
                timestamp: i as u64,
                price,
            });
        }

        history
    }

    // Simple Box-Muller normal random generator for testing
    fn rand_normal() -> f64 {
        let u1 = 0.1
            + 0.8
                * (((std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .subsec_nanos() as f64)
                    * 0.000001)
                    % 1.0);
        let u2 = 0.1
            + 0.8
                * (((std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .subsec_nanos() as f64
                    * 1.1)
                    * 0.000001)
                    % 1.0);

        (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
    }

    #[test]
    fn test_garch_volatility_calculation() {
        let model = GarchModel::default();
        let history = create_test_price_history();

        let vol_score = model.calculate_volatility(&history, None);

        // Volatility should be positive
        assert!(vol_score.value > 0.0);
        // Confidence should be reasonable
        assert!(vol_score.confidence > 0.3);
    }

    #[test]
    fn test_garch_parameter_estimation() {
        let model = GarchModel::default();
        let history = create_test_price_history();

        let returns = if model.use_log_returns {
            history.get_log_returns()
        } else {
            history.get_returns()
        };

        let (omega, alpha, beta) = model.estimate_parameters(&returns);

        // Parameters should be in reasonable ranges
        assert!(omega > 0.0);
        assert!(alpha > 0.0 && alpha < 0.5);
        assert!(beta > 0.5 && beta < 1.0);

        // Stationarity condition: alpha + beta < 1
        assert!(alpha + beta < 1.0);
    }

    #[test]
    fn test_garch_volatility_forecast() {
        let model = GarchModel::default();
        let history = create_test_price_history();

        let returns = if model.use_log_returns {
            history.get_log_returns()
        } else {
            history.get_returns()
        };

        let forecast_1day = model.forecast_volatility(&returns, 1);
        let forecast_10day = model.forecast_volatility(&returns, 10);
        let forecast_30day = model.forecast_volatility(&returns, 30);

        // All forecasts should be positive
        assert!(forecast_1day > 0.0);
        assert!(forecast_10day > 0.0);
        assert!(forecast_30day > 0.0);
    }

    #[test]
    fn test_adaptive_garch_volatility() {
        let model = GarchModel::default();
        let history = create_test_price_history();

        let adaptive_vol = model.calculate_adaptive_volatility(&history, None);

        // Adaptive volatility should be positive
        assert!(adaptive_vol.value > 0.0);
        // Should have reasonable confidence
        assert!(adaptive_vol.confidence > 0.3);
    }
}
