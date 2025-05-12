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
/// √1.0001^MIN_TICK  × Q64  = floor(2^64 / 1.0001^887272)
pub const MIN_SQRT_PRICE: u128 = 0;

/// The maximum square root price limit for swaps
///
/// Corresponds to the maximum tick and represents the highest possible
/// sqrt(price) in Q64.64 fixed-point representation.
/// √1.0001^MAX_TICK  × Q64  = floor(1.0001^887272 × 2^64)
pub const MAX_SQRT_PRICE: u128 = 340269576636625053602161358042262667264;

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

/// Fixed-point scale
pub const Q64: u128 = 1u128 << 64; // single unified format

/// BPS Denominator
pub const BPS_DENOMINATOR: u128 = 10_000; // basis points denominator

/// Powers of √1.0001 for binary exponentiation.
/// Stores `floor((√1.0001)^(2^i) * Q64)` for `i = 0..19`.
/// `Q64 = 1u128 << 64`.
/// This table is used by `math::binary_pow` to calculate `(√1.0001)^exponent`.
pub const POWERS: [u128; 20] = [
    18_447_666_387_855_959_850,
    18_448_588_748_116_922_571,
    18_450_433_606_991_734_263,
    18_454_123_878_217_468_680,
    18_461_506_635_090_006_701,
    18_476_281_010_653_910_144,
    18_505_865_242_158_250_041,
    18_565_175_891_880_433_522,
    18_684_368_066_214_940_582,
    18_925_053_041_275_764_671,
    19_415_764_168_677_886_926,
    20_435_687_552_633_177_494,
    22_639_080_592_224_303_007,
    27_784_196_929_998_399_742,
    41_848_122_137_994_986_128,
    94_936_283_578_220_370_716,
    488_590_176_327_622_479_860,
    12_941_056_668_319_229_769_860,
    9_078_618_265_828_848_800_676_189,
    4_468_068_147_273_140_139_091_016_147_737,
];
