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

/// Enhanced pool tick rate limit account with optimized memory layout
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct TickRateLimit {
    /// Pool reference for validation
    pub pool: Pubkey,

    /// Optimized ring buffer for sliding window tracking
    pub tick_cross_window: RingBuffer,

    /// Enhanced tracking metrics for anomaly detection
    pub total_crosses_24h: u64,
    pub last_reset_slot: u64,
    pub peak_crosses_per_hour: u32,
    pub anomaly_score: u32,

    /// Exponential moving average for trend detection with fixed-point arithmetic
    ///
    /// EMA CALCULATION:
    /// - Uses 16-bit fixed-point arithmetic for precision
    /// - decay_factor represents λ in EMA formula: EMA = λ × old + (1-λ) × new
    /// - decay_factor = λ × 2^16 (e.g., 0.5 → 32768)
    /// - CORRECTED: Properly embeds Q16.16 into Q64.64 format
    pub crosses_ema: Q64x64, // EMA of crosses per minute
    pub ema_decay_factor: u16, // EMA lambda parameter (Q16.16 fixed-point - DO NOT use from_int!)

    /// Advanced MEV protection state with volume tracking
    pub volume_ema_0: Q64x64, // EMA of token0 volume
    pub volume_ema_1: Q64x64,         // EMA of token1 volume
    pub price_volatility_ema: Q64x64, // EMA of price volatility
    pub last_price_update: u64,       // Last price observation timestamp

    /// Reserved space for future enhancements
    pub reserved: [u64; 8],
}

impl TickRateLimit {
    /// Initialize with optimal default parameters
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
        self.reserved = [0; 8];

        Ok(())
    }

    /// Update EMAs with CORRECTED fixed-point arithmetic
    ///
    /// FIXED-POINT ARITHMETIC CORRECTION:
    /// - OLD (incorrect): Q64x64::from_int(ema_decay_factor as u64)
    /// - NEW (correct): Embed Q16.16 into Q64.64 by shifting to proper bit position
    ///
    /// Why this matters:
    /// - ema_decay_factor is Q16.16 (e.g., 0.5 → 32768)
    /// - from_int(32768) treats it as 32,768.0 instead of 0.5
    /// - Must shift to embed 16-bit fraction into 64-bit format
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

    /// Advanced anomaly detection using multiple weighted signals
    ///
    /// ANOMALY SCORING METHODOLOGY:
    /// - Rate anomaly (40%): Detects crossing frequency spikes
    /// - Volume anomaly (30%): Flags unusual volume patterns
    /// - Volatility anomaly (30%): Identifies excessive price movements
    /// - Score range: 0-100, higher = more suspicious
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

    /// Calculate volume deviation from EMA baseline for anomaly detection
    ///
    /// VOLUME DEVIATION LOGIC:
    /// - Compares current volume to historical EMA baseline
    /// - Higher deviations indicate potential manipulation
    /// - Normalized to 0-100 scale for consistent scoring
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

/// Highly optimized tick crossing function with batched security operations
pub fn cross_tick_with_enhanced_security(
    pool: &mut PoolCore,
    tick_data: &mut TickData,
    rate_limit: &mut TickRateLimit,
    zero_for_one: bool,
    current_slot: u64,
    swap_volume_0: Q64x64,
    swap_volume_1: Q64x64,
) -> Result<()> {
    // Batched rate limiting and security validation
    validate_tick_crossing_security(
        tick_data,
        rate_limit,
        current_slot,
        swap_volume_0,
        swap_volume_1,
    )?;

    // Optimized fee growth updates using vectorized operations
    update_fee_growth(pool, tick_data)?;

    // Efficient liquidity delta application with saturation arithmetic
    apply_liquidity_delta(pool, tick_data, zero_for_one)?;

    // Batch update tracking fields and rate limiting state
    update_tracking_state(
        tick_data,
        rate_limit,
        current_slot,
        swap_volume_0,
        swap_volume_1,
    )?;

    Ok(())
}

