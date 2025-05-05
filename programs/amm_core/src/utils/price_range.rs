/// Price Range Utility Module
///
/// This module provides utilities for working with price ranges in the Fluxa AMM.
/// It includes functions for converting between prices and ticks, calculating optimal
/// price ranges based on different strategies, and validating price inputs.
use crate::errors::ErrorCode;
use anchor_lang::prelude::*;

/// Enum defining standard price range presets for liquidity positions
#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub enum PriceRangePreset {
    /// Narrow range - typically ±5% around current price
    /// Optimizes for high capital efficiency and higher yield in stable markets
    Narrow,

    /// Medium range - typically ±15% around current price
    /// Balanced approach suitable for most market conditions
    Medium,

    /// Wide range - typically ±50% around current price
    /// More resistant to impermanent loss in volatile markets
    Wide,

    /// Custom range specified directly by the user
    Custom,
}

/// The PriceRange struct represents a price range for concentrated liquidity positions.
/// It provides utilities for converting between price values and tick indices,
/// as well as standardized presets for common liquidity provision strategies.
#[derive(Clone, Debug, PartialEq, AnchorSerialize, AnchorDeserialize)]
pub struct PriceRange {
    /// Lower price bound for the position
    pub lower_price: f64,

    /// Upper price bound for the position
    pub upper_price: f64,

    /// Lower tick index (derived from lower_price)
    pub lower_tick: i32,

    /// Upper tick index (derived from upper_price)
    pub upper_tick: i32,

    /// The preset used to create this range, if any
    pub preset: PriceRangePreset,
}

impl PriceRange {
    /// Tick base that determines the price increment per tick
    /// Follows Uniswap v3 convention: price = 1.0001^tick
    const TICK_BASE: f64 = 1.0001;

    /// Creates a new custom price range from explicit price bounds
    ///
    /// # Arguments
    /// * `lower_price` - Lower price bound
    /// * `upper_price` - Upper price bound
    ///
    /// # Returns
    /// A new PriceRange with the corresponding tick indices
    pub fn new_from_prices(lower_price: f64, upper_price: f64) -> Result<Self> {
        if lower_price >= upper_price {
            return Err(error!(ErrorCode::InvalidPriceRange));
        }

        let lower_tick = Self::price_to_tick(lower_price);
        let upper_tick = Self::price_to_tick(upper_price);

        Ok(Self {
            lower_price,
            upper_price,
            lower_tick,
            upper_tick,
            preset: PriceRangePreset::Custom,
        })
    }

    /// Creates a new custom price range from explicit tick indices
    ///
    /// # Arguments
    /// * `lower_tick` - Lower tick index
    /// * `upper_tick` - Upper tick index
    ///
    /// # Returns
    /// A new PriceRange with the corresponding price bounds
    pub fn new_from_ticks(lower_tick: i32, upper_tick: i32) -> Result<Self> {
        if lower_tick >= upper_tick {
            return Err(error!(ErrorCode::InvalidTickRange));
        }

        let lower_price = Self::tick_to_price(lower_tick);
        let upper_price = Self::tick_to_price(upper_tick);

        Ok(Self {
            lower_price,
            upper_price,
            lower_tick,
            upper_tick,
            preset: PriceRangePreset::Custom,
        })
    }

    /// Creates a standardized price range based on a preset and current price
    ///
    /// # Arguments
    /// * `preset` - The range preset (Narrow, Medium, Wide)
    /// * `current_price` - The current price of the pool
    ///
    /// # Returns
    /// A new PriceRange with bounds determined by the preset
    pub fn new_from_preset(preset: PriceRangePreset, current_price: f64) -> Result<Self> {
        if current_price <= 0.0 {
            return Err(error!(ErrorCode::InvalidPrice));
        }

        // Define width percentages based on preset
        let (lower_pct, upper_pct) = match preset {
            PriceRangePreset::Narrow => (0.95, 1.05), // ±5%
            PriceRangePreset::Medium => (0.85, 1.15), // ±15%
            PriceRangePreset::Wide => (0.50, 1.50),   // ±50%
            PriceRangePreset::Custom => return Err(error!(ErrorCode::InvalidPreset)),
        };

        // Calculate price bounds
        let lower_price = current_price * lower_pct;
        let upper_price = current_price * upper_pct;

        // Convert to tick indices
        let lower_tick = Self::price_to_tick(lower_price);
        let upper_tick = Self::price_to_tick(upper_price);

        Ok(Self {
            lower_price,
            upper_price,
            lower_tick,
            upper_tick,
            preset,
        })
    }

