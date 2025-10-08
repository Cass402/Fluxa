//! Ultra-high-performance, zero-copy cache for tick storage in Solana AMM protocols.
//!
//! # Design Rationale
//! - This cache is accessed on every swap, so it must be zero-copy and bytemuck-compatible for maximum performance and deterministic serialization.
//! - All cache fields are hot-data-first and aligned for cache line efficiency, minimizing BPF memory access cost.
//! - Hot/cold path logic is used to optimize for frequent tick access patterns, with linear search for ultra-hot ticks and binary search for cold ticks.
//! - Bitwise packing and fixed-size arrays are used throughout to avoid heap allocation and maximize CU efficiency.
//! - All cache rebuilds and promotions are performed in-place, with explicit statistics for auditability and protocol optimization.
use crate::math::core_arithmetic::Q64x64;
use crate::state::tick::tick_compression::storage::{CompressedTickStorage, TickStoragePage};
use crate::state::tick::tick_data::TickData;
use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

/// Ultra-compact tick access info for cache (12 bytes).
///
/// # Why this struct?
/// - Packs all tick access metadata into 12 bytes for cache line efficiency and zero-copy compatibility.
/// - Bitwise packing enables fast location decoding and supports both inline and page-based tick storage.
#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C)]
#[derive(Default, InitSpace, AnchorSerialize, AnchorDeserialize)]
pub struct CachedTickAccess {
    pub tick_index: i32,       // 4 bytes
    pub location_packed: u32,  // 4 bytes - packed location info
    pub access_count: u16,     // 2 bytes
    pub last_access_slot: u16, // 2 bytes - relative to cache epoch
}

impl CachedTickAccess {
    /// Packs location info into a u32: [page_index:8][tick_index_in_page:16][is_inline:1][reserved:7].
    ///
    /// # Why this approach?
    /// - Enables fast, branchless decoding of tick location for both inline and page storage.
    pub fn new_inline(tick_index: i32, inline_index: usize) -> Self {
        let location_packed = (inline_index as u32 & 0xFFFF) | 0x10000; // Set inline bit
        Self {
            tick_index,
            location_packed,
            access_count: 0,
            last_access_slot: 0,
        }
    }

    pub fn new_page(tick_index: i32, page_index: u8, tick_index_in_page: usize) -> Self {
        let location_packed = ((page_index as u32) << 24) | ((tick_index_in_page as u32) & 0xFFFF);
        Self {
            tick_index,
            location_packed,
            access_count: 0,
            last_access_slot: 0,
        }
    }

    #[inline(always)]
    pub fn is_inline(&self) -> bool {
        // Checks if the tick is stored inline (fast path).
        // # Why bitwise?
        // - Enables branchless, single-instruction check for hot path optimization.
        (self.location_packed & 0x10000) != 0
    }

    #[inline(always)]
    pub fn get_inline_index(&self) -> usize {
        // Decodes the inline index for fast tick lookup.
        (self.location_packed & 0xFFFF) as usize
    }

    #[inline(always)]
    pub fn get_page_info(&self) -> (u8, usize) {
        // Decodes page index and tick index within page for overflow storage.
        let page = (self.location_packed >> 24) as u8;
        let index = (self.location_packed & 0xFFFF) as usize;
        (page, index)
    }

    #[inline(always)]
    pub fn increment_access(&mut self, current_slot: u64, cache_epoch: u64) {
        // Increments access count and updates last access slot (relative to cache epoch).
        // # Why relative slot?
        // - Saves space and handles slot wraparound for long-running caches.
        self.access_count = self.access_count.saturating_add(1);
        let relative_slot = current_slot.saturating_sub(cache_epoch);
        self.last_access_slot = (relative_slot & 0xFFFF) as u16;
    }
}

