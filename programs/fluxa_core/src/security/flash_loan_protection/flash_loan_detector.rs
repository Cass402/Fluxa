use crate::math::core_arithmetic::Q64x64;
use crate::security::flash_loan_protection::compact_operation_slot_bucket::{
    CompactOperation, SlotBucketCounters,
};
use crate::security::flash_loan_protection::decay_accumulator_dual_bitset::DualBitsetAging;
use crate::security::flash_loan_protection::user_operation_tracker::UserOperationTracker;
use crate::utils::constants::{
    EWMA_ALPHA_Q64, GLOBAL_BUFFER_SIZE, MAX_DISTINCT_USERS, MIN_VOLUME_FLOR_Q64,
    PRECISION_FACTOR_Q64, SLOTS_PER_MINUTE, SLOT_BUCKET_COUNT,
};
use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

const COORDINATION_WINDOW_SLOTS: u64 = SLOTS_PER_MINUTE * 2;
const BLOCK_BASELINE: u8 = 80;
const CACHE_HIT_FLAG: u16 = 0x40;
const COORDINATION_FLAG: u16 = 0x01;
const TEMPORAL_FLAG: u16 = 0x02;
const PATTERN_FLAG: u16 = 0x04;
const GLOBAL_VOLUME_FLAG: u16 = 0x08;
const USER_VOLUME_FLAG: u16 = 0x10;
const PRECISION_FLAG: u16 = 0x20;

#[derive(Clone, Copy, Pod, Zeroable, InitSpace, AnchorSerialize, AnchorDeserialize)]
#[repr(C)]
pub struct UserCoordinationTracker {
    pub recent_users: [Pubkey; MAX_DISTINCT_USERS],
    pub user_slots: [u64; MAX_DISTINCT_USERS],
    pub user_volumes: [Q64x64; MAX_DISTINCT_USERS],
    pub total_coordination_volume: Q64x64,
    pub window_start_slot: u64,
    pub user_count: u8,
    pub coordination_score: u8,
    pub _padding: [u8; 6],
}

#[derive(Clone, Copy, Pod, Zeroable, InitSpace, AnchorSerialize, AnchorDeserialize)]
#[repr(C)]
pub struct DetectionStats {
    pub last_performance_update: u64,
    pub total_detections: u32,
    pub avg_detection_cu: u32,
    pub cache_hits: u32,
    pub cache_misses: u32,
    pub true_positives: u16,
    pub false_positives: u16,
    pub coordination_detections: u16,
    pub temporal_violations: u16,
    pub overflow_protections: u16,
    pub pattern_detections: u16,
    pub volume_anomalies: u16,
    pub precision_errors_prevented: u16,
    pub reserved: [u8; 64],
}

#[account(zero_copy)]
#[derive(InitSpace)]
#[repr(C)]
pub struct FlashLoanDetector {
    pub pool: Pubkey,
    pub detection_stats: DetectionStats,
    pub pattern_bitset: DualBitsetAging,
    pub global_operations: [CompactOperation; GLOBAL_BUFFER_SIZE],
    pub slot_counters: SlotBucketCounters,
    pub coordination_tracker: UserCoordinationTracker,
    pub total_volume_ewma: Q64x64,
    pub volume_deviation_threshold: Q64x64,
    pub suspicious_patterns: u32,
    pub enabled: u8,
    pub sensitivity: u8,
    pub global_head: u8,
    pub global_count: u8,
    pub reserved: [u8; 72],
}

#[derive(Clone, Copy, Default, AnchorSerialize, AnchorDeserialize)]
pub struct DetectionOutcome {
    pub blocked: bool,
    pub risk_score: u8,
    pub flags: u16,
    pub cache_hit: bool,
    pub coordination_score: u8,
}

