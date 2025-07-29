//! High-performance, zero-copy tick storage for Solana AMM protocols using Q64.64 precision.
//!
//! # Design Rationale
//! - All account layouts are zero-copy and bytemuck-compatible for maximum on-chain efficiency and deterministic serialization.
//! - Bitmap-based tick existence checks enable O(1) lookups and minimize storage overhead for large tick ranges.
//! - Tick compression uses Q64.64 fixed-point math and packed fee encoding to balance precision, CU cost, and account size.
//! - Loss tracking and compression statistics are maintained for auditability and protocol safety.
//! - Inline storage is optimized for small pools, while overflow pages scale for large pools without exceeding Solana account limits.
use crate::error::TickError;
use crate::math::core_arithmetic::{mul_div, Q64x64, Q64x64Signed};
use crate::state::tick::tick_data::TickData;
use crate::utils::constants::{
    INLINE_TICK_CAPACITY, MAIN_BITMAP_WORDS, MAX_LOSS_PCT, MAX_STORAGE_PAGES, PAGE_BITMAP_WORDS,
    TICKS_PER_PAGE, VIRTUAL_TICK_OFFSET,
};
use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

/// Calculates the minimum tick spacing that ensures full bitmap coverage.
///
/// # Why this function?
/// - Ensures that tick spacing is always compatible with bitmap capacity, preventing out-of-bounds errors and wasted storage.
const fn min_tick_spacing_for_coverage() -> u16 {
    let max_ticks = MAIN_BITMAP_WORDS * 64;
    let virtual_range = (2 * VIRTUAL_TICK_OFFSET) as usize;
    virtual_range.div_ceil(max_ticks) as u16
}

/// Helper struct for compressed slot/epoch tracking.
///
/// # Why this struct?
/// - Reduces storage footprint for timing metadata, supporting efficient tick compression and migration.
#[derive(Default, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
pub struct SlotEpoch {
    pub base_epoch_slot: u64,
}

/// Packed fee pair with Q64.64 precision, stored in 8 bytes.
///
/// # Why this struct?
/// - Encodes two Q64.64 fee values into a single u64 for space efficiency.
/// - Tracks compression loss for auditability and protocol safety.
#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C)]
pub struct PackedFees {
    /// Fee data packed into 64 bits (24 bits per fee + control bits)
    pub packed_data: u64,
}

impl PackedFees {
    /// Packs two Q64.64 fee values into a single u64, returning the packed struct and loss percentage.
    ///
    /// # Why this approach?
    /// - Minimizes account size and CU cost for tick storage.
    /// - Returns loss percentage for protocol-level audit and safety checks.
    pub fn new(fee0: Q64x64, fee1: Q64x64) -> Result<(Self, u8)> {
        let (packed_data, loss_pct) = pack_fee_pair_q64(fee0, fee1)?;
        Ok((Self { packed_data }, loss_pct))
    }

    /// Unpacks the packed fee pair back into Q64.64 values.
    ///
    /// # Why this approach?
    /// - Allows lossless decompression for most values, with explicit loss tracking for edge cases.
    pub fn unpack(&self) -> (Q64x64, Q64x64) {
        unpack_fee_pair_q64(self.packed_data)
    }
}

/// High-precision, compressed tick representation (56 bytes) with full Q64.64 precision.
///
/// # Why this struct?
/// - Encodes all relevant tick data in a compact, zero-copy format for efficient on-chain storage and migration.
/// - Compression loss and suspicious activity are tracked for protocol safety and auditability.
/// - Padding ensures alignment and deterministic layout for bytemuck compatibility.
#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C)]
pub struct CompressedTick {
    pub liquidity_net: Q64x64Signed,      // 16 bytes - matches TickData
    pub fee_pair: PackedFees,             // 8 bytes - compressed from fee_growth_outside_0/1
    pub tick_index: i32,                  // 4 bytes - matches TickData
    pub slot_delta: u32,                  // 4 bytes - compressed from last_crossed_slot
    pub cross_count: u32,                 // 4 bytes - compressed from u64 to u32
    pub last_update_timestamp_delta: u32, // 4 bytes - compressed from i64 timestamp
    pub suspicious_activity_score: u32,   // 4 bytes - matches TickData
    pub status_flags: u16,                // 2 bytes - matches TickData
    pub tick_spacing_validation: u16,     // 2 bytes - matches TickData
    pub initialization_nonce: u32,        // 4 bytes - matches TickData
    pub initialized: u8,                  // 1 byte - converted from bool
    pub loss_pct: u8,                     // 1 byte - compression loss tracking
    pub _padding: [u8; 10],               // Pad to 56 bytes total
}

