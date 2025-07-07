#[cfg(test)]
mod tests {
    use crate::math::core_arithmetic::*;
    use ethnum::U256;
    use proptest::prelude::*;

    // Helper strategies for bounded values
    fn valid_q64x64() -> impl Strategy<Value = Q64x64> {
        (0u128..=MAX_SAFE).prop_map(Q64x64::from_raw)
    }

    fn small_q64x64() -> impl Strategy<Value = Q64x64> {
        (0u128..=(MAX_SAFE >> 4)).prop_map(Q64x64::from_raw)
    }

    fn positive_q64x64() -> impl Strategy<Value = Q64x64> {
        (1u128..=MAX_SAFE).prop_map(Q64x64::from_raw)
    }

    fn sqrt_range_q64x64() -> impl Strategy<Value = Q64x64> {
        (MIN_SQRT_X64..=MAX_SQRT_X64).prop_map(Q64x64::from_raw)
    }

    fn valid_tick() -> impl Strategy<Value = i32> {
        MIN_TICK..=MAX_TICK
    }

    fn valid_amount() -> impl Strategy<Value = u64> {
        1u64..=u64::MAX
    }

    fn tiny_q64x64() -> impl Strategy<Value = Q64x64> {
        (1u128..=(MAX_SAFE >> 8)).prop_map(Q64x64::from_raw)
    }

    fn negative_tick() -> impl Strategy<Value = i32> {
        MIN_TICK..=-1i32
    }

