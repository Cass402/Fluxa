pub mod boundary_calculator;
pub mod il_estimator;
pub mod stochastic_model;
pub mod position_optimizer;

pub use boundary_calculator::BoundaryCalculator;
pub use il_estimator::ImpermanentLossEstimator;
pub use stochastic_model::StochasticModel;
pub use position_optimizer::PositionOptimizer;

// Re-export common types and interfaces
pub mod types {
    use std::fmt;
    
    #[derive(Debug, Clone)]
    pub struct PriceBoundary {
        pub lower_price: f64,
        pub upper_price: f64,
        pub current_price: f64,
        pub confidence: f64,
    }
    
    impl fmt::Display for PriceBoundary {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Price boundary: {:.4} to {:.4} (current: {:.4}, confidence: {:.1}%)",
                self.lower_price, self.upper_price, self.current_price, self.confidence * 100.0)
        }
    }
    
    #[derive(Debug, Clone)]
    pub struct OptimalPosition {
        pub boundaries: PriceBoundary,
        pub estimated_il_reduction: f64,
        pub estimated_fee_apy: f64,
        pub net_estimated_apy: f64,
        pub recommended_rebalance_frequency: u64, // In minutes
    }
    
    impl fmt::Display for OptimalPosition {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Optimal position: {}, IL reduction: {:.1}%, Est. Fee APY: {:.1}%, Net Est. APY: {:.1}%, Rebalance freq: {}min",
                self.boundaries, self.estimated_il_reduction * 100.0, 
                self.estimated_fee_apy * 100.0, self.net_estimated_apy * 100.0,
                self.recommended_rebalance_frequency)
        }
    }
    
    #[derive(Debug, Clone, Copy)]
    pub struct SimulationParameters {
        pub num_simulations: usize,
        pub time_horizon: f64,  // In days
        pub time_steps: usize,
        pub drift: f64,
        pub volatility: f64,
        pub mean_reversion_strength: Option<f64>,
    }
    
    impl Default for SimulationParameters {
        fn default() -> Self {
            Self {
                num_simulations: 1000,
                time_horizon: 30.0,  // 30 day simulation
                time_steps: 720,     // Hourly steps
                drift: 0.0,          // No drift by default
                volatility: 0.5,     // Annualized volatility 50%
                mean_reversion_strength: Some(0.1), // Mild mean reversion
            }
        }
    }
    
    #[derive(Debug, Clone)]
    pub struct ModelParameters {
        pub fee_tier: u16,           // In basis points (e.g., 30 = 0.3%)
        pub price_impact_factor: f64, // Factor for price impact when rebalancing
        pub gas_cost_estimate: f64,   // In USD
        pub slippage_tolerance: f64,  // In percentage (e.g., 0.01 = 1%)
    }
    
    impl Default for ModelParameters {
        fn default() -> Self {
            Self {
                fee_tier: 30,        // 0.3% fee tier
                price_impact_factor: 0.0001, // 1bps per $10K
                gas_cost_estimate: 5.0,    // $5 estimate
                slippage_tolerance: 0.005,  // 0.5% slippage tolerance
            }
        }
    }
}