use anchor_lang::prelude::*;

/// Event emitted on pool creation, providing full context for downstream indexers, analytics, and audits.
///
/// # Why this event?
/// - Captures all protocol-critical state at pool genesis, enabling deterministic reconstruction and forensic analysis.
/// - Includes all account keys, pool parameters, and PDA bumps for future instruction validation and replay protection.
/// - Downstream indexers rely on this event for pool discovery, historical analytics, and governance tracking.
#[event]
pub struct PoolCreatedEvent {
    /// Core pool account key.
    ///
    /// Why: Canonical source of truth for pool state; referenced by all downstream instructions and analytics.
    pub pool_core: Pubkey,

    /// Security account key for pool.
    ///
    /// Why: Tracks MEV protection, circuit breaker, and suspicious activity for protocol safety.
    pub pool_security: Pubkey,

    /// Configuration account key for pool.
    ///
    /// Why: Stores protocol fee, volatility tracker, and governance controls.
    pub pool_config: Pubkey,

    /// Factory shard account key managing this pool.
    ///
    /// Why: Enables tracking of pool distribution across shards for load balancing and analytics.
    pub factory_shard: Pubkey,

    /// Token mint addresses for the pool.
    ///
    /// Why: Immutable asset pair; used for all downstream validation and accounting.
    pub token_0: Pubkey,
    pub token_1: Pubkey,

    /// Vault account keys for custody of pool assets.
    ///
    /// Why: Ensures secure, program-controlled custody of user funds.
    pub vault_0: Pubkey,
    pub vault_1: Pubkey,

    /// Pool parameters at genesis.
    ///
    /// Why: Fee and tick spacing are protocol invariants; initial price and tick are required for deterministic state reconstruction.
    pub fee: u16,
    pub tick_spacing: u16,
    pub init_sqrt_price: u128,
    pub init_tick: i32,

    /// Cached PDA bumps for all pool accounts.
    ///
    /// Why: Enables deterministic address derivation and replay protection for future instructions.
    pub bump_core: u8,
    pub bump_security: u8,
    pub bump_config: u8,
    pub bump_vault_0: u8,
    pub bump_vault_1: u8,

    /// Metadata for auditability and analytics.
    ///
    /// Why: Creator, timestamp, and slot provide full provenance and enable historical analysis.
    pub creator: Pubkey,
    pub timestamp: i64,
    pub slot: u64,
}

/// Event emitted on position creation, providing full context for downstream indexers, analytics, and audits.
///
/// # Why this event?
/// - Captures all position-critical state at creation, enabling deterministic reconstruction and portfolio tracking.
/// - Includes position type (individual vs batched), slippage metrics, and precision optimization status.
/// - Downstream indexers rely on this event for LP position tracking, historical analytics, and yield calculations.
#[event]
pub struct PositionCreatedEvent {
    /// Position account key (for individual positions).
    ///
    /// Why: Canonical identifier for the position; None for batched positions stored in PositionBatch.
    pub position: Pubkey,

    /// Owner of the position.
    ///
    /// Why: Enables filtering and access control for position management operations.
    pub owner: Pubkey,

    /// Pool core account this position belongs to.
    ///
    /// Why: Links position to its parent pool for analytics and validation.
    pub pool_core: Pubkey,

    /// Tick range defining the position's price boundaries.
    ///
    /// Why: Core parameters determining when the position earns fees and capital efficiency.
    pub tick_lower: i32,
    pub tick_upper: i32,

    /// Liquidity amount in Q64x64 raw format.
    ///
    /// Why: The fundamental measure of the position's size; stored as u128 for event compatibility.
    pub liquidity: u128,

    /// Actual token amounts deposited after slippage.
    ///
    /// Why: Enables accurate portfolio tracking and reconciliation with on-chain balances.
    pub amount_0: u64,
    pub amount_1: u64,

    /// Whether this position is stored in a batch vs. individual account.
    ///
    /// Why: Affects how the position is managed and queried downstream.
    pub is_batched: bool,

    /// Batch identifier if position is batched.
    ///
    /// Why: Required for looking up batched positions; None for individual positions.
    pub batch_id: Option<u32>,

    /// Unique nonce for this position (used in PDA derivation).
    ///
    /// Why: Enables multiple positions with same tick range for same owner.
    pub position_nonce: u16,

    /// Actual slippage experienced in basis points.
    ///
    /// Why: Enables analytics on execution quality and MEV impact.
    pub slippage_0_bps: u16,
    pub slippage_1_bps: u16,

    /// Whether precision optimization was applied to liquidity calculation.
    ///
    /// Why: Tracks usage of optional optimization feature for analytics.
    pub precision_optimized: bool,

    /// Metadata for auditability and analytics.
    ///
    /// Why: Timestamp and slot provide full provenance and enable historical analysis.
    pub timestamp: i64,
    pub slot: u64,
}
