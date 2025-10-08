use crate::math::core_arithmetic::Q64x64;
use crate::security::flash_loan_protection::compact_operation_slot_bucket::CompactOperation;
use crate::utils::constants::{FLASH_SEQUENCE_PATTERN, OPERATION_WINDOW_SIZE, PATTERN_CACHE_SIZE};
use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

#[derive(Clone, Copy, Pod, Zeroable, InitSpace, AnchorSerialize, AnchorDeserialize)]
#[repr(C)]
pub struct OperationRingBuffer {
    pub operations: [CompactOperation; OPERATION_WINDOW_SIZE],
    pub total_volume_q64: Q64x64,
    pub rolling_hash_12bit: u16,
    pub pattern_cache: [u16; PATTERN_CACHE_SIZE],
    pub head: u8,
    pub count: u8,
    pub rolling_hash_6bit: u8,
    pub _padding: [u8; 3],
}

impl OperationRingBuffer {
    #[inline(always)]
    pub fn push(&mut self, operation: CompactOperation) -> Result<()> {
        let index = (self.head as usize) & (OPERATION_WINDOW_SIZE - 1);

        self.total_volume_q64 = self.total_volume_q64.checked_add(operation.amount_q64)?;

        self.operations[index] = operation;
        self.head = (self.head + 1) & ((OPERATION_WINDOW_SIZE - 1) as u8);

        if self.count < OPERATION_WINDOW_SIZE as u8 {
            self.count = self.count.saturating_add(1);
        }

        let operation_2bit = operation.get_2bit_encoding();
        self.rolling_hash_6bit = ((self.rolling_hash_6bit << 2) | operation_2bit) & 0b111111;
        self.rolling_hash_12bit = ((self.rolling_hash_12bit << 2) | operation_2bit as u16) & 0xFFF;

        if self.count >= 3 && self.rolling_hash_6bit == FLASH_SEQUENCE_PATTERN {
            self.push_fingerprint_to_cache(self.rolling_hash_12bit);
        }

        Ok(())
    }

    #[inline(always)]
    fn push_fingerprint_to_cache(&mut self, fingerprint: u16) {
        for existing in &self.pattern_cache {
            if *existing == fingerprint {
                return; // Already in cache
            }
        }

        self.pattern_cache.copy_within(0..PATTERN_CACHE_SIZE - 1, 1);
        self.pattern_cache[0] = fingerprint;
    }

    #[inline(always)]
    pub fn iter_recent(&'_ self, window_slots: u64, current_slot: u64) -> OperationIterator<'_> {
        OperationIterator {
            buffer: self,
            current_index: 0,
            window_slots,
            current_slot,
            processed_count: 0,
        }
    }

    #[inline(always)]
    pub fn get_pattern_fingerprint(&self) -> u16 {
        self.rolling_hash_12bit
    }

    #[inline(always)]
    pub fn check_pattern_cache(&self, fingerprint: u16) -> bool {
        self.pattern_cache.contains(&fingerprint)
    }

    #[inline(always)]
    pub fn get_volume_in_window(&self, window_slots: u64, current_slot: u64) -> Result<Q64x64> {
        let mut volume = Q64x64::zero();
        for operation in self.iter_recent(window_slots, current_slot) {
            volume = volume.checked_add(operation.amount_q64)?;
        }

        Ok(volume)
    }
}

pub struct OperationIterator<'a> {
    buffer: &'a OperationRingBuffer,
    current_index: u16,
    window_slots: u64,
    current_slot: u64,
    processed_count: u16,
}

impl<'a> Iterator for OperationIterator<'a> {
    type Item = &'a CompactOperation;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.current_index >= self.buffer.count as u16
                || self.processed_count >= OPERATION_WINDOW_SIZE as u16
            {
                return None;
            }

            let actual_index = if self.buffer.count == OPERATION_WINDOW_SIZE as u8 {
                ((self.buffer.head as u16 + self.current_index) as usize)
                    & (OPERATION_WINDOW_SIZE - 1)
            } else {
                self.current_index as usize
            };

            let operation = &self.buffer.operations[actual_index];
            self.current_index += 1;
            self.processed_count += 1;

            if self.current_slot >= operation.slot
                && self.current_slot - operation.slot < self.window_slots
            {
                return Some(operation);
            }
        }
    }
}
