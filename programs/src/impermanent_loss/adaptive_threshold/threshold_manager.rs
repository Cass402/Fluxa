use super::risk_scoring::RiskScoring;
use super::threshold_config::ThresholdConfig;
use super::types::{AdaptiveThreshold, PoolCategory, PoolCharacteristics};

/// Threshold manager that combines risk scoring and threshold configuration
/// to dynamically determine optimal thresholds for impermanent loss mitigation
#[derive(Debug)]
pub struct ThresholdManager {
    risk_scoring: RiskScoring,
    threshold_config: ThresholdConfig,
}

impl ThresholdManager {
    /// Create a new threshold manager with custom components
    pub fn new(risk_scoring: RiskScoring, threshold_config: ThresholdConfig) -> Self {
        Self {
            risk_scoring,
            threshold_config,
        }
    }

    /// Calculate adaptive thresholds based on pool characteristics and volatility
    pub fn calculate_adaptive_threshold(
        &self,
        pool_characteristics: &PoolCharacteristics,
        volatility: f64,
    ) -> AdaptiveThreshold {
        // Step 1: Categorize pool and calculate risk score
        let category = self.risk_scoring.categorize_pool(pool_characteristics);
        let risk_score = self.risk_scoring.calculate_risk_score(pool_characteristics);

        // Step 2: Calculate rebalance threshold and position width factor
        let rebalance_threshold = self
            .threshold_config
            .calculate_rebalance_threshold(&category, volatility, risk_score);

        let position_width_factor = self
            .threshold_config
            .calculate_position_width_factor(&category, volatility, risk_score);

        // Step 3: Calculate confidence level
        // Higher confidence with more data and stable conditions
        let confidence = self.calculate_confidence(pool_characteristics, volatility);

        AdaptiveThreshold {
            rebalance_threshold,
            position_width_factor,
            confidence,
        }
    }

    /// Calculate confidence level in the threshold recommendation
    fn calculate_confidence(&self, pool: &PoolCharacteristics, volatility: f64) -> f64 {
        // Base confidence starts high
        let mut confidence = 0.8;

        // Reduce confidence for newer pools (less historical data)
        if pool.age_in_days < 30 {
            confidence -= (30 - pool.age_in_days) as f64 * 0.01;
        }

        // Reduce confidence for low liquidity pools
        if pool.liquidity_depth < 100_000.0 {
            confidence -= 0.1;
        }

        // Reduce confidence for extremely high or low volatility
        if !(0.005..=0.5).contains(&volatility) {
            confidence -= 0.15;
        }

        // Constrain confidence to valid range
        confidence.clamp(0.3, 0.95)
    }

    /// Get pool category based on characteristics
    pub fn get_pool_category(&self, pool: &PoolCharacteristics) -> PoolCategory {
        self.risk_scoring.categorize_pool(pool)
    }

    /// Get risk score for a pool
    pub fn get_risk_score(&self, pool: &PoolCharacteristics) -> f64 {
        self.risk_scoring.calculate_risk_score(pool)
    }

    /// Access the risk scoring component
    pub fn risk_scoring(&self) -> &RiskScoring {
        &self.risk_scoring
    }

    /// Access the risk scoring component mutably
    pub fn risk_scoring_mut(&mut self) -> &mut RiskScoring {
        &mut self.risk_scoring
    }

    /// Access the threshold configuration component
    pub fn threshold_config(&self) -> &ThresholdConfig {
        &self.threshold_config
    }

    /// Access the threshold configuration component mutably
    pub fn threshold_config_mut(&mut self) -> &mut ThresholdConfig {
        &mut self.threshold_config
    }
}

// Implement the Default trait for ThresholdManager
impl Default for ThresholdManager {
    /// Create a default threshold manager
    fn default() -> Self {
        Self {
            risk_scoring: RiskScoring::default(),
            threshold_config: ThresholdConfig::default(),
        }
    }
}