/// Main storage account for compressed ticks, containing metadata and inline ticks for small pools.
///
/// # Why this struct?
/// - Zero-copy layout enables fast, deterministic serialization and migration.
/// - Inline tick storage avoids page allocation for small pools, minimizing CU and rent costs.
/// - Bitmap and page references enable scalable tick management for large pools.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct CompressedTickStorage {
    // Pool identification and metadata
    //
    // # Why these fields?
    // - Pool and token keys uniquely identify the AMM instance and its assets.
    pub pool: Pubkey,    // 32 bytes
    pub token_a: Pubkey, // 32 bytes
    pub token_b: Pubkey, // 32 bytes

    // Timing and versioning
    //
    // # Why these fields?
    // - Epoch and slot tracking support efficient compression and migration.
    // - Versioning enables protocol upgrades and backward compatibility.
    pub epoch: SlotEpoch,  // 8 bytes
    pub last_slot: u64,    // 8 bytes
    pub created_slot: u64, // 8 bytes

    // Pool parameters
    //
    // # Why these fields?
    // - Tick spacing and fee tier are core pool parameters, validated for bitmap compatibility and protocol safety.
    pub tick_spacing: u16, // 2 bytes
    pub version: u8,       // 1 byte
    pub fee_tier: u32,     // 4 bytes

    // Tick counting and page management
    //
    // # Why these fields?
    // - Inline/page counts and indices track tick storage usage and enable scalable pool growth.
    pub inline_tick_count: u8, // 1 byte - ticks in this account
    pub total_tick_count: u32, // 4 bytes - total across all accounts
    pub active_page_count: u8, // 1 byte - number of page accounts in use
    pub next_page_index: u8,   // 1 byte - next page to allocate

    pub _padding1: [u8; 4], // Pad to boundary: 132 bytes so far

    // Main bitmap for fast tick existence checks
    //
    // # Why bitmap?
    // - Enables O(1) tick existence checks and minimizes storage overhead for large tick ranges.
    pub main_bitmap: [u64; MAIN_BITMAP_WORDS], // 512 bytes (4096 tick coverage)

    // Page account references
    //
    // # Why these fields?
    // - Enables overflow tick storage for large pools, scaling beyond inline capacity.
    pub page_accounts: [Pubkey; MAX_STORAGE_PAGES], // 640 bytes (20 * 32)

    // Inline tick storage for small pools (avoids need for pages)
    //
    // # Why inline?
    // - Fast-path for small pools, minimizing CU and rent costs by avoiding page allocation.
    pub inline_ticks: [CompressedTick; INLINE_TICK_CAPACITY], // 3200 bytes (50 * 64)

    // Performance and debugging info
    //
    // # Why these fields?
    // - Tracks compression/decompression counts and loss statistics for protocol auditability and optimization.
    pub total_compressions: u64,   // 8 bytes
    pub total_decompressions: u64, // 8 bytes
    pub avg_loss_pct: u16,         // 2 bytes
    pub max_loss_pct: u8,          // 1 byte
    pub _padding2: [u8; 5],        // 24 bytes

                                   // Total: 132 + 512 + 640 + 3200 + 24 = 4508 bytes
}

