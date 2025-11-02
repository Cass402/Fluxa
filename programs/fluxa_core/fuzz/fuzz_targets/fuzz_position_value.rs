#![no_main]

// Fuzz target: calculate_position_value_at_price
// - Randomly generate Position with valid fields
// - Randomly generate USD prices from 0..=u64::MAX
// - Verify overflow protection (no panics; errors allowed)

use fluxa_core::math::core_arithmetic::{tick_to_sqrt_x64, Q64x64};
use fluxa_core::math::liquidity_math::calculate_position_value_at_price;
use fluxa_core::state::position::position_account::Position;
use libfuzzer_sys::fuzz_target;

// Re-export Pubkey from solana_program (available through fluxa_core)
use solana_program::pubkey::Pubkey;

fn read_u64(bytes: &[u8], idx: &mut usize) -> u64 {
    let mut buf = [0u8; 8];
    for i in 0..8 {
        buf[i] = bytes.get(*idx + i).copied().unwrap_or(0);
    }
    *idx += 8;
    u64::from_le_bytes(buf)
}

fn read_i32(bytes: &[u8], idx: &mut usize) -> i32 {
    let mut buf = [0u8; 4];
    for i in 0..4 {
        buf[i] = bytes.get(*idx + i).copied().unwrap_or(0);
    }
    *idx += 4;
    i32::from_le_bytes(buf)
}

fn read_u128(bytes: &[u8], idx: &mut usize) -> u128 {
    let mut buf = [0u8; 16];
    for i in 0..16 {
        buf[i] = bytes.get(*idx + i).copied().unwrap_or(0);
    }
    *idx += 16;
    u128::from_le_bytes(buf)
}

fuzz_target!(|data: &[u8]| {
    let mut idx = 0usize;

    // Generate ticks ensuring order
    let t1 = read_i32(data, &mut idx);
    let t2 = read_i32(data, &mut idx);
    let (tick_lower, mut tick_upper) = if t1 <= t2 { (t1, t2) } else { (t2, t1) };
    if tick_lower == tick_upper {
        tick_upper = tick_upper.saturating_add(1);
    }

    // Liquidity raw (bounded)
    let liq_raw = read_u128(data, &mut idx) & ((1u128 << 120) - 1);

    // Build position with sane defaults
    let owner = {
        let seed = read_u64(data, &mut idx);
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&seed.to_le_bytes());
        Pubkey::new_from_array(bytes)
    };

    let position = Position {
        owner,
        tick_lower,
        tick_upper,
        status_flags: Position::FLAG_ACTIVE,
        position_nonce: 0,
        _padding1: [0u8; 2],
        liquidity: Q64x64::from_raw(liq_raw | 1), // ensure non-zero
        fee_growth_inside_0_last: Q64x64::zero(),
        fee_growth_inside_1_last: Q64x64::zero(),
        tokens_owed_0: Q64x64::zero(),
        tokens_owed_1: Q64x64::zero(),
        total_fees_collected_0: Q64x64::zero(),
        total_fees_collected_1: Q64x64::zero(),
        creation_slot: 0,
        last_update_slot: 0,
        creation_timestamp: 0,
        position_hash: [0u8; 32],
        reserved: [0u64; 3],
    };

    // Current price: pick mid tick to avoid bounds most of the time
    let lower = tick_lower as i64;
    let upper = tick_upper as i64;
    let diff = upper - lower;
    let half_diff = diff / 2;
    let mid = lower + half_diff;
    let mid_tick = mid as i32;
    let current_sqrt = if let Ok(v) = tick_to_sqrt_x64(mid_tick) {
        v
    } else {
        return;
    };

    // USD prices can be any u64
    let p0 = read_u64(data, &mut idx);
    let p1 = read_u64(data, &mut idx);

    // Must not panic; Ok and Err are both acceptable outcomes depending on magnitude
    let _ = calculate_position_value_at_price(&position, current_sqrt, p0, p1);
});
