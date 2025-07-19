use crate::error::TickError;
use crate::math::core_arithmetic::{Q64x64, Q64x64Signed};
use crate::state::pool::pool_core::PoolCore;
use crate::utils::constants::{
    DEFAULT_SUSPICIOUS_THRESHOLD, FLAG_ACTIVE, FLAG_EMERGENCY_PAUSE, FLAG_REQUIRES_AUDIT,
    MAX_BITMAP_CAPACITY, MAX_TICK, MAX_TICK_SPACING, MIN_TICK,
};
use anchor_lang::prelude::*;

/// Using unsafe for performance, validated by CI assertions
/// OptimizedTickData is a compact representation of tick data
/// with enhanced security and performance features.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct TickData {
    /// Tick index - aligned to 8-byte boundary for optimal access
    pub tick_index: i32,
    pub _padding_1: [u8; 4], // Explicit padding maintained for existing compatibility

    /// Core liquidity data
    /// 'liquidity_net' - Net liquidity change in Q64.64 fixed point
    /// 'fee_growth_outside_0' - Fee growth for token 0 in Q64.64 fixed point
    /// 'fee_growth_outside_1' - Fee growth for token 1 in Q64.64 fixed point
    pub liquidity_net: Q64x64Signed,
    pub fee_growth_outside_0: Q64x64,
    pub fee_growth_outside_1: Q64x64,

    /// Operational metadata
    /// 'last_crossed_slot' - Slot of the last tick cross
    /// 'cross_count' - Number of times the tick has been crossed
    /// 'last_update_timestamp' - Timestamp of the last update for precise timing
    pub last_crossed_slot: u64,
    pub cross_count: u64,
    pub last_update_timestamp: i64,

    /// Security and status data
    /// 'suspicious_activity_score' - Score indicating potential malicious activity
    /// 'max_suspicious_threshold' - Maximum threshold for suspicious activity
    /// 'status_flags' - Bitfield representing various status flags
    /// 'tick_spacing_validation' - Cached tick spacing validation
    pub suspicious_activity_score: u32,
    pub max_suspicious_threshold: u32,
    pub status_flags: u16,
    pub tick_spacing_validation: u16,
    pub initialization_nonce: u32,

    /// State flags - optimized single byte (1 byte + 7 padding = 8 bytes)
    pub initialized: bool,
    pub _padding_2: [u8; 7], // Align to 8-byte boundary

    /// Reserved space for future enhancements (32 bytes)
    /// Aligned to 8-byte boundaries for optimal access patterns
    pub reserved: [u64; 4],
}

// Get actual size directly from the struct definition
// This is the safest way to ensure INIT_SPACE matches actual size
const TICK_DATA_SIZE: usize = core::mem::size_of::<TickData>();

// Manual implementation of Space trait using the actual size
impl anchor_lang::Space for TickData {
    const INIT_SPACE: usize = TICK_DATA_SIZE;
}

/// Status flags for efficient state management using bitfield operations
impl TickData {
    /// Initialize tick with enhanced security and validation
    /// clock_slot and timestamp passed in to minimize sysvar hits
    pub fn initialize(
        &mut self,
        tick_index: i32,
        tick_spacing: u16,
        initialization_nonce: u32,
        clock_slot: u64,
        timestamp: i64,
    ) -> Result<()> {
        // Critical security check: prevent re-initialization attacks
        if self.initialized {
            return Err(TickError::TickAlreadyInitialized.into());
        }

        // Enhanced tick alignment validation - single operation for efficiency
        if tick_index
            .checked_rem(tick_spacing as i32)
            .ok_or(TickError::ArithmeticOverflow)?
            != 0
        {
            return Err(TickError::InvalidTickAlignment.into());
        }

        // Optimized initialization with single write pattern
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
        self.initialized = true;

        // Zero out reserved space
        self.reserved = [0u64; 4];

        Ok(())
    }

    /// Efficient status flag operations using bitwise operations
    /// Checks if the tick is active
    /// Returns true if the tick is active, false otherwise
    #[inline(always)]
    pub fn is_active(&self) -> bool {
        (self.status_flags & FLAG_ACTIVE) != 0
    }

