pub const MIN_TICK: i32 = -443_636; // Minimum tick value for the Fluxa protocol
pub const MAX_TICK: i32 = 443_636; // Maximum tick value for the Fluxa protocol
pub const MIN_SQRT_X64: u128 = 4295128739; // Minimum square root value in Q64.64 format ()
pub const MAX_SQRT_X64: u128 = 79226673521066979257578248091u128; // Maximum square root value in Q64.64 format
pub const FRAC_BITS: u32 = 64; // Q64.64 fractional bits
pub const ONE_X64: u128 = 1u128 << FRAC_BITS; // Represents 1 in Q64.64 format
pub const MAX_SAFE: u128 = u128::MAX; // Maximum safe value for Q64.64 (to avoid overflow in calculations)
pub const MAX_TOKEN_AMOUNT: u64 = 1_000_000_000_000_000_000; // 1 billion tokens with 18 decimals

/// Security Authority constants
pub const AUTHORITY_CHANGE_DELAY: i64 = 48 * 3600; // 48 hours in seconds
pub const EMERGENCY_PAUSE_TIMEOUT: i64 = 24 * 7 * 3600; // 7 days in seconds

/// Timelock operation constants
pub const MIN_DELAY: i64 = 24 * 3600; // 24 hours
pub const MAX_DELAY: i64 = 30 * 24 * 3600; // 30 days

/// Factory constants
pub const MAX_FEE_TIERS: usize = 8; // Maximum number of supported fee tiers in the factory
pub const MAX_POOLS_PER_SHARD: usize = 256; // Maximum number of pools per shard in the factory
pub const DEFAULT_PROTOCOL_FEE: u32 = 100; // Default protocol fee rate in basis points (1%)
pub const POOL_CREATION_FEE: u64 = 1_000_000; // Creation fee for new pools in lamports (0.001 SOL)
pub const DEFAULT_FEE_TIERS: [u32; MAX_FEE_TIERS] = [100, 500, 3000, 10000, 0, 0, 0, 0];
pub const STATUS_NORMAL: u8 = 0x00; // Normal status flag for factory
pub const STATUS_PAUSED: u8 = 0x01; // Paused status flag for factory
pub const STATUS_EMERGENCY: u8 = 0x02; // Emergency status flag for factory
pub const STATUS_MAINTENANCE: u8 = 0x04; // Maintenance status flag for factory
pub const STATUS_DEPRECATED: u8 = 0x08; // Deprecated status flag for factory

/// Pool constants
pub const STANDARD_LAMBDA: u32 = 61604; // Standard RiskMetrics lambda value (0.94) in fixed point (Q16.16)
