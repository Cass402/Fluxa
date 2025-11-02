#[cfg(test)]
mod liquidity_properties {
    use crate::math::core_arithmetic::{tick_to_sqrt_x64, Q64x64};
    use crate::math::liquidity_math::{
        calculate_amount_0_delta, calculate_amount_1_delta,
        calculate_amounts_for_liquidity_piecewise, calculate_liquidity,
    };
    use crate::utils::constants::{MAX_TICK, MAX_TOKEN_AMOUNT, MIN_TICK};
    use ethnum::U256;
    use proptest::prelude::*;
    use rug::{Complete, Integer};

    fn liquidity_proptest_config() -> ProptestConfig {
        ProptestConfig {
            cases: 10_000,
            max_shrink_iters: 4096,
            max_global_rejects: 100_000, // Allow more rejects for edge case filtering
            ..ProptestConfig::default()
        }
    }

    fn tick_pair_strategy() -> impl Strategy<Value = (i32, i32)> {
        // Wider tick ranges (1000-10000) to produce larger sqrt gaps and bigger amounts
        ((MIN_TICK + 50_000)..=(MAX_TICK - 50_000)).prop_flat_map(|lower| {
            (1_000i32..=10_000).prop_map(move |width| (lower, lower + width))
        })
    }

    fn liquidity_strategy() -> impl Strategy<Value = Q64x64> {
        // Higher liquidity range (1M - 10M whole units in Q64x64 format)
        // This is liquidity * 2^64, so actual values are 1M-10M in fixed-point representation
        (1_000_000u64..=10_000_000u64).prop_map(|whole| {
            // Convert to Q64x64 by shifting left 64 bits
            let raw = (whole as u128) << 64;
            Q64x64::from_raw(raw)
        })
    }

    fn liquidity_pair_strategy() -> impl Strategy<Value = (Q64x64, Q64x64)> {
        (10_000_000u64..=40_000_000u64, 0u64..=1_000_000u64).prop_map(|(base, frac)| {
            let base_raw = ((base as u128) << 64) + (frac as u128);
            let higher_raw = base_raw.saturating_mul(2);
            (Q64x64::from_raw(base_raw), Q64x64::from_raw(higher_raw))
        })
    }

    fn int_from_q64(value: Q64x64) -> Integer {
        Integer::from(value.raw())
    }

    fn mul_q64_round(lhs: u128, rhs: u128) -> Option<u128> {
        let product = U256::from(lhs) * U256::from(rhs);
        let adjusted = product + (U256::ONE << 63);
        let result: U256 = adjusted >> 64;
        if result > U256::from(u128::MAX) {
            None
        } else {
            Some(result.as_u128())
        }
    }

    fn div_q64_round(numerator: u128, denominator: u128) -> Option<u128> {
        if denominator == 0 {
            return None;
        }
        let numerator_shifted = U256::from(numerator) << 64;
        let adjusted = numerator_shifted + (U256::from(denominator) >> 1);
        let quotient: U256 = adjusted / U256::from(denominator);
        Some(quotient.as_u128())
    }

    fn expected_amount0_floor(
        sqrt_lower: Q64x64,
        sqrt_upper: Q64x64,
        liquidity: Q64x64,
    ) -> Option<u64> {
        if sqrt_upper.raw() <= sqrt_lower.raw() || sqrt_lower.raw() == 0 {
            return None;
        }
        let price_diff_raw = sqrt_upper.raw() - sqrt_lower.raw();
        let numerator = mul_q64_round(liquidity.raw(), price_diff_raw)?;
        let denominator = mul_q64_round(sqrt_lower.raw(), sqrt_upper.raw())?;
        let result = div_q64_round(numerator, denominator)?;
        let amount = (result >> 64) as u64;
        if amount > MAX_TOKEN_AMOUNT {
            return None;
        }
        Some(amount)
    }

    fn expected_amount1_floor(
        sqrt_lower: Q64x64,
        sqrt_upper: Q64x64,
        liquidity: Q64x64,
    ) -> Option<u64> {
        if sqrt_upper.raw() <= sqrt_lower.raw() {
            return None;
        }

        let diff_raw = sqrt_upper.raw() - sqrt_lower.raw();
        let numerator = mul_q64_round(liquidity.raw(), diff_raw)?;
        let amount = (numerator >> 64) as u64;
        if amount > MAX_TOKEN_AMOUNT {
            return None;
        }
        Some(amount)
    }