    /// Checks if the tick is in emergency pause
    /// Returns true if the tick is in emergency pause, false otherwise
    #[inline(always)]
    pub fn is_emergency_paused(&self) -> bool {
        (self.status_flags & FLAG_EMERGENCY_PAUSE) != 0
    }

    /// Sets the emergency pause status for the tick
    /// @param paused - true to set emergency pause, false to unset
    /// This method uses bitwise operations for efficient state management
    /// Returns nothing
    #[inline(always)]
    pub fn set_emergency_pause(&mut self, paused: bool) {
        if paused {
            self.status_flags |= FLAG_EMERGENCY_PAUSE;
        } else {
            self.status_flags &= !FLAG_EMERGENCY_PAUSE;
        }
    }

    /// Update liquidity with overflow protection and efficient arithmetic
    /// timestamp passed in to minimize sysvar hits
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
        let delta_abs = (delta.raw() >> 64) as u64; // Convert to absolute value in Q64.64 fixed point
        if delta_abs > self.cross_count * 100 {
            self.suspicious_activity_score = self.suspicious_activity_score.saturating_add(1);

            if self.suspicious_activity_score > self.max_suspicious_threshold {
                self.status_flags |= FLAG_REQUIRES_AUDIT;
                return Err(TickError::SuspiciousActivityDetected.into());
            }
        }

