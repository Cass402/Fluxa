#![no_main]

use amm_core::math::*;
use libfuzzer_sys::fuzz_target;

// Fuzz test for Q96 arithmetic operations.
//
// This test generates random pairs of u128 values, constrains them to the Q96 range,
// and tests various Q96 arithmetic operations:
// - addition
// - subtraction (when a >= b)
// - multiplication
// - division (when b > 0)
// - square
// - square root
// - reciprocal (when a > 0)
// - sqrt price format conversions
fuzz_target!(|data: (u128, u128)| {
    let (a, b) = data;

    // Ensure values are in a reasonable range for Q64.96
    let a = a % (1 << 96);
    let b = b % (1 << 96);

    // Test addition
    let _ = add_q96(a, b);

    // Test subtraction (only if a >= b to avoid expected errors)
    if a >= b {
        let _ = sub_q96(a, b);
    }

    // Test multiplication
    let _ = mul_q96(a, b);

    // Test division (avoid division by zero)
    if b > 0 {
        let _ = div_q96(a, b);
    }

    // Test square
    let _ = square_q96(a);

    // Test sqrt
    let _ = sqrt_q96(a);

    // Test reciprocal (avoid division by zero)
    if a > 0 {
        let _ = reciprocal_q96(a);
    }

    // Test format conversions
    let _ = convert_sqrt_price_to_q96(a);
    if a > 0 {
        let _ = convert_sqrt_price_from_q96(a);
    }
});