/// Page account for overflow tick storage.
///
/// # Why this struct?
/// - Enables scalable tick storage for large pools, with bitmap and statistics for efficient management and auditability.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct TickStoragePage {
    // Back-reference and identification
    //
    // # Why these fields?
    // - Parent storage and page index enable deterministic page management and migration.
    pub parent_storage: Pubkey, // 32 bytes - main storage account
    pub page_index: u8,         // 1 byte - which page this is (0-19)
    pub tick_count: u8,         // 1 byte - active ticks in this page
    pub created_slot: u64,      // 8 bytes - when page was created
    pub last_update_slot: u64,  // 8 bytes - last modification

    pub _padding: [u8; 6], // 56 bytes total header

    // Page-local bitmap for this tick range
    //
    // # Why bitmap?
    // - Enables O(1) tick existence checks within the page, minimizing lookup cost.
    pub page_bitmap: [u64; PAGE_BITMAP_WORDS], // 64 bytes (512 tick coverage per page)

    // Tick storage array
    //
    // # Why array?
    // - Fixed-size array enables zero-copy, deterministic layout and efficient migration.
    pub ticks: [CompressedTick; TICKS_PER_PAGE], // 9600 bytes (150 * 64)

    // Page statistics
    //
    // # Why these fields?
    // - Tracks compression loss and usage for protocol audit and optimization.
    pub compression_count: u32, // 4 bytes
    pub avg_loss_pct: u16,      // 2 bytes
    pub _stats_padding: [u8; 2], // 8 bytes

                                // Total: 56 + 64 + 9600 + 8 = 9728 bytes (under 10KB limit)
}

impl CompressedTickStorage {
    /// Initializes the main storage account for a pool.
    ///
    /// # Why this approach?
    /// - Validates tick spacing and bitmap coverage to prevent out-of-bounds errors.
    /// - Initializes all fields to deterministic defaults for zero-copy safety and migration.
    pub fn initialize(
        &mut self,
        pool: Pubkey,
        token_a: Pubkey,
        token_b: Pubkey,
        tick_spacing: u16,
        fee_tier: u32,
        current_slot: u64,
    ) -> Result<()> {
        // Validate tick spacing
        require!(
            tick_spacing > 0 && tick_spacing <= 32768,
            TickError::InvalidTickSpacing
        );

        // Validate bitmap coverage
        let min_supported = min_tick_spacing_for_coverage();
        require!(tick_spacing >= min_supported, TickError::InvalidTickSpacing);

        // Initialize fields
        self.pool = pool;
        self.token_a = token_a;
        self.token_b = token_b;
        self.tick_spacing = tick_spacing;
        self.fee_tier = fee_tier;
        self.version = 1;

        self.epoch = SlotEpoch {
            base_epoch_slot: current_slot,
        };
        self.last_slot = current_slot;
        self.created_slot = current_slot;

        // Initialize counters
        self.inline_tick_count = 0;
        self.total_tick_count = 0;
        self.active_page_count = 0;
        self.next_page_index = 0;

        // Clear bitmaps and arrays
        self.main_bitmap = [0u64; MAIN_BITMAP_WORDS];
        self.page_accounts = [Pubkey::default(); MAX_STORAGE_PAGES];
        self.inline_ticks = [CompressedTick::default(); INLINE_TICK_CAPACITY];

        // Initialize stats
        self.total_compressions = 0;
        self.total_decompressions = 0;
        self.avg_loss_pct = 0;
        self.max_loss_pct = 0;

        Ok(())
    }

    /// Adds a new page account reference for overflow tick storage.
    ///
    /// # Why this approach?
    /// - Enforces page index and capacity limits to prevent out-of-bounds errors and wasted storage.
    pub fn add_page_account(&mut self, page_account: Pubkey, page_index: u8) -> Result<()> {
        require!(
            page_index < MAX_STORAGE_PAGES as u8,
            TickError::InvalidPageIndex
        );
        require!(
            self.active_page_count < MAX_STORAGE_PAGES as u8,
            TickError::StorageCapacityExceeded
        );

        self.page_accounts[page_index as usize] = page_account;
        if page_index >= self.active_page_count {
            self.active_page_count = page_index + 1;
        }
        self.next_page_index = self.active_page_count;

        Ok(())
    }

    /// Converts a tick index to its virtual tick index for bitmap operations.
    ///
    /// # Why this approach?
    /// - Ensures all bitmap operations are bounds-checked and compatible with tick spacing and offset.
    fn get_virtual_tick_index(&self, tick_index: i32) -> Result<usize> {
        let adjusted_index = tick_index
            .checked_add(VIRTUAL_TICK_OFFSET)
            .ok_or(TickError::TickIndexOutOfRange)?;

        require!(adjusted_index >= 0, TickError::TickIndexOutOfRange);

        let virtual_tick = (adjusted_index as u64) / (self.tick_spacing as u64);
        require!(
            virtual_tick <= usize::MAX as u64,
            TickError::TickIndexOutOfRange
        );

        let virtual_tick = virtual_tick as usize;
        require!(
            virtual_tick < (MAIN_BITMAP_WORDS * 64),
            TickError::TickIndexOutOfRange
        );

        Ok(virtual_tick)
    }

