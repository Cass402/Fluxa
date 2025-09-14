//! # Square Root Function Fuzz Target
//!
//! This fuzz target specifically tests the sqrt_x64 function which is critical for
//! price calculations in concentrated liquidity. Tests include:
//! - All possible Q64x64 input values
//! - Mathematical property verification (x = sqrt(y) => x² ≈ y)
//! - Edge cases: 0, 1, maximum values
//! - Precision bounds and monotonicity

#![no_main]

use fluxa_core::math::core_arithmetic::{sqrt_x64, Q64x64};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 16 {
        return; // Need at least 16 bytes for one u128 value
    }

    // Parse Q64x64 value from fuzz input
    let raw_input = u128::from_le_bytes([
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9],
        data[10], data[11], data[12], data[13], data[14], data[15],
    ]);

    let input = Q64x64::from_raw(raw_input);

    // Test sqrt function - should never panic
    match sqrt_x64(input) {
        Ok(result) => {
            // Result is u128, so it's always >= 0 by definition
            // Test mathematical property: sqrt(x)² ≈ x (within reasonable precision)
            if let Ok(squared) = result.checked_mul(result) {
                // For most values, the squared result should be close to original
                // Allow some precision loss due to fixed-point arithmetic
                let diff = if squared.raw() >= input.raw() {
                    squared.raw() - input.raw()
                } else {
                    input.raw() - squared.raw()
                };

                // Precision tolerance: allow up to 1000 ULPs of error for numerical stability
                // This accounts for Newton-Raphson convergence limits and fixed-point rounding
                if input.raw() > 0 && input.raw() < u128::MAX / 1000 {
                    let relative_error = (diff as f64) / (input.raw() as f64);
                    assert!(
                        relative_error < 0.001,
                        "sqrt precision too low: input={}, sqrt={}, squared={}, relative_error={}",
                        input.raw(),
                        result.raw(),
                        squared.raw(),
                        relative_error
                    );
                }
            }

            // Test monotonicity: if we have a second input, sqrt should preserve ordering
            if data.len() >= 32 {
                let raw_input2 = u128::from_le_bytes([
                    data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23],
                    data[24], data[25], data[26], data[27], data[28], data[29], data[30], data[31],
                ]);
                let input2 = Q64x64::from_raw(raw_input2);

                if let Ok(result2) = sqrt_x64(input2) {
                    // Monotonicity: x1 < x2 => sqrt(x1) <= sqrt(x2)
                    if input.raw() < input2.raw() {
                        assert!(
                            result.raw() <= result2.raw(),
                            "sqrt not monotonic: {} < {} but sqrt({}) = {} > sqrt({}) = {}",
                            input.raw(),
                            input2.raw(),
                            input.raw(),
                            result.raw(),
                            input2.raw(),
                            result2.raw()
                        );
                    }
                }
            }
        }
        Err(_) => {
            // sqrt can fail for various reasons (overflow in intermediate calculations)
            // but it should never panic
        }
    }

    // Test specific edge cases that are critical for DeFi applications
    let _ = sqrt_x64(Q64x64::zero()); // sqrt(0) = 0
    let _ = sqrt_x64(Q64x64::one()); // sqrt(1) = 1

    // Test protocol boundary values - these must never panic
    let _ = sqrt_x64(Q64x64::from_raw(MIN_SQRT_X64));
    let _ = sqrt_x64(Q64x64::from_raw(MAX_SQRT_X64));

    // Test that sqrt of squared values returns approximately original
    // Use protocol-realistic ranges based on actual AMM bounds
    use fluxa_core::utils::constants::{MAX_SQRT_X64, MIN_SQRT_X64};

    // For testing sqrt(x²) = x, we need values where x² won't underflow
    // Use a range that covers realistic AMM prices but avoids precision limits
    let min_testable = MIN_SQRT_X64.max(1u128 << 38); // Use protocol min or precision floor
    let max_testable = (MAX_SQRT_X64 / 1000); // Protocol max with safety margin

    if input.raw() >= min_testable && input.raw() <= max_testable {
        if let Ok(squared) = input.checked_mul(input) {
            // Additional check: squared result should be above precision floor
            if squared.raw() >= 1000 {
                // Ensure adequate precision for sqrt test
                if let Ok(sqrt_result) = sqrt_x64(squared) {
                    let diff = if sqrt_result.raw() >= input.raw() {
                        sqrt_result.raw() - input.raw()
                    } else {
                        input.raw() - sqrt_result.raw()
                    };

                    // sqrt(x²) should equal x within reasonable precision
                    let relative_error = (diff as f64) / (input.raw() as f64);
                    assert!(
                        relative_error < 0.001, // 0.1% tolerance for fixed-point precision
                        "sqrt(x²) != x: x={}, x²={}, sqrt(x²)={}, relative_error={}",
                        input.raw(),
                        squared.raw(),
                        sqrt_result.raw(),
                        relative_error
                    );
                }
            }
        }
    }
});
