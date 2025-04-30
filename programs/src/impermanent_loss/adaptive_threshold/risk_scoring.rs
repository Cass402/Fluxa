use super::types::{PoolCategory, PoolCharacteristics};
use std::collections::HashMap;

/// Risk scoring system for categorizing pools and determining
/// appropriate IL mitigation strategies based on pool characteristics
#[derive(Debug)]
pub struct RiskScoring {
    // Base risk scores for different pool categories
    category_base_scores: HashMap<PoolCategory, f64>,

    // Feature importance weights for scoring adjustment
    liquidity_weight: f64,
    volume_weight: f64,
    fee_tier_weight: f64,
    market_cap_weight: f64,

    // Market cap thresholds for categorization (in USD)
    major_token_threshold: f64,
    mid_cap_threshold: f64,
}

impl RiskScoring {
    /// Create a new RiskScoring instance with custom parameters
    pub fn new(
        liquidity_weight: f64,
        volume_weight: f64,
        fee_tier_weight: f64,
        market_cap_weight: f64,
        major_token_threshold: f64,
        mid_cap_threshold: f64,
    ) -> Self {
        let mut category_base_scores = HashMap::new();
        category_base_scores.insert(PoolCategory::StablePair, 0.2); // Lowest risk
        category_base_scores.insert(PoolCategory::MajorPair, 0.5); // Medium risk
        category_base_scores.insert(PoolCategory::MajorStable, 0.4); // Medium-low risk
        category_base_scores.insert(PoolCategory::MidCapPair, 0.7); // Medium-high risk
        category_base_scores.insert(PoolCategory::LongTailPair, 0.9); // High risk
        category_base_scores.insert(PoolCategory::Custom, 0.6); // Default custom risk

        Self {
            category_base_scores,
            liquidity_weight,
            volume_weight,
            fee_tier_weight,
            market_cap_weight,
            major_token_threshold,
            mid_cap_threshold,
        }
    }

    /// Calculate risk score for a pool based on its characteristics
    /// Returns a normalized risk score between 0 (lowest risk) and 1 (highest risk)
    pub fn calculate_risk_score(&self, pool: &PoolCharacteristics) -> f64 {
        // Step 1: Categorize the pool
        let category = self.categorize_pool(pool);

        // Step 2: Get base risk score for this category
        let base_score = self.category_base_scores.get(&category).unwrap_or(&0.6);

        // Step 3: Calculate score adjustments based on specific characteristics
        let liquidity_adj = self.calculate_liquidity_adjustment(pool.liquidity_depth);
        let volume_adj = self.calculate_volume_adjustment(pool.trading_volume_24h);
        let fee_tier_adj = self.calculate_fee_tier_adjustment(pool.fee_tier);
        let market_cap_adj = self
            .calculate_market_cap_adjustment(pool.token0_market_cap.min(pool.token1_market_cap));

        // Step 4: Apply weighted adjustments to base score
        let final_score = base_score
            + (self.liquidity_weight * liquidity_adj)
            + (self.volume_weight * volume_adj)
            + (self.fee_tier_weight * fee_tier_adj)
            + (self.market_cap_weight * market_cap_adj);

        // Step 5: Normalize to 0-1 range
        final_score.clamp(0.0, 1.0)
    }

    /// Categorize a pool based on its characteristics
    pub fn categorize_pool(&self, pool: &PoolCharacteristics) -> PoolCategory {
        let min_market_cap = pool.token0_market_cap.min(pool.token1_market_cap);
        let max_market_cap = pool.token0_market_cap.max(pool.token1_market_cap);

        // Check for stablecoin pairs (very narrow price range)
        if pool.price_range_width < 0.01 {
            // 1% price range or less
            return PoolCategory::StablePair;
        }

        // Check if both tokens are major tokens
        if min_market_cap >= self.major_token_threshold {
            return PoolCategory::MajorPair;
        }

        // Check if one token is major and the other is potentially a stablecoin
        if max_market_cap >= self.major_token_threshold && pool.price_range_width < 0.05 {
            return PoolCategory::MajorStable;
        }

        // Check for mid-cap pairs
        if min_market_cap >= self.mid_cap_threshold {
            return PoolCategory::MidCapPair;
        }

        // Default to long tail assets
        PoolCategory::LongTailPair
    }

    /// Calculate risk score adjustment based on liquidity depth
    fn calculate_liquidity_adjustment(&self, liquidity: f64) -> f64 {
        // Higher liquidity reduces risk
        if liquidity >= 10_000_000.0 {
            // $10M+ liquidity
            -0.2
        } else if liquidity >= 1_000_000.0 {
            // $1M+ liquidity
            -0.1
        } else if liquidity >= 100_000.0 {
            // $100K+ liquidity
            0.0
        } else if liquidity >= 10_000.0 {
            // $10K+ liquidity
            0.1
        } else {
            0.2 // Low liquidity increases risk
        }
    }

    /// Calculate risk score adjustment based on trading volume
    fn calculate_volume_adjustment(&self, volume: f64) -> f64 {
        // Higher volume reduces risk to a point, but extremely high volume may indicate volatility
        if volume >= 50_000_000.0 {
            // $50M+ daily volume
            -0.05 // Extremely high volume - slight reduction
        } else if volume >= 10_000_000.0 {
            // $10M+ daily volume
            -0.15 // High volume - optimal reduction
        } else if volume >= 1_000_000.0 {
            // $1M+ daily volume
            -0.1
        } else if volume >= 100_000.0 {
            // $100K+ daily volume
            0.0
        } else if volume >= 10_000.0 {
            // $10K+ daily volume
            0.1
        } else {
            0.2 // Low volume increases risk
        }
    }

    /// Calculate risk score adjustment based on fee tier
    fn calculate_fee_tier_adjustment(&self, fee_tier: u64) -> f64 {
        // Higher fees typically indicate expected higher volatility
        match fee_tier {
            1 => -0.15, // 0.01% fee - typically stablecoins
            5 => -0.1,  // 0.05% fee - typically liquid major pairs
            30 => 0.0,  // 0.3% fee - standard tier
            100 => 0.1, // 1% fee - higher volatility expected
            _ => 0.0,   // Custom or unknown fee tier
        }
    }

    /// Calculate risk score adjustment based on market cap
    fn calculate_market_cap_adjustment(&self, min_market_cap: f64) -> f64 {
        // Higher market cap reduces risk
        if min_market_cap >= 10_000_000_000.0 {
            // $10B+
            -0.2
        } else if min_market_cap >= 1_000_000_000.0 {
            // $1B+
            -0.1
        } else if min_market_cap >= 100_000_000.0 {
            // $100M+
            0.0
        } else if min_market_cap >= 10_000_000.0 {
            // $10M+
            0.1
        } else {
            0.2 // Low market cap increases risk
        }
    }

    /// Set a custom base risk score for a specific pool category
    pub fn set_category_base_score(&mut self, category: PoolCategory, score: f64) {
        let normalized_score = score.clamp(0.0, 1.0);
        self.category_base_scores.insert(category, normalized_score);
    }
}

// Implement the Default trait for RiskScoring
impl Default for RiskScoring {
    fn default() -> Self {
        Self::new(
            0.25,             // Liquidity weight
            0.20,             // Volume weight
            0.15,             // Fee tier weight
            0.40,             // Market cap weight
            10_000_000_000.0, // $10B for major tokens
            1_000_000_000.0,  // $1B for mid-cap tokens
        )
    }
}
