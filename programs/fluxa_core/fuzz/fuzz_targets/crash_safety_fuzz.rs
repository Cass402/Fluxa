//! # Comprehensive Crash Safety Fuzz Target
//!
//! This fuzz target is designed to ensure that NO combination of inputs to
//! any core arithmetic function can cause a panic, undefined behavior, or crash.
//! It's specifically designed for consensus-critical code where any panic could
//! break blockchain consensus and lock user funds.
//!
//! Tests include:
//! - All possible input combinations across full value ranges
//! - Extreme inputs: max values, zero, near-overflow conditions
//! - Malicious inputs designed to trigger integer overflow/underflow
//! - Resource exhaustion scenarios
//! - Invalid input combinations that should be gracefully rejected

#![no_main]

use ethnum::U256;
use fluxa_core::math::core_arithmetic::{
    liquidity_from_amount_0, liquidity_from_amount_1, mul_div, mul_div_q64, mul_div_round_up,
    sqrt_x64, tick_to_sqrt_x64, Q64x64,
};
use fluxa_core::utils::constants::{MAX_SQRT_X64, MAX_TICK, MIN_SQRT_X64, MIN_TICK};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // This fuzz target systematically exercises every public function
    // with extreme and adversarial inputs to ensure no panics occur

    if data.len() < 32 {
        return;
    }

    // Parse various types from the input data
    let raw1 = u128::from_le_bytes([
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9],
        data[10], data[11], data[12], data[13], data[14], data[15],
    ]);

    let raw2 = u128::from_le_bytes([
        data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23], data[24],
        data[25], data[26], data[27], data[28], data[29], data[30], data[31],
    ]);

    let q1 = Q64x64::from_raw(raw1);
    let q2 = Q64x64::from_raw(raw2);

    // Test all Q64x64 operations with any possible inputs - MUST NEVER PANIC
    let _ = q1.checked_add(q2);
    let _ = q1.checked_sub(q2);
    let _ = q1.checked_mul(q2);
    let _ = q1.checked_div(q2);
    let _ = q1.to_q64x64signed();
    let _ = q2.to_q64x64signed();

    // Test with extreme values specifically
    let max_q = Q64x64::from_raw(u128::MAX);
    let min_q = Q64x64::from_raw(0);
    let one_q = Q64x64::one();

    let _ = max_q.checked_add(max_q); // Should overflow gracefully
    let _ = min_q.checked_sub(one_q); // Should underflow gracefully
    let _ = max_q.checked_mul(max_q); // Should overflow gracefully
    let _ = one_q.checked_div(min_q); // Should handle division by zero

    // Test signed arithmetic with extreme values
    // Note: We'll test signed operations through the main Q64x64 conversion methods
    if let Ok(signed_from_q1) = q1.to_q64x64signed() {
        let _ = signed_from_q1.checked_add(signed_from_q1);
        let _ = signed_from_q1.negate();
        let _ = signed_from_q1.abs();
        let _ = signed_from_q1.to_q64x64();
    }

    // Test sqrt with all possible inputs - MUST NEVER PANIC
    let _ = sqrt_x64(q1);
    let _ = sqrt_x64(q2);
    let _ = sqrt_x64(min_q);
    let _ = sqrt_x64(max_q);
    let _ = sqrt_x64(one_q);

    // Test tick conversions with extreme values
    if data.len() >= 36 {
        let raw_tick = i32::from_le_bytes([data[32], data[33], data[34], data[35]]);

        // Test with the raw tick (may be out of bounds)
        let _ = tick_to_sqrt_x64(raw_tick);

        // Test boundary conditions
        let _ = tick_to_sqrt_x64(MIN_TICK);
        let _ = tick_to_sqrt_x64(MAX_TICK);
        let _ = tick_to_sqrt_x64(MIN_TICK - 1); // Out of bounds - should error gracefully
        let _ = tick_to_sqrt_x64(MAX_TICK + 1); // Out of bounds - should error gracefully
        let _ = tick_to_sqrt_x64(i32::MIN); // Extreme out of bounds
        let _ = tick_to_sqrt_x64(i32::MAX); // Extreme out of bounds
    }

    // Test mul_div operations with extreme combinations
    if data.len() >= 48 {
        let c = u128::from_le_bytes([
            data[32], data[33], data[34], data[35], data[36], data[37], data[38], data[39],
            data[40], data[41], data[42], data[43], data[44], data[45], data[46], data[47],
        ]);

        // Test all mul_div variations - MUST NEVER PANIC
        let _ = mul_div(raw1, raw2, c);
        let _ = mul_div_round_up(raw1, raw2, c);
        let _ = mul_div_q64(q1, q2, Q64x64::from_raw(c));

        // Test extreme combinations
        let _ = mul_div(u128::MAX, u128::MAX, 1); // Maximum possible product
        let _ = mul_div(u128::MAX, u128::MAX, u128::MAX); // All max values
        let _ = mul_div(raw1, raw2, 0); // Division by zero - should error gracefully
        let _ = mul_div(0, raw2, c); // Zero numerator
        let _ = mul_div(raw1, 0, c); // Zero in product

        // Same tests for ceiling division
        let _ = mul_div_round_up(u128::MAX, u128::MAX, 1);
        let _ = mul_div_round_up(u128::MAX, u128::MAX, u128::MAX);
        let _ = mul_div_round_up(raw1, raw2, 0); // Division by zero
    }

    // Test liquidity calculation functions with extreme inputs
    if data.len() >= 56 {
        let amount = u64::from_le_bytes([
            data[48], data[49], data[50], data[51], data[52], data[53], data[54], data[55],
        ]);

        // Create sqrt price values from our inputs
        let sqrt_a = Q64x64::from_raw(raw1.min(raw2)); // Ensure sqrt_a < sqrt_b
        let sqrt_b = Q64x64::from_raw(raw1.max(raw2).saturating_add(1)); // Ensure sqrt_b > sqrt_a, avoid overflow

        // Test liquidity functions - MUST NEVER PANIC
        let _ = liquidity_from_amount_0(sqrt_a, sqrt_b, amount);
        let _ = liquidity_from_amount_1(sqrt_a, sqrt_b, amount);

        // Test with extreme values
        let min_sqrt = Q64x64::from_raw(MIN_SQRT_X64);
        let max_sqrt = Q64x64::from_raw(MAX_SQRT_X64);

        let _ = liquidity_from_amount_0(min_sqrt, max_sqrt, u64::MAX);
        let _ = liquidity_from_amount_1(min_sqrt, max_sqrt, u64::MAX);
        let _ = liquidity_from_amount_0(min_sqrt, max_sqrt, 0);
        let _ = liquidity_from_amount_1(min_sqrt, max_sqrt, 0);

        // Test invalid orderings (sqrt_a >= sqrt_b) - should error gracefully
        let _ = liquidity_from_amount_0(sqrt_b, sqrt_a, amount); // Wrong order
        let _ = liquidity_from_amount_1(sqrt_b, sqrt_a, amount); // Wrong order
        let _ = liquidity_from_amount_0(sqrt_a, sqrt_a, amount); // Equal values
        let _ = liquidity_from_amount_1(sqrt_a, sqrt_a, amount); // Equal values
    }

    // Test chained operations that might accumulate errors or overflow
    let mut accumulator = q1;
    for i in 0..10 {
        if let Ok(new_val) = accumulator.checked_add(Q64x64::from_raw(i)) {
            accumulator = new_val;
            let _ = sqrt_x64(accumulator);
        } else {
            break; // Stop if we overflow, but don't panic
        }
    }

    // Test operations with bit patterns likely to trigger edge cases
    let patterns = [
        0u128,                                   // All zeros
        u128::MAX,                               // All ones
        1u128 << 127,                            // Sign bit for i128
        (1u128 << 64) - 1,                       // Maximum for lower 64 bits
        1u128 << 64,                             // Minimum for Q64.64 integer part
        0x55555555555555555555555555555555_u128, // Alternating bits pattern
        0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAA_u128,    // Alternating bits pattern
    ];

    for &pattern in &patterns {
        let q_pattern = Q64x64::from_raw(pattern);

        // All operations with pattern values must not panic
        let _ = q_pattern.checked_add(q1);
        let _ = q_pattern.checked_sub(q1);
        let _ = q_pattern.checked_mul(q1);
        let _ = q_pattern.checked_div(q1);
        let _ = sqrt_x64(q_pattern);
    }

    // Test all operations with values near fixed-point boundaries
    let near_boundaries = [
        1u128,             // Smallest positive
        (1u128 << 64) - 1, // Just under 1.0 in Q64.64
        1u128 << 64,       // Exactly 1.0 in Q64.64
        (1u128 << 64) + 1, // Just over 1.0 in Q64.64
        u128::MAX - 1,     // Near maximum
    ];

    for &boundary in &near_boundaries {
        let q_boundary = Q64x64::from_raw(boundary);

        let _ = q_boundary.checked_add(q1);
        let _ = q_boundary.checked_mul(q2);
        let _ = sqrt_x64(q_boundary);
        let _ = q_boundary.checked_div(Q64x64::from_raw(1)); // Division by tiny value
    }

    // Test U256 arithmetic operations that could be used in intermediate calculations
    // This ensures our functions handle extreme cases that require 256-bit precision
    if data.len() >= 64 {
        let u256_raw1 = U256::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8],
            data[9], data[10], data[11], data[12], data[13], data[14], data[15], data[16],
            data[17], data[18], data[19], data[20], data[21], data[22], data[23], data[24],
            data[25], data[26], data[27], data[28], data[29], data[30], data[31],
        ]);

        let u256_raw2 = U256::from_le_bytes([
            data[32], data[33], data[34], data[35], data[36], data[37], data[38], data[39],
            data[40], data[41], data[42], data[43], data[44], data[45], data[46], data[47],
            data[48], data[49], data[50], data[51], data[52], data[53], data[54], data[55],
            data[56], data[57], data[58], data[59], data[60], data[61], data[62], data[63],
        ]);

        // Test U256 operations that might be used in mul_div intermediate calculations
        let _ = u256_raw1.checked_add(u256_raw2);
        let _ = u256_raw1.checked_sub(u256_raw2);
        let _ = u256_raw1.checked_mul(u256_raw2);
        let _ = u256_raw1.checked_div(u256_raw2);

        // Test extreme U256 values
        let u256_max = U256::MAX;
        let u256_zero = U256::ZERO;
        let u256_one = U256::ONE;

        let _ = u256_max.checked_mul(u256_max); // Should overflow gracefully
        let _ = u256_zero.checked_sub(u256_one); // Should underflow gracefully
        let _ = u256_one.checked_div(u256_zero); // Division by zero

        // Test conversion between U256 and u128 for mul_div operations
        // This simulates the intermediate calculations in mul_div
        if u256_raw1 <= U256::from(u128::MAX) {
            let back_to_u128 = u256_raw1.as_u128();
            let q_from_u256 = Q64x64::from_raw(back_to_u128);
            let _ = sqrt_x64(q_from_u256);
        }

        // Test U256 arithmetic patterns that could cause issues in fixed-point math
        let patterns_u256 = [
            U256::ZERO,
            U256::ONE,
            U256::MAX,
            U256::from(u128::MAX),
            U256::from(1u128 << 127),      // i128 sign bit
            U256::from((1u128 << 64) - 1), // Q64.64 fractional part max
            U256::from(1u128 << 64),       // Q64.64 one
        ];

        for &pattern in &patterns_u256 {
            let _ = pattern.checked_mul(u256_raw1);
            let _ = pattern.checked_add(u256_raw2);

            // Test conversion back to Q64x64 ranges
            if pattern <= U256::from(u128::MAX) {
                let as_u128 = pattern.as_u128();
                let q_pattern = Q64x64::from_raw(as_u128);
                let _ = q_pattern.checked_mul(Q64x64::from_raw(raw1));
                let _ = sqrt_x64(q_pattern);
            }
        }
    }

    // Test that we never panic even with completely random data interpretations
    // This catches any assumptions about data structure or alignment
    for chunk in data.chunks(16) {
        if chunk.len() == 16 {
            let raw = u128::from_le_bytes([
                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
                chunk[8], chunk[9], chunk[10], chunk[11], chunk[12], chunk[13], chunk[14],
                chunk[15],
            ]);
            let q = Q64x64::from_raw(raw);
            let _ = sqrt_x64(q);
            let _ = q.checked_mul(Q64x64::one());
        }
    }
});
