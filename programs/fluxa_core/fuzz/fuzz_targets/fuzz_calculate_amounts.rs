#![no_main]

// Fuzz target: calculate_amounts_for_liquidity_piecewise
// - Randomly generate valid Q64x64 prices and liquidity values
// - Ensure no panics for valid inputs
// - Verify error cases return Err (no unwrap/panic)
// - Exercise edge cases near MIN_SQRT_X64 and MAX_SQRT_X64

use fluxa_core::math::core_arithmetic::Q64x64;
use fluxa_core::math::liquidity_math::calculate_amounts_for_liquidity_piecewise;
use fluxa_core::utils::constants::{MAX_SQRT_X64, MAX_TOKEN_AMOUNT, MIN_SQRT_X64};
use libfuzzer_sys::fuzz_target;

fn read_u128(bytes: &[u8], idx: &mut usize) -> u128 {
    let mut buf = [0u8; 16];
    for i in 0..16 {
        buf[i] = bytes.get(*idx + i).copied().unwrap_or(0);
    }
    *idx += 16;
    u128::from_le_bytes(buf)
}

fn clamp_raw_sqrt(mut v: u128) -> u128 {
    let min = MIN_SQRT_X64 as u128 + 1; // avoid zero lower bound
    let max = MAX_SQRT_X64 as u128;
    if v < min {
        v = min;
    }
    if v > max {
        v = max;
    }
    v
}

fn make_ordered(a: u128, b: u128) -> (u128, u128) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fuzz_target!(|data: &[u8]| {
    let mut idx = 0usize;

    // Draw raw values
    let r1 = read_u128(data, &mut idx);
    let r2 = read_u128(data, &mut idx);
    let r3 = read_u128(data, &mut idx);
    let liq_raw = read_u128(data, &mut idx);

    // Clamp to sqrt raw ranges
    let a = clamp_raw_sqrt(r1);
    let b = clamp_raw_sqrt(r2);
    let (lower_raw, upper_raw) = make_ordered(a, b);

    // If equal, nudge upper
    let upper_raw = if upper_raw == lower_raw {
        upper_raw.saturating_add(1)
    } else {
        upper_raw
    };
    let current_hint = clamp_raw_sqrt(r3);

    let sqrt_lower = Q64x64::from_raw(lower_raw);
    let sqrt_upper = Q64x64::from_raw(upper_raw);

    // Liquidity: ensure non-zero but bounded
    let liquidity_raw = (liq_raw | 1) & ((1u128 << 120) - 1); // keep in a sane range
    let liquidity = Q64x64::from_raw(liquidity_raw);

    // 1) Valid input path: choose current inside (lower, upper)
    if sqrt_upper.raw() > sqrt_lower.raw() + 1 {
        let mid = Q64x64::from_raw(sqrt_lower.raw() + (sqrt_upper.raw() - sqrt_lower.raw()) / 2);
        // Valid case must not panic and should return Ok with bounded amounts
        if let Ok((a0, a1)) =
            calculate_amounts_for_liquidity_piecewise(mid, sqrt_lower, sqrt_upper, liquidity)
        {
            // Bounds check to catch any overflow behavior reaching outside protocol caps
            assert!(a0 <= MAX_TOKEN_AMOUNT);
            assert!(a1 <= MAX_TOKEN_AMOUNT);
        }
    }

    // 2) Edge-case near MIN
    let near_min_lower = Q64x64::from_raw((MIN_SQRT_X64 as u128 + 1));
    let near_min_upper = Q64x64::from_raw((MIN_SQRT_X64 as u128 + 2));
    let _ = calculate_amounts_for_liquidity_piecewise(
        near_min_lower,
        near_min_lower,
        near_min_upper,
        liquidity,
    );

    // 3) Edge-case near MAX
    let near_max_upper = Q64x64::from_raw(MAX_SQRT_X64 as u128);
    let near_max_lower = Q64x64::from_raw((MAX_SQRT_X64 as u128).saturating_sub(2));
    let _ = calculate_amounts_for_liquidity_piecewise(
        near_max_lower,
        near_max_lower,
        near_max_upper,
        liquidity,
    );

    // 4) Arbitrary current anywhere between lower and upper or at boundaries
    let current = Q64x64::from_raw(current_hint);
    let _ = calculate_amounts_for_liquidity_piecewise(current, sqrt_lower, sqrt_upper, liquidity);
});
