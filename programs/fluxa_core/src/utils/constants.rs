pub const MIN_TICK: i32 = -443_636; // Minimum tick value for the Fluxa protocol
pub const MAX_TICK: i32 = 443_636; // Maximum tick value for the Fluxa protocol
pub const MIN_SQRT_X64: u128 = 4295128739; // Minimum square root value in Q64.64 format ()
pub const MAX_SQRT_X64: u128 = 79226673521066979257578248091u128; // Maximum square root value in Q64.64 format
pub const FRAC_BITS: u32 = 64; // Q64.64 fractional bits
pub const ONE_X64: u128 = 1u128 << FRAC_BITS; // Represents 1 in Q64.64 format
pub const MAX_SAFE: u128 = u128::MAX; // Maximum safe value for Q64.64 (to avoid overflow in calculations)
pub const MAX_TOKEN_AMOUNT: u64 = 1_000_000_000_000_000_000; // 1 billion tokens with 18 decimals

/// Fee tier constants for common pool configurations
pub const FEE_TIER_0_01: u32 = 100; // 0.01%
pub const FEE_TIER_0_05: u32 = 500; // 0.05%
pub const FEE_TIER_0_30: u32 = 3000; // 0.30%
pub const FEE_TIER_1_00: u32 = 10000; // 1.00%

/// Security Authority constants
pub const AUTHORITY_CHANGE_DELAY: i64 = 48 * 3600; // 48 hours in seconds
pub const EMERGENCY_PAUSE_TIMEOUT: i64 = 24 * 7 * 3600; // 7 days in seconds

/// Timelock operation constants
pub const MIN_DELAY: i64 = 24 * 3600; // 24 hours
pub const MAX_DELAY: i64 = 30 * 24 * 3600; // 30 days
