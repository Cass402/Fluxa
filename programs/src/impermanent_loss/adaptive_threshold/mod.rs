pub mod risk_scoring;
pub mod threshold_config;
pub mod threshold_manager;

pub use risk_scoring::RiskScoring;
pub use threshold_config::ThresholdConfig;
pub use threshold_manager::ThresholdManager;

// Re-export common types and interfaces
pub mod types {
    use std::fmt;

    #[derive(Debug, Clone, Copy)]
    pub struct PoolCharacteristics {
        pub liquidity_depth: f64,    // Total value locked in the pool
        pub trading_volume_24h: f64, // 24-hour trading volume
        pub fee_tier: u64,           // Fee tier in basis points (e.g., 30 = 0.3%)
        pub price_range_width: f64,  // Width of the price range as a percentage
        pub age_in_days: u64,        // Age of the pool in days
        pub token0_market_cap: f64,  // Market cap of token0
        pub token1_market_cap: f64,  // Market cap of token1
    }

    #[derive(Debug, Clone)]
    pub struct ThresholdParameters {
        pub volatility_base: f64,   // Base threshold for volatility
        pub liquidity_factor: f64,  // Adjustment factor for liquidity depth
        pub volume_factor: f64,     // Adjustment factor for trading volume
        pub fee_factor: f64,        // Adjustment factor for fee tier
        pub range_factor: f64,      // Adjustment factor for price range width
        pub market_cap_factor: f64, // Adjustment factor for market capitalization
    }

    impl Default for ThresholdParameters {
        fn default() -> Self {
            Self {
                volatility_base: 0.02,  // 2% base volatility threshold
                liquidity_factor: 0.5,  // Higher liquidity reduces threshold
                volume_factor: 0.3,     // Higher volume increases threshold
                fee_factor: 2.0,        // Higher fees increase threshold
                range_factor: 0.7,      // Wider ranges reduce threshold
                market_cap_factor: 0.4, // Higher market cap reduces threshold
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct AdaptiveThreshold {
        pub rebalance_threshold: f64, // Price movement % that triggers rebalance
        pub position_width_factor: f64, // Factor to determine optimal position width
        pub confidence: f64,          // Confidence in the threshold (0-1)
    }

    impl fmt::Display for AdaptiveThreshold {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "Rebalance at: {:.2}% movement, Position width factor: {:.2}, Confidence: {:.1}%",
                self.rebalance_threshold * 100.0,
                self.position_width_factor,
                self.confidence * 100.0
            )
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum PoolCategory {
        StablePair,   // Two stablecoins (e.g., USDC-DAI)
        MajorPair,    // Major crypto pairs (e.g., ETH-BTC)
        MajorStable,  // Major crypto to stablecoin (e.g., ETH-USDC)
        MidCapPair,   // Mid-cap crypto assets
        LongTailPair, // Lower-cap, volatile assets
        Custom,       // Custom categorization
    }

    impl fmt::Display for PoolCategory {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                PoolCategory::StablePair => write!(f, "Stablecoin Pair"),
                PoolCategory::MajorPair => write!(f, "Major Crypto Pair"),
                PoolCategory::MajorStable => write!(f, "Major-Stable Pair"),
                PoolCategory::MidCapPair => write!(f, "Mid-Cap Crypto Pair"),
                PoolCategory::LongTailPair => write!(f, "Long Tail Assets"),
                PoolCategory::Custom => write!(f, "Custom Pool"),
            }
        }
    }
}
