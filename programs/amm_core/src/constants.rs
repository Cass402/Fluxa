/// Fluxa AMM Core Protocol Constants
///
/// This module defines the fundamental protocol parameters and boundaries that govern
/// the operation of the Fluxa AMM. These constants are crucial for maintaining protocol
/// security, economic stability, and operational functionality across all implementations.
/// The minimum tick index supported in the protocol
///
/// Defines the lowest possible price representation in the system.
/// Calculated as log_1.0001(minimum representable price).
/// At this tick, the price is approximately 0.000000000000000000000000000000000001 (10^-36).
pub const MIN_TICK: i32 = -887272;

/// The maximum tick index supported in the protocol
///
/// Defines the highest possible price representation in the system.
/// Calculated as log_1.0001(maximum representable price).
/// At this tick, the price is approximately 10^36.
pub const MAX_TICK: i32 = 887272;

/// The minimum liquidity amount that can be provided to a position
///
/// This prevents dust positions and ensures a meaningful minimum economic value
/// for any position in the system. Denominated in L-units (liquidity units).
pub const MIN_LIQUIDITY: u128 = 1000;

/// The minimum square root price limit for swaps
///
/// Corresponds to the minimum tick and represents the lowest possible
/// sqrt(price) in Q64.64 fixed-point representation.
/// This value equals approximately 4.295e-9 * 2^64.
pub const MIN_SQRT_PRICE: u128 = 4295128739;

/// The maximum square root price limit for swaps
///
/// Corresponds to the maximum tick and represents the highest possible
/// sqrt(price) in Q64.64 fixed-point representation.
/// Using a value close to the maximum representable u128
pub const MAX_SQRT_PRICE: u128 = u128::MAX;

/// Standard fee tiers available (in basis points)
///
/// Low fee tier (0.01%)
/// Optimized for stable pairs (e.g., USDC-USDT) with minimal price impact.
pub const FEE_TIER_LOW: u16 = 100;

/// Medium fee tier (0.05%)
/// Balanced for mainstream token pairs with moderate volatility.
pub const FEE_TIER_MEDIUM: u16 = 500;

/// High fee tier (0.3%)
/// Designed for exotic pairs or high volatility tokens.
pub const FEE_TIER_HIGH: u16 = 3000;

/// Tick spacing per fee tier
///
/// Tick spacing for the low fee tier (0.01%)
/// Each tick represents a 0.01% price change, using single-tick granularity.
pub const TICK_SPACING_LOW: i32 = 1;

/// Tick spacing for the medium fee tier (0.05%)
/// Price changes of 0.1% (10 * 0.01%) using coarser granularity.
pub const TICK_SPACING_MEDIUM: i32 = 10;

/// Tick spacing for the high fee tier (0.3%)
/// Price changes of 0.6% (60 * 0.01%) using even coarser granularity.
pub const TICK_SPACING_HIGH: i32 = 60;

/// Protocol fee denominator
///
/// Used to calculate protocol fees as a fraction of collected trading fees.
/// For example, if protocol fee is set to 1667, the protocol receives
/// 1667/10000 (≈16.67%) of all collected fees.
pub const PROTOCOL_FEE_DENOMINATOR: u16 = 10000;