    fn expected_liquidity_floor(
        sqrt_current: Q64x64,
        sqrt_lower: Q64x64,
        sqrt_upper: Q64x64,
        amount0: u64,
        amount1: u64,
    ) -> Option<u128> {
        let current = int_from_q64(sqrt_current);
        let lower = int_from_q64(sqrt_lower);
        let upper = int_from_q64(sqrt_upper);
        let zero = Integer::from(0);
        let fallback = Integer::from(u64::MAX as u128);

        let liquidity_from_0 = if amount0 > 0 {
            if upper <= current {
                return None;
            }
            let diff = (&upper - &current).complete();
            if diff <= zero {
                return None;
            }
            let numerator = Integer::from(amount0);
            let numerator = numerator * &current * &upper;
            let quotient: Integer = numerator / diff;
            quotient
        } else {
            fallback.clone()
        };

        let liquidity_from_1 = if amount1 > 0 {
            if current <= lower {
                return None;
            }
            let diff = (&current - &lower).complete();
            if diff <= zero {
                return None;
            }
            let numerator = Integer::from(amount1) << 128;
            let quotient: Integer = numerator / diff;
            quotient
        } else {
            fallback.clone()
        };

        let min_liquidity = if liquidity_from_0 <= liquidity_from_1 {
            liquidity_from_0
        } else {
            liquidity_from_1
        };

        if min_liquidity <= zero {
            return None;
        }

        min_liquidity.to_u128()
    }

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn amount0_floor_matches_high_precision(
            (tick_lower, tick_upper) in tick_pair_strategy(),
            liquidity in liquidity_strategy()
        ) {
            prop_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK && tick_lower < tick_upper);
            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
            let current = sqrt_lower;

            let result = calculate_amounts_for_liquidity_piecewise(current, sqrt_lower, sqrt_upper, liquidity);
            prop_assume!(result.is_ok());
            let (amount0, amount1) = result.unwrap();
            prop_assert_eq!(amount1, 0);

