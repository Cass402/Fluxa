use crate::math::core_arithmetic::Q64x64;

/// Core protocol bounds for tick and price math.
///
/// # Why
/// These constants define the minimum and maximum tick and sqrt price values allowed by the protocol.
/// They are chosen to ensure all math remains within safe, deterministic, and auditable bounds, and to prevent overflows or underflows in Q64.64 math.
pub const MIN_TICK: i32 = -443_636;
pub const MAX_TICK: i32 = 443_636;
pub const MIN_SQRT_X64: u128 = 4295128739;
pub const MAX_SQRT_X64: u128 = 79226673521066979257578248091u128;
pub const FRAC_BITS: u32 = 64; // Q64.64: 64 fractional bits for fixed-point math, maximizing precision and range.
pub const ONE_X64: u128 = 1u128 << FRAC_BITS; // Canonical representation of 1.0 in Q64.64, used for normalization and protocol invariants.
pub const MAX_SAFE: u128 = u128::MAX; // Used for overflow checks in Q64.64 math; ensures all calculations remain safe.
pub const MAX_TOKEN_AMOUNT: u64 = 1_000_000_000_000_000_000; // Protocol-imposed cap to prevent overflow and DoS via excessive token amounts.

/// Security authority and timelock parameters.
///
/// # Why
/// These delays are designed to balance protocol safety (time to react to governance or admin changes) with usability.
/// They prevent instant privilege escalation or rug pulls, and give users/auditors time to respond to changes.
pub const AUTHORITY_CHANGE_DELAY: i64 = 48 * 3600; // 48h: Minimum notice for authority changes, deterring governance attacks.
pub const EMERGENCY_PAUSE_TIMEOUT: i64 = 24 * 7 * 3600; // 7d: Maximum duration for emergency pause, ensuring protocol liveness.
pub const MIN_DELAY: i64 = 24 * 3600; // 24h: Minimum timelock for sensitive operations, enforcing transparency.
pub const MAX_DELAY: i64 = 30 * 24 * 3600; // 30d: Maximum timelock, preventing indefinite lockup of protocol actions.

/// Factory-level protocol configuration.
///
/// # Why
/// These constants define the maximums and defaults for pool creation, fee tiers, and protocol status.
/// They are chosen to ensure scalability, prevent resource exhaustion, and enable efficient bitwise status management.
pub const MAX_FEE_TIERS: usize = 8; // Limits fee tier array size for zero-copy and deterministic account layouts.
pub const MAX_POOLS_PER_SHARD: usize = 256; // Prevents a single shard from exhausting compute/memory.
pub const MAX_SHARDS: usize = 64; // Maximum number of shards per factory for efficient management and storage.
pub const DEFAULT_PROTOCOL_FEE: u32 = 100; // 1% default, balancing protocol revenue and user cost.
pub const POOL_CREATION_FEE: u64 = 1_000_000; // Small fee to deter spam and cover storage costs.
pub const DEFAULT_FEE_TIERS: [u32; MAX_FEE_TIERS] = [100, 500, 3000, 10000, 0, 0, 0, 0]; // Preallocated for zero-copy, unused slots are zeroed.
                                                                                         // Status flags use bitwise encoding for efficient, atomic updates and multi-flag support.
pub const STATUS_NORMAL: u8 = 0x00;
pub const STATUS_PAUSED: u8 = 0x01;
pub const STATUS_EMERGENCY: u8 = 0x02;
pub const STATUS_MAINTENANCE: u8 = 0x04;
pub const STATUS_DEPRECATED: u8 = 0x08;

/// Pool-level risk and analytics parameters.
///
/// # Why
/// These values are used for volatility and risk calculations, e.g., EWMA volatility tracking.
/// Chosen to match industry standards (RiskMetrics) and to ensure protocol safety under stress.
pub const STANDARD_LAMBDA: u32 = 61604; // Q16.16 fixed-point encoding of 0.94, for volatility decay.
pub const TICK_SPACING_PER_FEE: [(u32, u16); 4] = [(100, 1), (500, 10), (3000, 60), (10000, 200)];
pub const SECURITY_FLAG_DEFAULT: u32 = 0x01; // Default security flag for new pools, indicating normal operation.