impl UserCoordinationTracker {
    #[inline(always)]
    pub fn add_user(&mut self, user: Pubkey, current_slot: u64, volume: Q64x64) -> Result<()> {
        self.evict_stale_entries(current_slot)?;

        if let Some(idx) = self.find_user_index(user) {
            self.total_coordination_volume = self
                .total_coordination_volume
                .checked_sub(self.user_volumes[idx])?;
            self.user_volumes[idx] = self.user_volumes[idx].checked_add(volume)?;
            self.user_slots[idx] = current_slot;
            self.total_coordination_volume = self
                .total_coordination_volume
                .checked_add(self.user_volumes[idx])?;
        } else {
            let mut insert_index = self.user_count as usize;
            if insert_index >= MAX_DISTINCT_USERS {
                let drop_idx = self.find_oldest_index();
                self.remove_index(drop_idx)?;
                insert_index = self.user_count as usize;
            }

            if insert_index >= MAX_DISTINCT_USERS {
                insert_index = MAX_DISTINCT_USERS - 1;
            }

            self.recent_users[insert_index] = user;
            self.user_slots[insert_index] = current_slot;
            self.user_volumes[insert_index] = volume;
            self.user_count = self
                .user_count
                .saturating_add(1)
                .min(MAX_DISTINCT_USERS as u8);
            self.total_coordination_volume = self.total_coordination_volume.checked_add(volume)?;
        }

        self.window_start_slot = if self.window_start_slot == 0 {
            current_slot
        } else {
            self.window_start_slot.min(current_slot)
        };
        let user_factor = (self.user_count as u16) * 4;
        let base = MIN_VOLUME_FLOR_Q64.max(self.total_coordination_volume);
        let ratio = self.total_coordination_volume.checked_div(base)?;
        let scaled = ratio.checked_mul(PRECISION_FACTOR_Q64)?;
        let magnitude = (scaled.raw() >> 64).min(80) as u16;
        self.coordination_score = (user_factor + magnitude) as u8;

        Ok(())
    }

    #[inline(always)]
    fn find_user_index(&self, user: Pubkey) -> Option<usize> {
        (0..self.user_count as usize).find(|&i| self.recent_users[i] == user)
    }

    #[inline(always)]
    fn find_oldest_index(&self) -> usize {
        let mut oldest_slot = u64::MAX;
        let mut index = 0usize;
        for i in 0..self.user_count as usize {
            if self.user_slots[i] < oldest_slot {
                oldest_slot = self.user_slots[i];
                index = i;
            }
        }
        index
    }

    fn evict_stale_entries(&mut self, current_slot: u64) -> Result<()> {
        if self.user_count == 0 {
            self.window_start_slot = current_slot;
            return Ok(());
        }

        let min_slot = current_slot.saturating_sub(COORDINATION_WINDOW_SLOTS);
        let mut write_index = 0usize;
        self.total_coordination_volume = Q64x64::zero();

        for read_index in 0..self.user_count as usize {
            if self.user_slots[read_index] >= min_slot {
                if write_index != read_index {
                    self.recent_users[write_index] = self.recent_users[read_index];
                    self.user_slots[write_index] = self.user_slots[read_index];
                    self.user_volumes[write_index] = self.user_volumes[read_index];
                }
                self.total_coordination_volume = self
                    .total_coordination_volume
                    .checked_add(self.user_volumes[write_index])?;
                write_index += 1;
            }
        }

        for idx in write_index..self.user_count as usize {
            self.recent_users[idx] = Pubkey::default();
            self.user_slots[idx] = 0;
            self.user_volumes[idx] = Q64x64::zero();
        }

        self.user_count = write_index as u8;
        self.window_start_slot = current_slot;
        Ok(())
    }

    #[inline(always)]
    fn remove_index(&mut self, index: usize) -> Result<()> {
        if self.user_count == 0 || index >= self.user_count as usize {
            return Ok(());
        }

        let last_idx = (self.user_count - 1) as usize;
        self.total_coordination_volume = self
            .total_coordination_volume
            .checked_sub(self.user_volumes[index])?;

        if index != last_idx {
            self.recent_users[index] = self.recent_users[last_idx];
            self.user_slots[index] = self.user_slots[last_idx];
            self.user_volumes[index] = self.user_volumes[last_idx];
        }

        self.recent_users[last_idx] = Pubkey::default();
        self.user_slots[last_idx] = 0;
        self.user_volumes[last_idx] = Q64x64::zero();
        self.user_count -= 1;
        Ok(())
    }
}