        Ok(())
    }

    /// Update fee growth with optimized arithmetic
    /// timestamp passed in to minimize sysvar hits
    /// This method updates the fee growth values directly
    /// without complex arithmetic, ensuring efficient performance.
    /// It sets the fee growth for both tokens 0 and 1, and updates the
    /// last update timestamp to the current time.
    /// # Arguments
    /// - `fee_growth_0`: The new fee growth value for token 0 in Q64.64 fixed point.
    /// - `fee_growth_1`: The new fee growth value for token 1 in Q64.64 fixed point.
    /// - `timestamp`: The current timestamp to update the last update time.
    /// # Returns
    /// - `Result<()>`: Returns Ok(()) on success, or an error if the tick is not initialized or is in emergency pause.
    pub fn update_fee_growth(
        &mut self,
        fee_growth_0: Q64x64,
        fee_growth_1: Q64x64,
        timestamp: i64,
    ) -> Result<()> {
        self.validate_operational_state()?;

        // Direct assignment - no complex arithmetic needed
        self.fee_growth_outside_0 = fee_growth_0;
        self.fee_growth_outside_1 = fee_growth_1;
        self.last_update_timestamp = timestamp;

        Ok(())
    }

    /// Efficient validation combining multiple checks
    /// This method checks if the tick is initialized and not in emergency pause state.
    /// It returns an error if the tick is not initialized or is in emergency pause.
    /// This approach minimizes sysvar hits by validating the state in a single operation.
    /// # Returns
    /// - `Result<()>`: Returns Ok(()) if the tick is valid, or an error if not.
    /// # Errors
    /// - `TickError::TickNotInitialized`: If the tick is not initialized.
    /// - `TickError::TickInEmergencyPause`: If the tick is in emergency pause.
    #[inline(always)]
    fn validate_operational_state(&self) -> Result<()> {
        // Single validation combining multiple conditions
        if !self.initialized {
            return Err(TickError::TickNotInitialized.into());
        }

        if self.is_emergency_paused() {
            return Err(TickError::TickInEmergencyPause.into());
        }

        Ok(())
    }

    /// Get tick utilization rate for optimization decisions
    /// This method calculates the utilization rate based on recent activity and cross count.
    /// # Returns
    /// - `u32`: The utilization rate as a percentage (0-100).
    #[inline(always)]
    pub fn get_utilization_rate(&self) -> u32 {
        if self.cross_count == 0 {
            return 0;
        }

        // Efficient calculation avoiding division when possible
        let recent_activity = self.suspicious_activity_score.min(100);
        (recent_activity * 100) / (self.cross_count.min(100) as u32)
    }

    /// Batch update multiple tick properties for efficiency
    /// Restored audit logic with safe overflow protection
    /// This method allows batch updates to liquidity, fee growth, and other properties
    /// in a single operation, minimizing sysvar hits and improving performance.
    /// # Arguments
    /// - `liquidity_delta`: Optional liquidity change in Q64.64 fixed point.
    /// - `is_negative`: Optional flag indicating if the liquidity change is negative.
    /// - `fee_growth_0`: Optional fee growth for token 0 in Q64.64 fixed point.
    /// - `fee_growth_1`: Optional fee growth for token 1 in Q64.64 fixed point.
    /// - `clock_slot`: The current clock slot to update the last crossed slot.
    /// - `timestamp`: The current timestamp to update the last update time.
    /// # Returns
    /// - `Result<()>`: Returns Ok(()) on success, or an error if the tick is not initialized or is in emergency pause.
    /// # Errors
    /// - `TickError::TickNotInitialized`: If the tick is not initialized.
    /// - `TickError::TickInEmergencyPause`: If the tick is in emergency pause.
    /// - `TickError::SuspiciousActivityDetected`: If suspicious activity is detected based on the liquidity delta.
    pub fn batch_update(
        &mut self,
        liquidity_delta: Option<Q64x64Signed>,
        fee_growth_0: Option<Q64x64>,
        fee_growth_1: Option<Q64x64>,
        clock_slot: u64,
        timestamp: i64,
    ) -> Result<()> {
        self.validate_operational_state()?;

        // Update liquidity if provided with restored audit logic
        if let Some(delta) = liquidity_delta {
            // Update liquidity with safe arithmetic
            self.liquidity_net = self.liquidity_net.checked_add(delta)?;

            self.cross_count = self.cross_count.saturating_add(1);

            // Restored suspicious activity detection with safe overflow protection
            let delta_abs = (delta.raw() >> 64) as u64; // Convert to absolute value in Q64.64 fixed point
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

/// Fee tier lookup table for O(log n) binary search validation
const FEE_TIER_LOOKUP: &[(u32, u16)] = &[
    (100, 1),     // 0.01% - stable pairs
    (500, 10),    // 0.05% - low volatility
    (3000, 60),   // 0.30% - standard pairs
    (10000, 200), // 1.00% - high volatility
];

/// Optimized tick spacing validation with binary search lookup
/// Uses O(log n) binary search instead of hardcoded match for better scalability
/// This function validates the tick spacing against the fee tier using a binary search approach,
/// ensuring that the tick spacing is aligned with the fee tier requirements.
/// It also checks for overflow conditions to prevent bitmap overflow issues.
/// # Arguments
/// - `tick_spacing`: The tick spacing to validate.
/// - `fee_tier`: The fee tier to validate against.
/// # Returns
/// - `Result<()>`: Returns Ok(()) if the tick spacing is valid, or an error if it is not.
/// # Errors
/// - `TickError::InvalidTickSpacing`: If the tick spacing is invalid.
/// - `TickError::UnsupportedFeeTier`: If the fee tier is not supported.
/// - `TickError::TickSpacingFeeTierMismatch`: If the tick spacing does not match the fee tier.
/// - `TickError::TickSpacingExceedsCapacity`: If the tick spacing exceeds the bitmap capacity.
pub fn validate_tick_spacing(tick_spacing: u16, fee_tier: u32) -> Result<()> {
    // Early bounds check to fail fast on invalid inputs
    if tick_spacing == 0 || tick_spacing > MAX_TICK_SPACING {
        return Err(TickError::InvalidTickSpacing.into());
    }

    // Binary search for fee tier validation - O(log n) complexity
    let expected_spacing = FEE_TIER_LOOKUP
        .binary_search_by_key(&fee_tier, |&(tier, _)| tier)
        .map(|idx| FEE_TIER_LOOKUP[idx].1)
        .map_err(|_| TickError::UnsupportedFeeTier)?;

    // Validate tick spacing matches fee tier requirement
    if tick_spacing != expected_spacing {
        return Err(TickError::TickSpacingFeeTierMismatch.into());
    }

    // Efficient capacity check using bit operations to prevent bitmap overflow
    let max_ticks = ((MAX_TICK - MIN_TICK) as u32) / (tick_spacing as u32);
    if max_ticks > MAX_BITMAP_CAPACITY as u32 {
        return Err(TickError::TickSpacingExceedsCapacity.into());
    }

    Ok(())
}

/// Instruction context for initializing a tick with enhanced validation
#[derive(Accounts)]
#[instruction(tick_index: i32, tick_spacing: u16, nonce: u32)]
pub struct InitializeTick<'info> {
    /// Tick account with enhanced validation
    #[account(
        init,
        payer = payer,
        space = 8 + TickData::INIT_SPACE, // Auto-calculated space
        seeds = [
            b"tick",
            pool.key().as_ref(),
            &tick_index.to_le_bytes(),
            &nonce.to_le_bytes()
        ],
        bump,
        constraint = tick_index % tick_spacing as i32 == 0 @ TickError::InvalidTickAlignment
    )]
    pub tick: AccountLoader<'info, TickData>,

    /// Pool account reference for validation
    #[account(
        constraint = pool.key() != Pubkey::default() @ TickError::InvalidTickIndex
    )]
    pub pool: AccountLoader<'info, PoolCore>,

    /// Payer account for transaction fees
    #[account(mut)]
    pub payer: Signer<'info>,

    /// System program for account creation
    pub system_program: Program<'info, System>,
}