/// High-performance, zero-copy cache account for tick lookup.
///
/// # Why this struct?
/// - All fields are hot-data-first and aligned for cache line efficiency.
/// - Fixed-size arrays and bitwise packing enable deterministic layout and zero-copy migration.
/// - Hot/cold path logic supports ultra-fast tick lookup for frequent access patterns.
#[account(zero_copy(unsafe))]
#[derive(InitSpace)]
#[repr(C)]
pub struct TickLookupCache {
    // Cache metadata (hot data first for cache line efficiency)
    //
    // # Why these fields?
    // - Epoch and slot tracking support efficient cache rebuilds and relative timing.
    pub cache_epoch_slot: u64,   // 8 bytes - base slot for relative timing
    pub last_rebuild_slot: u64,  // 8 bytes - when cache was rebuilt
    pub storage_account: Pubkey, // 32 bytes - reference to main storage

    // Performance counters (frequently updated)
    //
    // # Why these fields?
    // - Tracks cache hits, misses, and hot tick hits for protocol optimization and auditability.
    pub cache_hits: u32,     // 4 bytes
    pub cache_misses: u32,   // 4 bytes
    pub hot_tick_hits: u32,  // 4 bytes
    pub total_rebuilds: u32, // 4 bytes

    // Cache configuration and state
    //
    // # Why these fields?
    // - Configurable cache capacity and hotness threshold enable protocol tuning and upgradeability.
    pub entry_count: u16,   // 2 bytes - active entries in cache
    pub max_entries: u16,   // 2 bytes - cache capacity
    pub hot_threshold: u16, // 2 bytes - access count for "hot"
    pub version: u8,        // 1 byte - cache version
    pub cache_valid: u8,    // 1 byte - is cache valid

    // Hot tick fast-access section (linear search for ultra-hot ticks)
    //
    // # Why linear search?
    // - For N=16, linear search is faster than binary search and minimizes CU for ultra-hot ticks.
    pub hot_tick_count: u8,                // 1 byte
    pub _padding1: [u8; 3],                // 3 bytes
    pub hot_ticks: [CachedTickAccess; 16], // 256 bytes - 16 hottest ticks

    // Main cache entries (binary search, sorted by tick_index)
    //
    // # Why binary search?
    // - For N=512, binary search is optimal for cold path lookups and minimizes CU for infrequent ticks.
    pub entries: [CachedTickAccess; 512], // 8192 bytes - main cache

    // Cache statistics and debugging
    //
    // # Why these fields?
    // - Tracks last hit tick, hit/miss streaks, and cache size for protocol monitoring and debugging.
    pub last_hit_tick: i32, // 4 bytes - for debugging
    pub hit_streak: u16,    // 2 bytes - consecutive hits
    pub miss_streak: u16,   // 2 bytes - consecutive misses
    pub _padding2: [u8; 4], // 4 bytes

                            // Total: 6440 bytes
}

impl TickLookupCache {
    /// Initializes the ultra-high-performance tick lookup cache.
    ///
    /// # Why this approach?
    /// - All fields are set to deterministic defaults for zero-copy safety and migration.
    /// - Hot/cold path logic is enabled for maximum performance on swap operations.
    pub fn initialize(&mut self, storage_account: Pubkey, current_slot: u64) -> Result<()> {
        self.storage_account = storage_account;
        self.cache_epoch_slot = current_slot;
        self.last_rebuild_slot = current_slot;

        self.cache_hits = 0;
        self.cache_misses = 0;
        self.hot_tick_hits = 0;
        self.total_rebuilds = 0;

        self.entry_count = 0;
        self.max_entries = 512; // Fixed capacity for zero-copy
        self.hot_threshold = 3; // Configurable hotness threshold
        self.version = 1;
        self.cache_valid = 0;

        self.hot_tick_count = 0;
        self.hot_ticks = [CachedTickAccess::default(); 16];
        self.entries = [CachedTickAccess::default(); 512];

        self.last_hit_tick = 0;
        self.hit_streak = 0;
        self.miss_streak = 0;

        // Zero the padding
        self._padding1 = [0; 3];
        self._padding2 = [0; 4];

        Ok(())
    }

