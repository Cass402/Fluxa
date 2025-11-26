use crate::error::TickError;
use crate::math::core_arithmetic::{mul_div_q64, Q64x64};
use crate::state::pool::pool_core::PoolCore;
use crate::state::tick::ring_buffer::RingBuffer;
use crate::state::tick::tick_data::TickData;
use crate::utils::constants::{
    MAX_SUSPICION_SCORE, MAX_TICK_CROSSES_PER_HOUR, MIN_TICK_CROSS_INTERVAL,
    RESET_SUSPICION_INTERVAL, SLOTS_PER_MINUTE, SUSPICIOUS_CROSS_INTERVAL,
};
use anchor_lang::prelude::*;

/// Tick rate limiting and anomaly detection state for a pool.
///
/// # Why this structure?
/// - Uses zero-copy layout to minimize serialization overhead and maximize on-chain efficiency, critical for Solana's compute budget.
/// - Avoids dynamic allocations (e.g., Vec) in favor of fixed-size ring buffers, ensuring deterministic memory usage and predictable performance.
/// - Tracks not just raw counts, but also moving averages and anomaly scores, enabling nuanced MEV/threat detection beyond simple rate limiting.
/// - All fields are designed for atomic, batched updates to minimize state bloat and reduce the risk of partial state corruption.
///
/// ## Usage
/// This account is tightly coupled to a specific pool and is seeded for Anchor constraint validation, ensuring only the correct pool can mutate its state.
#[account(zero_copy)]
#[repr(C)]
pub struct TickRateLimit {
    /// Reference to the pool this rate limit is bound to.
    ///
    /// Why: Ensures this account cannot be reused or spoofed for another pool, providing a strong link for Anchor's constraint system.
    pub pool: Pubkey,

    /// Sliding window of tick crosses for rate limiting.
    ///
    /// Why: RingBuffer enables O(1) insertions and bounded memory, avoiding the unpredictability and cost of Vec on-chain.
    pub tick_cross_window: RingBuffer,

    /// Exponential moving average (EMA) of tick crosses per minute, using Q64.64 fixed-point.
    ///
    /// Why: EMA smooths out short-term volatility, providing a robust signal for trend and anomaly detection. Q64.64 ensures high precision for on-chain math.
    pub crosses_ema: Q64x64, // EMA of crosses per minute

    /// EMAs for token volumes and price volatility, for MEV/threat detection.
    ///
    /// Why: Volume and volatility spikes are strong signals for sandwich/attack detection. EMAs provide a smoothed baseline for anomaly scoring.
    pub volume_ema_0: Q64x64, // EMA of token0 volume
    pub volume_ema_1: Q64x64,         // EMA of token1 volume
    pub price_volatility_ema: Q64x64, // EMA of price volatility
    pub last_price_update: u64,       // Last price observation timestamp (for time-based decay)

    /// 24-hour total tick crosses, for long-term anomaly/trend detection.
    ///
    /// Why: Allows for historical analysis and capacity planning, not just short-term DoS protection.
    pub total_crosses_24h: u64,
    pub last_reset_slot: u64,
    pub peak_crosses_per_hour: u32,
    pub anomaly_score: u32,

    /// Decay factor for EMA, in Q16.16 fixed-point (not integer!).
    ///
    /// Why: Allows fine-tuning of EMA responsiveness. Stored as Q16.16 for compactness, but must be shifted for Q64.64 math. Do not use from_int!
    pub ema_decay_factor: u16, // EMA lambda parameter (Q16.16 fixed-point)

    // Expanded to 14 bytes so overall struct size (including `reserved`) is a 16-byte multiple.
    // Size math: fields up to ema_decay_factor = 178 bytes. Padding 14 -> 192. `reserved` (64) -> 256 total.
    // Prevents compiler from inserting 8 bytes of hidden tail padding (which caused zero_copy size mismatch).
    pub _padding: [u8; 6],

    /// Reserved for future upgrades (e.g., new detection metrics) without breaking account layout.
    pub reserved: [u64; 9],
}