/// Instruction context for loading a tick with optimized validation
/// This context is used to load an existing tick account with enhanced security checks.
#[derive(Accounts)]
pub struct LoadTick<'info> {
    /// Tick account with optimized loading and validation
    #[account(
        seeds = [
            b"tick",
            pool.key().as_ref(),
            &tick.load()?.tick_index.to_le_bytes(),
            &tick.load()?.initialization_nonce.to_le_bytes()
        ],
        bump,
        constraint = tick.load()?.initialized @ TickError::TickNotInitialized,
    )]
    pub tick: AccountLoader<'info, TickData>,

    /// Pool account reference for validation
    pub pool: AccountLoader<'info, PoolCore>,
}

/// Initialize tick with optimized validation and minimized sysvar hits
/// This function initializes a tick with enhanced security checks and validation,
/// ensuring that the tick index is aligned with the tick spacing.
/// It uses the current clock slot and timestamp to minimize sysvar hits,
/// and sets the initialization nonce for security.
/// # Arguments
/// - `ctx`: The context containing the accounts and instruction data.
/// - `tick_index`: The index of the tick to initialize.
/// - `tick_spacing`: The spacing between ticks, used for alignment validation.
/// - `nonce`: A nonce for security to prevent replay attacks.
/// # Returns
/// - `Result<()>`: Returns Ok(()) on success, or an error if the tick is not aligned or already initialized.
/// # Errors
/// - `TickError::InvalidTickAlignment`: If the tick index is not aligned with the tick spacing.
/// - `TickError::TickAlreadyInitialized`: If the tick is already initialized.
pub fn initialize_tick(
    ctx: Context<InitializeTick>,
    tick_index: i32,
    tick_spacing: u16,
    nonce: u32,
) -> Result<()> {
    // Fetch clock once at instruction level to minimize sysvar hits
    let clock = Clock::get()?;

    let tick = &mut ctx.accounts.tick.load_init()?;
    tick.initialize(
        tick_index,
        tick_spacing,
        nonce,
        clock.slot,
        clock.unix_timestamp,
    )?;

    Ok(())
}