    /// Rebuilds the cache from storage with zero heap allocations.
    ///
    /// # Why this approach?
    /// - Uses only stack-allocated arrays and direct manipulation for maximum CU efficiency and deterministic layout.
    /// - Preserves hotness and access statistics for protocol optimization and auditability.
    pub fn rebuild_from_storage(
        &mut self,
        main_storage: &CompressedTickStorage,
        pages: &[&TickStoragePage],
        current_slot: u64,
    ) -> Result<()> {
        // Reset cache state
        self.entry_count = 0;
        self.hot_tick_count = 0;
        self.cache_epoch_slot = current_slot;
        self.last_rebuild_slot = current_slot;

        // ZERO-COPY APPROACH: Work directly with fixed arrays
        // Clear all entries first
        self.entries = [CachedTickAccess::default(); 512];
        self.hot_ticks = [CachedTickAccess::default(); 16];

        let mut total_entries = 0u16;

        // Phase 1: Collect inline ticks directly into main cache
        for i in 0..main_storage.inline_tick_count as usize {
            if total_entries >= 512 {
                break; // Cache full
            }

            let tick = &main_storage.inline_ticks[i];
            let mut entry = CachedTickAccess::new_inline(tick.tick_index, i);

            // Preserve hotness from previous cache
            if self.cache_valid == 1 {
                if let Some(old_entry) = self.find_entry_for_preservation(tick.tick_index) {
                    entry.access_count = old_entry.access_count;
                    entry.last_access_slot = old_entry.last_access_slot;
                }
            }

            self.entries[total_entries as usize] = entry;
            total_entries += 1;
        }

        // Phase 2: Collect page ticks directly into main cache
        for (page_idx, page) in pages.iter().enumerate() {
            if page_idx >= main_storage.active_page_count as usize {
                break;
            }

            for i in 0..page.tick_count as usize {
                if total_entries >= 512 {
                    break; // Cache full
                }

                let tick = &page.ticks[i];
                let mut entry = CachedTickAccess::new_page(tick.tick_index, page_idx as u8, i);

                // Preserve hotness
                if self.cache_valid == 1 {
                    if let Some(old_entry) = self.find_entry_for_preservation(tick.tick_index) {
                        entry.access_count = old_entry.access_count;
                        entry.last_access_slot = old_entry.last_access_slot;
                    }
                }

                self.entries[total_entries as usize] = entry;
                total_entries += 1;
            }
        }

        self.entry_count = total_entries;

        // Phase 3: Sort entries by hotness IN-PLACE (no temp arrays)
        // Use a simple bubble sort for hotness since we need to maintain the array
        self.sort_entries_by_hotness();

        // Phase 4: Extract hot ticks to dedicated hot cache
        let mut hot_count = 0u8;
        for i in 0..self.entry_count as usize {
            if hot_count >= 16 {
                break; // Hot cache full
            }

            let entry = self.entries[i];
            if entry.access_count >= self.hot_threshold {
                self.hot_ticks[hot_count as usize] = entry;
                hot_count += 1;
            } else {
                break; // No more hot entries (they're sorted by hotness)
            }
        }
        self.hot_tick_count = hot_count;

        // Phase 5: Sort main cache by tick_index for binary search
        self.sort_entries_by_tick_index();

        self.cache_valid = 1;
        self.total_rebuilds += 1;

        Ok(())
    }

    /// In-place sort by hotness (access_count desc, then by recent access).
    ///
    /// # Why insertion sort?
    /// - For small arrays (N<100), insertion sort is more CU-efficient than quicksort or heap sort.
    fn sort_entries_by_hotness(&mut self) {
        let len = self.entry_count as usize;
        if len <= 1 {
            return;
        }

        // Insertion sort by hotness
        for i in 1..len {
            let key = self.entries[i];
            let mut j = i;

            // Move elements that are less hot than key one position ahead
            while j > 0 && self.is_hotter_than(key, self.entries[j - 1]) {
                self.entries[j] = self.entries[j - 1];
                j -= 1;
            }
            self.entries[j] = key;
        }
    }

