#![no_main]

// Fuzz target: calculate_liquidity
// - Generate valid ticks and token amounts up to MAX_TOKEN_AMOUNT
// - Ensure function handles edge cases gracefully
// - Verify no integer overflows in intermediate calculations by relying on Result

use fluxa_core::math::core_arithmetic::{tick_to_sqrt_x64, Q64x64};
use fluxa_core::math::liquidity_math::calculate_liquidity;
use fluxa_core::utils::constants::MAX_TOKEN_AMOUNT;
use libfuzzer_sys::fuzz_target;

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

fuzz_target!(|data: &[u8]| {
    let mut idx = 0usize;

    // Ticks
    let t1 = read_i32(data, &mut idx);
    let t2 = read_i32(data, &mut idx);
    let (mut tick_lower, mut tick_upper) = if t1 <= t2 { (t1, t2) } else { (t2, t1) };

    // Avoid degenerate
    if tick_lower == tick_upper {
        tick_upper = tick_upper.saturating_add(1);
    }

    // Current tick inside range
    let diff = tick_upper.saturating_sub(tick_lower);
    let mid = tick_lower.saturating_add(diff / 2);

    // Convert to sqrt prices; skip if invalid
    let sqrt_lower = if let Ok(v) = tick_to_sqrt_x64(tick_lower) {
        v
    } else {
        return;
    };
    let sqrt_upper = if let Ok(v) = tick_to_sqrt_x64(tick_upper) {
        v
    } else {
        return;
    };
    let sqrt_current = if let Ok(v) = tick_to_sqrt_x64(mid) {
        v
    } else {
        return;
    };

    // Token amounts up to MAX_TOKEN_AMOUNT; ensure at least one is non-zero
    let mut amount_0 = read_u64(data, &mut idx) % (MAX_TOKEN_AMOUNT.saturating_add(1));
    let mut amount_1 = read_u64(data, &mut idx) % (MAX_TOKEN_AMOUNT.saturating_add(1));
    if amount_0 == 0 && amount_1 == 0 {
        amount_0 = 1;
    }

    // Valid path: expect Ok or bounded errors; must not panic
    let _ = calculate_liquidity(sqrt_current, sqrt_lower, sqrt_upper, amount_0, amount_1);

    // Also try boundary current at lower+epsilon and upper-epsilon
    if sqrt_upper.raw() > sqrt_lower.raw() + 2 {
        let epsilon = Q64x64::from_raw(1);
        let curr_low = Q64x64::from_raw(sqrt_lower.raw() + 1);
        let curr_up = Q64x64::from_raw(sqrt_upper.raw() - 1);
        let _ = calculate_liquidity(curr_low, sqrt_lower, sqrt_upper, amount_0, amount_1);
        let _ = calculate_liquidity(curr_up, sqrt_lower, sqrt_upper, amount_0, amount_1);
    }
});
