#![no_main]

use amm_core::math::*;
use libfuzzer_sys::fuzz_target;

// Fuzzing target for testing math helper functions.
//
// This target tests:
// - `div_by_sqrt_price_x64`: Division by square root price
// - `mul_by_sqrt_price_x64`: Multiplication by square root price
// - `sqrt_u128`: Square root calculation for u128 values
//
// For the square root function, it also verifies the correctness of the result
// by checking that the squared result is less than or equal to the input,
// and that the next integer's square is greater than the input (when possible).
fuzz_target!(|data: (u128, u128)| {
    let (value, sqrt_price) = data;

    // Only test with reasonable sqrt_price values
    let sqrt_price = sqrt_price.max(1);

    // Test div_by_sqrt_price_x64
    let _ = div_by_sqrt_price_x64(value, sqrt_price);

    // Test mul_by_sqrt_price_x64
    let _ = mul_by_sqrt_price_x64(value, sqrt_price);

    // Test sqrt_u128
    let sqrt_result = sqrt_u128(value);

    // Verify sqrt result (if it's not too large to square)
    if sqrt_result < (1u128 << 64) {
        let squared = sqrt_result * sqrt_result;
        assert!(squared <= value, "sqrt result too large when squared");
        if sqrt_result > 0 {
            let next_squared = (sqrt_result + 1) * (sqrt_result + 1);
            assert!(
                next_squared > value || next_squared < squared,
                "sqrt result too small"
            );
        }
    }
});