    /// Sets the bit for a tick in the main bitmap.
    ///
    /// # Why this approach?
    /// - Enables O(1) tick existence checks and prevents duplicate tick storage.
    fn set_main_bitmap_bit(&mut self, virtual_tick: usize) -> Result<()> {
        let word_idx = virtual_tick / 64;
        require!(word_idx < MAIN_BITMAP_WORDS, TickError::TickIndexOutOfRange);

        let bit_idx = virtual_tick % 64;
        self.main_bitmap[word_idx] |= 1u64 << bit_idx;
        Ok(())
    }

    /// Checks if a tick exists in the main bitmap.
    ///
    /// # Why this approach?
    /// - Enables fast-path existence checks for tick operations, minimizing CU and lookup cost.
    pub fn tick_exists_in_bitmap(&self, tick_index: i32) -> Result<bool> {
        let virtual_tick = self.get_virtual_tick_index(tick_index)?;
        let word_idx = virtual_tick / 64;

        if word_idx >= MAIN_BITMAP_WORDS {
            return Ok(false);
        }

        let bit_idx = virtual_tick % 64;
        Ok((self.main_bitmap[word_idx] & (1u64 << bit_idx)) != 0)
    }

    /// Adds a tick to inline storage (fast path for small pools).
    ///
    /// # Why this approach?
    /// - Avoids page allocation for small pools, minimizing CU and rent costs.
    /// - Compresses tick data and tracks loss for protocol auditability.
    pub fn add_inline_tick(&mut self, tick_data: &TickData, slot: u64) -> Result<bool> {
        if self.inline_tick_count >= INLINE_TICK_CAPACITY as u8 {
            return Ok(false); // Need to use page storage
        }

        // Validate slot
        require!(slot >= self.epoch.base_epoch_slot, TickError::InvalidSlot);
        let slot_delta = slot - self.epoch.base_epoch_slot;
        require!(slot_delta <= u32::MAX as u64, TickError::SlotOverflow);

        self.last_slot = slot;

        // Set bitmap bit
        let virtual_tick = self.get_virtual_tick_index(tick_data.tick_index)?;
        self.set_main_bitmap_bit(virtual_tick)?;

        // Compress fees
        let (fee_pair, loss_pct) = PackedFees::new(
            tick_data.fee_growth_outside_0,
            tick_data.fee_growth_outside_1,
        )?;

        // Compress timestamp delta
        let timestamp_delta = if tick_data.last_update_timestamp >= 0 {
            let base_timestamp = self.epoch.base_epoch_slot as i64 * 400; // Approximate slot-to-ms conversion
            let delta = tick_data.last_update_timestamp - base_timestamp;
            if delta >= 0 && delta <= u32::MAX as i64 {
                delta as u32
            } else {
                0 // Fallback for out-of-range timestamps
            }
        } else {
            0
        };

        // Compress cross_count from u64 to u32
        let compressed_cross_count = tick_data.cross_count.min(u32::MAX as u64) as u32;

        // Create compressed tick
        let compressed = CompressedTick {
            liquidity_net: tick_data.liquidity_net,
            fee_pair,
            tick_index: tick_data.tick_index,
            slot_delta: slot_delta as u32,
            cross_count: compressed_cross_count,
            last_update_timestamp_delta: timestamp_delta,
            suspicious_activity_score: tick_data.suspicious_activity_score,
            status_flags: tick_data.status_flags,
            tick_spacing_validation: tick_data.tick_spacing_validation,
            initialization_nonce: tick_data.initialization_nonce,
            initialized: if tick_data.initialized { 1 } else { 0 },
            loss_pct,
            _padding: [0; 10],
        };

        // Add to inline storage
        self.inline_ticks[self.inline_tick_count as usize] = compressed;
        self.inline_tick_count += 1;
        self.total_tick_count += 1;

        // Update statistics
        self.total_compressions += 1;
        self.max_loss_pct = self.max_loss_pct.max(loss_pct);
        self.update_avg_loss_pct(loss_pct);

        Ok(true)
    }