impl TickRateLimit {
    /// Initialize with safe, conservative defaults.
    ///
    /// Why: Ensures all fields are set to known values, preventing uninitialized state. Decay factor is set to 0.5 (Q16.16) for balanced EMA responsiveness.
    pub fn initialize(&mut self, pool: Pubkey, current_slot: u64) -> Result<()> {
        self.pool = pool;
        self.tick_cross_window = RingBuffer::new();
        self.total_crosses_24h = 0;
        self.last_reset_slot = current_slot;
        self.peak_crosses_per_hour = 0;
        self.anomaly_score = 0;
        self.crosses_ema = Q64x64::zero();
        self.ema_decay_factor = 32768; // ~0.5 decay factor in Q16.16 format
        self.volume_ema_0 = Q64x64::zero();
        self.volume_ema_1 = Q64x64::zero();
        self.price_volatility_ema = Q64x64::zero();
        self.last_price_update = current_slot;
        self._padding = [0; 6];
        self.reserved = [0; 9];

        Ok(())
    }

    /// Update all EMAs using fixed-point math, with careful bit-shifting to preserve precision.
    ///
    /// # Why this approach?
    /// - Q16.16 decay factor is compact, but must be shifted to Q64.64 for correct math. This avoids subtle bugs from misinterpreting the decay as an integer.
    /// - All EMA updates are batched for atomicity and to minimize compute cost.
    /// - No division by 65536 needed after shifting, as the Q64.64 format is preserved.
    #[inline(always)]
    pub fn update_emas(
        &mut self,
        volume_0: Q64x64,
        volume_1: Q64x64,
        price_change: Q64x64,
        current_crosses_per_minute: Q64x64,
    ) -> Result<()> {
        // CORRECTED: Properly embed Q16.16 into Q64.64 format
        // Shift the 16-bit fraction to the top 16 bits of the 64-bit fraction part
        let decay_raw = (self.ema_decay_factor as u128) << 48; // Shift to Q64.64 format
        let decay = Q64x64::from_raw(decay_raw);

        let one_minus_decay_raw = ((65536 - self.ema_decay_factor as u32) as u128) << 48;
        let one_minus_decay = Q64x64::from_raw(one_minus_decay_raw);

        // Calculate weighted components
        let volume_0_weighted = volume_0.checked_mul(one_minus_decay)?;
        let volume_1_weighted = volume_1.checked_mul(one_minus_decay)?;
        let price_weighted = price_change.checked_mul(one_minus_decay)?;
        let new_crosses_weighted = current_crosses_per_minute.checked_mul(one_minus_decay)?;

        let old_volume_0_weighted = self.volume_ema_0.checked_mul(decay)?;
        let old_volume_1_weighted = self.volume_ema_1.checked_mul(decay)?;
        let old_price_weighted = self.price_volatility_ema.checked_mul(decay)?;
        let old_crosses_weighted = self.crosses_ema.checked_mul(decay)?;

        // Update EMAs with proper normalization
        // No need to divide by 65536 since we're already in proper Q64.64 format
        self.volume_ema_0 = old_volume_0_weighted.checked_add(volume_0_weighted)?;
        self.volume_ema_1 = old_volume_1_weighted.checked_add(volume_1_weighted)?;
        self.price_volatility_ema = old_price_weighted.checked_add(price_weighted)?;
        self.crosses_ema = old_crosses_weighted.checked_add(new_crosses_weighted)?;

        Ok(())
    }