    proptest! {
        #[test]
        fn test_q64x64_creation_and_accessors(x in 0u64..=u64::MAX) {
            let q = Q64x64::from_int(x);
            prop_assert_eq!(q.raw(), (x as u128) << FRAC_BITS);

            let raw_val = x as u128;
            let q_raw = Q64x64::from_raw(raw_val);
            prop_assert_eq!(q_raw.raw(), raw_val);
        }

        #[test]
        fn test_q64x64_constants(_x in 0u8..1u8) {
            prop_assert_eq!(Q64x64::zero().raw(), 0);
            prop_assert_eq!(Q64x64::one().raw(), ONE_X64);
        }

        #[test]
        fn test_q64x64_addition_commutativity(a in small_q64x64(), b in small_q64x64()) {
            let sum_ab = a.checked_add(b);
            let sum_ba = b.checked_add(a);

            match (sum_ab, sum_ba) {
                (Ok(ab), Ok(ba)) => prop_assert_eq!(ab, ba),
                (Err(_), Err(_)) => (), // Both overflow is fine
                _ => panic!("Addition commutativity violated"),
            }
        }

        #[test]
        fn test_q64x64_addition_identity(a in valid_q64x64()) {
            let result = a.checked_add(Q64x64::zero()).unwrap();
            prop_assert_eq!(result, a);
        }

        #[test]
        fn test_q64x64_multiplication_commutativity(a in small_q64x64(), b in small_q64x64()) {
            let prod_ab = a.checked_mul(b);
            let prod_ba = b.checked_mul(a);

            match (prod_ab, prod_ba) {
                (Ok(ab), Ok(ba)) => prop_assert_eq!(ab, ba),
                (Err(_), Err(_)) => (), // Both overflow is fine
                _ => panic!("Multiplication commutativity violated"),
            }
        }

        #[test]
        fn test_q64x64_multiplication_identity(a in valid_q64x64()) {
            let result = a.checked_mul(Q64x64::one()).unwrap();
            prop_assert_eq!(result, a);
        }

        #[test]
        fn test_q64x64_multiplication_zero(a in valid_q64x64()) {
            let result = a.checked_mul(Q64x64::zero()).unwrap();
            prop_assert_eq!(result, Q64x64::zero());
        }

        // NEW: Test associativity with rounding tolerance
        #[test]
        fn test_q64x64_multiplication_associativity(a in tiny_q64x64(), b in tiny_q64x64(), c in tiny_q64x64()) {
            prop_assume!(a.raw() > 0 && b.raw() > 0 && c.raw() > 0);

            if let (Ok(ab), Ok(bc)) = (a.checked_mul(b), b.checked_mul(c)) {
                if let (Ok(ab_c), Ok(a_bc)) = (ab.checked_mul(c), a.checked_mul(bc)) {
                    // (a * b) * c ≈ a * (b * c), allowing for rounding errors
                    let diff = if ab_c.raw() > a_bc.raw() {
                        ab_c.raw() - a_bc.raw()
                    } else {
                        a_bc.raw() - ab_c.raw()
                    };

                    // Allow small tolerance for rounding in associativity
                    let tolerance = (a.raw().max(b.raw()).max(c.raw()) >> 48).max(4);
                    prop_assert!(diff <= tolerance,
                        "Associativity violated: (a*b)*c = {}, a*(b*c) = {}, diff = {}",
                        ab_c.raw(), a_bc.raw(), diff);
                }
            }
        }

        #[test]
        fn test_q64x64_division_identity(a in positive_q64x64()) {
            let result = a.checked_div(Q64x64::one()).unwrap();
            prop_assert_eq!(result, a);
        }

        #[test]
        fn test_q64x64_division_self(a in positive_q64x64()) {
            let result = a.checked_div(a).unwrap();
            // Should be close to one, allowing for rounding
            let diff = if result.raw() > ONE_X64 {
                result.raw() - ONE_X64
            } else {
                ONE_X64 - result.raw()
            };
            prop_assert!(diff <= 1, "Division by self should yield ~1, got diff: {}", diff);
        }

        #[test]
        fn test_q64x64_mul_div_inverse(a in small_q64x64(), b in positive_q64x64()) {
            prop_assume!(a.raw() > 0);

            if let (Ok(product), Ok(_quotient)) = (a.checked_mul(b), a.checked_div(b)) {
                // (a * b) / b ≈ a
                if let Ok(result) = product.checked_div(b) {
                    let diff = if result.raw() > a.raw() {
                        result.raw() - a.raw()
                    } else {
                        a.raw() - result.raw()
                    };
                    prop_assert!(diff <= 2, "Multiplication/division inverse failed");
                }
            }
        }

        // NEW: Test precision drift over chains of operations
        #[test]
        fn test_q64x64_precision_drift_chains(x in positive_q64x64()) {
            prop_assume!(x.raw() > (ONE_X64 >> 10)); // Avoid very small values

            // Chain: ((x / x) * x) / x) * x
            if let Ok(step1) = x.checked_div(x) { // should be ~1
                if let Ok(step2) = step1.checked_mul(x) { // should be ~x
                    if let Ok(step3) = step2.checked_div(x) { // should be ~1
                        if let Ok(final_result) = step3.checked_mul(x) { // should be ~x
                            let diff = if final_result.raw() > x.raw() {
                                final_result.raw() - x.raw()
                            } else {
                                x.raw() - final_result.raw()
                            };

                            // Allow tolerance proportional to input magnitude
                            let tolerance = (x.raw() >> 30).max(10);
                            prop_assert!(diff <= tolerance,
                                "Precision drift too large: original = {}, final = {}, diff = {}",
                                x.raw(), final_result.raw(), diff);
                        }
                    }
                }
            }
        }

        #[test]
        fn test_mul_div_basic_properties(a in 1u128..=u128::MAX >> 8, b in 1u128..=u128::MAX >> 8, c in 1u128..=u128::MAX >> 8) {
            if let Ok(result) = mul_div(a, b, c) {
                // Result should not exceed (a * b) / c bounds
                // (No need to check result <= u128::MAX, as result is u128)

                // If a and b are small enough, verify against direct calculation
                if a <= u64::MAX as u128 && b <= u64::MAX as u128 {
                    let expected = (a * b) / c;
                    prop_assert_eq!(result, expected);
                }
            }
        }

        // NEW: Test extreme ratios in mul_div
        #[test]
        fn test_mul_div_extreme_ratios(a in 1u128..=u128::MAX >> 4, b in 1u128..=u128::MAX >> 4) {
            // Test case where a * b would overflow u128
            let large_a = u128::MAX >> 1;
            let large_b = u128::MAX >> 1;
            prop_assert!(mul_div(large_a, large_b, 1).is_err(), "Should overflow");

            // Test case where a * b < c (result should be 0)
            let small_a = 1u128;
            let small_b = 1u128;
            let large_c = u128::MAX >> 8;
            if let Ok(result) = mul_div(small_a, small_b, large_c) {
                prop_assert_eq!(result, 0, "Small numerator should give 0");
            }

            // Only consider a,b such that the full product fits in u128
            prop_assume!(U256::from(a) * U256::from(b) <= U256::from(u128::MAX));
            let c = a * b; // safe now
            prop_assert_eq!(
                mul_div(a, b, c)?,
                1,
                "a*b / (a*b) should equal 1"
            );
        }

        #[test]
        fn test_mul_div_round_up_vs_mul_div(a in 1u128..=u128::MAX >> 8, b in 1u128..=u128::MAX >> 8, c in 1u128..=u128::MAX >> 8) {
            if let (Ok(normal), Ok(rounded_up)) = (mul_div(a, b, c), mul_div_round_up(a, b, c)) {
                // Rounded up should be >= normal result
                prop_assert!(rounded_up >= normal);
                // Should differ by at most 1
                prop_assert!(rounded_up - normal <= 1);
            }
        }

        #[test]
        fn test_mul_div_q64_consistency(a in small_q64x64(), b in small_q64x64(), c in positive_q64x64()) {
            if let (Ok(raw_result), Ok(q64_result)) = (
                mul_div(a.raw(), b.raw(), c.raw()),
                mul_div_q64(a, b, c)
            ) {
                prop_assert_eq!(raw_result, q64_result.raw());
            }
        }

        #[test]
        fn test_sqrt_x64_basic_properties(x in 0u128..=(MAX_SAFE >> 2)) {
            let input = Q64x64::from_raw(x);

            if let Ok(sqrt_result) = sqrt_x64(input) {
                // sqrt(0) = 0
                if x == 0 {
                    prop_assert_eq!(sqrt_result, Q64x64::zero());
                    return Ok(());
                }

                // sqrt(x) should be in valid range
                prop_assert!(sqrt_result.raw() >= MIN_SQRT_X64);
                prop_assert!(sqrt_result.raw() <= MAX_SQRT_X64);

                // sqrt(x)^2 ≈ x (within reasonable tolerance)
                if let Ok(squared) = sqrt_result.checked_mul(sqrt_result) {
                    let diff = if squared.raw() > x {
                        squared.raw() - x
                    } else {
                        x - squared.raw()
                    };
                    let tolerance = (x >> 20).max(1); // Relative tolerance
                    prop_assert!(diff <= tolerance, "sqrt property violated: sqrt({})^2 = {}, diff = {}", x, squared.raw(), diff);
                }
            }
        }

        #[test]
        fn test_sqrt_x64_monotonicity(x1 in 0u128..=(MAX_SAFE >> 3), x2 in 0u128..=(MAX_SAFE >> 3)) {
            prop_assume!(x1 < x2);

            let input1 = Q64x64::from_raw(x1);
            let input2 = Q64x64::from_raw(x2);

            if let (Ok(sqrt1), Ok(sqrt2)) = (sqrt_x64(input1), sqrt_x64(input2)) {
                // sqrt should be monotonic: if x1 < x2, then sqrt(x1) <= sqrt(x2)
                prop_assert!(sqrt1.raw() <= sqrt2.raw(), "sqrt monotonicity violated: sqrt({}) = {}, sqrt({}) = {}", x1, sqrt1.raw(), x2, sqrt2.raw());
            }
        }

        #[test]
        fn test_sqrt_x64_known_values(_x in 0u8..1u8) {
            // Test perfect squares
            let one = Q64x64::one();
            let sqrt_one = sqrt_x64(one).unwrap();
            let diff = if sqrt_one.raw() > ONE_X64 { sqrt_one.raw() - ONE_X64 } else { ONE_X64 - sqrt_one.raw() };
            prop_assert!(diff <= 1, "sqrt(1) should be ~1");

            let four = Q64x64::from_int(4);
            let sqrt_four = sqrt_x64(four).unwrap();
            let two = Q64x64::from_int(2);
            let diff = if sqrt_four.raw() > two.raw() { sqrt_four.raw() - two.raw() } else { two.raw() - sqrt_four.raw() };
            prop_assert!(diff <= (1 << 10), "sqrt(4) should be ~2");
        }

        #[test]
        fn test_tick_to_sqrt_x64_bounds(tick in valid_tick()) {
            let sqrt_price = tick_to_sqrt_x64(tick).unwrap();

            // Result should be in valid sqrt price range
            prop_assert!(sqrt_price.raw() >= MIN_SQRT_X64);
            prop_assert!(sqrt_price.raw() <= MAX_SQRT_X64);
        }

        #[test]
        fn test_tick_to_sqrt_x64_monotonicity(tick1 in valid_tick(), tick2 in valid_tick()) {
            prop_assume!(tick1 < tick2);

            let sqrt1 = tick_to_sqrt_x64(tick1).unwrap();
            let sqrt2 = tick_to_sqrt_x64(tick2).unwrap();

            // Higher tick should give higher sqrt price
            prop_assert!(sqrt1.raw() < sqrt2.raw(), "tick_to_sqrt monotonicity violated: tick {} -> {}, tick {} -> {}", tick1, sqrt1.raw(), tick2, sqrt2.raw());
        }

        // NEW: Test monotonicity specifically for negative ticks
        #[test]
        fn test_tick_to_sqrt_x64_negative_monotonicity(tick1 in negative_tick(), tick2 in negative_tick()) {
            prop_assume!(tick1 < tick2); // Both negative, tick1 more negative

            let sqrt1 = tick_to_sqrt_x64(tick1).unwrap();
            let sqrt2 = tick_to_sqrt_x64(tick2).unwrap();

            // Even for negative ticks, higher tick should give higher sqrt price
            prop_assert!(sqrt1.raw() < sqrt2.raw(),
                "Negative tick monotonicity violated: tick {} -> {}, tick {} -> {}",
                tick1, sqrt1.raw(), tick2, sqrt2.raw());
        }

        #[test]
        fn test_tick_to_sqrt_x64_zero_tick(_x in 0u8..1u8) {
            let sqrt_price = tick_to_sqrt_x64(0).unwrap();

            // At tick 0, sqrt_price should be close to ONE_X64 (price = 1)
            let diff = if sqrt_price.raw() > ONE_X64 {
                sqrt_price.raw() - ONE_X64
            } else {
                ONE_X64 - sqrt_price.raw()
            };

            // Allow some tolerance for tick 0
            prop_assert!(diff <= (ONE_X64 >> 10), "tick 0 should give sqrt_price ~1, got diff: {}", diff);
        }

        #[test]
        fn test_tick_to_sqrt_x64_symmetric_ticks(tick in 1i32..=10000) {
            prop_assume!((-MAX_TICK..=MAX_TICK).contains(&tick));
            prop_assume!(-tick >= MIN_TICK && -tick <= MAX_TICK);

            let sqrt_pos = tick_to_sqrt_x64(tick).unwrap();
            let sqrt_neg = tick_to_sqrt_x64(-tick).unwrap();

            // sqrt_price(-tick) * sqrt_price(tick) should be close to ONE_X64
            if let Ok(product) = sqrt_pos.checked_mul(sqrt_neg) {
                let diff = if product.raw() > ONE_X64 {
                    product.raw() - ONE_X64
                } else {
                    ONE_X64 - product.raw()
                };

                let tolerance = ONE_X64 >> 15; // Small tolerance for numerical errors
                prop_assert!(diff <= tolerance, "Symmetric tick property violated for tick {}: product = {}, diff = {}", tick, product.raw(), diff);
            }
        }

        // NEW: Test extreme tick magnitudes for symmetric behavior
        #[test]
        fn test_tick_to_sqrt_x64_extreme_symmetric_ticks(tick in 100000i32..=MAX_TICK) {
            prop_assume!(-tick >= MIN_TICK); // Ensure -tick is valid

            let sqrt_pos = tick_to_sqrt_x64(tick).unwrap();
            let sqrt_neg = tick_to_sqrt_x64(-tick).unwrap();

            // Even for extreme ticks, symmetric property should hold
            if let Ok(product) = sqrt_pos.checked_mul(sqrt_neg) {
                let diff = if product.raw() > ONE_X64 {
                    product.raw() - ONE_X64
                } else {
                    ONE_X64 - product.raw()
                };

                // Allow larger tolerance for extreme values due to clamping
                let tolerance = ONE_X64 >> 10;
                prop_assert!(diff <= tolerance,
                    "Extreme symmetric tick property violated for tick {}: product = {}, diff = {}",
                    tick, product.raw(), diff);
            }
        }

        #[test]
        fn test_liquidity_from_amount_0_properties(
            sqrt_a in sqrt_range_q64x64(),
            sqrt_b in sqrt_range_q64x64(),
            amount0 in valid_amount()
        ) {
            prop_assume!(sqrt_a.raw() < sqrt_b.raw());
            prop_assume!(amount0 > 0);

            if let Ok(liquidity) = liquidity_from_amount_0(sqrt_a, sqrt_b, amount0) {
                // Liquidity should be positive for positive amount
                prop_assert!(liquidity > 0);

                // Larger amount should give larger liquidity (monotonicity)
                if amount0 < u64::MAX / 2 {
                    if let Ok(liquidity2) = liquidity_from_amount_0(sqrt_a, sqrt_b, amount0 * 2) {
                        prop_assert!(liquidity2 > liquidity, "Liquidity should increase with amount");
                    }
                }
            }
        }

        #[test]
        fn test_liquidity_from_amount_1_properties(
            sqrt_a in sqrt_range_q64x64(),
            sqrt_b in sqrt_range_q64x64(),
            amount1 in valid_amount()
        ) {
            prop_assume!(sqrt_a.raw() < sqrt_b.raw());
            prop_assume!(amount1 > 0);

            if let Ok(liquidity) = liquidity_from_amount_1(sqrt_a, sqrt_b, amount1) {
                // Liquidity should be positive for positive amount
                prop_assert!(liquidity > 0);

                // Larger amount should give larger liquidity (monotonicity)
                if amount1 < u64::MAX / 2 {
                    if let Ok(liquidity2) = liquidity_from_amount_1(sqrt_a, sqrt_b, amount1 * 2) {
                        prop_assert!(liquidity2 > liquidity, "Liquidity should increase with amount");
                    }
                }
            }
        }

        #[test]
        fn test_liquidity_price_range_effect(
            sqrt_a in sqrt_range_q64x64(),
            sqrt_b in sqrt_range_q64x64(),
            amount in valid_amount()
        ) {
            prop_assume!(sqrt_a.raw() < sqrt_b.raw());
            prop_assume!(amount > 0);

            // Test that narrower price ranges give higher liquidity for same amount
            if sqrt_b.raw() - sqrt_a.raw() > MIN_SQRT_X64 * 2 {
                let sqrt_mid = Q64x64::from_raw((sqrt_a.raw() + sqrt_b.raw()) / 2);

                if let (Ok(liq_wide), Ok(liq_narrow)) = (
                    liquidity_from_amount_1(sqrt_a, sqrt_b, amount),
                    liquidity_from_amount_1(sqrt_a, sqrt_mid, amount)
                ) {
                    prop_assert!(liq_narrow > liq_wide, "Narrower range should give higher liquidity");
                }
            }
        }

        #[test]
        fn test_error_conditions_division_by_zero(_x in 0u8..1u8) {
            let a = Q64x64::one();
            let zero = Q64x64::zero();

            prop_assert!(a.checked_div(zero).is_err());
            prop_assert!(mul_div(1, 1, 0).is_err());
            prop_assert!(mul_div_round_up(1, 1, 0).is_err());
        }

        #[test]
        fn test_error_conditions_overflow(a in (u128::MAX >> 1)..=u128::MAX, b in (u128::MAX >> 1)..=u128::MAX) {
            let qa = Q64x64::from_raw(a);
            let qb = Q64x64::from_raw(b);

            // Large values should overflow in multiplication
            prop_assert!(qa.checked_mul(qb).is_err());
        }

        #[test]
        fn test_error_conditions_out_of_range(_x in 0u8..1u8) {
            // Test tick out of range
            prop_assert!(tick_to_sqrt_x64(MAX_TICK + 1).is_err());
            prop_assert!(tick_to_sqrt_x64(MIN_TICK - 1).is_err());

            // Test invalid sqrt order in liquidity functions
            let sqrt_a = Q64x64::from_raw(MAX_SQRT_X64);
            let sqrt_b = Q64x64::from_raw(MIN_SQRT_X64);
            prop_assert!(liquidity_from_amount_0(sqrt_a, sqrt_b, 1000).is_err());
            prop_assert!(liquidity_from_amount_1(sqrt_a, sqrt_b, 1000).is_err());
        }

        #[test]
        fn test_edge_cases_max_values(_x in 0u8..1u8) {
            // Test with maximum safe values
            let max_safe_q64 = Q64x64::from_raw(MAX_SAFE >> 8); // Scaled down to avoid overflow

            // Should not panic or overflow
            let _ = sqrt_x64(max_safe_q64);
            let _ = max_safe_q64.checked_add(Q64x64::zero());
            let _ = max_safe_q64.checked_mul(Q64x64::zero());
        }

        #[test]
        fn test_precision_bounds(x in 1u128..=1000000u128) {
            // Test that small values maintain reasonable precision
            let small_val = Q64x64::from_raw(x);

            if let Ok(sqrt_result) = sqrt_x64(small_val) {
                if let Ok(squared) = sqrt_result.checked_mul(sqrt_result) {
                    let relative_error = if squared.raw() > x {
                        (squared.raw() - x) * 1000000 / x.max(1)
                    } else {
                        (x - squared.raw()) * 1000000 / x.max(1)
                    };

                    // Relative error should be small (less than 0.1%)
                    prop_assert!(relative_error < 1000, "High relative error: {}‰ for input {}", relative_error, x);
                }
            }
        }
    }
}