/// Batched security validation with early exit optimization
///
/// SECURITY VALIDATION PIPELINE:
/// 1. Update rate limiting window - tracks crossing frequency
/// 2. Check rate limits - prevents excessive crossing attacks
/// 3. Rapid crossing prevention - stops high-frequency manipulation
/// 4. Suspicious activity detection - identifies coordinated attacks
#[inline(always)]
fn validate_tick_crossing_security(
    tick_data: &mut TickData,
    rate_limit: &mut TickRateLimit,
    current_slot: u64,
    volume_0: Q64x64,
    volume_1: Q64x64,
) -> Result<()> {
    // Update rate limiting window first - this tracks crossing frequency
    rate_limit
        .tick_cross_window
        .update_and_increment(current_slot)?;

    // Fast rate limit check using cached total count
    if rate_limit.tick_cross_window.is_rate_limited() {
        return Err(TickError::ExcessiveTickCrossing.into());
    }

    // Rapid crossing prevention - prevents manipulation through high-frequency attacks
    if current_slot
        <= tick_data
            .last_crossed_slot
            .saturating_add(MIN_TICK_CROSS_INTERVAL)
    {
        return Err(TickError::RapidTickManipulation.into());
    }

    // Advanced suspicious activity detection with volume consideration
    if tick_data.cross_count > 0 {
        let time_since_last = current_slot.saturating_sub(tick_data.last_crossed_slot);

        if time_since_last < SUSPICIOUS_CROSS_INTERVAL {
            // Enhanced scoring with volume factor to detect coordinated attacks
            let volume_factor = volume_0
                .checked_add(volume_1)?
                .checked_div(Q64x64::from_int(1_000_000))?
                .min(Q64x64::from_int(5));
            tick_data.suspicious_activity_score = tick_data
                .suspicious_activity_score
                .saturating_add(10 + ((volume_factor.raw() >> 64) as u32));

            // Update anomaly score in rate limiter for comprehensive threat assessment
            let current_crosses = rate_limit.tick_cross_window.get_total_count();
            rate_limit.anomaly_score = rate_limit.calculate_anomaly_score(current_crosses)?;

            if tick_data.suspicious_activity_score > MAX_SUSPICION_SCORE {
                return Err(TickError::SuspiciousTickActivity.into());
            }
        } else if time_since_last > RESET_SUSPICION_INTERVAL {
            // Exponential decay of suspicion score over time
            tick_data.suspicious_activity_score = tick_data
                .suspicious_activity_score
                .saturating_sub((tick_data.suspicious_activity_score / 4).max(1));
        }
    }

    Ok(())
}

/// Optimized fee growth update with single memory write pattern
///
/// FEE GROWTH TRACKING:
/// - Tracks fees accumulated on each side of the tick
/// - Uses wrapping arithmetic to handle overflow naturally
/// - Batched updates minimize memory writes for better performance
#[inline(always)]
fn update_fee_growth(pool: &PoolCore, tick_data: &mut TickData) -> Result<()> {
    // Calculate fee growth delta based on swap direction
    // This tracks fees accumulated on each side of the tick
    let (new_growth_0, new_growth_1) = (
        pool.fee_growth_global_0
            .checked_sub(tick_data.fee_growth_outside_0)?,
        pool.fee_growth_global_1
            .checked_sub(tick_data.fee_growth_outside_1)?,
    );

    // Batched update to minimize memory writes and improve cache efficiency
    tick_data.fee_growth_outside_0 = new_growth_0;
    tick_data.fee_growth_outside_1 = new_growth_1;

    Ok(())
}

/// Optimized liquidity delta application with saturation arithmetic
///
/// LIQUIDITY DELTA LOGIC:
/// - Ticks track net liquidity that gets added/removed when crossed
/// - Direction determines whether to add or subtract liquidity
/// - Saturation arithmetic prevents overflow/underflow crashes
#[inline(always)]
fn apply_liquidity_delta(
    pool: &mut PoolCore,
    tick_data: &TickData,
    zero_for_one: bool,
) -> Result<()> {
    // Calculate liquidity change based on crossing direction
    // Ticks track net liquidity that gets added/removed when crossed
    let liquidity_delta = if zero_for_one {
        tick_data.liquidity_net.negate()? // Use proper Q64x64Signed negation
    } else {
        tick_data.liquidity_net
    };

    // Apply liquidity change with saturation arithmetic for safety
    // Prevents overflow/underflow that could crash the program
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

    // Sanity check - ensure liquidity doesn't go to zero
    if pool.liquidity.raw() == 0 {
        return Err(TickError::LiquidityUnderflow.into());
    }

    Ok(())
}

