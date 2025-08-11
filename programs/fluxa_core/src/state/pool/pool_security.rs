use crate::{math::core_arithmetic::Q64x64, utils::constants::SECURITY_FLAG_DEFAULT};
use anchor_lang::prelude::*;

/// Security and risk management state for a concentrated liquidity pool.
///
/// # Why this structure?
/// - Uses zero-copy layout for maximum on-chain efficiency and deterministic account size, critical for Solana's rent and compute model.
/// - All fields are fixed-size and aligned, with no dynamic allocations, ensuring safety and predictable performance.
/// - Packs all security, MEV protection, and circuit breaker logic into a single account for atomic updates and easier auditing.
/// - Bitfields and booleans are used for efficient flag management, minimizing storage and compute costs.
///
/// ## Usage
/// This struct is the canonical security and risk control state for a pool, referenced by all swap, admin, and monitoring logic.
#[account(zero_copy(unsafe))]
#[derive(InitSpace)] // Expecting 208 bytes
#[repr(C)]
pub struct PoolSecurity {
    /// Reference to the associated PoolCore account.
    ///
    /// Why: Ensures this security state is always bound to a specific pool, preventing misconfiguration or spoofing. Used for Anchor constraint validation.
    pub pool_core: Pubkey,

    /// Security flags packed into a single u32 bitfield.
    ///
    /// Why: Bitfields allow multiple security statuses (e.g., MEV protection, emergency pause) to be tracked compactly and atomically, minimizing storage and compute. Enables efficient flag checks and updates.
    pub security_flags: u32,

    /// Volume and position tracking for risk monitoring.
    ///
    /// - `total_swap_volume_0`/`total_swap_volume_1`: Cumulative swap volume for each token, in Q64.64. Why: Enables detection of abnormal activity and supports circuit breaker logic. Fixed-point ensures precision and overflow safety.
    /// - `active_positions_count`: Number of active LP positions. Why: Used for monitoring pool health and potential attack surface.
    pub active_positions_count: u32,
    pub total_swap_volume_0: Q64x64,
    pub total_swap_volume_1: Q64x64,

    /// Security monitoring and anomaly detection.
    ///
    /// - `last_security_check`: Last slot when security checks were performed. Why: Enables time-based logic and replay protection.
    /// - `suspicious_activity_score`: Composite score for suspicious activity (0-1000). Why: Allows for nuanced threat detection and automated circuit breaker triggers.
    pub last_security_check: u64,
    pub suspicious_activity_score: u32,

    /// MEV protection parameters and emergency contacts.
    ///
    /// - `mev_protection_enabled`: Enables/disables MEV protection logic. Why: Allows for dynamic risk management and protocol upgrades without redeploying the contract.
    /// - `emergency_contacts`: Pubkey for emergency response (e.g., multisig, DAO). Why: Enables rapid intervention in case of attack or critical failure.
    pub mev_protection_enabled: bool,
    // Future: Add MEV protection window, price impact, and volume spike thresholds for more granular controls.
    pub emergency_contacts: Pubkey,
    pub security_coordinator: Pubkey,

    /// Circuit breaker state and configuration.
    ///
    /// - `circuit_breaker_triggered_at`: Slot when circuit breaker was last triggered. Why: Enables time-based lockouts and post-mortem analysis.
    /// - `circuit_breaker_threshold`: Threshold for triggering circuit breaker (e.g., suspicious activity score, volume spike). Why: Allows for flexible, automated risk controls.
    pub circuit_breaker_triggered_at: u64,
    pub circuit_breaker_threshold: u32,

    /// Alignment and future expansion.
    ///
    /// - `_padding`: Ensures 8-byte alignment for Anchor zero-copy safety and future extensibility.
    /// - `reserved`: Pre-allocated space for future upgrades (e.g., new risk controls, monitoring fields) without breaking account layout.
    pub bump_security: u8, // Cache bump for efficiency
    pub enterprise_mode: bool, // Whether this pool is in enterprise mode with enhanced security/compliance
    pub _padding: [u8; 2],
    pub reserved: [u64; 4],
}

impl Default for PoolSecurity {
    fn default() -> Self {
        Self {
            pool_core: Pubkey::default(),
            security_flags: SECURITY_FLAG_DEFAULT,
            active_positions_count: 0,
            total_swap_volume_0: Q64x64::zero(),
            total_swap_volume_1: Q64x64::zero(),
            last_security_check: 0,
            suspicious_activity_score: 0,
            mev_protection_enabled: true,
            emergency_contacts: Pubkey::default(),
            security_coordinator: Pubkey::default(),
            circuit_breaker_triggered_at: 0,
            circuit_breaker_threshold: 0,
            bump_security: 0,
            enterprise_mode: false,
            _padding: [0; 2],
            reserved: [0; 4],
        }
    }
}