    /// Converts a price to the nearest tick index
    ///
    /// # Arguments
    /// * `price` - The price to convert
    ///
    /// # Returns
    /// The corresponding tick index
    pub fn price_to_tick(price: f64) -> i32 {
        (price.ln() / Self::TICK_BASE.ln()).floor() as i32
    }

    /// Converts a tick index to its corresponding price
    ///
    /// # Arguments
    /// * `tick` - The tick index to convert
    ///
    /// # Returns
    /// The corresponding price
    pub fn tick_to_price(tick: i32) -> f64 {
        Self::TICK_BASE.powf(tick as f64)
    }

    /// Returns the width of the range as a percentage
    ///
    /// # Returns
    /// The range width as a percentage
    pub fn width_percentage(&self) -> f64 {
        ((self.upper_price / self.lower_price) - 1.0) * 100.0
    }

    /// Checks if the current price is within this range
    ///
    /// # Arguments
    /// * `current_price` - The price to check
    ///
    /// # Returns
    /// True if the price is within the range, false otherwise
    pub fn price_in_range(&self, current_price: f64) -> bool {
        current_price >= self.lower_price && current_price <= self.upper_price
    }

    /// Bounds a price range to ensure it fits within the allowable tick range
    ///
    /// # Arguments
    /// * `lower_tick` - The initial lower tick boundary
    /// * `upper_tick` - The initial upper tick boundary
    ///
    /// # Returns
    /// * `(i32, i32)` - The bounded (lower_tick, upper_tick) values
    pub fn bound_tick_range(lower_tick: i32, upper_tick: i32) -> (i32, i32) {
        const MIN_TICK: i32 = -887272; // Corresponds to min price ~= 4.3e-39
        const MAX_TICK: i32 = 887272; // Corresponds to max price ~= 2.3e38

        let bounded_lower = lower_tick.max(MIN_TICK);
        let bounded_upper = upper_tick.min(MAX_TICK);

        (bounded_lower, bounded_upper)
    }

    /// Adjusts a tick value to align with a given tick spacing
    ///
    /// # Arguments
    /// * `tick` - The tick to adjust
    /// * `spacing` - The tick spacing to align with
    /// * `round_up` - Whether to round up (true) or down (false)
    ///
    /// # Returns
    /// * `i32` - The adjusted tick that aligns with the spacing
    pub fn align_tick_to_spacing(tick: i32, spacing: i32, round_up: bool) -> i32 {
        if spacing <= 1 {
            return tick;
        }

        let remainder = tick % spacing;
        if remainder == 0 {
            return tick;
        }

        if remainder > 0 {
            if round_up {
                tick + (spacing - remainder)
            } else {
                tick - remainder
            }
        } else if round_up {
            tick - remainder
        } else {
            tick - (spacing + remainder)
        }
    }
}

/// Calculate the impermanent loss for a given price movement within a range
///
/// # Arguments
/// * `start_price` - Initial price when position was created
/// * `current_price` - Current price
/// * `lower_price` - Lower bound of the position's range
/// * `upper_price` - Upper bound of the position's range
///
/// # Returns
/// * `f64` - Impermanent loss as a percentage (e.g., 0.05 for 5% loss)
pub fn calculate_impermanent_loss(
    start_price: u128,
    current_price: u128,
    lower_price: u128,
    upper_price: u128,
) -> f64 {
    // Convert to f64 for calculations
    let p0 = start_price as f64;
    let p1 = current_price as f64;
    let pl = lower_price as f64;
    let pu = upper_price as f64;

    // Price ratio
    let price_ratio = p1 / p0;

    // For a concentrated liquidity position, IL is more complex than standard AMM
    let sqrt_ratio = price_ratio.sqrt();

    // If price is outside the range, calculate differently
    if p1 < pl || p1 > pu {
        // Position is entirely in one asset, calculate IL accordingly
        let hodl_value = if p1 < pl {
            // All in token B
            2.0 * p0 / (p0 + p1)
        } else {
            // All in token A
            2.0 * p1 / (p0 + p1)
        };
        return hodl_value - 1.0;
    }

    // For price in range, use standard concentrated liquidity IL formula
    let lp_value = (2.0 * sqrt_ratio) / (1.0 + price_ratio);
    lp_value - 1.0
}