impl Default for FlashLoanDetector {
    fn default() -> Self {
        Self {
            pool: Pubkey::default(),
            detection_stats: DetectionStats {
                last_performance_update: 0,
                total_detections: 0,
                avg_detection_cu: 0,
                cache_hits: 0,
                cache_misses: 0,
                true_positives: 0,
                false_positives: 0,
                coordination_detections: 0,
                temporal_violations: 0,
                overflow_protections: 0,
                pattern_detections: 0,
                volume_anomalies: 0,
                precision_errors_prevented: 0,
                reserved: [0u8; 64],
            },
            pattern_bitset: DualBitsetAging {
                bits_new: 0,
                bits_old: 0,
                last_aging_slot: 0,
            },
            global_operations: [CompactOperation::default(); GLOBAL_BUFFER_SIZE],
            slot_counters: SlotBucketCounters {
                counters: [0u32; SLOT_BUCKET_COUNT],
                volume_buckets: [Q64x64::zero(); SLOT_BUCKET_COUNT],
                current_bucket_slot: 0,
                _padding: [0u8; 8],
            },
            coordination_tracker: UserCoordinationTracker {
                recent_users: [Pubkey::default(); MAX_DISTINCT_USERS],
                user_slots: [0u64; MAX_DISTINCT_USERS],
                user_volumes: [Q64x64::zero(); MAX_DISTINCT_USERS],
                total_coordination_volume: Q64x64::zero(),
                window_start_slot: 0,
                user_count: 0,
                coordination_score: 0,
                _padding: [0u8; 6],
            },
            total_volume_ewma: MIN_VOLUME_FLOR_Q64,
            volume_deviation_threshold: MIN_VOLUME_FLOR_Q64,
            suspicious_patterns: 0,
            enabled: 1,
            sensitivity: 70,
            global_head: 0,
            global_count: 0,
            reserved: [0u8; 72],
        }
    }
}

impl FlashLoanDetector {
    #[inline(always)]
    pub fn observe_operation(
        &mut self,
        user_tracker: &mut UserOperationTracker,
        operation: CompactOperation,
        unix_timestamp: i64,
        current_slot: u64,
    ) -> Result<DetectionOutcome> {
        let signals = user_tracker.record_operation(operation, current_slot, unix_timestamp)?;

        if self.enabled == 0 {
            return Ok(DetectionOutcome {
                blocked: false,
                risk_score: signals.risk_score,
                flags: 0,
                cache_hit: false,
                coordination_score: 0,
            });
        }

        self.pattern_bitset.maybe_age(current_slot);
        self.slot_counters
            .increment(current_slot, operation.amount_q64)?;
        self.coordination_tracker
            .add_user(operation.user, current_slot, operation.amount_q64)?;

        let overflowed = self.record_global_operation(operation);
        if overflowed {
            self.detection_stats.overflow_protections =
                self.detection_stats.overflow_protections.saturating_add(1);
        }

        let (volume_alert, volume_delta_score) = self.update_volume_models(operation.amount_q64)?;
        let burst_score = self.slot_counters.get_burst_score(current_slot);
        let volume_burst_score = self.slot_counters.get_volume_burst_score(current_slot)?;
        let coordination_score = self.coordination_tracker.coordination_score;
        let pattern_hash = Self::fingerprint_hash(signals.pattern_fingerprint);
        let cache_hit = self.pattern_bitset.contains(pattern_hash);
        let user_pattern_cached = user_tracker
            .operations
            .check_pattern_cache(signals.pattern_fingerprint);

        let mut flags = 0u16;
        let mut risk = signals.risk_score;

        if signals.temporal_burst_score > 50 || burst_score > 70 {
            flags |= TEMPORAL_FLAG;
            self.detection_stats.temporal_violations =
                self.detection_stats.temporal_violations.saturating_add(1);
            risk = risk.saturating_add(10);
        }

        if coordination_score > 60 {
            flags |= COORDINATION_FLAG;
            self.detection_stats.coordination_detections = self
                .detection_stats
                .coordination_detections
                .saturating_add(1);
            risk = risk.saturating_add((coordination_score / 4).min(25));
        }

        if cache_hit || user_pattern_cached {
            flags |= PATTERN_FLAG;
            self.detection_stats.pattern_detections =
                self.detection_stats.pattern_detections.saturating_add(1);
            risk = risk.saturating_add(12);
        }

        if volume_alert || volume_burst_score > 60 {
            flags |= GLOBAL_VOLUME_FLAG;
            self.detection_stats.volume_anomalies =
                self.detection_stats.volume_anomalies.saturating_add(1);
            risk = risk.saturating_add(volume_delta_score);
        }

        if signals.volume_deviation_score > 40
            || signals.short_window_volume
                > signals.day_window_volume.checked_div(Q64x64::from_int(4))?
        {
            flags |= USER_VOLUME_FLAG;
            risk = risk.saturating_add(signals.volume_deviation_score / 2);
        }

        if operation.price_impact_q64 > Q64x64::one() {
            flags |= PRECISION_FLAG;
            self.detection_stats.precision_errors_prevented = self
                .detection_stats
                .precision_errors_prevented
                .saturating_add(1);
        }

        if cache_hit {
            flags |= CACHE_HIT_FLAG;
            self.detection_stats.cache_hits = self.detection_stats.cache_hits.saturating_add(1);
        } else {
            self.detection_stats.cache_misses = self.detection_stats.cache_misses.saturating_add(1);
        }

        let density = self.pattern_bitset.get_density();
        risk = risk.saturating_add((density / 4).min(10));

        let threshold = self.block_threshold();
        let blocked = risk >= threshold
            || (flags & (COORDINATION_FLAG | PATTERN_FLAG | GLOBAL_VOLUME_FLAG) != 0
                && risk + 5 >= threshold);

        if flags != 0 {
            self.detection_stats.total_detections =
                self.detection_stats.total_detections.saturating_add(1);
            if blocked {
                self.detection_stats.true_positives =
                    self.detection_stats.true_positives.saturating_add(1);
                self.pattern_bitset.insert(pattern_hash);
            } else {
                self.detection_stats.false_positives =
                    self.detection_stats.false_positives.saturating_add(1);
            }
        }

        self.suspicious_patterns = self.pattern_bitset.get_density() as u32;
        self.detection_stats.last_performance_update = current_slot;
        let estimated_cu = 1_500u32
            .saturating_add(flags.count_ones() * 90)
            .saturating_add(signals.recent_operation_count as u32 * 20);
        self.detection_stats.avg_detection_cu =
            ((self.detection_stats.avg_detection_cu as u64 * 7 + estimated_cu as u64) / 8) as u32;

        Ok(DetectionOutcome {
            blocked,
            risk_score: risk.min(100),
            flags,
            cache_hit,
            coordination_score,
        })
    }