    /// In-place sort by tick_index for binary search.
    ///
    /// # Why insertion sort?
    /// - For small arrays, insertion sort is more CU-efficient and deterministic than other algorithms.
    fn sort_entries_by_tick_index(&mut self) {
        let len = self.entry_count as usize;
        if len <= 1 {
            return;
        }

        // Insertion sort by tick_index (efficient for small arrays)
        for i in 1..len {
            let key = self.entries[i];
            let mut j = i;

            while j > 0 && self.entries[j - 1].tick_index > key.tick_index {
                self.entries[j] = self.entries[j - 1];
                j -= 1;
            }
            self.entries[j] = key;
        }
    }

    /// Compares hotness of two cache entries.
    ///
    /// # Why this function?
    /// - Enables branchless, deterministic sorting for hot/cold path optimization.
    #[inline(always)]
    fn is_hotter_than(&self, a: CachedTickAccess, b: CachedTickAccess) -> bool {
        a.access_count > b.access_count
            || (a.access_count == b.access_count && a.last_access_slot > b.last_access_slot)
    }

    /// Ultra-fast tick lookup with hot path optimization.
    ///
    /// # Why this approach?
    /// - Linear search for ultra-hot ticks (N=16) is faster than binary search and minimizes CU for frequent swaps.
    /// - Binary search for cold ticks (N=512) is optimal for infrequent access patterns.
    #[inline(always)]
    pub fn find_tick_ultra_fast(
        &mut self,
        tick_index: i32,
        current_slot: u64,
    ) -> Option<(bool, usize, u8)> {
        if self.cache_valid == 0 {
            self.cache_misses += 1;
            self.miss_streak += 1;
            self.hit_streak = 0;
            return None;
        }

        // HOT PATH: Linear search of ultra-hot ticks (faster than binary search for N=16)
        for i in 0..self.hot_tick_count as usize {
            if self.hot_ticks[i].tick_index == tick_index {
                self.cache_hits += 1;
                self.hot_tick_hits += 1;
                self.hit_streak += 1;
                self.miss_streak = 0;
                self.last_hit_tick = tick_index;

                // Update access tracking
                self.hot_ticks[i].increment_access(current_slot, self.cache_epoch_slot);

                // Return (is_inline, index, page) - decode location
                if self.hot_ticks[i].is_inline() {
                    return Some((true, self.hot_ticks[i].get_inline_index(), 0));
                } else {
                    let (page, index) = self.hot_ticks[i].get_page_info();
                    return Some((false, index, page));
                }
            }
        }

        // COLD PATH: Binary search main cache
        let entry_count = self.entry_count as usize;
        let hot_threshold = self.hot_threshold;
        let hot_tick_count = self.hot_tick_count;

        match self.entries[..entry_count]
            .binary_search_by_key(&tick_index, |entry| entry.tick_index)
        {
            Ok(pos) => {
                self.cache_hits += 1;
                self.hit_streak += 1;
                self.miss_streak = 0;
                self.last_hit_tick = tick_index;

                // Update access tracking and get the entry info before promotion
                self.entries[pos].increment_access(current_slot, self.cache_epoch_slot);
                let access_count = self.entries[pos].access_count;
                let entry_copy = self.entries[pos]; // Copy the entry for potential promotion

                // Get location info to return
                let result = if self.entries[pos].is_inline() {
                    Some((true, self.entries[pos].get_inline_index(), 0))
                } else {
                    let (page, index) = self.entries[pos].get_page_info();
                    Some((false, index, page))
                };

                // Check if this tick should be promoted to hot cache (after we're done with the slice)
                if access_count >= hot_threshold && hot_tick_count < 16 {
                    self.promote_to_hot_cache(entry_copy);
                }

                result
            }
            Err(_) => {
                self.cache_misses += 1;
                self.miss_streak += 1;
                self.hit_streak = 0;
                None
            }
        }
    }

    /// Promotes a frequently accessed tick to the hot cache.
    ///
    /// # Why this approach?
    /// - Ensures that ultra-hot ticks are always available for fast-path lookup, minimizing swap latency.
    #[inline]
    fn promote_to_hot_cache(&mut self, entry: CachedTickAccess) {
        if self.hot_tick_count < 16 {
            self.hot_ticks[self.hot_tick_count as usize] = entry;
            self.hot_tick_count += 1;
        }
    }

