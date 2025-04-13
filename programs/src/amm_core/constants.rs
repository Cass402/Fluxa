/// The minimum tick index supported in the protocol
pub const MIN_TICK: i32 = -887272;

/// The maximum tick index supported in the protocol
pub const MAX_TICK: i32 = 887272;

/// The minimum liquidity amount that can be provided to a position
pub const MIN_LIQUIDITY: u128 = 1000;

/// The minimum price limit for swaps
pub const MIN_SQRT_PRICE: u128 = 4295128739;

/// The maximum price limit for swaps
pub const MAX_SQRT_PRICE: u128 = 1461446703485210103287273052203988822378723970342;

/// Standard fee tiers available (in basis points)
pub const FEE_TIER_LOW: u16 = 100;    // 0.01%
pub const FEE_TIER_MEDIUM: u16 = 500;  // 0.05%
pub const FEE_TIER_HIGH: u16 = 3000;  // 0.3%

/// Tick spacing per fee tier
pub const TICK_SPACING_LOW: i32 = 1;    // For FEE_TIER_LOW
pub const TICK_SPACING_MEDIUM: i32 = 10;  // For FEE_TIER_MEDIUM
pub const TICK_SPACING_HIGH: i32 = 60;   // For FEE_TIER_HIGH

/// Protocol constants
pub const PROTOCOL_FEE_DENOMINATOR: u16 = 10000;