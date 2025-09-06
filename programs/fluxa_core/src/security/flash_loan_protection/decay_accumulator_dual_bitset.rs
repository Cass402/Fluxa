use crate::math::core_arithmetic::Q64x64;
use crate::utils::constants::{BITSET_AGING_SLOTS, RISK_DECAY_RATE_Q64};
use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

#[derive(Clone, Copy, Pod, Zeroable, InitSpace)]
#[repr(C)]
pub struct RiskDecayAccumulator {
    pub accumulated_decay: Q64x64,
    pub last_decay_slot: u64,
    pub _padding: [u8; 8],
}

impl RiskDecayAccumulator {
    #[inline(always)]
    pub fn apply_smooth_decay(&mut self, current_slot: u64, current_risk: u8) -> Result<u8> {
        let slots_passed = current_slot.saturating_sub(self.last_decay_slot);

        if slots_passed == 0 {
            return Ok(current_risk);
        }

        let slots_q64 = Q64x64::from_int(slots_passed);
        let decay_increment = slots_q64.checked_mul(RISK_DECAY_RATE_Q64)?;

        self.accumulated_decay = self.accumulated_decay.checked_add(decay_increment)?;

        let decay_amount = if self.accumulated_decay >= Q64x64::one() {
            let integer_decay = (self.accumulated_decay.raw() >> 64) as u8;
            self.accumulated_decay =
                Q64x64::from_raw(self.accumulated_decay.raw() & ((1u128 << 64) - 1));
            integer_decay
        } else {
            0
        };

        self.last_decay_slot = current_slot;

        Ok(current_risk.saturating_sub(decay_amount))
    }
}

#[derive(Clone, Copy, Pod, Zeroable, InitSpace)]
#[repr(C)]
pub struct DualBitsetAging {
    pub bits_new: u64,
    pub bits_old: u64,
    pub last_aging_slot: u64,
}

impl DualBitsetAging {
    #[inline(always)]
    pub fn insert(&mut self, hash: u8) {
        let bit_pos = hash & 63;
        self.bits_new |= 1u64 << bit_pos;
    }

    #[inline(always)]
    pub fn contains(&self, hash: u8) -> bool {
        let bit_pos = hash & 63;
        let mask = 1u64 << bit_pos;
        (self.bits_new & mask) != 0 || (self.bits_old & mask) != 0
    }

    #[inline(always)]
    pub fn maybe_age(&mut self, current_slot: u64) {
        if current_slot > self.last_aging_slot + BITSET_AGING_SLOTS {
            self.bits_old = self.bits_new;
            self.bits_new = 0;
            self.last_aging_slot = current_slot;
        }
    }

    #[inline(always)]
    pub fn get_density(&self) -> u8 {
        (self.bits_new.count_ones() + self.bits_old.count_ones()) as u8
    }
}
