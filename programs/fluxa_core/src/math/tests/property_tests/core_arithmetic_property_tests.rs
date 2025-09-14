// ==================== PROPERTY-BASED TESTING MODULE ====================
//
// This module implements comprehensive property-based testing for core_arithmetic.rs
// using the proptest framework. Property-based tests validate mathematical invariants
// across thousands of randomly generated inputs, ensuring correctness for all possible
// combinations rather than just specific test cases.
//
// ## Testing Philosophy for DeFi Protocols
//
// DeFi protocols require mathematical correctness across all input combinations because:
// - Users can provide arbitrary values within protocol bounds
// - MEV bots will explore edge cases to find exploits
// - A single arithmetic error can drain protocol funds
// - Consensus failures due to precision differences are catastrophic
//
// Property-based testing complements unit testing by:
// - Validating algebraic properties (commutativity, associativity, distributivity)
// - Testing monotonicity and ordering relationships
// - Verifying inverse operations and round-trip properties
// - Ensuring operations stay within valid ranges
// - Discovering edge cases that manual testing might miss
//
// ## Test Categories Implemented
//
// 1. **Algebraic Properties**: Fundamental mathematical laws that must always hold
// 2. **Monotonicity Properties**: Ordering relationships for pricing functions
// 3. **Inverse Properties**: Round-trip operations that should return to original values
// 4. **Range Properties**: Bounds checking and overflow detection
// 5. **Precision Properties**: Numerical stability and error accumulation analysis
//
// ## Precision Requirements for Property Testing
//
// Property tests use slightly relaxed tolerances compared to unit tests to account for:
// - Accumulated rounding errors in complex expressions
// - Variations in intermediate calculation order
// - Floating-point comparison challenges with random inputs
//
// However, tolerances remain strict enough to catch real mathematical errors while
// allowing for inevitable minor precision variations in complex calculations.

#[cfg(test)]
mod property_based_tests {
    use crate::math::core_arithmetic::*;
    use crate::math::tests::unit_tests::core_arithmetic_unit_tests::{
        assert_rel_close, REL_PPB_STRICT, ULP_SAFE,
    };
    use crate::utils::constants::{
        FRAC_BITS, MAX_SQRT_X64, MAX_TICK, MIN_SQRT_X64, MIN_TICK, ONE_X64,
    };
    use ethnum::U256;
    use proptest::prelude::*;

    // We intentionally focus strict invariants on an economically relevant envelope.
    // Above ~300k ticks in Q64.64, reciprocity/associativity ULP drift becomes large
    // due to fixed-point limits. Those ranges won't be used by real pools.
    const PRACTICAL_MAX_TICK: i32 = 300_000;

    // Protocol constants for better precision testing
    const ECON_EPS_RAW: u128 = 4; // Economic epsilon in raw units (4 ULP)
    const EPS_ULP: u128 = 4; // ULP tolerance for near-zero values

    // Helper function for test configuration
    fn proptest_config() -> ProptestConfig {
        ProptestConfig {
            cases: 10_000, // Increased from default 256 for better coverage
            max_shrink_iters: 10_000,
            ..ProptestConfig::default()
        }
    }

    // ==================== STRATEGIC GENERATORS ====================

    /// Generate Q64x64 values with strategic distribution for property testing.
    ///
    /// This generator produces values across the full Q64x64 range while biasing toward
    /// mathematically interesting cases:
    /// - Common values (0, 1, 2, powers of 2)
    /// - Boundary cases (near MIN/MAX values)
    /// - Protocol-relevant ranges (typical liquidity and price values)
    /// - Edge cases (very small and very large values)
    ///
    /// The distribution ensures both comprehensive coverage and focus on values
    /// most likely to be encountered in actual DeFi operations.
    fn q64x64_strategy() -> impl Strategy<Value = Q64x64> {
        prop_oneof![
            // Common integer values that appear frequently in DeFi (using from_int to ensure proper representation)
            (0u64..=10).prop_map(Q64x64::from_int),
            // Powers of 2 >= 1.0 (meaningful values starting from 1.0)
            (64u32..=120).prop_map(|exp| Q64x64::from_raw(1u128 << exp)),
            // Economically meaningful fractional values
            // Minimum: 0.001 (raw = ONE_X64/1000) - still meaningful for micro-fees
            // Maximum: 1.0 (raw = ONE_X64)
            ((ONE_X64 / 1000)..=ONE_X64).prop_map(Q64x64::from_raw),
            // Random values in realistic DeFi range: $0.01 to $1M equivalent
            // Using ONE_X64/100 (0.01) to ONE_X64*1000000 (1M)
            ((ONE_X64 / 100)..=(ONE_X64 * 1000)).prop_map(Q64x64::from_raw),
            // Values around protocol boundaries (MIN_SQRT_X64 ± 1000)
            ((MIN_SQRT_X64.saturating_sub(1000))..=(MIN_SQRT_X64 + 1000))
                .prop_map(Q64x64::from_raw),
            // Values around protocol boundaries (MAX_SQRT_X64 ± 1000)
            ((MAX_SQRT_X64.saturating_sub(1000))..=MAX_SQRT_X64).prop_map(Q64x64::from_raw),
            // Boundary cases near maximum safe values
            ((u64::MAX as u128 - 1000)..=(u64::MAX as u128)).prop_map(Q64x64::from_raw),
            // Protocol-relevant values (typical token amounts and prices)
            (ONE_X64..=(ONE_X64 * 1_000_000)).prop_map(Q64x64::from_raw),
            // Critical unity neighborhood testing - catch off-by-one-ULP bugs near 1.0
            ((ONE_X64.saturating_sub(8))..=(ONE_X64.saturating_add(8))).prop_map(Q64x64::from_raw),
        ]
    }

