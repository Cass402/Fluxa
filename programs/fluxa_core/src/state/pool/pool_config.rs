use crate::state::pool::volatility_tracker::EwmaVolatilityTracker;
use anchor_lang::prelude::*;

/// Highly optimized Pool Config with EWMA volatility tracking
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct OptimizedPoolConfig {
    /// Pool Account reference
    pub pool_core: Pubkey,

    /// Protocol fees
    /// 'protocol_fee_0' - Fee in basis points for token 0
    /// 'protocol_fee_1' - Fee in basis points for token 1
    /// 'protocol_fees_token_0' - Accumulated fees in token 0
    /// 'protocol_fees_token_1' - Accumulated fees in token 1
    pub protocol_fee_0: u32,
    pub protocol_fee_1: u32,
    pub protocol_fees_token_0: u64,
    pub protocol_fees_token_1: u64,

    /// Optimized EWMA volatility tracker
    pub volatility_tracker: EwmaVolatilityTracker,

    /// Dynamic fee configuration
    //pub dynamic_fee_config: OptimizedDynamicFeeConfig,

    /// Trading limits and controls
    /// 'max_swap_limit' - Maximum swap limit
    /// 'daily_volume_limit' - Rolling 24-hour volume limit
    /// 'last_volume_reset' - Slot of last volume reset
    pub max_swap_limit: u64, // 0 means no limit
    pub daily_volume_limit: u64, // Rolling 24-hour limit
    pub last_volume_reset: u64,  // Slot of last reset

    /// Core authority for Governance and upgrades
    pub core_authority: Pubkey, // Authority for critical changes

    /// Future expansion
    pub reserved: [u64; 8],
}
