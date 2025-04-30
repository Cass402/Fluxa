use super::types::{PriceBoundary, SimulationParameters};
use rand::Rng;
use rand_distr::{Distribution, Normal};

/// Stochastic model for price simulation to calculate optimal
/// position boundaries for LP positions
#[derive(Debug)]
pub struct StochasticModel {
    params: SimulationParameters,
    rng: rand::rngs::ThreadRng,
}

impl StochasticModel {
    /// Create a new stochastic model with custom parameters
    pub fn new(params: SimulationParameters) -> Self {
        Self {
            params,
            rng: rand::thread_rng(),
        }
    }

    /// Run simulations for price paths and generate price boundaries
    pub fn simulate_price_boundaries(&mut self, current_price: f64) -> PriceBoundary {
        // Run the specified number of simulations
        let price_paths = self.generate_price_paths(current_price);

        // Calculate price boundaries based on simulation results
        let (lower_bound, upper_bound, confidence) = self.calculate_boundaries(&price_paths);

        PriceBoundary {
            lower_price: lower_bound,
            upper_price: upper_bound,
            current_price,
            confidence,
        }
    }

    /// Generate price paths using Geometric Brownian Motion with optional mean reversion
    fn generate_price_paths(&mut self, initial_price: f64) -> Vec<Vec<f64>> {
        let mut price_paths = Vec::with_capacity(self.params.num_simulations);

        // Time step size in years (assuming time_horizon is in days)
        let dt = self.params.time_horizon / (365.0 * self.params.time_steps as f64);

        // Normal distribution for random numbers
        let normal = Normal::new(0.0, 1.0).unwrap();

        for _ in 0..self.params.num_simulations {
            let mut path = Vec::with_capacity(self.params.time_steps + 1);
            path.push(initial_price);

            let mut current_price = initial_price;

            for _ in 0..self.params.time_steps {
                // Generate random normal variable
                let z: f64 = normal.sample(&mut self.rng);

                // Calculate price change based on model
                let price_change = if let Some(mean_rev) = self.params.mean_reversion_strength {
                    // Ornstein-Uhlenbeck process (mean reverting)
                    let mean_reversion_term = mean_rev * (initial_price - current_price) * dt;
                    let volatility_term = self.params.volatility * current_price * z * dt.sqrt();
                    mean_reversion_term + volatility_term
                } else {
                    // Standard Geometric Brownian Motion
                    let drift_term = self.params.drift * current_price * dt;
                    let volatility_term = self.params.volatility * current_price * z * dt.sqrt();
                    drift_term + volatility_term
                };

                // Update price
                current_price += price_change;

                // Ensure price doesn't go negative
                current_price = current_price.max(0.00001 * initial_price);

                path.push(current_price);
            }

            price_paths.push(path);
        }

        price_paths
    }

    /// Calculate price boundaries from the simulated paths
    fn calculate_boundaries(&self, price_paths: &[Vec<f64>]) -> (f64, f64, f64) {
        // Extract final prices from all simulations
        let mut final_prices: Vec<f64> = price_paths
            .iter()
            .map(|path| *path.last().unwrap())
            .collect();

        // Sort prices for percentile calculations
        final_prices.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // Get confidence interval (e.g., 95% interval)
        let confidence_level = 0.95;
        let lower_percentile = (1.0 - confidence_level) / 2.0;
        let upper_percentile = 1.0 - lower_percentile;

        let lower_index = (final_prices.len() as f64 * lower_percentile).round() as usize;
        let upper_index = (final_prices.len() as f64 * upper_percentile).round() as usize;

        // Ensure indices are within valid range
        let lower_index = lower_index.max(0).min(final_prices.len() - 1);
        let upper_index = upper_index.max(0).min(final_prices.len() - 1);

        // Get boundary prices
        let lower_bound = final_prices[lower_index];
        let upper_bound = final_prices[upper_index];

        // Calculate percentage of prices within the bounds as a confidence measure
        let prices_in_bounds = final_prices
            .iter()
            .filter(|&p| *p >= lower_bound && *p <= upper_bound)
            .count();

        let empirical_confidence = prices_in_bounds as f64 / final_prices.len() as f64;

        (lower_bound, upper_bound, empirical_confidence)
    }

    /// Update simulation parameters
    pub fn update_parameters(&mut self, params: SimulationParameters) {
        self.params = params;
    }

    /// Get current simulation parameters
    pub fn parameters(&self) -> &SimulationParameters {
        &self.params
    }

    /// Calculate expected price range with adaptable confidence level
    pub fn calculate_price_range(
        &mut self,
        current_price: f64,
        volatility: f64,
        confidence_level: f64,
        time_horizon_days: Option<f64>,
    ) -> PriceBoundary {
        // Update volatility parameter
        let mut params = self.params;
        params.volatility = volatility;

        // Update time horizon if specified
        if let Some(days) = time_horizon_days {
            params.time_horizon = days;
        }

        self.update_parameters(params);

        // Run simulations
        let price_paths = self.generate_price_paths(current_price);

        // Extract all prices (not just final ones) to calculate overall range
        let mut all_prices = Vec::new();
        for path in &price_paths {
            all_prices.extend(path.iter());
        }

        // Sort prices for percentile calculations
        all_prices.sort_by(|a: &f64, b: &f64| a.partial_cmp(b).unwrap());

        // Calculate percentiles based on requested confidence level
        let lower_percentile = (1.0 - confidence_level) / 2.0;
        let upper_percentile = 1.0 - lower_percentile;

        let lower_index = (all_prices.len() as f64 * lower_percentile).round() as usize;
        let upper_index = (all_prices.len() as f64 * upper_percentile).round() as usize;

        // Ensure indices are within valid range
        let lower_index = lower_index.max(0).min(all_prices.len() - 1);
        let upper_index = upper_index.max(0).min(all_prices.len() - 1);

        // Get boundary prices
        let lower_bound = all_prices[lower_index];
        let upper_bound = all_prices[upper_index];

        PriceBoundary {
            lower_price: lower_bound,
            upper_price: upper_bound,
            current_price,
            confidence: confidence_level,
        }
    }
}

// Implement the Default trait for StochasticModel
impl Default for StochasticModel {
    /// Create a default stochastic model
    fn default() -> Self {
        Self::new(SimulationParameters::default())
    }
}
