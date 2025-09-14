//! # Mul_Div Operations Fuzz Target
//!
//! This fuzz target tests the critical mul_div and mul_div_round_up functions
//! which implement high-precision (a * b) / c operations. These are essential
//! for AMM calculations where precision loss can lead to economic exploits.
//!
//! Tests include:
//! - Protocol-realistic ranges based on actual AMM bounds
//! - Precision preservation properties for liquidity calculations
//! - Price impact calculations and fee computations
//! - Comparison between mul_div and naive (a * b) / c
//! - Ceiling vs floor rounding behavior for swap calculations
//! - Edge cases: overflow, division by zero

#![no_main]

use ethnum::U256;
use fluxa_core::math::core_arithmetic::{mul_div, mul_div_q64, mul_div_round_up, Q64x64};
use fluxa_core::utils::constants::{MAX_SQRT_X64, MAX_TOKEN_AMOUNT, MIN_SQRT_X64, ONE_X64};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 48 {
        return; // Need at least 48 bytes for three u128 values
    }

    // Parse three u128 values from fuzz input, bias toward protocol-realistic ranges
    let raw_a = u128::from_le_bytes([
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9],
        data[10], data[11], data[12], data[13], data[14], data[15],
    ]);

    let raw_b = u128::from_le_bytes([
        data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23], data[24],
        data[25], data[26], data[27], data[28], data[29], data[30], data[31],
    ]);

    let raw_c = u128::from_le_bytes([
        data[32], data[33], data[34], data[35], data[36], data[37], data[38], data[39], data[40],
        data[41], data[42], data[43], data[44], data[45], data[46], data[47],
    ]);

    // Bias toward realistic AMM values: 70% protocol range, 30% full range
    let use_amm_range = (raw_a % 10) < 7;

    let a = if use_amm_range {
        // Realistic token amounts: up to MAX_TOKEN_AMOUNT (1e18)
        // Use checked arithmetic to avoid overflow in test harness
        let max_amount = (MAX_TOKEN_AMOUNT as u128).saturating_mul(ONE_X64);
        (raw_a % max_amount) + 1
    } else {
        raw_a
    };

    let b = if use_amm_range {
        // Realistic price ranges: MIN_SQRT_X64 to MAX_SQRT_X64
        let price_range = MAX_SQRT_X64 - MIN_SQRT_X64;
        (raw_b % price_range) + MIN_SQRT_X64
    } else {
        raw_b
    };

    let c = if use_amm_range {
        // Avoid division by tiny numbers, use realistic denominators
        // Use checked arithmetic to avoid overflow in test harness
        let max_amount = (MAX_TOKEN_AMOUNT as u128).saturating_mul(ONE_X64);
        (raw_c % max_amount) + ONE_X64
    } else {
        raw_c.max(1) // Ensure non-zero for division
    };

    // Test mul_div function - should never panic
    match mul_div(a, b, c) {
        Ok(result) => {
            // Verify result is reasonable relative to inputs using U256 to avoid overflow
            if c > 0 {
                // Use U256 for high-precision verification without overflow concerns
                let a_u256 = U256::from(a);
                let b_u256 = U256::from(b);
                let c_u256 = U256::from(c);

                // Compute expected result using U256 arithmetic
                let ab_u256 = a_u256 * b_u256;
                let expected_u256 = ab_u256 / c_u256;

                // Only verify if the expected result fits in u128
                if expected_u256 <= U256::from(u128::MAX) {
                    let expected = expected_u256.as_u128();

                    // Results should be close (within 1 due to rounding differences)
                    let diff = if result >= expected {
                        result - expected
                    } else {
                        expected - result
                    };
                    assert!(
                        diff <= 1,
                        "mul_div({}, {}, {}) = {} but U256 ({}*{})/{} = {}, diff={}",
                        a,
                        b,
                        c,
                        result,
                        a,
                        b,
                        c,
                        expected,
                        diff
                    );
                }
            }

            // Test mathematical properties
            if c > 0 {
                // mul_div(0, b, c) should equal 0
                assert_eq!(mul_div(0, b, c).unwrap_or(u128::MAX), 0);

                // mul_div(a, 0, c) should equal 0
                assert_eq!(mul_div(a, 0, c).unwrap_or(u128::MAX), 0);

                // mul_div(a, c, c) should equal a (if no overflow)
                if a <= u128::MAX / c {
                    assert_eq!(mul_div(a, c, c).unwrap(), a);
                }
            }
        }
        Err(_) => {
            // mul_div can fail due to division by zero or overflow
            // but should never panic
        }
    }

    // Test mul_div_round_up function
    match mul_div_round_up(a, b, c) {
        Ok(result_ceil) => {
            // Compare with regular mul_div
            if let Ok(result_floor) = mul_div(a, b, c) {
                // Ceiling result should be >= floor result
                assert!(
                    result_ceil >= result_floor,
                    "mul_div_round_up({}, {}, {}) = {} < mul_div(...) = {}",
                    a,
                    b,
                    c,
                    result_ceil,
                    result_floor
                );

                // Difference should be at most 1
                assert!(
                    result_ceil - result_floor <= 1,
                    "mul_div_round_up({}, {}, {}) = {} vs mul_div(...) = {}, diff = {}",
                    a,
                    b,
                    c,
                    result_ceil,
                    result_floor,
                    result_ceil - result_floor
                );
            }

            // Test that for exact divisions, both functions give same result
            if c > 0 && a % c == 0 && b % c == 0 {
                if let Ok(result_floor) = mul_div(a, b, c) {
                    assert_eq!(
                        result_ceil, result_floor,
                        "For exact division mul_div_round_up({}, {}, {}) = {} != mul_div(...) = {}",
                        a, b, c, result_ceil, result_floor
                    );
                }
            }
        }
        Err(_) => {
            // mul_div_round_up can fail for same reasons as mul_div
        }
    }

    // Test Q64x64 wrapper function
    let qa = Q64x64::from_raw(a);
    let qb = Q64x64::from_raw(b);
    let qc = Q64x64::from_raw(c);

    match mul_div_q64(qa, qb, qc) {
        Ok(qresult) => {
            // Should match the raw mul_div result
            if let Ok(raw_result) = mul_div(a, b, c) {
                assert_eq!(
                    qresult.raw(),
                    raw_result,
                    "mul_div_q64 wrapper inconsistent: raw={}, wrapped={}",
                    raw_result,
                    qresult.raw()
                );
            }
        }
        Err(_) => {
            // Should fail for same reasons as raw mul_div
        }
    }

    // Test division by zero handling
    let _ = mul_div(a, b, 0); // Should return Err, not panic
    let _ = mul_div_round_up(a, b, 0); // Should return Err, not panic

    // Test edge cases with maximum values
    let _ = mul_div(u128::MAX, u128::MAX, u128::MAX);
    let _ = mul_div(u128::MAX, 1, u128::MAX);
    let _ = mul_div(1, u128::MAX, 1);

    // Test precision preservation with realistic DeFi amounts
    if c > 1000 {
        // Avoid division by very small numbers for this test
        // Simulate fee calculations: (amount * fee_rate) / fee_denominator
        let amount = a % 1_000_000_000_000_000_000u128; // 1e18 (realistic token amount)
        let fee_rate = b % 10000u128; // basis points (0-100%)
        let fee_denominator = 10000u128;

        let _ = mul_div(amount, fee_rate, fee_denominator);
        let _ = mul_div_round_up(amount, fee_rate, fee_denominator);

        // Fee calculations should never overflow for reasonable inputs
        if amount < u128::MAX / 10000 {
            match mul_div(amount, fee_rate, fee_denominator) {
                Ok(fee) => {
                    // Fee should be reasonable relative to amount
                    assert!(
                        fee <= amount,
                        "Fee {} should not exceed amount {} for rate {}/{}",
                        fee,
                        amount,
                        fee_rate,
                        fee_denominator
                    );
                }
                Err(_) => {
                    // Should not fail for reasonable fee calculations
                    if amount < u128::MAX / 100 && fee_rate < 10000 {
                        panic!(
                            "Reasonable fee calculation failed: amount={}, rate={}, denom={}",
                            amount, fee_rate, fee_denominator
                        );
                    }
                }
            }
        }
    }

    // Test associativity property: mul_div(mul_div(a, b, c), d, e) behavior
    if data.len() >= 64 {
        let d = u128::from_le_bytes([
            data[48], data[49], data[50], data[51], data[52], data[53], data[54], data[55],
            data[56], data[57], data[58], data[59], data[60], data[61], data[62], data[63],
        ]);

        if c > 0 && d > 0 {
            if let Ok(intermediate) = mul_div(a, b, c) {
                // Test chained mul_div operations don't panic
                let _ = mul_div(intermediate, d, c);
                let _ = mul_div_round_up(intermediate, d, c);
            }
        }
    }
});