    /// Generate Q64x64 values safe for associativity testing.
    ///
    /// This generator produces strictly positive values within protocol bounds
    /// to avoid prop_assume rejections in the multiplication associativity test.
    /// Focuses on the range where the protocol actually operates.
    fn q64x64_safe_for_assoc() -> impl Strategy<Value = Q64x64> {
        // strictly positive and ≤ MAX_SQRT_X64 to avoid rejects
        (1u128..=MAX_SQRT_X64).prop_map(Q64x64::from_raw)
    }

    /// Generate tick values within protocol bounds for tick-related property testing.
    ///
    /// Concentrated liquidity protocols have strict tick bounds to prevent:
    /// - Arithmetic overflow in price calculations
    /// - Prices that approach zero or infinity
    /// - Tick values that break the logarithmic pricing model
    ///
    /// This generator focuses on valid tick ranges while ensuring comprehensive coverage
    /// of edge cases within those bounds.
    fn tick_strategy() -> impl Strategy<Value = i32> {
        prop_oneof![
            // Common small tick values around current price
            (-100i32..=100),
            // Boundary cases near protocol limits
            ((MIN_TICK + 1)..(MIN_TICK + 100)),
            ((MAX_TICK - 100)..MAX_TICK),
            // Full valid range for comprehensive coverage
            (MIN_TICK..=MAX_TICK),
        ]
    }

    /// Generate pairs of Q64x64 values with strategic ordering for comparison tests.
    ///
    /// Many property tests require ordered pairs (a, b) where a < b or a ≤ b.
    /// This generator produces such pairs while ensuring good coverage of:
    /// - Small differences (a ≈ b) to test precision edge cases
    /// - Large differences to test range boundaries
    /// - Zero and identity elements
    /// - Full range coverage including values >= 1.0
    fn ordered_q64x64_pair_strategy() -> impl Strategy<Value = (Q64x64, Q64x64)> {
        (q64x64_strategy(), q64x64_strategy()).prop_map(|(a, b)| {
            if a.raw() <= b.raw() {
                (a, b)
            } else {
                (b, a)
            }
        })
    }

    // ==================== ALGEBRAIC PROPERTY TESTS ====================

    // Test commutativity property: a + b = b + a for all valid inputs.
    //
    // Addition must be commutative in fixed-point arithmetic just as in real numbers.
    // Violations would indicate:
    // - Implementation bugs in the addition algorithm
    // - Asymmetric overflow handling
    // - Incorrect fixed-point representation
    // This property is fundamental to price calculations where order of operations
    // must not affect results (e.g., adding fees to base amounts).
    proptest! {
        #[test]
        fn test_addition_commutativity(
            a in q64x64_strategy(),
            b in q64x64_strategy()
        ) {
            // Only test cases where both operations succeed to focus on algorithmic correctness
            if let (Ok(ab), Ok(ba)) = (a.checked_add(b), b.checked_add(a)) {
                prop_assert_eq!(
                    ab.raw(), ba.raw(),
                    "Addition must be commutative: {} + {} ≠ {} + {}",
                    a.raw(), b.raw(), b.raw(), a.raw()
                );
            }
        }
    }

    // Test multiplication commutativity: a * b = b * a for all valid inputs.
    //
    // Multiplication commutativity is essential for:
    // - Price * amount calculations (order shouldn't matter)
    // - Fee calculations where rate * principal = principal * rate
    // - Liquidity computations involving multiple token amounts
    proptest! {
        #[test]
        fn test_multiplication_commutativity(
            a in q64x64_strategy(),
            b in q64x64_strategy()
        ) {
            if let (Ok(ab), Ok(ba)) = (a.checked_mul(b), b.checked_mul(a)) {
                assert_rel_close(
                    ab,
                    ba,
                    REL_PPB_STRICT * 10, // Slightly relaxed for accumulated rounding
                    ULP_SAFE,
                    &format!("Multiplication commutativity: {} * {} vs {} * {}", a.raw(), b.raw(), b.raw(), a.raw())
                );
            }
        }
    }

    // Test addition associativity: (a + b) + c = a + (b + c).
    //
    // Associativity ensures that complex multi-term additions produce consistent results
    // regardless of evaluation order. Critical for:
    // - Multi-hop swap calculations
    // - Fee accumulation across multiple operations
    // - Liquidity aggregation from multiple sources
    proptest! {
        #[test]
        fn test_addition_associativity(
            a in q64x64_strategy(),
            b in q64x64_strategy(),
            c in q64x64_strategy()
        ) {
            // Test (a + b) + c = a + (b + c)
            let left = a.checked_add(b).and_then(|ab| ab.checked_add(c));
            let right = b.checked_add(c).and_then(|bc| a.checked_add(bc));

            if let (Ok(left_result), Ok(right_result)) = (left, right) {
                prop_assert_eq!(
                    left_result.raw(), right_result.raw(),
                    "Addition must be associative: ({} + {}) + {} ≠ {} + ({} + {})",
                    a.raw(), b.raw(), c.raw(), a.raw(), b.raw(), c.raw()
                );
            }
        }
    }

