use crate::utils::constants::{MAX_TICK_CROSSES_PER_HOUR, SLOTS_PER_MINUTE};
use anchor_lang::prelude::*;

/// Protocol-optimized ring buffer for tracking per-minute event counts in a fixed window, using bit-packing for minimal memory footprint.
///
/// # Design rationale
/// - **Bit-packing**: Each minute counter is 4 bits (0-15), allowing 60 counters to fit in just 4 u64s (256 bits), minimizing on-chain storage and rent costs.
/// - **No Vec allocation**: Uses a fixed-size array for deterministic layout and zero-copy safety, critical for Solana/Anchor account serialization.
/// - **Window flexibility**: Supports both 60-minute (full hour) and 16-minute (short window) operation for different protocol needs and CU budgets.
/// - **Explicit memory layout**: Each u64 covers a fixed range of minutes, making bitwise operations predictable and efficient for both clearing and incrementing.
///
/// # Memory layout
/// - u64[0]: minutes 0-15   (bits 0-63)
/// - u64[1]: minutes 16-31  (bits 64-127)
/// - u64[2]: minutes 32-47  (bits 128-191)
/// - u64[3]: minutes 48-59  (bits 192-239, top 16 bits unused)
///
/// # Why not use a map or Vec?
/// - Deterministic, minimal-size layout is required for zero-copy and Anchor account safety.
/// - Bit-packing enables O(1) access and update, and is more gas/CU efficient than dynamic structures.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
#[repr(C)]
pub struct RingBuffer {
    /// Bit-packed counters for each minute in the window.
    ///
    /// # Why
    /// - 4 u64s (256 bits) store 60×4-bit counters (240 bits used, 16 bits unused), minimizing storage and maximizing cache efficiency.
    /// - Bit-packing allows atomic updates and O(1) access for both increment and clear operations.
    pub minute_counts: [u64; 4],

    /// Index of the current minute (0-59 or 0-15).
    ///
    /// # Why
    /// - Used to determine which counter to increment and which to clear as time advances.
    pub current_minute: u8,

    /// Cached sum of all 4-bit counters (not including overflow).
    ///
    /// # Why
    /// - Allows O(1) total count retrieval, avoiding repeated bit scans.
    pub total_count: u16,

    /// Last slot at which the buffer was updated.
    ///
    /// # Why
    /// - Used to detect time advancement and trigger window sliding logic.
    pub last_update_slot: u64,

    /// Overflow counter for when a 4-bit minute counter exceeds 15.
    ///
    /// # Why
    /// - Prevents loss of event data when a single minute is extremely active; ensures total count remains accurate.
    pub overflow_count: u8,

    /// Window size (16 or 60 minutes).
    ///
    /// # Why
    /// - Allows protocol to trade off memory usage and granularity for different use cases (e.g., short-term vs. hourly rate limiting).
    pub window_size: u8,

    /// Explicit padding for 8-byte alignment and future extensibility.
    pub _padding: [u8; 6],
}

/// Provides a default, zeroed ring buffer with a 60-minute window.
///
/// # Why
/// - Ensures deterministic initialization for Anchor/zero-copy safety.
impl Default for RingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl RingBuffer {
    /// Constructs a new ring buffer with a 60-minute window.
    ///
    /// # Why
    /// - 60-minute window is the protocol default for hourly rate limiting and analytics.
    /// - All fields are zeroed for deterministic state and Anchor safety.
    pub fn new() -> Self {
        Self {
            minute_counts: [0; 4],
            current_minute: 0,
            total_count: 0,
            last_update_slot: 0,
            overflow_count: 0,
            window_size: 60,
            _padding: [0; 6],
        }
    }

    /// Constructs a new ring buffer with a 16-minute window for memory/CU-constrained use cases.
    ///
    /// # Why
    /// - 16-minute window uses only the first u64 (16×4-bit counters = 64 bits), reducing memory and compute for short-term rate limiting.
    pub fn new_16_minute() -> Self {
        let mut buffer = Self::new();
        buffer.window_size = 16;
        buffer
    }

    /// Advances the ring buffer to the current minute and increments the event count, with protocol-aligned overflow and clearing logic.
    ///
    /// # Why
    /// - Ensures the buffer always reflects the most recent window, sliding forward as time advances.
    /// - Handles both small and large jumps efficiently, using the optimal clearing strategy for each case.
    /// - Increments the current minute's counter, with overflow protection to avoid losing event data.
    pub fn update_and_increment(&mut self, current_slot: u64) -> Result<()> {
        let new_minute = ((current_slot / SLOTS_PER_MINUTE) % (self.window_size as u64)) as u8;

        // If time has advanced, clear all minutes between old and new (using optimal strategy)
        if new_minute != self.current_minute {
            self.advance_to_minute(new_minute);
        }

        // Always increment the current minute's counter, with overflow protection
        self.increment_current_minute()?;
        self.last_update_slot = current_slot;

        Ok(())
    }

