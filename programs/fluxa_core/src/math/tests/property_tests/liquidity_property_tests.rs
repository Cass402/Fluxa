#[cfg(test)]
mod liquidity_properties {
    use crate::math::core_arithmetic::{tick_to_sqrt_x64, Q64x64};
    use crate::math::liquidity_math::{
        calculate_amounts_for_liquidity_piecewise, calculate_liquidity,
    };
    use crate::utils::constants::{MAX_TICK, MAX_TOKEN_AMOUNT, MIN_TICK};
    use ethnum::U256;
    use proptest::prelude::*;
    use rug::{Complete, Integer};

    fn liquidity_proptest_config() -> ProptestConfig {
        ProptestConfig {
            cases: 2048,
            max_shrink_iters: 2048,
            ..ProptestConfig::default()
        }
    }

    fn tick_pair_strategy() -> impl Strategy<Value = (i32, i32)> {
        ((MIN_TICK + 50_000)..=(MAX_TICK - 50_000))
            .prop_flat_map(|lower| (200i32..=4_000).prop_map(move |width| (lower, lower + width)))
    }

    fn liquidity_strategy() -> impl Strategy<Value = Q64x64> {
        (1u64..=50_000_000u64, 0u64..=1_000_000u64).prop_map(|(whole, frac)| {
            let raw = ((whole as u128) << 64) + (frac as u128);
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
            Some(quotient)
        } else {
            Some(fallback.clone())
        };

        let liquidity_from_0 = match liquidity_from_0 {
            Some(value) => value,
            None => return None,
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
            Some(quotient)
        } else {
            Some(fallback.clone())
        };

        let liquidity_from_1 = match liquidity_from_1 {
            Some(value) => value,
            None => return None,
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
                prop_assume!(eh0 > el0 && eh1 > el1);
                if high0 == low0 || high1 == low1 {
                    prop_assume!(false);
                }
                prop_assert!(high0 > low0);
                prop_assert!(high1 > low1);
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
}