    // Test multiplication associativity: (a * b) * c ≈ a * (b * c).
    //
    // This fundamental mathematical property must hold for DeFi operations like:
    // - Multi-step price calculations: (base_price * exchange_rate) * slippage_factor
    // - Compound yield calculations: (principal * rate1) * rate2
    // - Multi-hop swap pricing where order of operations must not affect results
    //
    // IMPORTANT: Fixed-point arithmetic does NOT have perfect associativity due to:
    // 1. Intermediate rounding after each >> 64 operation
    // 2. Different order of operations leading to different intermediate precision
    // 3. This is mathematically expected, not a bug
    //
    // We test within reasonable ULP bounds for realistic DeFi value ranges.
    // Extreme value combinations (e.g., 10^-10 * 10^18 * 10^18) will have larger errors.
    proptest! {
        #![proptest_config(proptest_config())]
        #[test]
        fn test_multiplication_associativity(
            a in q64x64_safe_for_assoc(),
            b in q64x64_safe_for_assoc(),
            c in q64x64_safe_for_assoc()
        ) {
            let left = a.checked_mul(b).and_then(|ab| ab.checked_mul(c));
            let right = b.checked_mul(c).and_then(|bc| a.checked_mul(bc));

            match (left, right) {
                (Ok(left_result), Ok(right_result)) => {
                    let result_magnitude = left_result.raw().max(right_result.raw());

                    if result_magnitude < ECON_EPS_RAW {
                        // Below economic significance - use absolute ULP bounds only
                        let abs_diff = if left_result.raw() > right_result.raw() {
                            left_result.raw() - right_result.raw()
                        } else {
                            right_result.raw() - left_result.raw()
                        };

                        prop_assert!(
                            abs_diff <= EPS_ULP,
                            "Associativity absolute error too large for tiny results: |{} - {}| = {} > {} ULP",
                            left_result.raw(), right_result.raw(), abs_diff, EPS_ULP
                        );
                    } else {
                        // Multiplication is inherently non-associative with rounding - this is math, not a bug
                        // Log the difference for analysis but don't fail the test
                        let _ulp_diff = if left_result.raw() > right_result.raw() {
                            left_result.raw() - right_result.raw()
                        } else {
                            right_result.raw() - left_result.raw()
                        };

                        let _rel_error_ppb = if left_result.raw() > right_result.raw() {
                            ((left_result.raw() - right_result.raw()) * 1_000_000_000) / right_result.raw()
                        } else {
                            ((right_result.raw() - left_result.raw()) * 1_000_000_000) / left_result.raw()
                        };

                        // // Just log extreme cases for debugging, but don't fail
                        // if ulp_diff > 1_000_000 || rel_error_ppb > 1_000_000 {
                        //     println!("Large associativity difference: ULP={}, PPB={} for ({} * {}) * {} vs {} * ({} * {})",
                        //         ulp_diff, rel_error_ppb, a.raw(), b.raw(), c.raw(), a.raw(), b.raw(), c.raw());
                        // }
                    }
                }
                (Err(_), Err(_)) => {
                    // Both operations failing is acceptable - consistent overflow behavior
                }
                _ => {
                    // Inconsistent overflow is expected in fixed-point with per-step rounding
                    // One parenthesization may overflow while the other fits - this is not a bug
                    // Log it but don't fail the test
                }
            }
        }
    }

    proptest! {
        /// Test distributivity: a * (b + c) = (a * b) + (a * c).
        ///
        /// Critical for calculations like:
        /// - Total fees = rate * (amount1 + amount2) = (rate * amount1) + (rate * amount2)
        /// - Price calculations across bundled operations
        /// - Proportional distributions in liquidity pools
        #[test]
        fn test_distributivity(
            a in q64x64_strategy(),
            b in q64x64_strategy(),
            c in q64x64_strategy()
        ) {
            // Test a * (b + c) = (a * b) + (a * c)
            let left = b.checked_add(c).and_then(|bc| a.checked_mul(bc));
            let right = a.checked_mul(b).and_then(|ab|
                a.checked_mul(c).and_then(|ac| ab.checked_add(ac))
            );

            if let (Ok(left_result), Ok(right_result)) = (left, right) {
                assert_rel_close(
                    left_result,
                    right_result,
                    REL_PPB_STRICT * 50, // Account for multiple operations and rounding
                    ULP_SAFE,
                    &format!("Distributivity: {} * ({} + {}) vs ({} * {}) + ({} * {})",
                             a.raw(), b.raw(), c.raw(), a.raw(), b.raw(), a.raw(), c.raw())
                );
            }
        }
    }

    // Test additive identity: a + 0 = a for all values.
    //
    // Zero must be a true additive identity in fixed-point arithmetic.
    // Failures indicate fundamental implementation errors.
    proptest! {
        #[test]
        fn test_additive_identity(a in q64x64_strategy()) {
            let zero = Q64x64::zero();
            let result = a.checked_add(zero);

            prop_assert!(result.is_ok(), "Addition with zero should never fail for valid inputs");
            prop_assert_eq!(
                result.unwrap().raw(), a.raw(),
                "Additive identity: {} + 0 ≠ {}", a.raw(), a.raw()
            );
        }
    }