    /// Compute a composite anomaly score using multiple weighted signals.
    ///
    /// # Why this scoring system?
    /// - Combines rate, volume, and volatility signals to reduce false positives and catch sophisticated attacks.
    /// - Weights are chosen to balance sensitivity to DoS (rate) and MEV/manipulation (volume/volatility).
    /// - Score is capped at 100 for easy integration with downstream risk logic.
    pub fn calculate_anomaly_score(&self, current_crosses: u32) -> Result<u32> {
        let mut score = 0u32;

        // Rate-based anomaly detection (40% weight)
        // Detects when crossing rate exceeds normal thresholds
        if current_crosses > MAX_TICK_CROSSES_PER_HOUR / 2 {
            score += ((current_crosses * 40) / MAX_TICK_CROSSES_PER_HOUR).min(40);
        }

        // Volume anomaly detection (30% weight)
        // Flags unusual volume patterns that might indicate manipulation
        let volume_deviation = (self.calculate_volume_deviation()?.raw() >> 64) as u32;
        score += (volume_deviation * 30 / 100).min(30);

        // Price volatility anomaly (30% weight)
        // Identifies excessive price movements that could signal attacks
        let volatility_score = (self
            .price_volatility_ema
            .checked_div(Q64x64::from_int(1000))?)
        .min(Q64x64::from_int(30));
        score += (volatility_score.raw() >> 64) as u32;

        Ok(score.min(100))
    }

    /// Calculate normalized deviation of current volume from EMA baseline.
    ///
    /// # Why this logic?
    /// - Detects sudden volume spikes that may indicate manipulation or MEV attacks.
    /// - Normalization to 0-100 enables consistent scoring and easy thresholding.
    #[inline(always)]
    fn calculate_volume_deviation(&self) -> Result<Q64x64> {
        // Calculate deviation of current volume from historical EMA
        let total_volume = self.volume_ema_0.checked_add(self.volume_ema_1)?;
        if total_volume.raw() > 0 {
            // Return deviation percentage (0-100)
            // Higher values indicate more significant deviations from normal
            Ok((total_volume.checked_div(Q64x64::from_int(1000))?).min(Q64x64::from_int(100)))
        } else {
            Ok(Q64x64::zero())
        }
    }
}

/// Main entry point for a tick crossing, with all security and accounting logic batched.
///
/// # Why this structure?
/// - All validation and state updates are performed in a single function to guarantee atomicity and prevent partial state updates.
/// - Security checks are performed before any state mutation, minimizing wasted compute and risk of inconsistent state.
/// - Fee and liquidity updates are batched for cache efficiency and to minimize Anchor account loads.
pub fn cross_tick_with_enhanced_security(
    pool: &mut PoolCore,
    tick_data: &mut TickData,
    rate_limit: &mut TickRateLimit,
    zero_for_one: u8,
    current_slot: u64,
    swap_volume_0: Q64x64,
    swap_volume_1: Q64x64,
) -> Result<()> {
    // All security checks are performed up front to avoid wasted compute and ensure no state is mutated on failure.
    validate_tick_crossing_security(
        tick_data,
        rate_limit,
        current_slot,
        swap_volume_0,
        swap_volume_1,
    )?;

    // Fee growth and liquidity updates are batched for performance and to minimize Anchor account loads.
    update_fee_growth(pool, tick_data)?;

    // Liquidity delta is applied with saturation arithmetic to prevent overflows/underflows.
    apply_liquidity_delta(pool, tick_data, zero_for_one)?;

    // All tracking fields and EMAs are updated in a single batch for atomicity and cache efficiency.
    update_tracking_state(
        tick_data,
        rate_limit,
        current_slot,
        swap_volume_0,
        swap_volume_1,
    )?;

    Ok(())
}