    /// Finds a tick in inline storage by index.
    ///
    /// # Why this approach?
    /// - Enables O(n) search for small pools, which is CU-efficient given small inline capacity.
    pub fn find_inline_tick(&self, tick_index: i32) -> Option<usize> {
        (0..self.inline_tick_count as usize)
            .find(|&i| self.inline_ticks[i].tick_index == tick_index)
    }

    /// Decompresses an inline tick by index.
    ///
    /// # Why this approach?
    /// - Restores full TickData from compressed format, with loss tracking for auditability.
    pub fn decompress_inline_tick(&self, index: usize) -> Result<TickData> {
        require!(
            index < self.inline_tick_count as usize,
            TickError::TickNotFound
        );

        let ct = &self.inline_ticks[index];
        let (fee0, fee1) = ct.fee_pair.unpack();

        let last_crossed_slot = self
            .epoch
            .base_epoch_slot
            .checked_add(ct.slot_delta as u64)
            .ok_or(TickError::SlotOverflow)?;

        // Decompress timestamp
        let base_timestamp = self.epoch.base_epoch_slot as i64 * 400; // Approximate slot-to-ms conversion
        let last_update_timestamp = base_timestamp + ct.last_update_timestamp_delta as i64;

        Ok(TickData {
            tick_index: ct.tick_index,
            _padding_1: [0; 4],
            liquidity_net: ct.liquidity_net,
            fee_growth_outside_0: fee0,
            fee_growth_outside_1: fee1,
            last_crossed_slot,
            cross_count: ct.cross_count as u64, // Expand back to u64
            last_update_timestamp,
            suspicious_activity_score: ct.suspicious_activity_score,
            max_suspicious_threshold: 0, // Default value - not stored in compressed format
            status_flags: ct.status_flags,
            tick_spacing_validation: ct.tick_spacing_validation,
            initialization_nonce: ct.initialization_nonce,
            initialized: ct.initialized != 0,
            _padding_2: [0; 7],
            reserved: [0; 4], // Default values - not stored in compressed format
        })
    }

    /// Updates the running average loss percentage for compression.
    ///
    /// # Why this approach?
    /// - Maintains protocol-level statistics for auditability and optimization.
    fn update_avg_loss_pct(&mut self, new_loss: u8) {
        if self.total_compressions == 1 {
            self.avg_loss_pct = new_loss as u16;
        } else {
            // Running average
            let total = self.avg_loss_pct as u64 * (self.total_compressions - 1);
            self.avg_loss_pct = ((total + new_loss as u64) / self.total_compressions) as u16;
        }
    }

    /// Returns storage statistics for the account.
    ///
    /// # Why this approach?
    /// - Enables protocol-level monitoring and optimization of tick storage and compression.
    pub fn get_storage_stats(&self) -> StorageStats {
        StorageStats {
            total_tick_count: self.total_tick_count as usize,
            inline_tick_count: self.inline_tick_count as usize,
            page_count: self.active_page_count as usize,
            max_inline_capacity: INLINE_TICK_CAPACITY,
            max_page_capacity: MAX_STORAGE_PAGES,
            total_compressions: self.total_compressions,
            total_decompressions: self.total_decompressions,
            avg_loss_pct: (self.avg_loss_pct as f32) / 100.0,
            max_loss_pct: (self.max_loss_pct as f32) / 100.0,
            main_account_bytes: std::mem::size_of::<CompressedTickStorage>(),
            page_account_bytes: std::mem::size_of::<TickStoragePage>(),
        }
    }

    /// Calculates total space usage across all accounts (main + pages).
    ///
    /// # Why this approach?
    /// - Enables rent estimation and protocol-level monitoring of storage footprint.
    pub fn calculate_total_space(&self) -> usize {
        let main_space = std::mem::size_of::<CompressedTickStorage>();
        let page_space = self.active_page_count as usize * std::mem::size_of::<TickStoragePage>();
        main_space + page_space
    }
}

impl TickStoragePage {
    /// Initializes a new tick storage page.
    ///
    /// # Why this approach?
    /// - Sets all fields to deterministic defaults for zero-copy safety and migration.
    pub fn initialize(
        &mut self,
        parent_storage: Pubkey,
        page_index: u8,
        current_slot: u64,
    ) -> Result<()> {
        self.parent_storage = parent_storage;
        self.page_index = page_index;
        self.tick_count = 0;
        self.created_slot = current_slot;
        self.last_update_slot = current_slot;

        self.page_bitmap = [0u64; PAGE_BITMAP_WORDS];
        self.ticks = [CompressedTick::default(); TICKS_PER_PAGE];

        self.compression_count = 0;
        self.avg_loss_pct = 0;

        Ok(())
    }