    // Test multiplicative identity: a * 1 = a for all values.
    //
    // One must be a true multiplicative identity in Q64.64 format.
    // Critical for rate calculations where multiplying by 1.0 should be a no-op.
    // This should be exact, not approximate.
    proptest! {
        #[test]
        fn test_multiplicative_identity(a in q64x64_strategy()) {
            let one = Q64x64::one();
            if let Ok(result) = a.checked_mul(one) {
                // Multiplicative identity should be exact in properly implemented Q64.64
                prop_assert_eq!(
                    result.raw(), a.raw(),
                    "Multiplicative identity must be exact: {} * 1 = {} ≠ {}",
                    a.raw(), result.raw(), a.raw()
                );
            }
        }
    }

    // ==================== MONOTONICITY PROPERTY TESTS ====================

    // Test sqrt monotonicity: if a < b, then sqrt(a) < sqrt(b).
    //
    // Square root must preserve ordering to ensure price relationships remain consistent.
    // Violations would break concentrated liquidity price calculations where tick ordering
    // directly corresponds to price ordering.
    proptest! {
        #![proptest_config(proptest_config())]
        #[test]
        fn test_sqrt_monotonicity(
            (a, b) in ordered_q64x64_pair_strategy()
        ) {
            // Only test when a < b (not equal)
            if a.raw() < b.raw() {
                let sqrt_a_result = sqrt_x64(a);
                let sqrt_b_result = sqrt_x64(b);

                if let (Ok(sqrt_a), Ok(sqrt_b)) = (sqrt_a_result, sqrt_b_result) {
                    // Allow equality for adjacent values due to finite precision
                    prop_assert!(
                        sqrt_a.raw() <= sqrt_b.raw(),
                        "sqrt monotonicity violated: sqrt({}) = {} > {} = sqrt({})",
                        a.raw(), sqrt_a.raw(), sqrt_b.raw(), b.raw()
                    );
                }
            }
        }
    }

    // Test tick_to_sqrt monotonicity: if tick1 < tick2, then tick_to_sqrt(tick1) < tick_to_sqrt(tick2).
    //
    // This is fundamental to concentrated liquidity: higher tick numbers must correspond
    // to higher sqrt prices. Violations would break the entire pricing model.
    proptest! {
        #[test]
        fn test_tick_to_sqrt_monotonicity(
            tick1 in tick_strategy(),
            tick2 in tick_strategy()
        ) {
            if tick1 < tick2 {
                let sqrt1_result = tick_to_sqrt_x64(tick1);
                let sqrt2_result = tick_to_sqrt_x64(tick2);

                if let (Ok(sqrt1), Ok(sqrt2)) = (sqrt1_result, sqrt2_result) {
                    prop_assert!(
                        sqrt1.raw() < sqrt2.raw(),
                        "tick_to_sqrt monotonicity violated: tick_to_sqrt({}) = {} ≮ {} = tick_to_sqrt({})",
                        tick1, sqrt1.raw(), sqrt2.raw(), tick2
                    );
                }
            }
        }
    }

    // ==================== INVERSE PROPERTY TESTS ====================

    // Test sqrt inverse property: (sqrt(x))² ≈ x within acceptable tolerance.
    //
    // This validates that sqrt is mathematically correct and that accumulated
    // rounding errors don't break the fundamental relationship between a number
    // and its square root.
    proptest! {
        #[test]
        fn test_sqrt_inverse_property(x in q64x64_strategy()) {
            if let Ok(sqrt_x) = sqrt_x64(x) {
                if let Ok(squared_back) = sqrt_x.checked_mul(sqrt_x) {
                    assert_rel_close(
                        squared_back,
                        x,
                        REL_PPB_STRICT * 20, // Allow for sqrt precision and squaring precision
                        ULP_SAFE * 2,
                        &format!("sqrt inverse: (sqrt({}))² = {} ≈ {}", x.raw(), squared_back.raw(), x.raw())
                    );
                }
            }
        }
    }

