//! # Q64x64 Basic Arithmetic Operations Fuzz Target
//!
//! This fuzz target exercises all basic Q64x64 arithmetic operations within
//! protocol-realistic ranges based on actual AMM bounds:
//! - Addition, subtraction, multiplication, division
//! - Conversions between signed/unsigned
//! - Edge cases around overflow/underflow boundaries
//! - Ensures no panics occur under any input combination
//! - Focus on values that could occur in actual DeFi operations

#![no_main]

use fluxa_core::math::core_arithmetic::Q64x64;
use fluxa_core::utils::constants::{MAX_SQRT_X64, MIN_SQRT_X64, ONE_X64};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 32 {
        return; // Need at least 32 bytes for two u128 values
    }

    // Parse two Q64x64 values from fuzz input
    let raw1 = u128::from_le_bytes([
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9],
        data[10], data[11], data[12], data[13], data[14], data[15],
    ]);

    let raw2 = u128::from_le_bytes([
        data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23], data[24],
        data[25], data[26], data[27], data[28], data[29], data[30], data[31],
    ]);

    // Bias inputs toward protocol-realistic ranges while still testing edge cases
    // 75% of tests use protocol-realistic values, 25% use full range for edge case discovery
    let use_protocol_range = (raw1 % 4) != 0; // 75% probability

    let a = if use_protocol_range {
        // Generate values in realistic AMM range: [MIN_SQRT_X64/100, MAX_SQRT_X64/100]
        // This covers typical liquidity pool prices and amounts
        let range = (MAX_SQRT_X64 / 100) - (MIN_SQRT_X64 / 100);
        let offset = (raw1 % range) + (MIN_SQRT_X64 / 100);
        Q64x64::from_raw(offset)
    } else {
        Q64x64::from_raw(raw1)
    };

    let b = if use_protocol_range {
        let range = (MAX_SQRT_X64 / 100) - (MIN_SQRT_X64 / 100);
        let offset = (raw2 % range) + (MIN_SQRT_X64 / 100);
        Q64x64::from_raw(offset)
    } else {
        Q64x64::from_raw(raw2)
    };

    // Test all basic arithmetic operations - these should never panic
    let _ = a.checked_add(b);
    let _ = a.checked_sub(b);
    let _ = a.checked_mul(b);
    let _ = a.checked_div(b);

    // Test conversions to signed (may fail but should not panic)
    let _ = a.to_q64x64signed();
    let _ = b.to_q64x64signed();

    // Test signed arithmetic if conversions succeed
    if let (Ok(signed_a), Ok(signed_b)) = (a.to_q64x64signed(), b.to_q64x64signed()) {
        let _ = signed_a.checked_add(signed_b);
        let _ = signed_a.checked_sub(signed_b);
        let _ = signed_a.is_negative();
        let _ = signed_a.negate();
        let _ = signed_a.abs();
        let _ = signed_a.to_q64x64();
    }

    // Test edge cases with zero and one
    let _ = a.checked_add(Q64x64::zero());
    let _ = a.checked_mul(Q64x64::one());
    let _ = a.checked_div(Q64x64::one());

    // Test with protocol constants that appear frequently in AMM calculations
    let _ = a.checked_mul(Q64x64::from_raw(MIN_SQRT_X64));
    let _ = b.checked_mul(Q64x64::from_raw(MAX_SQRT_X64));
    let _ = a.checked_div(Q64x64::from_raw(ONE_X64));

    // Test common DeFi operations: fee calculations (typically 0.01% to 1%)
    let fee_001 = Q64x64::from_raw(ONE_X64 / 10000); // 0.01%
    let fee_1 = Q64x64::from_raw(ONE_X64 / 100); // 1%
    let _ = a.checked_mul(fee_001);
    let _ = b.checked_mul(fee_1);

    // Test chained operations (common source of precision/overflow issues)
    if let (Ok(sum), Ok(prod)) = (a.checked_add(b), a.checked_mul(b)) {
        let _ = sum.checked_div(prod);
    }

    // Verify basic mathematical invariants where possible
    if a == Q64x64::zero() {
        // 0 + x = x
        assert_eq!(a.checked_add(b), Ok(b));
        // 0 * x = 0
        assert_eq!(a.checked_mul(b), Ok(Q64x64::zero()));
    }

    if b == Q64x64::one() && a != Q64x64::zero() {
        // x * 1 = x
        assert_eq!(a.checked_mul(b), Ok(a));
        // x / 1 = x
        assert_eq!(a.checked_div(b), Ok(a));
    }
});