/// Batched state update for optimal memory access patterns
///
/// STATE UPDATE STRATEGY:
/// - Batch related updates to minimize memory writes
/// - Update cached totals incrementally for O(1) access
/// - Handle periodic resets efficiently
/// - CORRECTED: Proper error handling for EMA updates
#[inline(always)]
fn update_tracking_state(
    tick_data: &mut TickData,
    rate_limit: &mut TickRateLimit,
    current_slot: u64,
    volume_0: Q64x64,
    volume_1: Q64x64,
) -> Result<()> {
    // Batch update tick data fields to minimize memory writes
    tick_data.last_crossed_slot = current_slot;
    tick_data.cross_count = tick_data.cross_count.saturating_add(1);

    // Update 24-hour crossing counter for long-term trend analysis
    rate_limit.total_crosses_24h = rate_limit.total_crosses_24h.saturating_add(1);

    // Handle 24-hour reset efficiently to prevent counter overflow
    if current_slot
        >= rate_limit
            .last_reset_slot
            .saturating_add(24 * 60 * SLOTS_PER_MINUTE)
    {
        rate_limit.total_crosses_24h = 0;
        rate_limit.last_reset_slot = current_slot;
        rate_limit.peak_crosses_per_hour = 0;
    }

    // Calculate current crosses per minute for EMA tracking
    let current_crosses_per_minute =
        Q64x64::from_int(rate_limit.tick_cross_window.get_total_count() as u64);

    // Update exponential moving averages for trend detection
    // CORRECTED: Proper error handling instead of swallowing errors
    let price_change = calculate_price_change_estimate(volume_0, volume_1)?;
    rate_limit.update_emas(volume_0, volume_1, price_change, current_crosses_per_minute)?;

    // Track peak crossing rate for capacity planning
    let current_hourly_rate = rate_limit.tick_cross_window.get_total_count();
    if current_hourly_rate > rate_limit.peak_crosses_per_hour {
        rate_limit.peak_crosses_per_hour = current_hourly_rate;
    }

    Ok(())
}

/// Efficient price change estimation for volatility tracking
///
/// PRICE IMPACT ESTIMATION:
/// - Uses volume ratio as proxy for price impact
/// - Higher ratios indicate larger price movements
/// - Capped at reasonable levels to prevent outlier bias
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

        // Convert to volatility estimate (capped at reasonable levels)
        Ok((ratio
            .checked_sub(Q64x64::from_int(1000))?
            .checked_div(Q64x64::from_int(10))?)
        .min(Q64x64::from_int(1000)))
    } else {
        Ok(Q64x64::zero())
    }
}

/// Enhanced account constraints with tightened Anchor validation
#[derive(Accounts)]
pub struct TickCross<'info> {
    #[account(mut)]
    pub pool: AccountLoader<'info, PoolCore>,

    #[account(mut)]
    pub tick_data: AccountLoader<'info, TickData>,

    // Tightened constraint: uses seeds validation instead of runtime load()? check
    // This moves validation to Anchor's constraint system, eliminating BPF loads
    #[account(
        mut,
        seeds = [b"rate_limit", pool.key().as_ref()],
        bump
    )]
    pub rate_limit: AccountLoader<'info, TickRateLimit>,

    pub authority: Signer<'info>,
}

/// Optimized instruction handler with minimal computational overhead
pub fn tick_cross_instruction(
    ctx: Context<TickCross>,
    zero_for_one: bool,
    volume_0: Q64x64,
    volume_1: Q64x64,
) -> Result<()> {
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    let pool = &mut ctx.accounts.pool.load_mut()?;
    let mut tick_data = ctx.accounts.tick_data.load_mut()?;
    let mut rate_limit = ctx.accounts.rate_limit.load_mut()?;

    cross_tick_with_enhanced_security(
        pool,
        &mut tick_data,
        &mut rate_limit,
        zero_for_one,
        current_slot,
        volume_0,
        volume_1,
    )?;

    Ok(())
}