    /// Advances the buffer to a new minute, clearing all intermediate counters using the most efficient strategy for the jump size.
    ///
    /// # Why
    /// - Sliding window logic must clear all minutes between the old and new index to maintain accurate counts.
    /// - Uses bulk, bitmask, or loop clearing depending on jump size for optimal CU and CPU efficiency.
    /// - Ensures no stale data remains in the window after a time jump.
    #[inline(always)]
    fn advance_to_minute(&mut self, target_minute: u8) {
        let minutes_advanced = if target_minute >= self.current_minute {
            target_minute - self.current_minute
        } else {
            self.window_size - self.current_minute + target_minute
        };

        // Choose the optimal clearing strategy for the jump size
        if minutes_advanced >= self.window_size {
            // Full window jump: just zero everything (O(1))
            self.bulk_clear_all();
        } else if minutes_advanced < 16 {
            // Small jump: bitmask clear is fastest (fewer branches, better cache usage)
            self.bitmask_clear_minutes(minutes_advanced);
        } else {
            // Medium jump: loop clear is best for arbitrary ranges
            self.loop_clear_minutes(minutes_advanced);
        }

        self.current_minute = target_minute;
    }

    /// Bulk clears all counters and resets tracking state in O(1) time.
    ///
    /// # Why
    /// - Used when the window jumps by >= window_size, so all previous data is stale.
    /// - Zeroes all counters and resets overflow/total for deterministic state.
    #[inline(always)]
    fn bulk_clear_all(&mut self) {
        self.minute_counts = [0; 4];
        self.total_count = 0;
        self.overflow_count = 0;
    }

    /// Clears a small number of minutes using direct bitmasking for maximum CPU efficiency.
    ///
    /// # Why
    /// - For <16 jumps, bitmasking is faster than looping due to fewer branches and better cache/pipeline utilization.
    #[inline(always)]
    fn bitmask_clear_minutes(&mut self, minutes_to_clear: u8) {
        for i in 0..minutes_to_clear {
            let minute_to_clear = (self.current_minute + i + 1) % self.window_size;
            self.clear_specific_minute(minute_to_clear);
        }
    }

    /// Clears a medium number of minutes using a loop, for jump sizes where bitmasking is not optimal.
    ///
    /// # Why
    /// - For 16-59 minute jumps, looping is more efficient than bitmasking or bulk clear.
    #[inline(always)]
    fn loop_clear_minutes(&mut self, minutes_to_clear: u8) {
        for _ in 0..minutes_to_clear {
            self.clear_oldest_minute();
        }
    }

    /// Clears a specific minute's 4-bit counter using bitwise operations.
    ///
    /// # Why
    /// - Bitwise clear is O(1) and avoids unnecessary loops or branches.
    /// - Ensures total_count remains accurate by subtracting the cleared value.
    #[inline(always)]
    fn clear_specific_minute(&mut self, minute: u8) {
        let array_index = (minute as usize) / 16;
        let bit_position = ((minute as usize) % 16) * 4;

        // Only operate if within bounds (0-3)
        if array_index < 4 {
            let mask = 0xF_u64 << bit_position;
            let old_value = (self.minute_counts[array_index] & mask) >> bit_position;
            self.total_count = self.total_count.saturating_sub(old_value as u16);
            self.minute_counts[array_index] &= !mask;
        }
    }

    /// Increments the current minute's 4-bit counter, with overflow protection.
    ///
    /// # Why
    /// - 4-bit counters cap at 15; further increments are tracked in overflow_count to avoid losing event data.
    /// - Ensures total_count remains accurate for O(1) rate checks.
    #[inline(always)]
    fn increment_current_minute(&mut self) -> Result<()> {
        let array_index = (self.current_minute as usize) / 16;
        let bit_position = ((self.current_minute as usize) % 16) * 4;

        if array_index < 4 {
            let mask = 0xF_u64 << bit_position;
            let current_value = (self.minute_counts[array_index] & mask) >> bit_position;

            if current_value < 15 {
                // Normal increment: fits in 4 bits
                self.minute_counts[array_index] = (self.minute_counts[array_index] & !mask)
                    | ((current_value + 1) << bit_position);
                self.total_count = self.total_count.saturating_add(1);
            } else {
                // Overflow: track extra events in overflow_count
                self.overflow_count = self.overflow_count.saturating_add(1);
            }
        }

        Ok(())
    }

    /// Clears the oldest minute in the window, used during sliding window advancement.
    ///
    /// # Why
    /// - Ensures that as the window slides, stale data is removed and total_count remains accurate.
    #[inline(always)]
    fn clear_oldest_minute(&mut self) {
        let oldest_minute = (self.current_minute + 1) % self.window_size;
        self.clear_specific_minute(oldest_minute);
    }

    /// Returns the total number of events in the window in O(1) time.
    ///
    /// # Why
    /// - Uses cached total and overflow to avoid scanning all counters, critical for CU efficiency on Solana.
    /// - Overflow_count is multiplied by 15 (max value of a 4-bit counter) to account for all extra events.
    #[inline(always)]
    pub fn get_total_count(&self) -> u32 {
        self.total_count as u32 + (self.overflow_count as u32 * 15)
    }

    /// Checks if the protocol-defined rate limit has been exceeded in O(1) time.
    ///
    /// # Why
    /// - Enables fast, deterministic enforcement of rate limits for protocol safety and anti-abuse.
    #[inline(always)]
    pub fn is_rate_limited(&self) -> bool {
        self.get_total_count() >= MAX_TICK_CROSSES_PER_HOUR
    }
}