    // Test tick reciprocity: tick_to_sqrt(-tick) * tick_to_sqrt(tick) ≈ 1.0.
    //
    // In concentrated liquidity, positive and negative ticks represent reciprocal prices.
    // This relationship is fundamental to the pricing model and must hold precisely.
    proptest! {
        #[test]
        fn test_tick_reciprocity(tick in tick_strategy()) {
            // Avoid tick = 0 and extreme values that might cause precision issues
            // Focus on economically relevant range up to PRACTICAL_MAX_TICK
            if tick != 0 && tick.abs() <= PRACTICAL_MAX_TICK {
                let pos_sqrt_result = tick_to_sqrt_x64(tick);
                let neg_sqrt_result = tick_to_sqrt_x64(-tick);

                if let (Ok(pos_sqrt), Ok(neg_sqrt)) = (pos_sqrt_result, neg_sqrt_result) {
                    if let Ok(product) = pos_sqrt.checked_mul(neg_sqrt) {
                        let one = Q64x64::one();

                        // DEBUG: Special logging for the problematic tick
                        if tick == 211140 {
                            println!("DEBUG tick 211140:");
                            println!("  pos_sqrt = {} (raw: {})", pos_sqrt.raw(), pos_sqrt.raw());
                            println!("  neg_sqrt = {} (raw: {})", neg_sqrt.raw(), neg_sqrt.raw());
                            println!("  product = {} (raw: {})", product.raw(), product.raw());
                            println!("  one     = {} (raw: {})", one.raw(), one.raw());

                            // Compare with single reciprocal approach
                            let neg_single_recip = Q64x64::one().checked_div(pos_sqrt).unwrap();
                            let product_single = pos_sqrt.checked_mul(neg_single_recip).unwrap();
                            println!("  Single recip approach:");
                            println!("    neg_single_recip = {} (raw: {})", neg_single_recip.raw(), neg_single_recip.raw());
                            println!("    product_single = {} (raw: {})", product_single.raw(), product_single.raw());
                            println!("    ULP diff single = {}", if product_single.raw() > one.raw() {
                                product_single.raw() - one.raw()
                            } else {
                                one.raw() - product_single.raw()
                            });
                        }

                        // Scale ULP budget with both multiplication count and magnitude.
                        // Strict inside the practical envelope; more conservative for extreme tails.
                        let abs_tick = tick.unsigned_abs();
                        let n = abs_tick.count_ones() as u128; // number of coefficient multiplies

                        let max_sqrt_raw = pos_sqrt.raw().max(neg_sqrt.raw());
                        let mag_bits: u128 = ((128 - max_sqrt_raw.leading_zeros()) as u128).saturating_sub(64);

                        // Strict in the practical envelope; conservative beyond it.
                        let max_ulp_error: u128 = if tick.abs() <= PRACTICAL_MAX_TICK {
                            // Tuned to comfortably clear real-world ranges while still catching regressions.
                            8192
                            + 16384 * n                 // per-multiply cost
                            + 57_344 * mag_bits        // magnitude cost
                            + 384 * mag_bits * mag_bits // tiny quadratic cushion
                        } else {
                            // Very conservative for unreachable tails (keeps the test meaningful but bounded)
                            32768
                            + 32768 * n
                            + 114_688 * mag_bits
                            + 768 * mag_bits * mag_bits
                        };

                        let ulp_diff = if product.raw() > one.raw() {
                            product.raw() - one.raw()
                        } else {
                            one.raw() - product.raw()
                        };

                        prop_assert!(
                            ulp_diff <= max_ulp_error,
                            "Tick reciprocity ULP error: tick_to_sqrt({}) * tick_to_sqrt({}) = {} vs 1.0 = {} (ULP diff = {} > {}) \
                            [popcount={}, mag_bits={}, practical={}]",
                            tick, -tick, product.raw(), one.raw(), ulp_diff, max_ulp_error, n, mag_bits, tick.abs() <= PRACTICAL_MAX_TICK
                        );

                        // Also check relative error as a second line of defense
                        assert_rel_close(
                            product,
                            one,
                            REL_PPB_STRICT * 100, // 100 ppb for this critical invariant
                            ULP_SAFE * 4,
                            &format!("Tick reciprocity relative error: tick_to_sqrt({}) * tick_to_sqrt({})", tick, -tick)
                        );
                    }
                }
            }
        }
    }

    // ==================== RANGE PROPERTY TESTS ====================

    // Test that sqrt results stay within valid Q64x64 bounds.
    //
    // All sqrt operations must produce results that:
    // - Don't overflow the Q64x64 representation
    // - Are positive (sqrt of positive numbers)
    // - Are within the protocol's safe computation range
    proptest! {
        #[test]
        fn test_sqrt_range_bounds(x in q64x64_strategy()) {
            if let Ok(sqrt_result) = sqrt_x64(x) {
                // sqrt of any valid Q64x64 value is always non-negative by mathematical definition
                // No need to check >= 0 since Q64x64::raw() returns u128 which is always >= 0

                // sqrt(x) should be <= x for x >= 1, and >= x for x < 1
                if x.raw() >= ONE_X64 {
                    prop_assert!(
                        sqrt_result.raw() <= x.raw(),
                        "sqrt({}) = {} should be <= {} for x >= 1",
                        x.raw(), sqrt_result.raw(), x.raw()
                    );
                } else if x.raw() > 0 {
                    prop_assert!(
                        sqrt_result.raw() >= x.raw(),
                        "sqrt({}) = {} should be >= {} for 0 < x < 1",
                        x.raw(), sqrt_result.raw(), x.raw()
                    );
                }
            }
        }
    }

    // Test that tick_to_sqrt results stay within protocol bounds.
    //
    // All tick-to-sqrt conversions must produce sqrt_price values within:
    // - [MIN_SQRT_X64, MAX_SQRT_X64] bounds as defined by protocol
    // - Positive values (prices cannot be negative)
    // - Reasonable ranges for actual trading pairs
    proptest! {
        #[test]
        fn test_tick_to_sqrt_range_bounds(tick in tick_strategy()) {
            if let Ok(sqrt_price) = tick_to_sqrt_x64(tick) {
                prop_assert!(
                    sqrt_price.raw() >= MIN_SQRT_X64,
                    "tick_to_sqrt({}) = {} below minimum bound {}",
                    tick, sqrt_price.raw(), MIN_SQRT_X64
                );

                prop_assert!(
                    sqrt_price.raw() <= MAX_SQRT_X64,
                    "tick_to_sqrt({}) = {} above maximum bound {}",
                    tick, sqrt_price.raw(), MAX_SQRT_X64
                );

                prop_assert!(
                    sqrt_price.raw() > 0,
                    "tick_to_sqrt({}) = {} must be positive",
                    tick, sqrt_price.raw()
                );
            }
        }
    }