/// Tick-level protocol and security parameters.
///
/// # Why
/// These constants are used for anomaly detection, tick bitmap management, and efficient status flagging.
/// Bitwise flags enable atomic, multi-flag status updates with minimal compute.
pub const DEFAULT_SUSPICIOUS_THRESHOLD: u32 = 1000; // Threshold for flagging suspicious tick activity, deterring manipulation.
pub const FLAG_ACTIVE: u16 = 0x01; // Bitwise: enables efficient status checks and updates.
pub const FLAG_EMERGENCY_PAUSE: u16 = 0x02;
pub const FLAG_REQUIRES_AUDIT: u16 = 0x04;
pub const FLAG_HIGH_VOLUME: u16 = 0x08;
pub const MAX_TICK_SPACING: u16 = 1000; // Prevents excessive tick fragmentation, improving AMM efficiency.
pub const MAX_BITMAP_CAPACITY: i32 = 65536; // 2^16: Chosen for efficient bitmap storage and lookup.
pub const SLOTS_PER_MINUTE: u64 = 150; // Solana-specific: used for time-based rate limiting and analytics.
pub const MAX_TICK_CROSSES_PER_HOUR: u32 = 10000; // Prevents DoS via excessive tick crossing.
pub const MIN_TICK_CROSS_INTERVAL: u64 = 2; // Enforces minimum time between tick crosses, deterring bots.
pub const SUSPICIOUS_CROSS_INTERVAL: u64 = 10; // Used for anomaly detection in tick crossing patterns.
pub const RESET_SUSPICION_INTERVAL: u64 = 1000; // Resets suspicion score after inactivity, preventing permanent flagging.
pub const MAX_SUSPICION_SCORE: u32 = 100; // Caps suspicion score to prevent overflow and ensure bounded state.
pub const TICKS_PER_PAGE: usize = 150; // ~7.2KB per page account
pub const MAX_STORAGE_PAGES: usize = 20; // Support up to 3000 total ticks
pub const INLINE_TICK_CAPACITY: usize = 50; // Ticks stored in main account
pub const MAX_LOSS_PCT: u8 = 5; // Precision guard
pub const VIRTUAL_TICK_OFFSET: i32 = 262_144; // 2^18 for wider range coverage
pub const MAX_CACHE_ENTRIES: usize = 256; // Cache size (stack allocated)
pub const HOT_TICK_THRESHOLD: u16 = 3; // Minimum access count to be "hot"
pub const MAIN_BITMAP_WORDS: usize = 64; // 4096 bits in main account
pub const PAGE_BITMAP_WORDS: usize = 8; // 512 bits per page

/// Position and storage constraints.
///
/// # Why
/// These constants are chosen to ensure efficient, zero-copy account layouts and to prevent resource exhaustion.
/// They also enable Merkle proofs and batch operations for scalability and auditability.
pub const MAX_POSITIONS_PER_BATCH: usize = 200; // Bounded for compute/memory safety in batch ops.
pub const POSITION_HASH_SIZE: usize = 32; // 32 bytes = 256 bits, matches cryptographic hash output.
pub const MERKLE_TREE_DEPTH: usize = 20; // Chosen for balance between proof size and storage efficiency.
pub const ACCOUNT_SIZE_LIMIT: usize = 10_240; // 10 KiB: fits within Solana account size limits, prevents overuse.

/// Account Optimization constants
/// Maximum Solana account size limit (10 KiB)
pub const MAX_ACCOUNT_SIZE: usize = 10 * 1024;
/// Account overhead bytes (discriminator + metadata + padding)
/// This reserves space for account discriminator, rent exemption data, and alignment
pub const ACCOUNT_OVERHEAD_BYTES: usize = 128;
/// Rent safety buffer as bit-shift amount (1/32 = ~3.125% buffer)
/// Using bit-shift for gas-efficient division: amount >> 5 ≈ amount / 32
pub const RENT_BUFFER_SHIFT: u32 = 5;

/// Flash loan protection parameters.
pub const EWMA_ALPHA_Q64: Q64x64 = Q64x64::from_raw(3_689_348_814_741_910_323); // 0.2 in Q64.64
pub const RISK_DECAY_RATE_Q64: Q64x64 = Q64x64::from_raw(461_168_601_842_738_790); // 0.025 in Q64.64
pub const MIN_VOLUME_FLOR_Q64: Q64x64 = Q64x64::from_raw(18_446_744_073_709_551_616_000_000); // 1,000,000(1M) in Q64.64
pub const PRECISION_FACTOR_Q64: Q64x64 = Q64x64::from_raw(184_467_440_737_095_516_160_000); // 10,000(1e4) in Q64.64
pub const SLOT_BUCKET_COUNT: usize = 8; // Number of slot buckets for tracking
pub const OPERATION_WINDOW_SIZE: usize = 32; // power of 2 for efficiency
                                             // Fixed-point precision for smooth decay
pub const DECAY_PRECISION: u32 = 10_000; // 4 decimal places
                                         // Bitset aging configuration
pub const BITSET_AGING_SLOTS: u64 = 100; // Number of slots before aging bitsets
pub const PATTERN_CACHE_SIZE: usize = 4; // LRU cache size
pub const HIGH_IMPACT_THRESHOLD: Q64x64 = Q64x64::from_raw(922337203685477581); // 5% impact (0.05 in Q64.64)
pub const LARGE_AMOUNT_THRESHOLD: Q64x64 = Q64x64::from_int(10_000_000); // 10 million (10,000,000) in Q64.64
pub const FLASH_SEQUENCE_PATTERN: u8 = 0b010010; // Add (01) -> Swap (00) -> Remove (10) = 0b010010 in 6-bit window (decimal equivalent: 18)
pub const MAX_DISTINCT_USERS: usize = 32;
pub const GLOBAL_BUFFER_SIZE: usize = 64;
