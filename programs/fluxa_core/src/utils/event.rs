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

    /// Security authority keys for protocol safety.
    /// Why: Ensures all security and risk controls are auditable and traceable to specific accounts.
    pub secureity_coordinator: Pubkey,
    pub multisig_config: Pubkey,
    pub core_authority: Pubkey,
    pub audit_trail_head: Pubkey,
    pub emergency_contacts: Pubkey,

    /// Multisig threshold and required confirmations for governance actions.
    /// Why: Enforces protocol governance and risk controls; ensures all critical actions are auditable and require consensus
    /// for security.
    pub multisig_threshold: u8,
    pub required_confirmations: u8,
}