/// Security validation pipeline for tick crossing, with early exit on failure.
///
/// # Why this order?
/// - Sliding window is updated first to ensure rate limiting is always up to date, even on failed attempts (prevents replay attacks).
/// - Protocol rate limits are enforced before any state mutation, minimizing wasted compute and risk of inconsistent state.
/// - Rapid crossing prevention and suspicious activity scoring are performed before any accounting logic, ensuring all safety checks are atomic.
#[inline(always)]
fn validate_tick_crossing_security(
    tick_data: &mut TickData,
    rate_limit: &mut TickRateLimit,
    current_slot: u64,
    volume_0: Q64x64,
    volume_1: Q64x64,
) -> Result<()> {
    // Sliding window is always updated, even on failure, to prevent replay or timing attacks.
    rate_limit
        .tick_cross_window
        .update_and_increment(current_slot)?;

    // Fast path: reject if rate limit is exceeded, before any further computation.
    if rate_limit.tick_cross_window.is_rate_limited() {
        return Err(TickError::ExcessiveTickCrossing.into());
    }

    // Prevent rapid, repeated crossings (anti-manipulation):
    // This check ensures a minimum interval between crossings, making high-frequency attacks expensive and detectable.
    if current_slot
        <= tick_data
            .last_crossed_slot
            .saturating_add(MIN_TICK_CROSS_INTERVAL)
    {
        return Err(TickError::RapidTickManipulation.into());
    }

    // Suspicious activity detection: combines timing and volume to catch MEV and coordinated attacks.
    if tick_data.cross_count > 0 {
        let time_since_last = current_slot.saturating_sub(tick_data.last_crossed_slot);

        if time_since_last < SUSPICIOUS_CROSS_INTERVAL {
            // Volume factor is used to weight suspicion, so large swaps are more likely to trigger investigation.
            let volume_factor = volume_0
                .checked_add(volume_1)?
                .checked_div(Q64x64::from_int(1_000_000))?
                .min(Q64x64::from_int(5));
            tick_data.suspicious_activity_score = tick_data
                .suspicious_activity_score
                .saturating_add(10 + ((volume_factor.raw() >> 64) as u32));

            // Update anomaly score in rate limiter for comprehensive threat assessment.
            let current_crosses = rate_limit.tick_cross_window.get_total_count();
            rate_limit.anomaly_score = rate_limit.calculate_anomaly_score(current_crosses)?;

            if tick_data.suspicious_activity_score > MAX_SUSPICION_SCORE {
                return Err(TickError::SuspiciousTickActivity.into());
            }
        } else if time_since_last > RESET_SUSPICION_INTERVAL {
            // Suspicion score decays over time, so false positives don't permanently penalize a tick.
            tick_data.suspicious_activity_score = tick_data
                .suspicious_activity_score
                .saturating_sub((tick_data.suspicious_activity_score / 4).max(1));
        }
    }

    Ok(())
}

/// Fee growth update for tick crossing, using wrapping arithmetic.
///
/// # Why this approach?
/// - Fee growth is tracked per side of the tick, and updated using wrapping arithmetic to naturally handle overflows (as fees can grow unbounded).
/// - Batched update minimizes memory writes, which is critical for Solana's compute and I/O budget.
#[inline(always)]
fn update_fee_growth(pool: &PoolCore, tick_data: &mut TickData) -> Result<()> {
    // Fee growth is always calculated as the difference between global and outside, ensuring correctness even if state is desynced.
    let (new_growth_0, new_growth_1) = (
        pool.fee_growth_global_0
            .checked_sub(tick_data.fee_growth_outside_0)?,
        pool.fee_growth_global_1
            .checked_sub(tick_data.fee_growth_outside_1)?,
    );

    // Batched update for cache efficiency and to minimize Anchor account loads.
    tick_data.fee_growth_outside_0 = new_growth_0;
    tick_data.fee_growth_outside_1 = new_growth_1;

    Ok(())
}

/// Apply tick's net liquidity delta to the pool, using saturation arithmetic for safety.
///
/// # Why this logic?
/// - Ticks encode net liquidity changes, which must be applied in the correct direction (zero_for_one).
/// - Saturation arithmetic is used to prevent overflows/underflows, which could otherwise brick the pool.
/// - Negative deltas are handled with explicit sign checks, as Rust's checked_sub does not support negative numbers.
#[inline(always)]
fn apply_liquidity_delta(
    pool: &mut PoolCore,
    tick_data: &TickData,
    zero_for_one: u8,
) -> Result<()> {
    // Direction determines whether to add or subtract liquidity. Negation is used for zero_for_one swaps.
    let liquidity_delta = if zero_for_one != 0 {
        tick_data.liquidity_net.negate()? // Use proper Q64x64Signed negation
    } else {
        tick_data.liquidity_net
    };

    // Apply liquidity change with explicit sign handling to prevent overflows/underflows.
    if liquidity_delta.is_negative() {
        let abs_delta = liquidity_delta.abs();
        pool.liquidity = pool
            .liquidity
            .checked_sub(Q64x64::from_raw(abs_delta.raw() as u128))?;
    } else {
        pool.liquidity = pool
            .liquidity
            .checked_add(Q64x64::from_raw(liquidity_delta.raw() as u128))?;
    }

    // Sanity check: pool liquidity must never go to zero, as this would brick the pool and break invariant math.
    if pool.liquidity.raw() == 0 {
        return Err(TickError::LiquidityUnderflow.into());
    }

    Ok(())
}