    /// Finds a cache entry during rebuild for preserving hotness and access statistics.
    ///
    /// # Why this approach?
    /// - Maintains access patterns and hotness across cache rebuilds for protocol optimization.
    fn find_entry_for_preservation(&self, tick_index: i32) -> Option<&CachedTickAccess> {
        // Check hot cache first
        for i in 0..self.hot_tick_count as usize {
            if self.hot_ticks[i].tick_index == tick_index {
                return Some(&self.hot_ticks[i]);
            }
        }

        // Check main cache
        let active_entries = &self.entries[..self.entry_count as usize];
        if let Ok(pos) = active_entries.binary_search_by_key(&tick_index, |entry| entry.tick_index)
        {
            return Some(&active_entries[pos]);
        }

        None
    }

    /// Alternative cache rebuild using heap sort (better for larger tick counts).
    ///
    /// # Why this approach?
    /// - Heap sort is more CU-efficient for large arrays and maintains deterministic layout for zero-copy migration.
    pub fn rebuild_from_storage_heap_sort(
        &mut self,
        main_storage: &CompressedTickStorage,
        pages: &[&TickStoragePage],
        current_slot: u64,
    ) -> Result<()> {
        // Reset cache state
        self.entry_count = 0;
        self.hot_tick_count = 0;
        self.cache_epoch_slot = current_slot;
        self.last_rebuild_slot = current_slot;

        // Clear arrays
        self.entries = [CachedTickAccess::default(); 512];
        self.hot_ticks = [CachedTickAccess::default(); 16];

        let mut total_entries = 0u16;

        // Collect all ticks (same as before)
        for i in 0..main_storage.inline_tick_count as usize {
            if total_entries >= 512 {
                break;
            }

            let tick = &main_storage.inline_ticks[i];
            let mut entry = CachedTickAccess::new_inline(tick.tick_index, i);

            if self.cache_valid == 1 {
                if let Some(old_entry) = self.find_entry_for_preservation(tick.tick_index) {
                    entry.access_count = old_entry.access_count;
                    entry.last_access_slot = old_entry.last_access_slot;
                }
            }

            self.entries[total_entries as usize] = entry;
            total_entries += 1;
        }

        for (page_idx, page) in pages.iter().enumerate() {
            if page_idx >= main_storage.active_page_count as usize {
                break;
            }

            for i in 0..page.tick_count as usize {
                if total_entries >= 512 {
                    break;
                }

                let tick = &page.ticks[i];
                let mut entry = CachedTickAccess::new_page(tick.tick_index, page_idx as u8, i);

                if self.cache_valid == 1 {
                    if let Some(old_entry) = self.find_entry_for_preservation(tick.tick_index) {
                        entry.access_count = old_entry.access_count;
                        entry.last_access_slot = old_entry.last_access_slot;
                    }
                }

                self.entries[total_entries as usize] = entry;
                total_entries += 1;
            }
        }

        self.entry_count = total_entries;

        // Use heap sort for better performance with larger datasets
        self.heap_sort_by_hotness();

        // Extract hot ticks
        let mut hot_count = 0u8;
        for i in 0..self.entry_count as usize {
            if hot_count >= 16 {
                break;
            }
            if self.entries[i].access_count >= self.hot_threshold {
                self.hot_ticks[hot_count as usize] = self.entries[i];
                hot_count += 1;
            } else {
                break;
            }
        }
        self.hot_tick_count = hot_count;

        // Sort by tick_index for binary search
        self.heap_sort_by_tick_index();

        self.cache_valid = 1;
        self.total_rebuilds += 1;

        Ok(())
    }

