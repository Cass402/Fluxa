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

/// Tick constants
pub const DEFAULT_SUSPICIOUS_THRESHOLD: u32 = 1000; // Default threshold for suspicious activity detection
                                                    // Status flag bit positions for efficient operations
pub const FLAG_ACTIVE: u16 = 0x01; // 0000 0001
pub const FLAG_EMERGENCY_PAUSE: u16 = 0x02; // 0000 0010
pub const FLAG_REQUIRES_AUDIT: u16 = 0x04; // 0000 0100
pub const FLAG_HIGH_VOLUME: u16 = 0x08; // 0000 1000
pub const MAX_TICK_SPACING: u16 = 1000; // Maximum tick spacing for the Fluxa protocol
pub const MAX_BITMAP_CAPACITY: i32 = 65536; // Maximum capacity for tick bitmap (2^16)
pub const SLOTS_PER_MINUTE: u64 = 150; // Number of slots per minute
pub const MAX_TICK_CROSSES_PER_HOUR: u32 = 10000; // Maximum tick crosses allowed per hour
pub const MIN_TICK_CROSS_INTERVAL: u64 = 2; // Minimum interval between tick crosses
pub const SUSPICIOUS_CROSS_INTERVAL: u64 = 10; // Interval for suspicious tick crosses
pub const RESET_SUSPICION_INTERVAL: u64 = 1000; // Interval to reset suspicion
pub const MAX_SUSPICION_SCORE: u32 = 100; // Maximum suspicion score

/// Position constants
pub const MAX_POSITIONS_PER_BATCH: usize = 200; // Maximum number of positions that can be processed in a single batch
pub const POSITION_HASH_SIZE: usize = 32; // Size of the position hash in bytes
pub const MERKLE_TREE_DEPTH: usize = 20; // Depth of the Merkle tree for position storage
pub const ACCOUNT_SIZE_LIMIT: usize = 10_240; // 10 KiB limit