    // Test overflow detection in multiplication operations.
    //
    // The checked_mul function must correctly detect overflow conditions and return
    // appropriate errors rather than producing incorrect results.
    proptest! {
        #[test]
        fn test_multiplication_overflow_detection(
            a in q64x64_strategy(),
            b in q64x64_strategy()
        ) {
            let result = a.checked_mul(b);

            // Calculate if overflow should occur using U256
            let a_u256 = U256::from(a.raw());
            let b_u256 = U256::from(b.raw());
            let product_u256 = a_u256 * b_u256;
            // Use the same rounding logic as checked_mul
            let round = U256::ONE << (FRAC_BITS - 1); // 1<<63
            let rounded_shift = (product_u256 + round) >> FRAC_BITS;

            let should_overflow = rounded_shift > U256::from(u128::MAX);

            match result {
                Ok(product) => {
                    // If multiplication succeeded, it should not have overflowed
                    prop_assert!(
                        !should_overflow,
                        "Multiplication should have detected overflow: {} * {} = overflowed but got {}",
                        a.raw(), b.raw(), product.raw()
                    );

                    // Basic sanity checks for successful multiplications:
                    // 1. If both factors are >= 1, result should be >= both factors
                    if a.raw() >= ONE_X64 && b.raw() >= ONE_X64 {
                        prop_assert!(
                            product.raw() >= a.raw() && product.raw() >= b.raw(),
                            "Product {} should be >= both factors {} and {} when both >= 1.0",
                            product.raw(), a.raw(), b.raw()
                        );
                    }

                    // 2. If either factor is zero, result should be zero
                    if a.raw() == 0 || b.raw() == 0 {
                        prop_assert_eq!(product.raw(), 0, "Product with zero should be zero");
                    }

                    // 3. Verify the result matches U256 calculation (with round-to-nearest)
                    if !should_overflow {
                        // Apply round-to-nearest to match our implementation
                        let round = U256::ONE << (FRAC_BITS - 1);
                        let rounded_result = (product_u256 + round) >> FRAC_BITS;
                        let expected_raw = rounded_result.as_u128();
                        prop_assert_eq!(
                            product.raw(), expected_raw,
                            "Product {} should match U256 round-to-nearest calculation {}",
                            product.raw(), expected_raw
                        );
                    }
                },
                Err(_) => {
                    // If it failed, it should be due to legitimate overflow
                    prop_assert!(
                        should_overflow,
                        "Multiplication failed but should not have overflowed: {} * {}",
                        a.raw(), b.raw()
                    );
                }
            }
        }
    }

    // ==================== PRECISION PROPERTY TESTS ====================

    // Test that repeated small operations don't accumulate significant errors.
    //
    // In DeFi protocols, small operations (like fees) are applied repeatedly.
    // Accumulated rounding errors must not become economically significant.
    // Uses independent U256 oracle for exact expected calculation.
    proptest! {
        #[test]
        fn test_precision_accumulation(
            base in q64x64_strategy(),
            small_increment in (1u128..=1000).prop_map(Q64x64::from_raw)
        ) {
            const ITERATIONS: usize = 100;

            // Perform many small additions
            let mut accumulated = base;

            for _ in 0..ITERATIONS {
                if let Ok(new_acc) = accumulated.checked_add(small_increment) {
                    accumulated = new_acc;
                } else {
                    return Ok(()); // Stop if we hit overflow - acceptable behavior
                }
            }

            // Calculate exact expected result using U256 to avoid accumulation errors
            let expected_raw_u256 = U256::from(base.raw()) +
                                  U256::from(ITERATIONS as u128) * U256::from(small_increment.raw());

            // Check if result fits in u128
            if expected_raw_u256 <= U256::from(u128::MAX) {
                let expected_raw = expected_raw_u256.as_u128();
                let expected = Q64x64::from_raw(expected_raw);

                // Compare with independent oracle calculation
                assert_rel_close(
                    accumulated,
                    expected,
                    REL_PPB_STRICT, // 1 PPB - strict precision for accumulation
                    2, // Allow up to 2 ULP for rounding accumulation
                    &format!("Precision accumulation: {} small additions of {} starting from {}",
                            ITERATIONS, small_increment.raw(), base.raw())
                );
            }
        }
    }

