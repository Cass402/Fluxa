/// Utility modules for Fluxa AMM Core
///
/// This directory contains various utility modules that provide
/// supporting functionality for the core AMM components.
pub mod price_range;

// Re-export commonly used utilities for easier access
pub use price_range::calculate_impermanent_loss;
pub use price_range::PriceRange;
pub use price_range::PriceRangePreset;