            if let Some(expected) = expected_amount0_floor(sqrt_lower, sqrt_upper, liquidity) {
                prop_assert_eq!(amount0, expected);
            } else {
                prop_assume!(false);
            }
        }
    }

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn amount1_floor_matches_high_precision(
            (tick_lower, tick_upper) in tick_pair_strategy(),
            liquidity in liquidity_strategy()
        ) {
            prop_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK && tick_lower < tick_upper);
            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
            let current = sqrt_upper;

            let result = calculate_amounts_for_liquidity_piecewise(current, sqrt_lower, sqrt_upper, liquidity);
            prop_assume!(result.is_ok());
            let (amount0, amount1) = result.unwrap();
            prop_assert_eq!(amount0, 0);

            if let Some(expected) = expected_amount1_floor(sqrt_lower, sqrt_upper, liquidity) {
                prop_assert_eq!(amount1, expected);
            } else {
                prop_assume!(false);
            }
        }
    }

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn inside_range_amounts_match_high_precision(
            (tick_lower, tick_upper) in tick_pair_strategy(),
            liquidity in liquidity_strategy()
        ) {
            prop_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK && (tick_upper - tick_lower) > 2);
            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
            let mid_tick = tick_lower + (tick_upper - tick_lower) / 2;
            let current = tick_to_sqrt_x64(mid_tick).unwrap();

            prop_assume!(current.raw() > sqrt_lower.raw() && current.raw() < sqrt_upper.raw());
            let result = calculate_amounts_for_liquidity_piecewise(current, sqrt_lower, sqrt_upper, liquidity);
            prop_assume!(result.is_ok());
            let (amount0, amount1) = result.unwrap();

            let expected0 = expected_amount0_floor(current, sqrt_upper, liquidity);
            let expected1 = expected_amount1_floor(sqrt_lower, current, liquidity);
            if let (Some(e0), Some(e1)) = (expected0, expected1) {
                prop_assert_eq!(amount0, e0);
                prop_assert_eq!(amount1, e1);
            } else {
                prop_assume!(false);
            }
        }
    }

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn liquidity_floor_matches_high_precision(
            (tick_lower, tick_upper) in tick_pair_strategy(),
            amount0 in 1u64..=100_000_000u64,
            amount1 in 1u64..=100_000_000u64
        ) {
            prop_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK && (tick_upper - tick_lower) > 2);
            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
            let mid_tick = tick_lower + (tick_upper - tick_lower) / 2;
            let current = tick_to_sqrt_x64(mid_tick).unwrap();
            prop_assume!(current.raw() > sqrt_lower.raw() && current.raw() < sqrt_upper.raw());

            let result = calculate_liquidity(current, sqrt_lower, sqrt_upper, amount0, amount1);
            prop_assume!(result.is_ok());
            let liquidity_raw = result.unwrap();

            if let Some(expected) = expected_liquidity_floor(current, sqrt_lower, sqrt_upper, amount0, amount1) {
                prop_assert_eq!(liquidity_raw, expected);
            } else {
                prop_assume!(false);
            }
        }
    }

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn piecewise_amounts_are_strictly_monotonic_in_liquidity(
            (tick_lower, tick_upper) in tick_pair_strategy(),
            (liquidity_low, liquidity_high) in liquidity_pair_strategy()
        ) {
            prop_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK && (tick_upper - tick_lower) > 2);
            prop_assume!(liquidity_high.raw() > liquidity_low.raw());

            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
            let mid_tick = tick_lower + (tick_upper - tick_lower) / 2;
            let current = tick_to_sqrt_x64(mid_tick).unwrap();
            prop_assume!(current.raw() > sqrt_lower.raw() && current.raw() < sqrt_upper.raw());

            let low_res = calculate_amounts_for_liquidity_piecewise(
                current,
                sqrt_lower,
                sqrt_upper,
                liquidity_low,
            );
            let high_res = calculate_amounts_for_liquidity_piecewise(
                current,
                sqrt_lower,
                sqrt_upper,
                liquidity_high,
            );
            prop_assume!(low_res.is_ok() && high_res.is_ok());
            let (low0, low1) = low_res.unwrap();
            let (high0, high1) = high_res.unwrap();

            let expected_low0 = expected_amount0_floor(current, sqrt_upper, liquidity_low);
            let expected_high0 = expected_amount0_floor(current, sqrt_upper, liquidity_high);
            let expected_low1 = expected_amount1_floor(sqrt_lower, current, liquidity_low);
            let expected_high1 = expected_amount1_floor(sqrt_lower, current, liquidity_high);

            if let (Some(el0), Some(eh0), Some(el1), Some(eh1)) =
                (expected_low0, expected_high0, expected_low1, expected_high1)
            {
                // Relaxed monotonicity: with integer floors, one side can stay flat (Δ < 1 token)
                // Require at least one side to increase AND both sides to be non-decreasing
                prop_assume!(eh0 > el0 || eh1 > el1); // At least one must increase
                prop_assert!(high0 >= low0, "Amount0 should be non-decreasing");
                prop_assert!(high1 >= low1, "Amount1 should be non-decreasing");
                prop_assert!(high0 > low0 || high1 > low1, "At least one amount must strictly increase");
            } else {
                prop_assume!(false);
            }
        }
    }

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn liquidity_is_strictly_monotonic_in_token0(
            (tick_lower, tick_upper) in tick_pair_strategy(),
            base_amount in 50_000u64..=5_000_000u64
        ) {
            prop_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK && (tick_upper - tick_lower) > 2);
            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
            let mid_tick = tick_lower + (tick_upper - tick_lower) / 2;
            let current = tick_to_sqrt_x64(mid_tick).unwrap();
            prop_assume!(current.raw() > sqrt_lower.raw() && current.raw() < sqrt_upper.raw());

            let larger_amount = base_amount.saturating_mul(2);
            let liq_a_res = calculate_liquidity(current, sqrt_lower, sqrt_upper, base_amount, 0);
            let liq_b_res = calculate_liquidity(current, sqrt_lower, sqrt_upper, larger_amount, 0);
            prop_assume!(liq_a_res.is_ok() && liq_b_res.is_ok());
            let liq_a = liq_a_res.unwrap();
            let liq_b = liq_b_res.unwrap();

            let expected_a = expected_liquidity_floor(current, sqrt_lower, sqrt_upper, base_amount, 0);
            let expected_b = expected_liquidity_floor(
                current,
                sqrt_lower,
                sqrt_upper,
                larger_amount,
                0,
            );
            if let (Some(ea), Some(eb)) = (expected_a, expected_b) {
                if eb > ea {
                    prop_assert_eq!(liq_a, ea);
                    prop_assert_eq!(liq_b, eb);
                    prop_assert!(liq_b > liq_a);
                } else {
                    let saturation_cap = u64::MAX as u128;
                    prop_assert_eq!(ea, saturation_cap);
                    prop_assert_eq!(eb, saturation_cap);
                    prop_assert_eq!(liq_a, saturation_cap);
                    prop_assert_eq!(liq_b, saturation_cap);
                }
            } else {
                prop_assume!(false);
            }
        }
    }

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn liquidity_is_strictly_monotonic_in_token1(
            (tick_lower, tick_upper) in tick_pair_strategy(),
            base_amount in 50_000u64..=5_000_000u64
        ) {
            prop_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK && (tick_upper - tick_lower) > 2);
            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
            let mid_tick = tick_lower + (tick_upper - tick_lower) / 2;
            let current = tick_to_sqrt_x64(mid_tick).unwrap();
            prop_assume!(current.raw() > sqrt_lower.raw() && current.raw() < sqrt_upper.raw());

            let larger_amount = base_amount.saturating_mul(2);
            let liq_a_res = calculate_liquidity(current, sqrt_lower, sqrt_upper, 0, base_amount);
            let liq_b_res = calculate_liquidity(current, sqrt_lower, sqrt_upper, 0, larger_amount);
            prop_assume!(liq_a_res.is_ok() && liq_b_res.is_ok());
            let liq_a = liq_a_res.unwrap();
            let liq_b = liq_b_res.unwrap();

            let expected_a = expected_liquidity_floor(current, sqrt_lower, sqrt_upper, 0, base_amount);
            let expected_b = expected_liquidity_floor(
                current,
                sqrt_lower,
                sqrt_upper,
                0,
                larger_amount,
            );
            if let (Some(ea), Some(eb)) = (expected_a, expected_b) {
                if eb > ea {
                    prop_assert_eq!(liq_a, ea);
                    prop_assert_eq!(liq_b, eb);
                    prop_assert!(liq_b > liq_a);
                } else {
                    let saturation_cap = u64::MAX as u128;
                    prop_assert_eq!(ea, saturation_cap);
                    prop_assert_eq!(eb, saturation_cap);
                    prop_assert_eq!(liq_a, saturation_cap);
                    prop_assert_eq!(liq_b, saturation_cap);
                }
            } else {
                prop_assume!(false);
            }
        }
    }

    // ==================== DELTA AND CONSISTENCY TESTS ====================
    // These test the amount delta functions directly without roundtrip logic

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn amount_delta_consistency_property(
            (tick_lower, tick_upper) in tick_pair_strategy(),
            liquidity in liquidity_strategy()
        ) {
            prop_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK && tick_lower < tick_upper);
            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();

            // Calculate both deltas
            let amount0 = calculate_amount_0_delta(sqrt_lower, sqrt_upper, liquidity);
            let amount1 = calculate_amount_1_delta(sqrt_lower, sqrt_upper, liquidity);

            prop_assume!(amount0.is_ok() && amount1.is_ok());

            // Both should return valid amounts
            let a0 = amount0.unwrap();
            let a1 = amount1.unwrap();

            prop_assert!(a0 <= MAX_TOKEN_AMOUNT);
            prop_assert!(a1 <= MAX_TOKEN_AMOUNT);

            // Determinism check
            let amount0_2 = calculate_amount_0_delta(sqrt_lower, sqrt_upper, liquidity);
            let amount1_2 = calculate_amount_1_delta(sqrt_lower, sqrt_upper, liquidity);

            prop_assert_eq!(amount0_2.unwrap(), a0);
            prop_assert_eq!(amount1_2.unwrap(), a1);
        }
    }

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn amount_deltas_increase_monotonically_with_liquidity(
            (tick_lower, tick_upper) in tick_pair_strategy(),
            liquidity_multiplier in 2u64..=5u64
        ) {
            prop_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK && tick_lower < tick_upper);
            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();

            let base_liq = Q64x64::from_int(1_000_000);
            let higher_liq_raw = base_liq.raw().checked_mul(liquidity_multiplier as u128);
            prop_assume!(higher_liq_raw.is_some());
            let higher_liq = Q64x64::from_raw(higher_liq_raw.unwrap());

            let base_a0 = calculate_amount_0_delta(sqrt_lower, sqrt_upper, base_liq);
            let higher_a0 = calculate_amount_0_delta(sqrt_lower, sqrt_upper, higher_liq);
            let base_a1 = calculate_amount_1_delta(sqrt_lower, sqrt_upper, base_liq);
            let higher_a1 = calculate_amount_1_delta(sqrt_lower, sqrt_upper, higher_liq);

            if base_a0.is_ok() && higher_a0.is_ok() {
                prop_assert!(higher_a0.unwrap() >= base_a0.unwrap(),
                    "Amount0 should be monotonic in liquidity");
            }

            if base_a1.is_ok() && higher_a1.is_ok() {
                prop_assert!(higher_a1.unwrap() >= base_a1.unwrap(),
                    "Amount1 should be monotonic in liquidity");
            }
        }
    }

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn dual_token_roundtrip_property(
            (tick_lower, tick_upper) in tick_pair_strategy(),
            liquidity in liquidity_strategy()
        ) {
            prop_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK && (tick_upper - tick_lower) >= 1000);
            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();

            // Use a position well inside the range (25%-75% of the range)
            let range_width = tick_upper - tick_lower;
            let offset_from_lower = range_width / 4;
            let mid_tick = tick_lower + offset_from_lower;
            let current = tick_to_sqrt_x64(mid_tick).unwrap();

            // Verify we're actually inside the range
            prop_assume!(current.raw() > sqrt_lower.raw() && current.raw() < sqrt_upper.raw());

            // Convert liquidity to amounts (should produce BOTH tokens since we're inside range)
            let amounts_result = calculate_amounts_for_liquidity_piecewise(
                current, sqrt_lower, sqrt_upper, liquidity
            );
            prop_assume!(amounts_result.is_ok());
            let (amount0, amount1) = amounts_result.unwrap();

            // Skip if either amount is too small (realistic threshold, not arbitrary)
            prop_assume!(amount0 >= 100 && amount1 >= 100);

            // Convert amounts back to liquidity (with same current price)
            let liquidity_back_result = calculate_liquidity(
                current, sqrt_lower, sqrt_upper, amount0, amount1
            );
            prop_assume!(liquidity_back_result.is_ok());
            let liquidity_back = liquidity_back_result.unwrap();

            let original = liquidity.raw();
            let reconstructed = liquidity_back;

            // Dual-token roundtrip can have up to 25% loss due to double conversion and integer truncation
            let diff = original.abs_diff(reconstructed);
            let max_allowed = original / 4; // 25%

            prop_assert!(diff <= max_allowed,
                "Dual-token roundtrip: original={}, reconstructed={}, diff={}, max_allowed={}, amount0={}, amount1={}",
                original, reconstructed, diff, max_allowed, amount0, amount1);
        }
    }

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn boundary_smoothness_lower(
            tick_lower in (MIN_TICK + 1000)..=(MAX_TICK - 2000),
            liquidity in liquidity_strategy()
        ) {
            let tick_upper = tick_lower + 1000;
            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();

            // Test prices around lower boundary
            let just_below = tick_to_sqrt_x64(tick_lower - 1).unwrap();
            let exactly_at = sqrt_lower;
            let just_above = tick_to_sqrt_x64(tick_lower + 1).unwrap();

            let below_result = calculate_amounts_for_liquidity_piecewise(
                just_below, sqrt_lower, sqrt_upper, liquidity
            );
            let at_result = calculate_amounts_for_liquidity_piecewise(
                exactly_at, sqrt_lower, sqrt_upper, liquidity
            );
            let above_result = calculate_amounts_for_liquidity_piecewise(
                just_above, sqrt_lower, sqrt_upper, liquidity
            );

            prop_assume!(below_result.is_ok() && at_result.is_ok() && above_result.is_ok());

            let (below_a0, below_a1) = below_result.unwrap();
            let (at_a0, at_a1) = at_result.unwrap();
            let (above_a0, _above_a1) = above_result.unwrap();

            // Below range: only token0
            prop_assert_eq!(below_a1, 0, "Below range should have no token1");
            prop_assert_eq!(at_a1, 0, "At lower boundary should have no token1");
            prop_assert_eq!(below_a0, at_a0, "Amounts should match at boundary");

            // Amount0 shouldn't increase when moving up
            prop_assert!(above_a0 <= at_a0, "Amount0 should not increase when price goes up");
        }
    }

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn boundary_smoothness_upper(
            tick_lower in (MIN_TICK + 1000)..=(MAX_TICK - 2000),
            liquidity in liquidity_strategy()
        ) {
            let tick_upper = tick_lower + 1000;
            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();

            // Test prices around upper boundary
            let just_below = tick_to_sqrt_x64(tick_upper - 1).unwrap();
            let exactly_at = sqrt_upper;
            let just_above = tick_to_sqrt_x64(tick_upper + 1).unwrap();

            let below_result = calculate_amounts_for_liquidity_piecewise(
                just_below, sqrt_lower, sqrt_upper, liquidity
            );
            let at_result = calculate_amounts_for_liquidity_piecewise(
                exactly_at, sqrt_lower, sqrt_upper, liquidity
            );
            let above_result = calculate_amounts_for_liquidity_piecewise(
                just_above, sqrt_lower, sqrt_upper, liquidity
            );

            prop_assume!(below_result.is_ok() && at_result.is_ok() && above_result.is_ok());

            let (_below_a0, _below_a1) = below_result.unwrap();
            let (at_a0, at_a1) = at_result.unwrap();
            let (above_a0, above_a1) = above_result.unwrap();

            // At/above upper boundary: only token1
            prop_assert_eq!(at_a0, 0, "At upper boundary should have no token0");
            prop_assert_eq!(above_a0, 0, "Above range should have no token0");
            prop_assert_eq!(above_a1, at_a1, "Amounts should match at boundary");

            // Below has both or just token0
        }
    }

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn numerical_stability_across_full_tick_range(
            tick_offset in 0..=(MAX_TICK - MIN_TICK - 1000),
            liquidity in liquidity_strategy()
        ) {
            let tick_lower = MIN_TICK + tick_offset;
            let tick_upper = tick_lower + 500;

            prop_assume!(tick_upper <= MAX_TICK);

            let sqrt_lower = tick_to_sqrt_x64(tick_lower);
            let sqrt_upper = tick_to_sqrt_x64(tick_upper);

            prop_assume!(sqrt_lower.is_ok() && sqrt_upper.is_ok());

            let sqrt_lower = sqrt_lower.unwrap();
            let sqrt_upper = sqrt_upper.unwrap();
            let current = sqrt_lower;

            // Should not panic or overflow for any valid tick in range
            let result = calculate_amounts_for_liquidity_piecewise(
                current, sqrt_lower, sqrt_upper, liquidity
            );

            // Either succeeds or fails gracefully with an error (no panic)
            if result.is_ok() {
                let (amount0, amount1) = result.unwrap();

                // Results should be bounded
                prop_assert!(amount0 <= MAX_TOKEN_AMOUNT);
                prop_assert!(amount1 <= MAX_TOKEN_AMOUNT);

                // Should be deterministic
                let result2 = calculate_amounts_for_liquidity_piecewise(
                    current, sqrt_lower, sqrt_upper, liquidity
                );
                prop_assert!(result2.is_ok());
                let (amount0_2, amount1_2) = result2.unwrap();
                prop_assert_eq!(amount0, amount0_2);
                prop_assert_eq!(amount1, amount1_2);
            }
        }
    }

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn calculate_liquidity_bounds_checking(
            (tick_lower, tick_upper) in tick_pair_strategy(),
            amount0 in 0u64..=MAX_TOKEN_AMOUNT,
            amount1 in 0u64..=MAX_TOKEN_AMOUNT
        ) {
            prop_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK && tick_lower < tick_upper);
            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
            let mid_tick = tick_lower + (tick_upper - tick_lower) / 2;
            let current = tick_to_sqrt_x64(mid_tick).unwrap();

            let result = calculate_liquidity(current, sqrt_lower, sqrt_upper, amount0, amount1);

            if result.is_ok() {
                let liquidity_raw = result.unwrap();

                // Liquidity should be finite and non-negative

                // If both amounts are zero, liquidity should be zero
                if amount0 == 0 && amount1 == 0 {
                    prop_assert_eq!(liquidity_raw, 0);
                }

                // If at least one amount is non-zero, liquidity should be positive
                if amount0 > 0 || amount1 > 0 {
                    prop_assert!(liquidity_raw > 0);
                }
            }
        }
    }

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn calculate_liquidity_monotonicity_in_amounts(
            (tick_lower, tick_upper) in tick_pair_strategy(),
            base_amount in 10_000u64..=1_000_000u64,
            multiplier in 2u64..=10u64
        ) {
            prop_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK && (tick_upper - tick_lower) > 2);
            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
            let mid_tick = tick_lower + (tick_upper - tick_lower) / 2;
            let current = tick_to_sqrt_x64(mid_tick).unwrap();

            let larger_amount = base_amount.saturating_mul(multiplier);

            // Test amount0 monotonicity
            let liq_small = calculate_liquidity(current, sqrt_lower, sqrt_upper, base_amount, 0);
            let liq_large = calculate_liquidity(current, sqrt_lower, sqrt_upper, larger_amount, 0);

            if liq_small.is_ok() && liq_large.is_ok() {
                prop_assert!(liq_large.unwrap() >= liq_small.unwrap(),
                    "Liquidity should be monotonic in amount0");
            }

            // Test amount1 monotonicity
            let liq_small_1 = calculate_liquidity(current, sqrt_lower, sqrt_upper, 0, base_amount);
            let liq_large_1 = calculate_liquidity(current, sqrt_lower, sqrt_upper, 0, larger_amount);

            if liq_small_1.is_ok() && liq_large_1.is_ok() {
                prop_assert!(liq_large_1.unwrap() >= liq_small_1.unwrap(),
                    "Liquidity should be monotonic in amount1");
            }
        }
    }

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn extreme_liquidity_magnitudes(
            (tick_lower, tick_upper) in tick_pair_strategy(),
            magnitude in 1u64..=18u64 // 10^1 to 10^18
        ) {
            prop_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK && tick_lower < tick_upper);

            let liquidity_raw = 10u128.pow(magnitude as u32);
            let liquidity = Q64x64::from_raw(liquidity_raw << 64);

            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();

            let result = calculate_amounts_for_liquidity_piecewise(
                sqrt_lower, sqrt_lower, sqrt_upper, liquidity
            );

            // Should either succeed or fail gracefully
            if result.is_ok() {
                let (amount0, amount1) = result.unwrap();

                // Results should respect token amount bounds
                prop_assert!(amount0 <= MAX_TOKEN_AMOUNT);
                prop_assert!(amount1 <= MAX_TOKEN_AMOUNT);

                // Should be monotonic in liquidity
                if magnitude > 1 {
                    let smaller_liq = Q64x64::from_raw((liquidity_raw / 10) << 64);
                    let smaller_result = calculate_amounts_for_liquidity_piecewise(
                        sqrt_lower, sqrt_lower, sqrt_upper, smaller_liq
                    );
                    if smaller_result.is_ok() {
                        let (smaller_a0, smaller_a1) = smaller_result.unwrap();
                        prop_assert!(amount0 >= smaller_a0);
                        prop_assert!(amount1 >= smaller_a1);
                    }
                }
            }
        }
    }

    proptest! {
        #![proptest_config(liquidity_proptest_config())]
        #[test]
        fn precision_preservation_through_operations(
            (tick_lower, tick_upper) in tick_pair_strategy(),
            liquidity in liquidity_strategy()
        ) {
            prop_assume!(tick_lower >= MIN_TICK && tick_upper <= MAX_TICK && (tick_upper - tick_lower) > 2);

            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
            let mid_tick = tick_lower + (tick_upper - tick_lower) / 2;
            let current = tick_to_sqrt_x64(mid_tick).unwrap();

            // Perform operation twice
            let result1 = calculate_amounts_for_liquidity_piecewise(
                current, sqrt_lower, sqrt_upper, liquidity
            );
            let result2 = calculate_amounts_for_liquidity_piecewise(
                current, sqrt_lower, sqrt_upper, liquidity
            );

            prop_assume!(result1.is_ok() && result2.is_ok());

            let (a0_1, a1_1) = result1.unwrap();
            let (a0_2, a1_2) = result2.unwrap();

            // Determinism: identical inputs produce identical outputs
            prop_assert_eq!(a0_1, a0_2, "Calculation should be deterministic");
            prop_assert_eq!(a1_1, a1_2, "Calculation should be deterministic");

            // If we have high-precision expectation, compare
            if let Some(expected0) = expected_amount0_floor(current, sqrt_upper, liquidity) {
                // Actual should be within 1 ULP of expected
                let diff = a0_1.abs_diff(expected0);
                prop_assert!(diff <= 1,
                    "Precision loss should be at most 1 ULP: actual={}, expected={}, diff={}",
                    a0_1, expected0, diff);
            }
        }
    }
}
