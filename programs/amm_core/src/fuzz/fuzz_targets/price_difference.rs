#![no_main]

use amm_core::math::*;
use libfuzzer_sys::fuzz_target;

/* Fuzzing target for the enhanced_price_difference_q96 function.
 *
 * This test:
 * 1. Takes two u128 values as input and constrains them to be within the Q96 range
 * 2. Calls enhanced_price_difference_q96 with these values
 * 3. Verifies that the result matches the expected absolute difference between the prices
 */
fuzz_target!(|data: (u128, u128)| {
    let (price_a, price_b) = data;

    // Keep values in reasonable range
    let price_a = price_a % (1 << 96);
    let price_b = price_b % (1 << 96);

    if let Ok(diff) = enhanced_price_difference_q96(price_a, price_b) {
        // Verify result
        if price_a >= price_b {
            assert_eq!(diff, price_a - price_b);
        } else {
            assert_eq!(diff, price_b - price_a);
        }
    }
});
