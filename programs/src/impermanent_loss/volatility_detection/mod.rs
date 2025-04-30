pub mod garch_model;
pub mod time_weighted;
pub mod volatility_calculator;

pub use garch_model::GarchModel;
pub use time_weighted::TimeWeightedVolatility;
pub use volatility_calculator::VolatilityCalculator;

// Re-export common types and interfaces
pub mod types {
    use core::fmt;
    use std::collections::VecDeque;

    #[derive(Debug, Clone)]
    pub struct PriceDataPoint {
        pub timestamp: u64,
        pub price: f64,
    }

    #[derive(Debug, Clone)]
    pub struct VolatilityScore {
        pub value: f64,
        pub confidence: f64,
        pub window_size: usize,
    }

    impl fmt::Display for VolatilityScore {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "Volatility: {:.4} (Confidence: {:.2}%)",
                self.value,
                self.confidence * 100.0
            )
        }
    }

    #[derive(Debug, Clone)]
    pub struct PriceHistory {
        pub data: VecDeque<PriceDataPoint>,
        pub max_size: usize,
    }

    impl PriceHistory {
        pub fn new(max_size: usize) -> Self {
            Self {
                data: VecDeque::with_capacity(max_size),
                max_size,
            }
        }

        pub fn add_price_point(&mut self, point: PriceDataPoint) {
            if self.data.len() >= self.max_size {
                self.data.pop_front();
            }
            self.data.push_back(point);
        }

        pub fn get_returns(&self) -> Vec<f64> {
            if self.data.len() < 2 {
                return Vec::new();
            }

            let mut returns = Vec::with_capacity(self.data.len() - 1);
            let mut prev_price = self.data[0].price;

            for i in 1..self.data.len() {
                let current_price = self.data[i].price;
                let ret = (current_price - prev_price) / prev_price;
                returns.push(ret);
                prev_price = current_price;
            }

            returns
        }

        pub fn get_log_returns(&self) -> Vec<f64> {
            if self.data.len() < 2 {
                return Vec::new();
            }

            let mut log_returns = Vec::with_capacity(self.data.len() - 1);
            let mut prev_price = self.data[0].price;

            for i in 1..self.data.len() {
                let current_price = self.data[i].price;
                let log_ret = (current_price / prev_price).ln();
                log_returns.push(log_ret);
                prev_price = current_price;
            }

            log_returns
        }
    }
}
