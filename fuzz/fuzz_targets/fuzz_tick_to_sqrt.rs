#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use fluxa_core::math::core_arithmetic::{
    tick_to_sqrt_x64, MAX_SQRT_X64, MAX_TICK, MIN_SQRT_X64, MIN_TICK,
};
use honggfuzz::fuzz;

#[derive(Debug, Clone)]
struct TickInput {
    tick: i32,
}

impl<'a> Arbitrary<'a> for TickInput {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let tick = match u.int_in_range(0..=100)? {
            0..=80 => {
                // Most cases: valid tick range
                u.int_in_range(MIN_TICK..=MAX_TICK)?
            }
            81..=90 => {
                // Edge cases: boundary values
                *u.choose(&[MIN_TICK, MAX_TICK, 0, -1, 1])?
            }
            _ => {
                // Invalid range testing
                u.arbitrary::<i32>()?
            }
        };

        Ok(TickInput { tick })
    }
}

fn fuzz_tick_to_sqrt(input: TickInput) {
    let result = tick_to_sqrt_x64(input.tick);

    match result {
        Ok(sqrt_price) => {
            // Verify tick was in valid range
            assert!(input.tick >= MIN_TICK && input.tick <= MAX_TICK);

            // Verify output is in valid sqrt price range
            assert!(sqrt_price.raw() >= MIN_SQRT_X64);
            assert!(sqrt_price.raw() <= MAX_SQRT_X64);

            // Test monotonicity: higher tick should give higher sqrt price
            // (except at boundaries due to clamping)
            if input.tick < MAX_TICK {
                if let Ok(next_sqrt) = tick_to_sqrt_x64(input.tick + 1) {
                    // Due to clamping, we might not always have strict monotonicity
                    // but it should never decrease significantly
                    assert!(
                        next_sqrt.raw() >= sqrt_price.raw()
                            || (sqrt_price.raw() - next_sqrt.raw()) < sqrt_price.raw() / 1000000
                    );
                }
            }

            // Test special cases
            if input.tick == 0 {
                // Tick 0 should give sqrt price close to 1.0 in Q64.64
                // Allow some tolerance for the actual implementation
                let one_q64 = 1u128 << 64;
                let diff = if sqrt_price.raw() > one_q64 {
                    sqrt_price.raw() - one_q64
                } else {
                    one_q64 - sqrt_price.raw()
                };
                // Should be reasonably close to 1.0
                assert!(diff < one_q64 / 100); // Within 1%
            }
        }
        Err(e) => {
            // Should only fail for out-of-range ticks
            let error_msg = format!("{:?}", e);
            assert!(error_msg.contains("OutOfRange"));
            assert!(input.tick < MIN_TICK || input.tick > MAX_TICK);
        }
    }
}

fn main() {
    loop {
        fuzz!(|data: TickInput| {
            fuzz_tick_to_sqrt(data);
        });
    }
}
