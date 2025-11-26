use crate::error::TickError;
use crate::math::core_arithmetic::{Q64x64, Q64x64Signed};
use crate::utils::constants::{
    DEFAULT_SUSPICIOUS_THRESHOLD, FLAG_ACTIVE, FLAG_EMERGENCY_PAUSE, FLAG_REQUIRES_AUDIT,
    MAX_BITMAP_CAPACITY, MAX_TICK, MAX_TICK_SPACING, MIN_TICK,
};
use anchor_lang::prelude::*;

/// TickData is a compact, zero-copy, and future-proof representation of a single tick's state in the AMM pool.
///
/// # Design rationale
/// - **Zero-copy**: Marked `unsafe` for maximum on-chain performance; all fields are 8-byte aligned and padding is explicit to guarantee safe memory access.
/// - **Explicit padding**: Maintains compatibility with previous versions and ensures deterministic layout for future upgrades.
/// - **Bitfield status flags**: Enables atomic, multi-state transitions and efficient status checks (active, paused, audit, etc.).
/// - **Reserved space**: Allocated for future protocol upgrades, allowing for seamless migrations and new features without breaking layout.
/// - **Security fields**: Suspicious activity tracking and nonce-based initialization prevent replay and re-initialization attacks.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct TickData {
    /// Net liquidity change at this tick (Q64.64 fixed point). Used for tick crossing and pool math.
    pub liquidity_net: Q64x64Signed,
    /// Fee growth for token 0 outside this tick (Q64.64 fixed point).
    pub fee_growth_outside_0: Q64x64,
    /// Fee growth for token 1 outside this tick (Q64.64 fixed point).
    pub fee_growth_outside_1: Q64x64,

    /// Slot of the last tick cross. Used for time-based analytics and anti-reorg logic.
    pub last_crossed_slot: u64,
    /// Number of times this tick has been crossed. Used for utilization and suspicious activity detection.
    pub cross_count: u64,
    /// Timestamp of the last update. Enables precise off-chain analytics and time-based triggers.
    pub last_update_timestamp: i64,

    /// The index of this tick. Aligned for fast access and used as a primary key in the pool.
    pub tick_index: i32,

    /// Score indicating potential malicious activity. Used for on-chain risk management and protocol defense.
    pub suspicious_activity_score: u32,
    /// Maximum threshold for suspicious activity before audit is required. Protocol-tunable for risk management.
    pub max_suspicious_threshold: u32,
    /// Bitfield representing various status flags (active, paused, audit, etc.).
    pub status_flags: u16,
    /// Cached tick spacing for fast validation and replay protection.
    pub tick_spacing_validation: u16,
    /// Nonce to prevent replay and ensure unique initialization.
    pub initialization_nonce: u32,

    /// True if this tick has been initialized. Single byte for fast checks; padded for alignment.
    pub initialized: u8,
    pub _padding_1: [u8; 3], // Maintains 8-byte alignment for zero-copy safety.

    /// Reserved for future protocol upgrades. Ensures backward compatibility and seamless migrations.
    pub reserved: [u64; 4],
}

// The safest way to ensure INIT_SPACE matches the actual struct size is to use core::mem::size_of.
const TICK_DATA_SIZE: usize = core::mem::size_of::<TickData>();

// Implements Anchor's Space trait for TickData, guaranteeing correct account allocation.
impl anchor_lang::Space for TickData {
    const INIT_SPACE: usize = TICK_DATA_SIZE;
}

impl TickData {
    /// Initializes a tick with all fields set for safety, security, and deterministic operation.
    ///
    /// # Why
    /// - Prevents re-initialization attacks by checking the `initialized` flag.
    /// - Validates tick alignment in a single operation for efficiency and replay protection.
    /// - All fields are explicitly set, and reserved space is zeroed for deterministic hashes and future migrations.
    /// - Uses clock_slot and timestamp as arguments to minimize sysvar hits and improve testability.
    pub fn initialize(
        &mut self,
        tick_index: i32,
        tick_spacing: u16,
        initialization_nonce: u32,
        clock_slot: u64,
        timestamp: i64,
    ) -> Result<()> {
        // Security: Prevent re-initialization and replay attacks
        if self.initialized == 1 {
            return Err(TickError::TickAlreadyInitialized.into());
        }

        // Validate tick alignment for protocol safety and price grid consistency
        if tick_index
            .checked_rem(tick_spacing as i32)
            .ok_or(TickError::ArithmeticOverflow)?
            != 0
        {
            return Err(TickError::InvalidTickAlignment.into());
        }

        // Set all fields explicitly for deterministic state
        self.tick_index = tick_index;
        self.liquidity_net = Q64x64Signed::zero();
        self.fee_growth_outside_0 = Q64x64::zero();
        self.fee_growth_outside_1 = Q64x64::zero();
        self.last_crossed_slot = clock_slot;
        self.cross_count = 0;
        self.last_update_timestamp = timestamp;
        self.suspicious_activity_score = 0;
        self.max_suspicious_threshold = DEFAULT_SUSPICIOUS_THRESHOLD;
        self.status_flags = FLAG_ACTIVE;
        self.tick_spacing_validation = tick_spacing;
        self.initialization_nonce = initialization_nonce;
        self.initialized = 1;
        self._padding_1 = [0u8; 3]; // Ensure padding is zeroed for deterministic hashes

        // Zero reserved space for deterministic hashes and future upgrades
        self.reserved = [0u64; 4];

        Ok(())
    }