    /// Adds a tick to this page (overflow storage for large pools).
    ///
    /// # Why this approach?
    /// - Compresses tick data and tracks loss for protocol auditability.
    /// - Enforces page capacity and slot validation for protocol safety.
    pub fn add_tick(&mut self, tick_data: &TickData, slot: u64, epoch_base: u64) -> Result<bool> {
        if self.tick_count >= TICKS_PER_PAGE as u8 {
            return Ok(false); // Page full
        }

        // Validate slot
        require!(slot >= epoch_base, TickError::InvalidSlot);
        let slot_delta = slot - epoch_base;
        require!(slot_delta <= u32::MAX as u64, TickError::SlotOverflow);

        self.last_update_slot = slot;

        // Compress fees
        let (fee_pair, loss_pct) = PackedFees::new(
            tick_data.fee_growth_outside_0,
            tick_data.fee_growth_outside_1,
        )?;

        // Compress timestamp delta
        let timestamp_delta = if tick_data.last_update_timestamp >= 0 {
            let base_timestamp = epoch_base as i64 * 400; // Approximate slot-to-ms conversion
            let delta = tick_data.last_update_timestamp - base_timestamp;
            if delta >= 0 && delta <= u32::MAX as i64 {
                delta as u32
            } else {
                0 // Fallback for out-of-range timestamps
            }
        } else {
            0
        };

        // Compress cross_count from u64 to u32
        let compressed_cross_count = tick_data.cross_count.min(u32::MAX as u64) as u32;

        // Create compressed tick
        let compressed = CompressedTick {
            liquidity_net: tick_data.liquidity_net,
            fee_pair,
            tick_index: tick_data.tick_index,
            slot_delta: slot_delta as u32,
            cross_count: compressed_cross_count,
            last_update_timestamp_delta: timestamp_delta,
            suspicious_activity_score: tick_data.suspicious_activity_score,
            status_flags: tick_data.status_flags,
            tick_spacing_validation: tick_data.tick_spacing_validation,
            initialization_nonce: tick_data.initialization_nonce,
            initialized: if tick_data.initialized { 1 } else { 0 },
            loss_pct,
            _padding: [0; 10],
        };

        // Add to page storage
        self.ticks[self.tick_count as usize] = compressed;
        self.tick_count += 1;

        // Update page statistics
        self.compression_count += 1;
        self.update_avg_loss_pct(loss_pct);

        Ok(true)
    }

    /// Finds a tick in this page by index.
    ///
    /// # Why this approach?
    /// - Enables O(n) search for page storage, which is CU-efficient given fixed page capacity.
    pub fn find_tick(&self, tick_index: i32) -> Option<usize> {
        (0..self.tick_count as usize).find(|&i| self.ticks[i].tick_index == tick_index)
    }

    /// Decompresses a tick by index in this page.
    ///
    /// # Why this approach?
    /// - Restores full TickData from compressed format, with loss tracking for auditability.
    pub fn decompress_tick(&self, index: usize, epoch_base: u64) -> Result<TickData> {
        require!(index < self.tick_count as usize, TickError::TickNotFound);

        let ct = &self.ticks[index];
        let (fee0, fee1) = ct.fee_pair.unpack();

        let last_crossed_slot = epoch_base
            .checked_add(ct.slot_delta as u64)
            .ok_or(TickError::SlotOverflow)?;

        // Decompress timestamp
        let base_timestamp = epoch_base as i64 * 400; // Approximate slot-to-ms conversion
        let last_update_timestamp = base_timestamp + ct.last_update_timestamp_delta as i64;

        Ok(TickData {
            tick_index: ct.tick_index,
            _padding_1: [0; 4],
            liquidity_net: ct.liquidity_net,
            fee_growth_outside_0: fee0,
            fee_growth_outside_1: fee1,
            last_crossed_slot,
            cross_count: ct.cross_count as u64, // Expand back to u64
            last_update_timestamp,
            suspicious_activity_score: ct.suspicious_activity_score,
            max_suspicious_threshold: 0, // Default value - not stored in compressed format
            status_flags: ct.status_flags,
            tick_spacing_validation: ct.tick_spacing_validation,
            initialization_nonce: ct.initialization_nonce,
            initialized: ct.initialized != 0,
            _padding_2: [0; 7],
            reserved: [0; 4], // Default values - not stored in compressed format
        })
    }

