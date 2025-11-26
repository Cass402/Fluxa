use crate::math::core_arithmetic::Q64x64;
use anchor_lang::prelude::*;

/// Concentrated liquidity pool core state with zero-copy optimization for high-frequency operations.
///
/// This structure serves as the single source of truth for all pool state, designed specifically
/// for Solana's constrained runtime environment where memory allocation and deserialization
/// overhead can severely impact transaction throughput and computational limits.
///
/// ## Zero-Copy Architecture Rationale
/// Uses `#[account(zero_copy(unsafe))]` because concentrated liquidity pools require frequent
/// state access during swaps, position management, and price updates. Traditional Anchor
/// deserialization would create significant overhead for every transaction, potentially
/// causing CU limit violations in complex operations. Zero-copy enables direct memory
/// manipulation without serialization costs, crucial for maintaining sub-millisecond
/// transaction processing in high-frequency DeFi environments.
///
/// ## Fixed-Size Field Strategy  
/// Every field uses fixed-size types (no Vec, String, or variable-length data) to achieve:
/// - Deterministic account size for predictable rent calculations and storage costs
/// - Cache-friendly memory layout enabling efficient CPU access patterns
/// - Prevention of memory fragmentation attacks where malicious users could create
///   oversized accounts to exhaust validator memory
/// - Elimination of dynamic allocation failures during critical operations
///
/// ## Memory Alignment Considerations
/// Uses `#[repr(C)]` to ensure consistent memory layout across different compilation
/// targets and prevent subtle bugs from memory padding differences. This is essential
/// for zero-copy operations where direct memory access assumes specific byte offsets.
///
/// ## Protocol State Isolation
/// Serves as the coordination point for all pool operations while maintaining clear
/// separation from security, configuration, and audit concerns through linked accounts.
/// This separation enables independent evolution of different protocol aspects without
/// requiring costly state migrations.
#[account(zero_copy(unsafe))]
#[derive(InitSpace)] // Expecting 224 bytes
#[repr(C)]
pub struct PoolCore {
    /// Immutable token pair defining the pool's trading assets with canonical ordering.
    ///
    /// These fields are set once during initialization and never modified, serving as the
    /// fundamental identity of the pool. The ordering (token_0 < token_1) is enforced
    /// during creation to ensure deterministic pool addresses and prevent duplicate
    /// pools for the same asset pair. This canonical ordering also simplifies swap
    /// logic by eliminating conditional branches for token direction handling.
    pub token_0: Pubkey,
    pub token_1: Pubkey,

    /// Current market state using fixed-point arithmetic for precision and safety.
    ///
    /// sqrt_price: Square root of current price in Q64.64 fixed-point format.
    /// This representation choice stems from Uniswap V3's insight that using √P instead
    /// of P enables more efficient tick calculations and reduces precision loss during
    /// price movements. Q64.64 provides sufficient precision for even micro-cent assets
    /// while avoiding floating-point arithmetic issues that could lead to exploitation.
    ///
    /// liquidity: Active liquidity available at current tick in Q64.64 format.
    /// Represents the amount of virtual tokens available for trading at the current
    /// price. Fixed-point arithmetic prevents rounding errors that could be exploited
    /// to drain pools through precision manipulation attacks.
    ///
    /// tick_current: Current tick index representing the price point.
    /// Ticks provide discrete price levels for range orders, with each tick representing
    /// a 0.01% price change. This discretization enables efficient liquidity management
    /// and prevents dust positions that could bloat state storage.
    ///
    /// tick_spacing: Minimum tick increment enforced by fee tier.
    /// Higher fees require wider tick spacing to ensure adequate compensation for
    /// liquidity providers relative to the granularity of positions. This prevents
    /// excessive fragmentation of liquidity that could degrade trading efficiency.
    ///
    /// fee: Trading fee in basis points (0-10000).
    /// u16 is sufficient for all practical fee structures while preventing overflow
    /// in fee calculations. Basis points provide fine-grained control needed for
    /// competitive fee optimization.
    pub sqrt_price: Q64x64,
    pub liquidity: Q64x64,
    pub tick_current: i32,
    pub tick_spacing: u16,
    pub fee: u16,

    pub _padding1: [u8; 8], // Padding to align next Q64.64 fields to 16-byte boundary

    /// Global fee accumulation tracking for precise liquidity provider compensation.
    ///
    /// These accumulators implement a gas-efficient mechanism for tracking fees earned
    /// by all liquidity providers without requiring iteration over individual positions.
    /// Each accumulator represents the cumulative fees per unit of liquidity since pool
    /// inception, enabling O(1) fee calculation for any position regardless of pool size.
    ///
    /// The Q64.64 precision ensures that even tiny fees accumulate accurately over
    /// millions of transactions, preventing value leakage that could be exploited
    /// or cause accounting discrepancies in long-running pools.
    ///
    /// This design choice trades a small amount of storage (2 * 128 bits) for massive
    /// computational savings compared to maintaining individual fee balances, crucial
    /// for maintaining transaction throughput as pool usage scales.
    pub fee_growth_global_0: Q64x64,
    pub fee_growth_global_1: Q64x64,

