use crate::math::core_arithmetic::Q64x64;
use crate::state::pool::volatility_tracker::EwmaVolatilityTracker;
use crate::utils::constants::DEFAULT_PROTOCOL_FEE;
use anchor_lang::prelude::*;

/// Pool configuration and risk controls for a concentrated liquidity pool.
///
/// # Why this structure?
/// - Uses zero-copy layout for maximum on-chain efficiency and deterministic account size, critical for Solana's rent and compute model.
/// - All fields are fixed-size and aligned, with no dynamic allocations, ensuring safety and predictable performance.
/// - Integrates protocol fee, risk, and governance controls in a single account for atomic updates and easier auditing.
///
/// ## Usage
/// This struct is the canonical configuration and risk control state for a pool, referenced by all admin, fee, and limit logic.
#[account(zero_copy(unsafe))]
#[derive(InitSpace)] // Expecting 260 bytes
#[repr(C)]
pub struct PoolConfig {
    /// Reference to the associated PoolCore account.
    ///
    /// Why: Ensures this config is always bound to a specific pool, preventing misconfiguration or spoofing. Used for Anchor constraint validation.
    pub pool_core: Pubkey,

    /// Protocol fee configuration and accounting.
    ///
    /// - `protocol_fee_0`/`protocol_fee_1`: Fee in basis points for each token. Why: Allows for asymmetric fee structures, supporting advanced protocol monetization.
    /// - `protocol_fees_token_0`/`protocol_fees_token_1`: Accumulated protocol fees, in Q64.64. Why: Fixed-point ensures precision and prevents rounding errors in fee accounting.
    pub protocol_fee_0: u32,
    pub protocol_fee_1: u32,
    pub protocol_fees_token_0: Q64x64,
    pub protocol_fees_token_1: Q64x64,

    /// Exponentially-weighted moving average (EWMA) volatility tracker.
    ///
    /// Why: Enables dynamic risk controls and fee adjustments based on real-time market volatility, without expensive on-chain computation. EWMA is chosen for its simplicity and low memory footprint.
    pub volatility_tracker: EwmaVolatilityTracker,

    // Dynamic fee configuration could be added here in the future for adaptive fee models.
    /// Trading limits and risk controls.
    ///
    /// - `max_swap_limit`: Maximum allowed swap size. Why: Protects against flash loan attacks and excessive slippage. 0 means no limit.
    /// - `daily_volume_limit`: Rolling 24-hour volume cap. Why: Prevents runaway volume and potential protocol abuse.
    /// - `last_volume_reset`: Slot of last volume reset. Why: Enables efficient rolling window logic without expensive on-chain iteration.
    pub max_swap_limit: u64, // 0 means no limit
    pub daily_volume_limit: u64, // Rolling 24-hour limit
    pub last_volume_reset: u64,  // Slot of last reset

    /// Core authority for governance and upgrades.
    ///
    /// Why: Only this authority can make critical changes, ensuring protocol safety and upgradability. Storing as Pubkey allows for multisig or DAO integration.
    pub core_authority: Pubkey,

    pub bump_config: u8,   // Cache bump for efficiency
    pub _padding: [u8; 7], // Align to 8-byte boundary

    /// Reserved for future upgrades (e.g., new risk controls, fee models) without breaking account layout.
    ///
    /// Why: Pre-allocating space allows for seamless upgrades and avoids costly migrations or rent increases.
    pub reserved: [u64; 8],
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            pool_core: Pubkey::default(),
            protocol_fee_0: DEFAULT_PROTOCOL_FEE,
            protocol_fee_1: DEFAULT_PROTOCOL_FEE,
            protocol_fees_token_0: Q64x64::zero(),
            protocol_fees_token_1: Q64x64::zero(),
            volatility_tracker: EwmaVolatilityTracker::new(60),
            max_swap_limit: 0,
            daily_volume_limit: 0,
            last_volume_reset: 0,
            core_authority: Pubkey::default(),
            bump_config: 0,
            _padding: [0; 7],
            reserved: [0; 8],
        }
    }
}