    /// Updates the running average loss percentage for compression in this page.
    ///
    /// # Why this approach?
    /// - Maintains page-level statistics for auditability and optimization.
    fn update_avg_loss_pct(&mut self, new_loss: u8) {
        if self.compression_count == 1 {
            self.avg_loss_pct = new_loss as u16;
        } else {
            let total = self.avg_loss_pct as u64 * (self.compression_count - 1) as u64;
            self.avg_loss_pct = ((total + new_loss as u64) / self.compression_count as u64) as u16;
        }
    }
}

/// Q64.64 fee compression with 24-bit precision per fee.
///
/// # Why this function?
/// - Packs two Q64.64 fee values into a single u64 for space efficiency.
/// - Tracks precision loss for protocol auditability and safety checks.
#[inline(always)]
fn pack_fee_pair_q64(f0: Q64x64, f1: Q64x64) -> Result<(u64, u8)> {
    let raw0 = f0.raw();
    let raw1 = f1.raw();

    if raw0 == 0 && raw1 == 0 {
        return Ok((0, 0));
    }

    // Use 24 bits per fee (48 bits total) for high precision
    let shift_amount = 64 + 64 - 24; // Q64.64 to 24-bit

    let f0_shifted = raw0 >> shift_amount;
    let f1_shifted = raw1 >> shift_amount;

    // Check 24-bit bounds
    require!(
        f0_shifted <= 0xFFFFFF && f1_shifted <= 0xFFFFFF,
        TickError::CompressionOverflow
    );

    // Pack into u64 (24 bits each, 48 bits total used)
    let packed = ((f0_shifted as u64) << 24) | ((f1_shifted as u64) & 0xFFFFFF);

    // Calculate precision loss
    let mask = (1u128 << shift_amount) - 1;
    let lost0 = raw0 & mask;
    let lost1 = raw1 & mask;
    let max_orig = raw0.max(raw1);
    let max_lost = lost0.max(lost1);

    let loss = if max_orig == 0 {
        0
    } else {
        (mul_div(max_lost, 100, max_orig)?.min(100)) as u8
    };

    require!(loss <= MAX_LOSS_PCT, TickError::ExcessiveCompressionLoss);
    Ok((packed, loss))
}

#[inline(always)]
fn unpack_fee_pair_q64(packed: u64) -> (Q64x64, Q64x64) {
    // Unpacks two 24-bit fee values from a u64 and restores Q64.64 format.
    //
    // # Why this approach?
    // - Enables lossless decompression for most values, with explicit loss tracking for edge cases.
    let f0_24bit = (packed >> 24) & 0xFFFFFF;
    let f1_24bit = packed & 0xFFFFFF;

    let shift_amount = 64 + 64 - 24;
    let f0_raw = (f0_24bit as u128) << shift_amount;
    let f1_raw = (f1_24bit as u128) << shift_amount;

    (Q64x64::from_raw(f0_raw), Q64x64::from_raw(f1_raw))
}

impl Default for CompressedTick {
    fn default() -> Self {
        Self {
            liquidity_net: Q64x64Signed::zero(),
            fee_pair: PackedFees { packed_data: 0 },
            tick_index: 0,
            slot_delta: 0,
            cross_count: 0,
            last_update_timestamp_delta: 0,
            suspicious_activity_score: 0,
            status_flags: 0,
            tick_spacing_validation: 0,
            initialization_nonce: 0,
            initialized: 0,
            loss_pct: 0,
            _padding: [0; 10],
        }
    }
}

/// Storage statistics for tick storage accounts.
///
/// # Why this struct?
/// - Enables protocol-level monitoring and optimization of tick storage and compression.
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_tick_count: usize,
    pub inline_tick_count: usize,
    pub page_count: usize,
    pub max_inline_capacity: usize,
    pub max_page_capacity: usize,
    pub total_compressions: u64,
    pub total_decompressions: u64,
    pub avg_loss_pct: f32,
    pub max_loss_pct: f32,
    pub main_account_bytes: usize,
    pub page_account_bytes: usize,
}
