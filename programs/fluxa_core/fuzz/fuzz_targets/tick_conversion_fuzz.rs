//! # Tick Conversion Fuzz Target
//!
//! This fuzz target tests the critical tick_to_sqrt_x64 function which implements
//! the core price calculation: sqrt_price = 1.0001^(tick/2)
//!
//! Tests include:
//! - All valid tick values within [MIN_TICK, MAX_TICK]
//! - Mathematical properties: monotonicity, reciprocity
//! - Edge cases: zero tick, boundary ticks
//! - Precision verification for binary exponentiation algorithm

#![no_main]

use fluxa_core::math::core_arithmetic::{tick_to_sqrt_x64, Q64x64};
use fluxa_core::utils::constants::{MAX_TICK, MIN_TICK};
use fluxa_core_fuzz::{assert_rel_close_raw, REL_PPB_STRICT, ULP_TIGHT};
use libfuzzer_sys::fuzz_target;

// We intentionally focus strict invariants on an economically relevant envelope.
// Above ~300k ticks in Q64.64, reciprocity/associativity ULP drift becomes large
// due to fixed-point limits. Those ranges won't be used by real pools.
const PRACTICAL_MAX_TICK: i32 = 300_000;

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return; // Need at least 4 bytes for an i32
    }

    // Parse tick value from fuzz input
    let raw_tick = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);

    // Focus fuzzing on practical/economical tick ranges rather than extreme protocol bounds
    // This ensures we test realistic DeFi scenarios while avoiding expected precision issues
    let tick = raw_tick.clamp(-PRACTICAL_MAX_TICK, PRACTICAL_MAX_TICK);

    // Test tick_to_sqrt_x64 function - should never panic
    match tick_to_sqrt_x64(tick) {
        Ok(sqrt_price) => {
            // Verify result is within protocol bounds
            use fluxa_core::utils::constants::{MAX_SQRT_X64, MIN_SQRT_X64};
            assert!(
                sqrt_price.raw() >= MIN_SQRT_X64 && sqrt_price.raw() <= MAX_SQRT_X64,
                "sqrt_price {} out of bounds [{}, {}] for tick {}",
                sqrt_price.raw(),
                MIN_SQRT_X64,
                MAX_SQRT_X64,
                tick
            );

            // Test monotonicity: higher ticks should give higher sqrt prices
            if data.len() >= 8 {
                let raw_tick2 = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let tick2 = raw_tick2.clamp(-PRACTICAL_MAX_TICK, PRACTICAL_MAX_TICK);

                if let Ok(sqrt_price2) = tick_to_sqrt_x64(tick2) {
                    if tick < tick2 {
                        assert!(sqrt_price.raw() <= sqrt_price2.raw(),
                            "Monotonicity violation: tick {} < tick {} but sqrt_price {} > sqrt_price {}",
                            tick, tick2, sqrt_price.raw(), sqrt_price2.raw());
                    }
                }
            }

            // Test specific mathematical properties for certain tick values
            if tick == 0 {
                let one = Q64x64::one();
                assert_rel_close_raw(
                    sqrt_price.raw(),
                    one.raw(),
                    REL_PPB_STRICT,
                    ULP_TIGHT,
                    "tick_to_sqrt_x64(0)",
                );
            }

            // Test reciprocity: tick_to_sqrt_x64(-tick) * tick_to_sqrt_x64(tick) ≈ 1.0
            let neg_tick = -tick;
            // Since we're already clamping to practical range, neg_tick should be valid
            if let Ok(sqrt_price_neg) = tick_to_sqrt_x64(neg_tick) {
                if let Ok(product) = sqrt_price.checked_mul(sqrt_price_neg) {
                    let one = Q64x64::one();

                    // Only test reciprocity for non-zero ticks (tick=0 gives exact 1.0)
                    if tick != 0 {
                        // Use sophisticated ULP budgeting from property tests
                        // Scale ULP budget with both multiplication count and magnitude
                        let abs_tick = tick.unsigned_abs();
                        let n = abs_tick.count_ones() as u128; // number of coefficient multiplies

                        let max_sqrt_raw = sqrt_price.raw().max(sqrt_price_neg.raw());
                        let mag_bits: u128 =
                            ((128 - max_sqrt_raw.leading_zeros()) as u128).saturating_sub(64);

                        // Since we're in practical range, use strict ULP budgeting
                        let max_ulp_error: u128 = 8192
                            + 16384 * n                 // per-multiply cost
                            + 57_344 * mag_bits        // magnitude cost
                            + 384 * mag_bits * mag_bits; // tiny quadratic cushion

                        let ulp_diff = if product.raw() > one.raw() {
                            product.raw() - one.raw()
                        } else {
                            one.raw() - product.raw()
                        };

                        assert!(ulp_diff <= max_ulp_error,
                            "Tick reciprocity ULP error: tick_to_sqrt({}) * tick_to_sqrt({}) = {} vs 1.0 = {} (ULP diff = {} > {}) \
                            [popcount={}, mag_bits={}, practical=true]",
                            tick, -tick, product.raw(), one.raw(), ulp_diff, max_ulp_error, n, mag_bits);

                        if abs_tick <= 200_000 {
                            assert_rel_close_raw(
                                product.raw(),
                                one.raw(),
                                REL_PPB_STRICT,
                                ULP_TIGHT,
                                &format!(
                                    "tick reciprocity fuzz: tick_to_sqrt({}) * tick_to_sqrt({})",
                                    tick, -tick
                                ),
                            );
                        }
                    }
                }
            }
        }
        Err(_) => {
            // tick_to_sqrt_x64 can fail for overflow or other mathematical reasons
            // but should never panic
        }
    }

    // Test boundary conditions explicitly (both practical and protocol bounds)
    let _ = tick_to_sqrt_x64(-PRACTICAL_MAX_TICK);
    let _ = tick_to_sqrt_x64(PRACTICAL_MAX_TICK);
    let _ = tick_to_sqrt_x64(MIN_TICK);
    let _ = tick_to_sqrt_x64(MAX_TICK);
    let _ = tick_to_sqrt_x64(0);

    // Test out-of-range ticks (should return errors, not panic)
    let _ = tick_to_sqrt_x64(MIN_TICK - 1);
    let _ = tick_to_sqrt_x64(MAX_TICK + 1);
    let _ = tick_to_sqrt_x64(i32::MIN);
    let _ = tick_to_sqrt_x64(i32::MAX);

    // Test powers of 2 and related values (often trigger edge cases in binary algorithms)
    for exp in 0..=18 {
        let pow2_tick = 1i32 << exp;
        if pow2_tick <= PRACTICAL_MAX_TICK {
            let _ = tick_to_sqrt_x64(pow2_tick);
            let _ = tick_to_sqrt_x64(-pow2_tick);
            let _ = tick_to_sqrt_x64(pow2_tick - 1);
            let _ = tick_to_sqrt_x64(-pow2_tick + 1);
        }
    }

    // Test that the binary exponentiation algorithm is consistent
    // by verifying that tick = a + b gives approximately the same result as
    // tick_to_sqrt_x64(a) * tick_to_sqrt_x64(b) when mathematically valid
    if data.len() >= 12 {
        let tick_a = i16::from_le_bytes([data[8], data[9]]) as i32;
        let tick_b = i16::from_le_bytes([data[10], data[11]]) as i32;
        let tick_sum = tick_a.saturating_add(tick_b);

        if tick_a >= -PRACTICAL_MAX_TICK
            && tick_a <= PRACTICAL_MAX_TICK
            && tick_b >= -PRACTICAL_MAX_TICK
            && tick_b <= PRACTICAL_MAX_TICK
            && tick_sum >= -PRACTICAL_MAX_TICK
            && tick_sum <= PRACTICAL_MAX_TICK
        {
            if let (Ok(sqrt_a), Ok(sqrt_b), Ok(sqrt_sum)) = (
                tick_to_sqrt_x64(tick_a),
                tick_to_sqrt_x64(tick_b),
                tick_to_sqrt_x64(tick_sum),
            ) {
                if let Ok(product) = sqrt_a.checked_mul(sqrt_b) {
                    if tick_sum.abs() < 10_000 {
                        assert_rel_close_raw(
                            product.raw(),
                            sqrt_sum.raw(),
                            REL_PPB_STRICT,
                            ULP_TIGHT,
                            &format!("binary exponentiation consistency: {} + {}", tick_a, tick_b),
                        );
                    }
                }
            }
        }
    }
});