    /// Checks if the tick is currently active using bitwise flag management.
    ///
    /// # Why
    /// - Bitwise checks are O(1) and allow for atomic, multi-state transitions.
    #[inline(always)]
    pub fn is_active(&self) -> bool {
        (self.status_flags & FLAG_ACTIVE) != 0
    }

    /// Checks if the tick is in emergency pause using bitwise flag management.
    ///
    /// # Why
    /// - Enables fast, atomic protocol-wide pausing for risk management.
    #[inline(always)]
    pub fn is_emergency_paused(&self) -> bool {
        (self.status_flags & FLAG_EMERGENCY_PAUSE) != 0
    }

    /// Sets or unsets the emergency pause status for the tick atomically.
    ///
    /// # Why
    /// - Bitwise flag management allows for efficient, atomic state transitions and future extensibility.
    #[inline(always)]
    pub fn set_emergency_pause(&mut self, paused: u8) {
        if paused != 0 {
            self.status_flags |= FLAG_EMERGENCY_PAUSE;
        } else {
            self.status_flags &= !FLAG_EMERGENCY_PAUSE;
        }
    }

    /// Updates the net liquidity at this tick, with overflow protection and suspicious activity detection.
    ///
    /// # Why
    /// - Ensures protocol invariants are maintained and prevents overflow attacks.
    /// - Suspicious activity detection is built-in to trigger audits if abnormal deltas are observed.
    /// - clock_slot and timestamp are passed in to minimize sysvar hits and improve testability.
    pub fn update_liquidity_delta(
        &mut self,
        delta: Q64x64Signed,
        clock_slot: u64,
        timestamp: i64,
    ) -> Result<()> {
        // Validate tick is initialized and active
        self.validate_operational_state()?;

        // Update liquidity with safe arithmetic
        self.liquidity_net = self.liquidity_net.checked_add(delta)?;

        // Update operational metadata
        self.last_crossed_slot = clock_slot;
        self.cross_count = self.cross_count.saturating_add(1);
        self.last_update_timestamp = timestamp;

        // Efficient suspicious activity detection with safe overflow protection
        let delta_abs = (delta.abs().raw() >> 64) as u64; // Convert to absolute value in Q64.64 fixed point
        if delta_abs > self.cross_count * 100 {
            self.suspicious_activity_score = self.suspicious_activity_score.saturating_add(1);

            if self.suspicious_activity_score > self.max_suspicious_threshold {
                self.status_flags |= FLAG_REQUIRES_AUDIT;
                return Err(TickError::SuspiciousActivityDetected.into());
            }
        }

        Ok(())
    }

    /// Updates the fee growth values for both tokens at this tick.
    ///
    /// # Why
    /// - Direct assignment is used for efficiency; no complex arithmetic is needed.
    /// - Ensures that fee growth is always up-to-date for accurate accounting and off-chain analytics.
    /// - timestamp is passed in to minimize sysvar hits and improve testability.
    pub fn update_fee_growth(
        &mut self,
        fee_growth_0: Q64x64,
        fee_growth_1: Q64x64,
        timestamp: i64,
    ) -> Result<()> {
        self.validate_operational_state()?;

        self.fee_growth_outside_0 = fee_growth_0;
        self.fee_growth_outside_1 = fee_growth_1;
        self.last_update_timestamp = timestamp;

        Ok(())
    }

    /// Validates that the tick is initialized and not in emergency pause, combining multiple checks for efficiency.
    ///
    /// # Why
    /// - Prevents operations on uninitialized or paused ticks, which could break protocol invariants or cause loss of funds.
    /// - Combines checks to minimize sysvar hits and improve performance.
    #[inline(always)]
    fn validate_operational_state(&self) -> Result<()> {
        if self.initialized == 0 {
            return Err(TickError::TickNotInitialized.into());
        }
        if self.is_emergency_paused() {
            return Err(TickError::TickInEmergencyPause.into());
        }
        Ok(())
    }

    /// Returns a utilization rate for this tick, used for protocol optimization and analytics.
    ///
    /// # Why
    /// - Provides a simple metric for how frequently a tick is used or crossed, which can inform fee tiering or risk management.
    #[inline(always)]
    pub fn get_utilization_rate(&self) -> u32 {
        if self.cross_count == 0 {
            return 0;
        }
        let recent_activity = self.suspicious_activity_score.min(100);
        (recent_activity * 100) / (self.cross_count.min(100) as u32)
    }

