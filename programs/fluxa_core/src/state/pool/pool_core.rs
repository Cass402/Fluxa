use crate::math::core_arithmetic::Q64x64;
use anchor_lang::prelude::*;

/// Core state for a concentrated liquidity pool.
///
/// # Why this structure?
/// - Uses zero-copy layout for maximum on-chain efficiency, avoiding serialization overhead and enabling direct memory access in Anchor.
/// - All fields are fixed-size and aligned for deterministic account size and predictable rent costs.
/// - No dynamic allocations (e.g., Vec), ensuring safety and performance on Solana's constrained runtime.
///
/// ## Usage
/// This struct is the canonical source of truth for pool state, referenced by all swap, liquidity, and admin instructions.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct PoolCore {
    /// Token mint addresses for the pool.
    ///
    /// Why: Immutable after initialization, these fields define the asset pair and are used for all downstream validation and accounting. Storing as Pubkey ensures compatibility with SPL tokens and Anchor constraints.
    pub token_0: Pubkey,
    pub token_1: Pubkey,

    /// Core price and liquidity state, all in fixed-point for precision and safety.
    ///
    /// - `sqrt_price`: Square root of the current price, in Q64.64. Why: Using sqrt(P) enables efficient tick math and price movement calculations, as in Uniswap v3.
    /// - `liquidity`: Current active liquidity, in Q64.64. Why: Fixed-point avoids rounding errors and overflows in swap math.
    /// - `tick_current`: Current tick index. Why: Ticks are the fundamental unit for range orders and liquidity management.
    /// - `tick_spacing`: Minimum tick step. Why: Enforces granularity and prevents excessive tick bloat.
    /// - `fee`: Fee in basis points (0-10000). Why: Basis points allow for fine-grained fee control, and u16 is sufficient for all practical use cases.
    pub sqrt_price: Q64x64,
    pub liquidity: Q64x64,
    pub tick_current: i32,
    pub tick_spacing: u16,
    pub fee: u16,

    /// Global fee growth accumulators for each token, in Q64.64.
    ///
    /// Why: Tracks total fees earned by all liquidity providers, enabling precise, gas-efficient fee accounting. Q64.64 ensures no loss of precision over long periods.
    pub fee_growth_global_0: Q64x64,
    pub fee_growth_global_1: Q64x64,

    /// Operational metadata for protocol upgrades and status management.
    ///
    /// - `last_update_slot`: Last slot when price/liquidity was updated. Why: Enables time-based logic (e.g., TWAP, rate limits) and replay protection.
    /// - `protocol_version`: Version for future upgrades. Why: Allows for safe migrations and backward compatibility.
    /// - `status_flags`: Bitfield for operational status (e.g., paused, emergency). Why: Bitwise flags allow multiple statuses to be tracked compactly and atomically, minimizing storage and compute.
    /// - `_padding`: Ensures 8-byte alignment for Anchor zero-copy safety and future extensibility.
    pub last_update_slot: u64,
    pub protocol_version: u16,
    pub status_flags: u16,
    pub _padding: [u8; 4],

    /// Reserved for future upgrades (e.g., new features, protocol extensions) without breaking account layout.
    ///
    /// Why: Pre-allocating space allows for seamless upgrades and avoids costly migrations or rent increases.
    pub reserved: [u64; 8],
}
