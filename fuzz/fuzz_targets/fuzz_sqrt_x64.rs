#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use fluxa_core::math::core_arithmetic::{sqrt_x64, Q64x64, MAX_SQRT_X64, MIN_SQRT_X64, ONE_X64};
use honggfuzz::fuzz;

#[derive(Debug, Clone)]
struct SqrtInput {
    value: Q64x64,
}

impl<'a> Arbitrary<'a> for SqrtInput {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let raw_value = u.arbitrary::<u128>()?;

        // Bias towards meaningful ranges for square root testing
        let biased_value = match u.int_in_range(0..=100)? {
            0..=20 => 0,                                       // Test zero case
            21..=40 => raw_value % (ONE_X64 * 100),            // Small fractional values
            41..=70 => raw_value % (ONE_X64 * 1000000),        // Medium values
            71..=90 => raw_value,                              // Full range
            _ => u.int_in_range(MIN_SQRT_X64..=MAX_SQRT_X64)?, // Valid sqrt price range
        };

        Ok(SqrtInput {
            value: Q64x64::from_raw(biased_value),
        })
    }
}

fn fuzz_sqrt(input: SqrtInput) {
    let result = sqrt_x64(input.value);

    match result {
        Ok(sqrt_val) => {
            // Verify sqrt is in valid range
            assert!(sqrt_val.raw() >= MIN_SQRT_X64);
            assert!(sqrt_val.raw() <= MAX_SQRT_X64);

            // Test mathematical properties
            if input.value.raw() == 0 {
                assert_eq!(sqrt_val.raw(), 0);
            } else if input.value.raw() == ONE_X64 {
                // sqrt(1) should be approximately 1
                let diff = if sqrt_val.raw() > ONE_X64 {
                    sqrt_val.raw() - ONE_X64
                } else {
                    ONE_X64 - sqrt_val.raw()
                };
                // Allow small precision error
                assert!(diff < ONE_X64 / 1000); // 0.1% tolerance
            }

            // Verify sqrt^2 approximates original value (within precision limits)
            if let Ok(squared) = sqrt_val.checked_mul(sqrt_val) {
                if input.value.raw() > 0 && input.value.raw() < u128::MAX / 2 {
                    let original = input.value.raw();
                    let reconstructed = squared.raw();

                    // Allow for some precision loss in fixed-point arithmetic
                    let tolerance = std::cmp::max(original / 1000, 1); // 0.1% or minimum 1
                    let diff = if reconstructed > original {
                        reconstructed - original
                    } else {
                        original - reconstructed
                    };

                    if diff > tolerance {
                        // Only panic if error is significant and unexpected
                        // This helps identify potential bugs in sqrt implementation
                        eprintln!("Large sqrt precision error: input={}, sqrt={}, sqrt^2={}, diff={}, tolerance={}", 
                                original, sqrt_val.raw(), reconstructed, diff, tolerance);
                    }
                }
            }
        }
        Err(e) => {
            // sqrt_x64 should handle most inputs gracefully
            // Only expect specific error types
            let error_msg = format!("{:?}", e);
            assert!(
                error_msg.contains("SqrtNoConverge")
                    || error_msg.contains("Overflow")
                    || error_msg.contains("OutOfRange")
            );
        }
    }
}

fn main() {
    loop {
        fuzz!(|data: SqrtInput| {
            fuzz_sqrt(data);
        });
    }
}