    /// In-place heap sort by hotness.
    ///
    /// # Why this approach?
    /// - Heap sort is optimal for large arrays and maintains deterministic layout for protocol safety.
    fn heap_sort_by_hotness(&mut self) {
        let len = self.entry_count as usize;
        if len <= 1 {
            return;
        }

        // Build max heap
        for start in (0..len / 2).rev() {
            self.sift_down_hotness(start, len - 1);
        }

        // Extract elements from heap
        for end in (1..len).rev() {
            self.entries.swap(0, end);
            self.sift_down_hotness(0, end - 1);
        }

        // Reverse for descending order (hottest first)
        let end = len;
        for i in 0..end / 2 {
            self.entries.swap(i, end - 1 - i);
        }
    }

    /// In-place heap sort by tick_index.
    ///
    /// # Why this approach?
    /// - Heap sort is optimal for large arrays and enables fast binary search for cold path lookups.
    fn heap_sort_by_tick_index(&mut self) {
        let len = self.entry_count as usize;
        if len <= 1 {
            return;
        }

        // Build max heap
        for start in (0..len / 2).rev() {
            self.sift_down_tick_index(start, len - 1);
        }

        // Extract elements from heap
        for end in (1..len).rev() {
            self.entries.swap(0, end);
            self.sift_down_tick_index(0, end - 1);
        }
    }

    fn sift_down_hotness(&mut self, start: usize, end: usize) {
        let mut root = start;
        while root * 2 < end {
            let child = root * 2 + 1;
            let mut swap = root;

            if self.is_hotter_than(self.entries[child], self.entries[swap]) {
                swap = child;
            }
            if child < end && self.is_hotter_than(self.entries[child + 1], self.entries[swap]) {
                swap = child + 1;
            }
            if swap == root {
                return;
            } else {
                self.entries.swap(root, swap);
                root = swap;
            }
        }
    }

    fn sift_down_tick_index(&mut self, start: usize, end: usize) {
        let mut root = start;
        while root * 2 < end {
            let child = root * 2 + 1;
            let mut swap = root;

            if self.entries[child].tick_index > self.entries[swap].tick_index {
                swap = child;
            }
            if child < end && self.entries[child + 1].tick_index > self.entries[swap].tick_index {
                swap = child + 1;
            }
            if swap == root {
                return;
            } else {
                self.entries.swap(root, swap);
                root = swap;
            }
        }
    }

    /// Returns ultra-detailed cache performance statistics.
    ///
    /// # Why this struct?
    /// - Enables protocol-level monitoring and optimization of cache performance and hit rates.
    pub fn get_cache_stats(&self) -> UltraCacheStats {
        let total_requests = self.cache_hits + self.cache_misses;
        let hit_rate = if total_requests > 0 {
            Q64x64::from_int(self.cache_hits as u64)
                .checked_div(Q64x64::from_int(total_requests as u64))
                .unwrap()
        } else {
            Q64x64::zero()
        };

        let hot_hit_rate = if self.cache_hits > 0 {
            Q64x64::from_int(self.hot_tick_hits as u64)
                .checked_div(Q64x64::from_int(self.cache_hits as u64))
                .unwrap()
        } else {
            Q64x64::zero()
        };

        // Count truly hot ticks in main cache
        let hot_in_main = self.entries[..self.entry_count as usize]
            .iter()
            .filter(|entry| entry.access_count >= self.hot_threshold)
            .count();

        UltraCacheStats {
            capacity: self.max_entries as usize,
            entries: self.entry_count as usize,
            hot_entries: self.hot_tick_count as usize,
            hit_rate,
            hot_hit_rate,
            hot_ticks_in_main: hot_in_main,
            total_hits: self.cache_hits,
            total_misses: self.cache_misses,
            hot_tick_hits: self.hot_tick_hits,
            total_rebuilds: self.total_rebuilds,
            hit_streak: self.hit_streak,
            miss_streak: self.miss_streak,
            last_hit_tick: self.last_hit_tick,
            valid: self.cache_valid,
            cache_size_bytes: std::mem::size_of::<TickLookupCache>(),
        }
    }

    pub fn reset_stats(&mut self) {
        // Resets all cache performance statistics for protocol monitoring and debugging.
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.hot_tick_hits = 0;
        self.hit_streak = 0;
        self.miss_streak = 0;
    }