    /// Batch updates multiple tick properties in a single operation for efficiency and auditability.
    ///
    /// # Why
    /// - Minimizes sysvar hits and enables atomic updates to liquidity, fee growth, and metadata.
    /// - Restores audit logic with overflow protection to detect suspicious activity.
    /// - Used for protocol-level batch operations and off-chain sync.
    pub fn batch_update(
        &mut self,
        liquidity_delta: Option<Q64x64Signed>,
        fee_growth_0: Option<Q64x64>,
        fee_growth_1: Option<Q64x64>,
        clock_slot: u64,
        timestamp: i64,
    ) -> Result<()> {
        self.validate_operational_state()?;

        // Update liquidity if provided, with audit logic
        if let Some(delta) = liquidity_delta {
            self.liquidity_net = self.liquidity_net.checked_add(delta)?;
            self.cross_count = self.cross_count.saturating_add(1);
            let delta_abs = (delta.raw() >> 64) as u64;
            if delta_abs > self.cross_count * 100 {
                self.suspicious_activity_score = self.suspicious_activity_score.saturating_add(1);
                if self.suspicious_activity_score > self.max_suspicious_threshold {
                    self.status_flags |= FLAG_REQUIRES_AUDIT;
                    return Err(TickError::SuspiciousActivityDetected.into());
                }
            }
        }

        // Update fee growth if provided
        if let Some(growth_0) = fee_growth_0 {
            self.fee_growth_outside_0 = growth_0;
        }
        if let Some(growth_1) = fee_growth_1 {
            self.fee_growth_outside_1 = growth_1;
        }

        // Single timestamp update for all changes
        self.last_crossed_slot = clock_slot;
        self.last_update_timestamp = timestamp;

        Ok(())
    }
}

/// Static lookup table mapping fee tiers to their required tick spacing.
///
/// # Why
/// - Enables O(log n) binary search for fee tier validation, which is more scalable and maintainable than a hardcoded match statement.
/// - Each tuple is (fee_tier_bps, tick_spacing), where tick_spacing is protocol-determined for safety and liquidity granularity.
/// - Explicitly ordered for binary search correctness and future extensibility.
const FEE_TIER_LOOKUP: &[(u32, u16)] = &[
    (100, 1),     // 0.01% fee tier: stable pairs, finest granularity
    (500, 10),    // 0.05% fee tier: low volatility, moderate granularity
    (3000, 60),   // 0.30% fee tier: standard pairs, coarser granularity
    (10000, 200), // 1.00% fee tier: high volatility, widest granularity
];

/// Validates that the tick spacing is correct for the given fee tier and does not exceed bitmap capacity.
///
/// # Design rationale
/// - Uses binary search for O(log n) validation, making it easy to add new fee tiers without rewriting logic.
/// - Ensures tick spacing is protocol-aligned, preventing liquidity fragmentation and price grid inconsistencies.
/// - Checks for overflow to guarantee bitmap safety and prevent DoS via excessive tick creation.
///
/// # Safety
/// - Fails fast on invalid or unsupported fee tiers.
/// - Prevents protocol misconfiguration and on-chain state corruption.
///
/// # Arguments
/// * `tick_spacing` - The tick spacing to validate (must match protocol for the fee tier).
/// * `fee_tier` - The fee tier in basis points.
///
/// # Errors
/// * `TickError::InvalidTickSpacing` - If tick_spacing is zero or exceeds protocol maximum.
/// * `TickError::UnsupportedFeeTier` - If the fee tier is not in the lookup table.
/// * `TickError::TickSpacingFeeTierMismatch` - If tick_spacing does not match the required value for the fee tier.
/// * `TickError::TickSpacingExceedsCapacity` - If the resulting number of ticks would overflow the bitmap.
pub fn validate_tick_spacing(tick_spacing: u16, fee_tier: u32) -> Result<()> {
    // Fast fail for obviously invalid tick spacing
    if tick_spacing == 0 || tick_spacing > MAX_TICK_SPACING {
        return Err(TickError::InvalidTickSpacing.into());
    }

    // O(log n) binary search for the fee tier; ensures extensibility and avoids match boilerplate
    let expected_spacing = FEE_TIER_LOOKUP
        .binary_search_by_key(&fee_tier, |&(tier, _)| tier)
        .map(|idx| FEE_TIER_LOOKUP[idx].1)
        .map_err(|_| TickError::UnsupportedFeeTier)?;

    // Enforce protocol tick spacing for the fee tier
    if tick_spacing != expected_spacing {
        return Err(TickError::TickSpacingFeeTierMismatch.into());
    }

    // Prevent bitmap overflow: ensures the number of ticks is within protocol limits
    let max_ticks = ((MAX_TICK - MIN_TICK) as u32) / (tick_spacing as u32);
    if max_ticks > MAX_BITMAP_CAPACITY as u32 {
        return Err(TickError::TickSpacingExceedsCapacity.into());
    }

    Ok(())
}
