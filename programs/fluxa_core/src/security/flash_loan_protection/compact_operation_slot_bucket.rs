use crate::math::core_arithmetic::Q64x64;
use crate::utils::constants::{HIGH_IMPACT_THRESHOLD, LARGE_AMOUNT_THRESHOLD, SLOT_BUCKET_COUNT};
use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

#[derive(Clone, Copy, Pod, Zeroable, InitSpace, AnchorSerialize, AnchorDeserialize, Default)]
#[repr(C)]
pub struct CompactOperation {
    pub user: Pubkey,
    pub amount_q64: Q64x64,
    pub price_impact_q64: Q64x64,
    pub slot: u64,
    pub operation_type: u8,
    pub _padding: [u8; 7],
}

impl CompactOperation {
    #[inline(always)]
    pub fn new(
        user: Pubkey,
        amount_q64: Q64x64,
        price_impact_q64: Q64x64,
        slot: u64,
        operation_type: u8,
    ) -> Self {
        Self {
            user,
            amount_q64,
            price_impact_q64,
            slot,
            operation_type,
            _padding: [0u8; 7],
        }
    }

    #[inline(always)]
    pub fn user_key(&self) -> Pubkey {
        self.user
    }

    #[inline(always)]
    pub fn amount(&self) -> Q64x64 {
        self.amount_q64
    }

    #[inline(always)]
    pub fn price_impact(&self) -> Q64x64 {
        self.price_impact_q64
    }

    #[inline(always)]
    pub fn get_2bit_encoding(&self) -> u8 {
        self.operation_type & 0b11
    }

    #[inline(always)]
    pub fn get_6bit_encoding(&self) -> u8 {
        let impact_bits = ((self.price_impact_q64.raw() >> 60) & 0b1111) as u8;
        ((self.operation_type & 0b11) << 4) | impact_bits
    }

    #[inline(always)]
    pub fn is_large_amount(&self) -> Result<bool> {
        Ok(self.amount_q64 >= LARGE_AMOUNT_THRESHOLD)
    }

    #[inline(always)]
    pub fn is_high_impact(&self) -> Result<bool> {
        Ok(self.price_impact_q64 >= HIGH_IMPACT_THRESHOLD)
    }
}

#[derive(Clone, Copy, Pod, Zeroable, InitSpace, AnchorSerialize, AnchorDeserialize)]
#[repr(C)]
pub struct SlotBucketCounters {
    pub counters: [u32; SLOT_BUCKET_COUNT],
    pub volume_buckets: [Q64x64; SLOT_BUCKET_COUNT],
    pub current_bucket_slot: u64,
    pub _padding: [u8; 8],
}

impl SlotBucketCounters {
    #[inline(always)]
    pub fn increment(&mut self, slot: u64, volume: Q64x64) -> Result<()> {
        let previous_slot = self.current_bucket_slot;
        let difference = slot.saturating_sub(previous_slot);

        if difference >= SLOT_BUCKET_COUNT as u64 {
            self.counters.fill(0);
            self.volume_buckets.fill(Q64x64::zero());
        } else if difference > 0 {
            for k in 1..difference {
                let clear_slot = previous_slot + k;
                let index = (clear_slot % SLOT_BUCKET_COUNT as u64) as usize;
                self.counters[index] = 0;
                self.volume_buckets[index] = Q64x64::zero();
            }
        }

        self.current_bucket_slot = slot;

        let bucket_index = (slot % SLOT_BUCKET_COUNT as u64) as usize;
        self.counters[bucket_index] = self.counters[bucket_index].saturating_add(1);
        self.volume_buckets[bucket_index] =
            self.volume_buckets[bucket_index].checked_add(volume)?;

        Ok(())
    }

    #[inline(always)]
    pub fn get_burst_score(&self, current_slot: u64) -> u8 {
        let recent_slots = 3;
        let mut burst_count = 0u32;

        for i in 0..recent_slots.min(SLOT_BUCKET_COUNT) {
            let slot = current_slot.saturating_sub(i as u64);
            let index = (slot % SLOT_BUCKET_COUNT as u64) as usize;
            burst_count = burst_count.saturating_add(self.counters[index]);
        }

        (burst_count * 10).min(100) as u8
    }

    #[inline(always)]
    pub fn get_volume_burst_score(&self, current_slot: u64) -> Result<u8> {
        let recent_slots = 3;
        let mut total_volume = Q64x64::zero();

        for i in 0..recent_slots.min(SLOT_BUCKET_COUNT) {
            let slot = current_slot.saturating_sub(i as u64);
            let index = (slot % SLOT_BUCKET_COUNT as u64) as usize;
            total_volume = total_volume.checked_add(self.volume_buckets[index])?;
        }

        let volume_threshold = Q64x64::from_int(10_000_000); // 1 million in Q64.64
        let score = if total_volume > volume_threshold {
            let ratio = total_volume.checked_div(volume_threshold)?;

            (ratio.raw() >> 64).min(100) as u8
        } else {
            0
        };

        Ok(score)
    }
}