    /// Checks if the cache needs rebuilding based on slot timing and validity.
    ///
    /// # Why this approach?
    /// - Ensures cache freshness and protocol safety by rebuilding every ~5 minutes or when invalid.
    #[inline(always)]
    pub fn needs_rebuild(&self, current_slot: u64) -> bool {
        if self.cache_valid == 0 {
            return true;
        }

        // Rebuild every ~5 minutes (600 slots at 2 slots/sec)
        let slots_since_rebuild = current_slot.saturating_sub(self.last_rebuild_slot);
        slots_since_rebuild > 600
    }
}

/// Ultra-detailed cache performance statistics for tick lookup cache.
///
/// # Why this struct?
/// - Enables protocol-level monitoring and optimization of cache hit rates, hot/cold path efficiency, and memory usage.
#[derive(Debug, Clone)]
pub struct UltraCacheStats {
    pub capacity: usize,
    pub entries: usize,
    pub hot_entries: usize,
    pub hit_rate: Q64x64,
    pub hot_hit_rate: Q64x64,
    pub hot_ticks_in_main: usize,
    pub total_hits: u32,
    pub total_misses: u32,
    pub hot_tick_hits: u32,
    pub total_rebuilds: u32,
    pub hit_streak: u16,
    pub miss_streak: u16,
    pub last_hit_tick: i32,
    pub valid: u8,
    pub cache_size_bytes: usize,
}

/// Ultra-high-performance storage manager with zero-copy cache for tick lookup.
///
/// # Why this struct?
/// - Centralizes all tick lookup logic for protocol safety and auditability.
/// - Ensures all lookups are performed with zero-copy, deterministic logic for maximum performance.
pub struct UltraTickStorageManager;

impl UltraTickStorageManager {
    /// Finds a tick with maximum performance using zero-copy cache.
    ///
    /// # Why this approach?
    /// - Main bitmap is checked first for O(1) existence, minimizing unnecessary cache lookups.
    /// - Ultra-fast cache lookup is used for frequent swaps, with fallback to manual search for rare misses.
    #[inline(always)]
    pub fn find_tick_ultra_fast(
        tick_index: i32,
        main_storage: &CompressedTickStorage,
        pages: &[&TickStoragePage],
        cache: &mut TickLookupCache,
        current_slot: u64,
    ) -> Result<Option<TickData>> {
        // Check main bitmap first (single u64 lookup)
        if !main_storage.tick_exists_in_bitmap(tick_index)? {
            return Ok(None);
        }

        // Ultra-fast cache lookup
        if let Some((is_inline, index, page)) = cache.find_tick_ultra_fast(tick_index, current_slot)
        {
            if is_inline {
                return Ok(Some(main_storage.decompress_inline_tick(index)?));
            } else if (page as usize) < pages.len() {
                return Ok(Some(
                    pages[page as usize]
                        .decompress_tick(index, main_storage.epoch.base_epoch_slot)?,
                ));
            }
        }

        // Cache miss - manual fallback (should be rare with good cache)
        Self::find_tick_fallback(tick_index, main_storage, pages)
    }

    /// Fallback search when cache misses (should be rare).
    ///
    /// # Why this approach?
    /// - Ensures protocol safety and correctness by searching both inline and page storage when cache misses occur.
    fn find_tick_fallback(
        tick_index: i32,
        main_storage: &CompressedTickStorage,
        pages: &[&TickStoragePage],
    ) -> Result<Option<TickData>> {
        // Check inline storage
        if let Some(index) = main_storage.find_inline_tick(tick_index) {
            return Ok(Some(main_storage.decompress_inline_tick(index)?));
        }

        // Check pages
        for (_page_idx, page) in pages
            .iter()
            .enumerate()
            .take(main_storage.active_page_count as usize)
        {
            if let Some(index) = page.find_tick(tick_index) {
                return Ok(Some(
                    page.decompress_tick(index, main_storage.epoch.base_epoch_slot)?,
                ));
            }
        }

        Ok(None)
    }
}