    #[inline(always)]
    fn record_global_operation(&mut self, operation: CompactOperation) -> bool {
        let wrapped = self.global_count == GLOBAL_BUFFER_SIZE as u8;
        let index = (self.global_head as usize) & (GLOBAL_BUFFER_SIZE - 1);
        self.global_operations[index] = operation;
        self.global_head = (self.global_head + 1) & ((GLOBAL_BUFFER_SIZE - 1) as u8);
        if !wrapped {
            self.global_count = self.global_count.saturating_add(1);
        }
        wrapped
    }

    fn update_volume_models(&mut self, amount: Q64x64) -> Result<(bool, u8)> {
        let alpha = EWMA_ALPHA_Q64;
        let one_minus_alpha = Q64x64::one().checked_sub(alpha)?;
        let new_ewma = self
            .total_volume_ewma
            .checked_mul(one_minus_alpha)?
            .checked_add(amount.checked_mul(alpha)?)?;

        let diff = q64_abs_diff(amount, self.total_volume_ewma)?;
        let new_threshold = self
            .volume_deviation_threshold
            .checked_mul(one_minus_alpha)?
            .checked_add(diff.checked_mul(alpha)?)?
            .max(MIN_VOLUME_FLOR_Q64);

        self.total_volume_ewma = new_ewma.max(MIN_VOLUME_FLOR_Q64);
        self.volume_deviation_threshold = new_threshold;

        let alert = diff > self.volume_deviation_threshold;
        let ratio = if self.total_volume_ewma > Q64x64::zero() {
            diff.checked_div(self.total_volume_ewma)?
        } else {
            Q64x64::zero()
        };
        let scaled = ratio.checked_mul(PRECISION_FACTOR_Q64)?;
        let score = ((scaled.raw() >> 64) as u8).min(40);
        Ok((alert, score))
    }

    #[inline(always)]
    fn block_threshold(&self) -> u8 {
        let sensitivity = self.sensitivity.clamp(0, 100);
        BLOCK_BASELINE.saturating_sub(sensitivity / 5)
    }

    #[inline(always)]
    fn fingerprint_hash(fingerprint: u16) -> u8 {
        (fingerprint as u8) ^ ((fingerprint >> 8) as u8)
    }
}

#[inline(always)]
fn q64_abs_diff(a: Q64x64, b: Q64x64) -> Result<Q64x64> {
    if a >= b {
        a.checked_sub(b)
    } else {
        b.checked_sub(a)
    }
}
