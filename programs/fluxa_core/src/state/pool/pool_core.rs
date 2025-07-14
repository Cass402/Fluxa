use crate::math::core_arithmetic::Q64x64;
use anchor_lang::prelude::*;

#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct PoolCore {
    /// Token addresses (64 bytes total)
    /// 'token_0' - Address of token 0
    /// 'token_1' - Address of token 1
    pub token_0: Pubkey,
    pub token_1: Pubkey,

    /// Core price and liquidity data (40 bytes)
    /// 'sqrt_price' - Current square root price in Q64.64 fixed point
    /// 'liquidity' - Current active liquidity in Q64.64 fixed point
    /// 'tick_current' - Current tick index
    /// 'tick_spacing' - Tick spacing for the pool
    /// 'fee' - Fee in basis points (0-10000)
    pub sqrt_price: Q64x64, // Q64.64 fixed point
    pub liquidity: Q64x64, // Current active liquidity
    pub tick_current: i32, // Current tick
    pub tick_spacing: u16, // Tick spacing
    pub fee: u16,          // Fee in basis points (0-10000)

    /// Fee growth tracking (32 bytes)
    /// 'fee_growth_global_0' - Global fee growth for token 0 in Q64.64 fixed point
    /// 'fee_growth_global_1' - Global fee growth for token 1 in Q64.64 fixed point
    pub fee_growth_global_0: Q64x64, // Q64.64 fixed point
    pub fee_growth_global_1: Q64x64, // Q64.64 fixed point

    /// Operational metadata (16 bytes)
    /// 'last_update_slot' - Slot of the last price update
    /// 'protocol_version' - Version of the protocol for future upgrades
    /// 'status_flags' - Bitfield for operational status (e.g., paused, emergency)
    /// '_padding' - Padding for alignment to 8-byte boundary
    pub last_update_slot: u64, // Last price update slot
    pub protocol_version: u16, // For future upgrades
    pub status_flags: u16,     // Bitfield for operational status
    pub _padding: [u8; 4],     // Align to 8-byte boundary

    /// Reserve space for future enhancements (64 bytes)
    pub reserved: [u64; 8],
}