    /// Operational metadata enabling temporal logic and protocol evolution.
    ///
    /// last_update_slot: Solana slot when pool state was last modified.
    /// Essential for implementing time-based logic like TWAP oracles, rate limiting,
    /// and MEV protection. Slot-based timing is preferred over Unix timestamps because
    /// slots provide guaranteed monotonic progression and are harder to manipulate,
    /// crucial for preventing timestamp-based attacks on time-sensitive features.
    ///
    /// protocol_version: Semantic version for handling protocol upgrades.
    /// Enables safe migration strategies and backward compatibility checks during
    /// protocol evolution. Critical for long-lived pools that must survive multiple
    /// protocol iterations without requiring costly state migrations or user action.
    ///
    /// status_flags: Bitfield encoding multiple operational states efficiently.
    /// Each bit represents a different status (paused, emergency, deprecated, etc.)
    /// allowing atomic status checks and updates. Bitwise operations are extremely
    /// efficient on Solana's BPF runtime, and compact encoding minimizes storage costs
    /// while enabling complex state combinations (e.g., "paused but emergency-withdrawable").
    ///
    /// Bump caching: Pre-computed PDA bumps for gas optimization.
    /// Storing bump seeds eliminates the need to recompute them during every transaction
    /// that accesses pool vaults or derives related accounts. This seemingly small
    /// optimization can save significant compute units in complex operations involving
    /// multiple account derivations.
    ///
    /// _padding: Ensures proper struct alignment for zero-copy safety.
    /// Memory alignment is critical for zero-copy operations to prevent undefined
    /// behavior from misaligned memory access, which could cause transaction failures
    /// or subtle corruption bugs.
    pub last_update_slot: u64,
    pub protocol_version: u16,
    pub status_flags: u16,
    pub bump_core: u8,
    pub bump_vault_0: u8,
    pub bump_vault_1: u8,
    pub _padding2: [u8; 1],

    /// Future-proofing space preventing costly account migrations during protocol evolution.
    ///
    /// Pre-allocating 64 bytes (8 * u64) provides substantial expansion room for new
    /// features without requiring users to migrate to new account layouts. This design
    /// choice reflects the lessons learned from early DeFi protocols where adding new
    /// features required complex and expensive migration processes that often left
    /// legacy pools stranded with outdated functionality.
    ///
    /// The reserved space enables seamless protocol upgrades for features like:
    /// - New fee structures or rebate mechanisms
    /// - Advanced MEV protection parameters
    /// - Cross-chain bridge integration metadata
    /// - Enhanced oracle price feeds
    /// - Regulatory compliance fields
    ///
    /// Using u64 array ensures proper alignment while providing flexibility for
    /// various future field types. The space cost (64 bytes per pool) is minimal
    /// compared to the value of upgrade flexibility for long-lived DeFi infrastructure.
    pub reserved: [u64; 8],
}

impl Default for PoolCore {
    fn default() -> Self {
        Self {
            token_0: Pubkey::default(),
            token_1: Pubkey::default(),
            sqrt_price: Q64x64::zero(),
            liquidity: Q64x64::zero(),
            tick_current: 0,
            tick_spacing: 0,
            fee: 0,
            _padding1: [0; 8],
            fee_growth_global_0: Q64x64::zero(),
            fee_growth_global_1: Q64x64::zero(),
            last_update_slot: 0,
            protocol_version: 1,
            status_flags: 0,
            bump_core: 0,
            bump_vault_0: 0,
            bump_vault_1: 0,
            _padding2: [0; 1],
            reserved: [0; 8],
        }
    }
}

impl PoolCore {
    /// Enterprise upgrade status flags for tracking multi-phase initialization
    pub const SECURITY_FOUNDATION_INITIALIZED: u16 = 0x01;
    pub const AUDIT_SYSTEM_INITIALIZED: u16 = 0x02;
    pub const ENTERPRISE_MODE_ACTIVE: u16 = 0x04;

    /// Check if pool has enterprise capabilities enabled
    pub fn is_enterprise(&self) -> bool {
        self.status_flags & Self::ENTERPRISE_MODE_ACTIVE != 0
    }

    /// Check if security foundation is initialized
    pub fn has_security_foundation(&self) -> bool {
        self.status_flags & Self::SECURITY_FOUNDATION_INITIALIZED != 0
    }

    /// Check if audit system is initialized
    pub fn has_audit_system(&self) -> bool {
        self.status_flags & Self::AUDIT_SYSTEM_INITIALIZED != 0
    }

    /// Check if pool is ready for enterprise upgrade finalization
    pub fn ready_for_enterprise_finalization(&self) -> bool {
        self.has_security_foundation() && self.has_audit_system() && !self.is_enterprise()
    }
}