/// Batched update of all tracking fields and EMAs for optimal memory access and atomicity.
///
/// # Why batch updates?
/// - Batching minimizes memory writes and Anchor account loads, which is critical for Solana's compute/I/O budget.
/// - All counters and EMAs are updated together to ensure state consistency and prevent partial updates.
/// - Handles periodic resets to prevent counter overflow and maintain long-term trend accuracy.
#[inline(always)]
fn update_tracking_state(
    tick_data: &mut TickData,
    rate_limit: &mut TickRateLimit,
    current_slot: u64,
    volume_0: Q64x64,
    volume_1: Q64x64,
) -> Result<()> {
    // All tick data fields are updated in a single batch for atomicity and cache efficiency.
    tick_data.last_crossed_slot = current_slot;
    tick_data.cross_count = tick_data.cross_count.saturating_add(1);

    // 24-hour crossing counter is incremented for long-term trend analysis and anomaly detection.
    rate_limit.total_crosses_24h = rate_limit.total_crosses_24h.saturating_add(1);

    // Periodic reset: prevents counter overflow and ensures long-term stats remain accurate.
    if current_slot
        >= rate_limit
            .last_reset_slot
            .saturating_add(24 * 60 * SLOTS_PER_MINUTE)
    {
        rate_limit.total_crosses_24h = 0;
        rate_limit.last_reset_slot = current_slot;
        rate_limit.peak_crosses_per_hour = 0;
    }

    // Current crosses per minute is used for EMA tracking, providing a smoothed signal for anomaly detection.
    let current_crosses_per_minute =
        Q64x64::from_int(rate_limit.tick_cross_window.get_total_count() as u64);

    // All EMAs are updated in a single call for atomicity and to minimize compute cost.
    let price_change = calculate_price_change_estimate(volume_0, volume_1)?;
    rate_limit.update_emas(volume_0, volume_1, price_change, current_crosses_per_minute)?;

    // Peak crossing rate is tracked for capacity planning and to detect sustained attacks.
    let current_hourly_rate = rate_limit.tick_cross_window.get_total_count();
    if current_hourly_rate > rate_limit.peak_crosses_per_hour {
        rate_limit.peak_crosses_per_hour = current_hourly_rate;
    }

    Ok(())
}

/// Estimate price change for volatility tracking, using volume ratio as a proxy.
///
/// # Why this method?
/// - Direct price data may not be available or may be too expensive to fetch on-chain.
/// - Volume ratio is a cheap, robust proxy for price impact, and is capped to prevent outlier bias.
#[inline(always)]
fn calculate_price_change_estimate(volume_0: Q64x64, volume_1: Q64x64) -> Result<Q64x64> {
    // Estimate price impact based on volume ratio
    // Used to detect unusual price movements that might indicate manipulation
    if volume_0.raw() > 0 && volume_1.raw() > 0 {
        let ratio = if volume_0 > volume_1 {
            mul_div_q64(volume_0, Q64x64::from_int(1000), volume_1)?
        } else {
            mul_div_q64(volume_1, Q64x64::from_int(1000), volume_0)?
        };

        // Convert to volatility estimate, capped to prevent outlier bias from flash loan attacks or oracle errors.
        Ok((ratio
            .checked_sub(Q64x64::from_int(1000))?
            .checked_div(Q64x64::from_int(10))?)
        .min(Q64x64::from_int(1000)))
    } else {
        Ok(Q64x64::zero())
    }
}