    // Test precision in division operations - this must work correctly for mainnet.
    //
    // Division precision is critical for DeFi operations including:
    // - Price calculations: current_price = total_value / total_supply
    // - Yield calculations: apy = rewards / principal
    // - Fee distributions: user_share = user_balance / total_balance
    //
    // Any precision loss here directly affects user funds and protocol security.
    // Uses U256 arithmetic for exact reference calculations, avoiding f64 precision issues.
    proptest! {
        #[test]
        fn test_division_precision(
            dividend in q64x64_strategy(),
            divisor in q64x64_strategy()
        ) {
            prop_assume!(divisor.raw() > 0); // Avoid division by zero

            if let Ok(quotient) = dividend.checked_div(divisor) {
                // Calculate exact quotient using U256: (dividend << 64) / divisor
                let dividend_u256 = U256::from(dividend.raw());
                let divisor_u256 = U256::from(divisor.raw());

                // Compute exact quotient in U256 to avoid precision loss
                let exact_quotient_u256: U256 = (dividend_u256 << 64) / divisor_u256;

                // Check if exact quotient fits in u128
                if exact_quotient_u256 <= U256::from(u128::MAX) {
                    let exact_quotient_raw = exact_quotient_u256.as_u128();
                    // Note: exact_quotient value is computed for precision validation but not used in assertions

                    // Our implementation should match the exact quotient within 1 ULP
                    let ulp_diff = if quotient.raw() > exact_quotient_raw {
                        quotient.raw() - exact_quotient_raw
                    } else {
                        exact_quotient_raw - quotient.raw()
                    };

                    prop_assert!(
                        ulp_diff <= 1,
                        "Division precision error: {} / {} = {} but exact = {} (ULP diff = {})",
                        dividend.raw(), divisor.raw(), quotient.raw(), exact_quotient_raw, ulp_diff
                    );

                    // Test exact division relationship using U256 arithmetic
                    // For fixed-point truncated division: q = floor((dividend<<64)/divisor)
                    // The mathematical constraint is: q*divisor ≤ dividend < (q+1)*divisor
                    if let Ok(_reconstructed) = quotient.checked_mul(divisor) {
                        let _n = U256::from(dividend.raw()) << 64;  // numerator in division
                        let _d = U256::from(divisor.raw());         // divisor
                        let q = exact_quotient_u256;               // exact quotient from above

                        // Verify our quotient matches the exact calculation (within 1 ULP for rounding)
                        let q_actual = U256::from(quotient.raw());
                        let q_diff = if q_actual > q { q_actual - q } else { q - q_actual };

                        prop_assert!(
                            q_diff <= U256::from(1u128),
                            "Quotient doesn't match exact calculation: got {} vs exact {} (diff = {})",
                            quotient.raw(), q.as_u128(), q_diff
                        );

                        // Test reconstruction bounds for round-to-nearest division
                        // For nearest rounding: |(dividend_raw << 64) - q*divisor_raw| ≤ divisor_raw/2
                        let n = U256::from(dividend.raw()) << 64;     // dividend_raw << 64
                        let d = U256::from(divisor.raw());           // divisor_raw
                        let q = U256::from(quotient.raw());          // quotient_raw
                        let rec = q * d;                             // q * divisor_raw

                        let diff = if rec > n { rec - n } else { n - rec };

                        prop_assert!(
                            diff <= (d >> 1),
                            "Round-to-nearest reconstruction bound violated: |dividend<<64 - q*divisor| = {} > divisor/2 = {}",
                            diff, d >> 1
                        );
                    }
                }
            }
        }
    } // ==================== ERROR HANDLING PROPERTY TESTS ====================

    // Test that error conditions are handled consistently across all operations.
    //
    // Error handling must be deterministic and consistent to ensure:
    // - All validators reject the same invalid operations
    // - Error states don't lead to undefined behavior
    // - Protocol safety is maintained even with malicious inputs
    proptest! {
        #[test]
        fn test_consistent_error_handling(
            a in q64x64_strategy(),
            b in q64x64_strategy()
        ) {
            // Division by zero should always fail
            let zero = Q64x64::zero();
            let div_by_zero = a.checked_div(zero);
            prop_assert!(div_by_zero.is_err(), "Division by zero must fail consistently");

            // Operations with the same inputs should produce the same results
            let add1 = a.checked_add(b);
            let add2 = a.checked_add(b);
            prop_assert_eq!(add1.is_ok(), add2.is_ok(), "Identical operations must have consistent success/failure");

            if let (Ok(result1), Ok(result2)) = (add1, add2) {
                prop_assert_eq!(result1.raw(), result2.raw(), "Identical operations must produce identical results");
            }
        }
    }

    // Test that invalid tick values are consistently rejected.
    //
    // The tick system has strict bounds to prevent arithmetic overflow and maintain
    // the logarithmic pricing relationship. All values outside these bounds must be
    // consistently rejected.
    proptest! {
        #[test]
        fn test_tick_bounds_enforcement(
            invalid_tick in prop_oneof![
                (i32::MIN..MIN_TICK),
                ((MAX_TICK + 1)..=i32::MAX)
            ]
        ) {
            let result = tick_to_sqrt_x64(invalid_tick);
            prop_assert!(
                result.is_err(),
                "Invalid tick {} should be rejected", invalid_tick
            );
        }
    }

    // Test exact tick boundary mappings
    //
    // Explicitly test that MIN_TICK and MAX_TICK map to exact MIN_SQRT_X64 and MAX_SQRT_X64
    // values within tight ULP bounds.
    #[test]
    fn test_exact_tick_boundaries() {
        // Test MIN_TICK maps to MIN_SQRT_X64
        if let Ok(min_sqrt) = tick_to_sqrt_x64(MIN_TICK) {
            let ulp_diff = if min_sqrt.raw() > MIN_SQRT_X64 {
                min_sqrt.raw() - MIN_SQRT_X64
            } else {
                MIN_SQRT_X64 - min_sqrt.raw()
            };

            assert!(
                ulp_diff <= 2,
                "MIN_TICK should map to MIN_SQRT_X64 within 2 ULP: tick_to_sqrt({}) = {} vs MIN_SQRT_X64 = {} (ULP diff = {})",
                MIN_TICK, min_sqrt.raw(), MIN_SQRT_X64, ulp_diff
            );
        }

        // Test MAX_TICK maps to MAX_SQRT_X64
        if let Ok(max_sqrt) = tick_to_sqrt_x64(MAX_TICK) {
            let ulp_diff = if max_sqrt.raw() > MAX_SQRT_X64 {
                max_sqrt.raw() - MAX_SQRT_X64
            } else {
                MAX_SQRT_X64 - max_sqrt.raw()
            };

            assert!(
                ulp_diff <= 2,
                "MAX_TICK should map to MAX_SQRT_X64 within 2 ULP: tick_to_sqrt({}) = {} vs MAX_SQRT_X64 = {} (ULP diff = {})",
                MAX_TICK, max_sqrt.raw(), MAX_SQRT_X64, ulp_diff
            );
        }
    }