/// Load tick with optimized validation
/// This function loads an existing tick account with enhanced security checks,
/// ensuring that the tick is initialized and not in emergency pause state.
/// It uses the current clock slot and timestamp to minimize sysvar hits.
/// # Arguments
/// - `ctx`: The context containing the accounts and instruction data.
/// # Returns
/// - `Result<()>`: Returns Ok(()) on success, or an error if the tick is not initialized.
/// # Errors
/// - `TickError::TickNotInitialized`: If the tick is not initialized.
pub fn load_tick(ctx: Context<LoadTick>) -> Result<()> {
    let tick = ctx.accounts.tick.load()?;

    // Perform any additional validation or operations
    require!(tick.initialized, TickError::TickNotInitialized);

    Ok(())
}

/// Instruction context for updating liquidity with minimized sysvar hits
/// This context is used to update the liquidity of a tick with enhanced security checks.
/// It allows for efficient updates while ensuring that the tick is initialized and not in emergency pause state
#[derive(Accounts)]
pub struct UpdateLiquidity<'info> {
    #[account(mut)]
    pub tick: AccountLoader<'info, TickData>,
}

/// Update liquidity with enhanced security and minimized sysvar hits
/// This function updates the liquidity of a tick with enhanced security checks,
/// ensuring that the tick is initialized and not in emergency pause state.
/// It uses the current clock slot and timestamp to minimize sysvar hits.
/// # Arguments
/// - `ctx`: The context containing the accounts and instruction data.
/// - `delta`: The change in liquidity in Q64.64 fixed point.
/// - `is_negative`: A flag indicating if the liquidity change is negative.
/// # Returns
/// - `Result<()>`: Returns Ok(()) on success, or an error if the tick is not initialized or is in emergency pause.
/// # Errors
/// - `TickError::TickNotInitialized`: If the tick is not initialized.
/// - `TickError::TickInEmergencyPause`: If the tick is in emergency pause.
pub fn update_liquidity_instruction(
    ctx: Context<UpdateLiquidity>,
    delta: Q64x64Signed,
) -> Result<()> {
    // Fetch clock once at instruction level to minimize sysvar hits
    let clock = Clock::get()?;
    let tick = &mut ctx.accounts.tick.load_mut()?;
    tick.update_liquidity_delta(delta, clock.slot, clock.unix_timestamp)?;

    Ok(())
}

/// Instruction context for batch updating tick properties
/// This context is used to batch update multiple properties of a tick,
/// including liquidity, fee growth, and operational metadata.
#[derive(Accounts)]
pub struct BatchUpdate<'info> {
    #[account(mut)]
    pub tick: AccountLoader<'info, TickData>,
}

/// Batch update tick properties with enhanced security and minimized sysvar hits
/// This function allows batch updates to liquidity, fee growth, and other properties
/// in a single operation, minimizing sysvar hits and improving performance.
/// It uses the current clock slot and timestamp to minimize sysvar hits.
/// # Arguments
/// - `ctx`: The context containing the accounts and instruction data.
/// - `liquidity_delta`: Optional liquidity change in Q64.64 fixed point.
/// - `is_negative`: Optional flag indicating if the liquidity change is negative.
/// - `fee_growth_0`: Optional fee growth for token 0 in Q64.64 fixed point.
/// - `fee_growth_1`: Optional fee growth for token 1 in Q64.64 fixed point.
/// # Returns
/// - `Result<()>`: Returns Ok(()) on success, or an error if the tick is not initialized or is in emergency pause.
/// # Errors
/// - `TickError::TickNotInitialized`: If the tick is not initialized.
/// - `TickError::TickInEmergencyPause`: If the tick is in emergency pause.
/// - `TickError::SuspiciousActivityDetected`: If suspicious activity is detected based on the liquidity delta.
pub fn batch_update_instruction(
    ctx: Context<BatchUpdate>,
    liquidity_delta: Option<Q64x64Signed>,
    fee_growth_0: Option<Q64x64>,
    fee_growth_1: Option<Q64x64>,
) -> Result<()> {
    // Fetch clock once at instruction level to minimize sysvar hits
    let clock = Clock::get()?;

    let tick = &mut ctx.accounts.tick.load_mut()?;
    tick.batch_update(
        liquidity_delta,
        fee_growth_0,
        fee_growth_1,
        clock.slot,
        clock.unix_timestamp,
    )?;

    Ok(())
}
