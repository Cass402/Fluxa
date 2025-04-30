use super::types::{PoolCategory, ThresholdParameters};
use std::collections::HashMap;

/// Configuration for adaptive thresholds based on pool categories
/// and various market conditions
#[derive(Debug)]
pub struct ThresholdConfig {
    // Base parameters for each pool category
    category_parameters: HashMap<PoolCategory, ThresholdParameters>,

    // Global threshold constraints
    min_rebalance_threshold: f64,
    max_rebalance_threshold: f64,
    min_position_width: f64,
    max_position_width: f64,

    // Dynamic adjustment parameters
    volatility_scaling_factor: f64,
    risk_scaling_factor: f64,
}

impl ThresholdConfig {
    /// Create a new threshold configuration with custom parameters
    pub fn new(
        min_rebalance_threshold: f64,
        max_rebalance_threshold: f64,
        min_position_width: f64,
        max_position_width: f64,
        volatility_scaling_factor: f64,
        risk_scaling_factor: f64,
    ) -> Self {
        let mut config = Self {
            category_parameters: HashMap::new(),
            min_rebalance_threshold,
            max_rebalance_threshold,
            min_position_width,
            max_position_width,
            volatility_scaling_factor,
            risk_scaling_factor,
        };

        // Initialize with default parameters for each category
        config.initialize_default_parameters();

        config
    }

    /// Initialize default threshold parameters for each pool category
    fn initialize_default_parameters(&mut self) {
        // Stablecoin pairs - very narrow price ranges, minimal IL
        self.category_parameters.insert(
            PoolCategory::StablePair,
            ThresholdParameters {
                volatility_base: 0.001, // 0.1% base volatility threshold
                liquidity_factor: 0.2,  // Liquidity has less impact
                volume_factor: 0.1,     // Volume has less impact
                fee_factor: 5.0,        // Fees have more impact
                range_factor: 0.5,      // Range width has moderate impact
                market_cap_factor: 0.1, // Market cap has less impact
            },
        );

        // Major token pairs - moderate volatility, significant liquidity
        self.category_parameters.insert(
            PoolCategory::MajorPair,
            ThresholdParameters {
                volatility_base: 0.01,  // 1% base volatility threshold
                liquidity_factor: 0.5,  // Moderate liquidity impact
                volume_factor: 0.3,     // Moderate volume impact
                fee_factor: 2.0,        // Moderate fee impact
                range_factor: 0.7,      // Significant range width impact
                market_cap_factor: 0.4, // Moderate market cap impact
            },
        );

        // Major token to stablecoin - asymmetric volatility
        self.category_parameters.insert(
            PoolCategory::MajorStable,
            ThresholdParameters {
                volatility_base: 0.015, // 1.5% base volatility threshold
                liquidity_factor: 0.4,  // Moderate liquidity impact
                volume_factor: 0.4,     // Higher volume impact
                fee_factor: 1.5,        // Moderate fee impact
                range_factor: 0.8,      // High range width impact
                market_cap_factor: 0.3, // Moderate market cap impact
            },
        );

        // Mid-cap token pairs - higher volatility
        self.category_parameters.insert(
            PoolCategory::MidCapPair,
            ThresholdParameters {
                volatility_base: 0.025, // 2.5% base volatility threshold
                liquidity_factor: 0.7,  // Higher liquidity impact
                volume_factor: 0.5,     // Higher volume impact
                fee_factor: 1.0,        // Lower fee impact
                range_factor: 0.6,      // Moderate range width impact
                market_cap_factor: 0.6, // Higher market cap impact
            },
        );

        // Long tail token pairs - highest volatility
        self.category_parameters.insert(
            PoolCategory::LongTailPair,
            ThresholdParameters {
                volatility_base: 0.05,  // 5% base volatility threshold
                liquidity_factor: 1.0,  // Highest liquidity impact
                volume_factor: 0.8,     // Highest volume impact
                fee_factor: 0.8,        // Lower fee impact
                range_factor: 0.5,      // Lower range width impact
                market_cap_factor: 1.0, // Highest market cap impact
            },
        );

        // Custom pool - moderate default parameters
        self.category_parameters
            .insert(PoolCategory::Custom, ThresholdParameters::default());
    }

    /// Get threshold parameters for a specific pool category
    pub fn get_parameters(&self, category: &PoolCategory) -> ThresholdParameters {
        self.category_parameters
            .get(category)
            .cloned()
            .unwrap_or_else(ThresholdParameters::default)
    }

    /// Set custom parameters for a specific pool category
    pub fn set_category_parameters(&mut self, category: PoolCategory, params: ThresholdParameters) {
        self.category_parameters.insert(category, params);
    }

    /// Calculate rebalance threshold based on volatility and risk score
    pub fn calculate_rebalance_threshold(
        &self,
        category: &PoolCategory,
        volatility: f64,
        risk_score: f64,
    ) -> f64 {
        let params = self.get_parameters(category);

        // Base threshold from volatility, scaled by category-specific factor
        let volatility_component =
            params.volatility_base * (1.0 + volatility * self.volatility_scaling_factor);

        // Risk adjustment component
        let risk_component = risk_score * self.risk_scaling_factor * params.volatility_base;

        // Combined threshold
        let rebalance_threshold = volatility_component + risk_component;

        // Constrain within global limits
        rebalance_threshold
            .max(self.min_rebalance_threshold)
            .min(self.max_rebalance_threshold)
    }

    /// Calculate optimal position width factor based on volatility and risk
    pub fn calculate_position_width_factor(
        &self,
        category: &PoolCategory,
        volatility: f64,
        risk_score: f64,
    ) -> f64 {
        let params = self.get_parameters(category);

        // Base width is influenced by volatility
        let base_width = 1.0 + (volatility * params.range_factor);

        // Adjust based on risk score - higher risk leads to narrower positions
        let risk_adjustment = 1.0 - (risk_score * 0.5);

        // Calculate final width factor
        let width_factor = base_width * risk_adjustment;

        // Constrain within global limits
        width_factor
            .max(self.min_position_width)
            .min(self.max_position_width)
    }

    /// Update global threshold constraints
    pub fn update_constraints(
        &mut self,
        min_rebalance: Option<f64>,
        max_rebalance: Option<f64>,
        min_width: Option<f64>,
        max_width: Option<f64>,
    ) {
        if let Some(min) = min_rebalance {
            self.min_rebalance_threshold = min;
        }

        if let Some(max) = max_rebalance {
            self.max_rebalance_threshold = max;
        }

        if let Some(min) = min_width {
            self.min_position_width = min;
        }

        if let Some(max) = max_width {
            self.max_position_width = max;
        }

        // Ensure min <= max
        self.min_rebalance_threshold = self
            .min_rebalance_threshold
            .min(self.max_rebalance_threshold);
        self.min_position_width = self.min_position_width.min(self.max_position_width);
    }

    /// Update scaling factors
    pub fn update_scaling_factors(
        &mut self,
        volatility_scaling: Option<f64>,
        risk_scaling: Option<f64>,
    ) {
        if let Some(vol) = volatility_scaling {
            self.volatility_scaling_factor = vol;
        }

        if let Some(risk) = risk_scaling {
            self.risk_scaling_factor = risk;
        }
    }
}

// Implement the Default trait for ThresholdConfig
impl Default for ThresholdConfig {
    fn default() -> Self {
        Self::new(
            0.005, // 0.5% minimum rebalance threshold
            0.10,  // 10% maximum rebalance threshold
            0.01,  // 1% minimum position width
            0.50,  // 50% maximum position width
            2.0,   // Volatility scaling
            1.5,   // Risk scaling
        )
    }
}
