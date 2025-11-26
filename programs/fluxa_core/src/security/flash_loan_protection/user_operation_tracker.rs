use crate::math::core_arithmetic::Q64x64;
use crate::security::flash_loan_protection::compact_operation_slot_bucket::CompactOperation;
use crate::security::flash_loan_protection::decay_accumulator_dual_bitset::RiskDecayAccumulator;
use crate::security::flash_loan_protection::operation_ring_buffer::OperationRingBuffer;
use crate::utils::constants::{
    EWMA_ALPHA_Q64, MIN_VOLUME_FLOR_Q64, PRECISION_FACTOR_Q64, SLOTS_PER_MINUTE,
};
use anchor_lang::prelude::*;
use bytemuck::Zeroable;

const USER_SHORT_WINDOW_SLOTS: u64 = SLOTS_PER_MINUTE * 2;
const SLOTS_PER_HOUR: u64 = SLOTS_PER_MINUTE * 60;
const SLOTS_PER_DAY: u64 = SLOTS_PER_HOUR * 24;
const TEMPORAL_STACK_WINDOW: u64 = 4;
const TEMPORAL_RELAX_WINDOW: u64 = 20;

#[account(zero_copy)]
#[derive(InitSpace)]
#[repr(C)]
pub struct UserOperationTracker {
    pub user: Pubkey,
    pub operations: OperationRingBuffer,
    pub risk_decay: RiskDecayAccumulator,
    pub total_volume_24h: Q64x64,
    pub average_operation_size: Q64x64,
    pub volume_variance: Q64x64,
    pub max_single_operation: Q64x64,
    pub last_activity_slot: u64,
    pub last_unix_timestamp: i64,
    pub detection_count: u32,
    pub temporal_risk_accumulator: u16,
    pub risk_score: u8,
    pub consecutive_suspicious: u8,
    pub benign_activity_score: u8,
    pub volume_deviation_score: u8,
    pub reserved: [u8; 70],
}

#[derive(Clone, Copy, Default)]
pub struct UserRiskSignals {
    pub short_window_volume: Q64x64,
    pub day_window_volume: Q64x64,
    pub temporal_burst_score: u8,
    pub recent_operation_count: u8,
    pub pattern_fingerprint: u16,
    pub large_amount: bool,
    pub high_impact: bool,
    pub volume_deviation_score: u8,
    pub risk_score: u8,
}

impl UserOperationTracker {
    #[inline(always)]
    pub fn record_operation(
        &mut self,
        operation: CompactOperation,
        current_slot: u64,
        unix_timestamp: i64,
    ) -> Result<UserRiskSignals> {
        let previous_slot = self.last_activity_slot;
        self.operations.push(operation)?;

        self.last_activity_slot = current_slot;
        self.last_unix_timestamp = unix_timestamp;

        if operation.amount_q64 > self.max_single_operation {
            self.max_single_operation = operation.amount_q64;
        }

        self.total_volume_24h = self
            .operations
            .get_volume_in_window(SLOTS_PER_DAY, current_slot)?;

        let short_window_volume = self
            .operations
            .get_volume_in_window(USER_SHORT_WINDOW_SLOTS, current_slot)?;
        let day_volume = self.total_volume_24h;

        let alpha = EWMA_ALPHA_Q64;
        let one_minus_alpha = Q64x64::one().checked_sub(alpha)?;

        if self.average_operation_size.raw() == 0 {
            self.average_operation_size = operation.amount_q64;
        } else {
            let decayed = self.average_operation_size.checked_mul(one_minus_alpha)?;
            let weighted_new = operation.amount_q64.checked_mul(alpha)?;
            self.average_operation_size = decayed.checked_add(weighted_new)?;
        }

        let deviation = q64_abs_diff(operation.amount_q64, self.average_operation_size)?;
        let deviation_squared = deviation.checked_mul(deviation)?;
        let variance_decay = self.volume_variance.checked_mul(one_minus_alpha)?;
        let variance_increment = deviation_squared.checked_mul(alpha)?;
        self.volume_variance = variance_decay.checked_add(variance_increment)?;

        let denom = self
            .average_operation_size
            .checked_add(MIN_VOLUME_FLOR_Q64)?;
        let deviation_ratio = deviation.checked_div(denom)?;
        let scaled_ratio = deviation_ratio.checked_mul(PRECISION_FACTOR_Q64)?;
        let deviation_points = ((scaled_ratio.raw() >> 64) as u8).min(100);
        self.volume_deviation_score = deviation_points;

        let large_amount = operation.is_large_amount()?;
        let high_impact = operation.is_high_impact()?;

        if previous_slot > 0 {
            let gap = current_slot.saturating_sub(previous_slot);
            if gap <= TEMPORAL_STACK_WINDOW {
                let penalty = (TEMPORAL_STACK_WINDOW - gap + 1) as u16;
                self.temporal_risk_accumulator =
                    self.temporal_risk_accumulator.saturating_add(penalty);
            } else if gap > TEMPORAL_RELAX_WINDOW {
                let relax = (gap / TEMPORAL_RELAX_WINDOW) as u16;
                self.temporal_risk_accumulator = self
                    .temporal_risk_accumulator
                    .saturating_sub(relax.saturating_add(1));
            } else {
                self.temporal_risk_accumulator = self.temporal_risk_accumulator.saturating_add(1);
            }
        }

        let temporal_burst_score = (self.temporal_risk_accumulator >> 1).min(100) as u8;
        let decay_risk = self
            .risk_decay
            .apply_smooth_decay(current_slot, self.risk_score)?;
        self.risk_score = decay_risk;

        let mut local_risk = decay_risk;
        if large_amount {
            local_risk = local_risk.saturating_add(12);
        }

        if high_impact {
            local_risk = local_risk.saturating_add(16);
        }

        local_risk = local_risk
            .saturating_add((temporal_burst_score >> 1).min(20))
            .saturating_add(deviation_points >> 1);
        self.risk_score = local_risk.min(100);

        let suspicious = large_amount && high_impact && temporal_burst_score > 20;
        if suspicious {
            self.consecutive_suspicious = self.consecutive_suspicious.saturating_add(1);
            self.benign_activity_score = self.benign_activity_score.saturating_sub(1);
            self.detection_count = self.detection_count.saturating_add(1);
        } else {
            self.consecutive_suspicious = 0;
            self.benign_activity_score = (self.benign_activity_score.saturating_add(2)).min(100);
        }

        let pattern_fingerprint = self.operations.get_pattern_fingerprint();
        let recent_operation_count = self.operations.len();

        Ok(UserRiskSignals {
            short_window_volume,
            day_window_volume: day_volume,
            temporal_burst_score,
            recent_operation_count,
            pattern_fingerprint,
            large_amount,
            high_impact,
            volume_deviation_score: deviation_points,
            risk_score: self.risk_score,
        })
    }
}

impl Default for UserOperationTracker {
    fn default() -> Self {
        Self {
            user: Pubkey::default(),
            operations: OperationRingBuffer::default(),
            risk_decay: RiskDecayAccumulator::zeroed(),
            total_volume_24h: Q64x64::zero(),
            average_operation_size: Q64x64::zero(),
            volume_variance: Q64x64::zero(),
            max_single_operation: Q64x64::zero(),
            last_activity_slot: 0,
            last_unix_timestamp: 0,
            detection_count: 0,
            temporal_risk_accumulator: 0,
            risk_score: 0,
            consecutive_suspicious: 0,
            benign_activity_score: 0,
            volume_deviation_score: 0,
            reserved: [0u8; 70],
        }
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