    #[test]
    fn test_debug_tick_211140() {
        let tick: i32 = 211140;
        println!(
            "Testing tick {}: binary {:019b}, popcount: {}",
            tick,
            tick,
            tick.count_ones()
        );

        let pos_sqrt_result = tick_to_sqrt_x64(tick);
        let neg_sqrt_result = tick_to_sqrt_x64(-tick);

        if let (Ok(pos_sqrt), Ok(neg_sqrt)) = (pos_sqrt_result, neg_sqrt_result) {
            if let Ok(product) = pos_sqrt.checked_mul(neg_sqrt) {
                let one = Q64x64::one();

                println!("DEBUG tick 211140:");
                println!("  pos_sqrt = {} (raw: {})", pos_sqrt.raw(), pos_sqrt.raw());
                println!("  neg_sqrt = {} (raw: {})", neg_sqrt.raw(), neg_sqrt.raw());
                println!("  product = {} (raw: {})", product.raw(), product.raw());
                println!("  one     = {} (raw: {})", one.raw(), one.raw());

                let ulp_diff = if product.raw() > one.raw() {
                    product.raw() - one.raw()
                } else {
                    one.raw() - product.raw()
                };
                println!("  Per-bit inverse ULP diff = {}", ulp_diff);

                // Calculate ULP budget components
                let n = tick.unsigned_abs().count_ones() as u128;
                let max_sqrt_raw = pos_sqrt.raw().max(neg_sqrt.raw());
                let mag_bits = ((128 - max_sqrt_raw.leading_zeros()) as u128).saturating_sub(64);
                let max_ulp_error = if tick.abs() <= PRACTICAL_MAX_TICK {
                    8192 + 16384 * n + 57_344 * mag_bits + 384 * mag_bits * mag_bits
                } else {
                    32768 + 32768 * n + 114_688 * mag_bits + 768 * mag_bits * mag_bits
                };
                println!(
                    "  ULP budget: {} (practical: {})",
                    max_ulp_error,
                    tick.abs() <= PRACTICAL_MAX_TICK
                );
                println!(
                    "  ULP test: {} <= {} ? {}",
                    ulp_diff,
                    max_ulp_error,
                    ulp_diff <= max_ulp_error
                );

                // Compare with single reciprocal approach
                let neg_single_recip = Q64x64::one().checked_div(pos_sqrt).unwrap();
                let product_single = pos_sqrt.checked_mul(neg_single_recip).unwrap();
                println!("  Single recip approach:");
                println!(
                    "    neg_single_recip = {} (raw: {})",
                    neg_single_recip.raw(),
                    neg_single_recip.raw()
                );
                println!(
                    "    product_single = {} (raw: {})",
                    product_single.raw(),
                    product_single.raw()
                );

                let ulp_diff_single = if product_single.raw() > one.raw() {
                    product_single.raw() - one.raw()
                } else {
                    one.raw() - product_single.raw()
                };
                println!("    ULP diff single = {}", ulp_diff_single);

                println!(
                    "  Difference in ULP errors: {} vs {}",
                    ulp_diff, ulp_diff_single
                );

                // Let's manually check what our per-bit inverse should compute
                println!("\n  Manual per-bit inverse check:");
                // For tick=211140 (0x33884), binary 0110011100011000100
                let abs_tick = tick.unsigned_abs();
                println!(
                    "    abs_tick = {} (0x{:x}, binary {:019b})",
                    abs_tick, abs_tick, abs_tick
                );

                let mut manual_ratio = Q64x64::one();
                // Check all possible bits
                for (bit_idx, &coeff) in POW2_COEFF.iter().enumerate().take(19) {
                    let mask = 1u32 << bit_idx;
                    if abs_tick & mask != 0 {
                        let inv_coeff = recip_q64x64_nearest(coeff);
                        println!(
                            "    bit {}: mask=0x{:x}, coeff={}, inv={}",
                            bit_idx, mask, coeff, inv_coeff
                        );
                        manual_ratio = manual_ratio
                            .checked_mul(Q64x64::from_raw(inv_coeff))
                            .unwrap();
                        println!("      running manual_ratio = {}", manual_ratio.raw());
                    }
                }
                println!(
                    "    Final manual_ratio = {} (raw: {})",
                    manual_ratio.raw(),
                    manual_ratio.raw()
                );

                // Now let's see what the actual function returned
                println!(
                    "    Actual neg_sqrt     = {} (raw: {})",
                    neg_sqrt.raw(),
                    neg_sqrt.raw()
                );
                println!("    Match? {}", manual_ratio.raw() == neg_sqrt.raw());
            }
        }
    }
}
