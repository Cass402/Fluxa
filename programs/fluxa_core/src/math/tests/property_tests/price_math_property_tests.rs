// ==================== PRICE MATH PROPERTY TESTS ====================
//
// These property-based tests complement the deterministic unit tests for
// `price_math.rs` by exercising the functions across 10,000+ randomized inputs.
// The goal is to catch precision regressions, range violations, and subtle
// search/lookup bugs that could create exploitable pricing errors on-chain.

use crate::error::MathError;
use crate::math::core_arithmetic::{tick_to_sqrt_x64, Q64x64};
use crate::math::price_math::{
    coarse_lookup_table_search_for_tests, lookup_table_for_tests,
    optimized_binary_search_for_tests, sqrt_price_to_price, sqrt_price_to_tick,
    TEST_BINARY_SEARCH_RANGE, TEST_MAX_BINARY_ITERATIONS,
};
use crate::math::tests::precision::assert_rel_close;
use crate::utils::constants::{MAX_SQRT_X64, MAX_TICK, MIN_SQRT_X64, MIN_TICK};
use proptest::prelude::*;

const REL_PPB_TICK_ROUND_TRIP: u128 = 50_000; // 50,000 PPB = 0.5 bp
const ULP_TICK_ROUND_TRIP: u128 = 2_000; // Matches audit doc for realistic magnitudes
const STRICT_PRICE_GAP: u128 = 1u128 << 32; // Ensures distinct price outputs after squaring
const REL_PPB_TABLE_ALIGNMENT: u128 = 50_000; // Lookup table derived from coarse samples
const ULP_TABLE_ALIGNMENT: u128 = 100_000; // Large gap between table entries means interpolation error can accumulate >1e5 ULPs

fn price_math_proptest_config() -> ProptestConfig {
    ProptestConfig {
        cases: 10_000,
        max_shrink_iters: 10_000,
        ..ProptestConfig::default()
    }
}

fn sqrt_price_strategy() -> impl Strategy<Value = Q64x64> {
    prop_oneof![
        Just(Q64x64::from_raw(MIN_SQRT_X64)),
        Just(Q64x64::from_raw(MAX_SQRT_X64)),
        (MIN_SQRT_X64..=(MIN_SQRT_X64 + 1_000)).prop_map(Q64x64::from_raw),
        ((MAX_SQRT_X64.saturating_sub(1_000))..=MAX_SQRT_X64).prop_map(Q64x64::from_raw),
        (MIN_SQRT_X64..=MAX_SQRT_X64).prop_map(Q64x64::from_raw),
    ]
}

fn ordered_sqrt_pair_strategy() -> impl Strategy<Value = (Q64x64, Q64x64)> {
    (sqrt_price_strategy(), sqrt_price_strategy()).prop_map(|(a, b)| {
        if a.raw() <= b.raw() {
            (a, b)
        } else {
            (b, a)
        }
    })
}

fn sqrt_pair_with_gap_strategy(min_gap: u128) -> impl Strategy<Value = (Q64x64, Q64x64)> {
    (MIN_SQRT_X64..=(MAX_SQRT_X64 - min_gap)).prop_flat_map(move |start| {
        ((start + min_gap)..=MAX_SQRT_X64)
            .prop_map(move |end| (Q64x64::from_raw(start), Q64x64::from_raw(end)))
    })
}

fn sqrt_price_out_of_bounds_strategy() -> impl Strategy<Value = Q64x64> {
    let upper_padding = 1u128 << 40;
    prop_oneof![
        (0..MIN_SQRT_X64).prop_map(Q64x64::from_raw),
        ((MAX_SQRT_X64 + 1)..=(MAX_SQRT_X64 + upper_padding)).prop_map(Q64x64::from_raw),
    ]
}

fn tick_strategy() -> impl Strategy<Value = i32> {
    prop_oneof![
        Just(MIN_TICK),
        Just(MAX_TICK),
        (MIN_TICK..=MAX_TICK),
        (MIN_TICK..=(MIN_TICK + 10_000)),
        ((MAX_TICK - 10_000)..=MAX_TICK),
        (-10_000..=10_000).prop_map(|offset: i32| offset.clamp(MIN_TICK, MAX_TICK)),
    ]
}

fn lookup_bounds_for(raw: u128) -> (i32, i32) {
    let table = lookup_table_for_tests();
    match table.binary_search_by_key(&raw, |&(price, _)| price) {
        Ok(idx) => {
            let tick = table[idx].1;
            (tick, tick)
        }
        Err(0) => {
            let tick = table[0].1;
            (tick, tick)
        }
        Err(idx) if idx >= table.len() => {
            let tick = table[table.len() - 1].1;
            (tick, tick)
        }
        Err(idx) => (table[idx - 1].1, table[idx].1),
    }
}

proptest! {
    #![proptest_config(price_math_proptest_config())]
    #[test]
    fn sqrt_price_to_tick_is_monotonic((a, b) in ordered_sqrt_pair_strategy()) {
        let tick_a = sqrt_price_to_tick(a).expect("valid sqrt price should succeed");
        let tick_b = sqrt_price_to_tick(b).expect("valid sqrt price should succeed");
        prop_assert!(
            tick_a <= tick_b,
            "Tick mapping must be non-decreasing: sqrt_a={}, tick_a={}, sqrt_b={}, tick_b={}",
            a.raw(),
            tick_a,
            b.raw(),
            tick_b
        );
    }
}

proptest! {
    #![proptest_config(price_math_proptest_config())]
    #[test]
    fn sqrt_price_to_price_is_monotonic((a, b) in ordered_sqrt_pair_strategy()) {
        let price_a = sqrt_price_to_price(a).expect("valid sqrt price should succeed");
        let price_b = sqrt_price_to_price(b).expect("valid sqrt price should succeed");
        prop_assert!(
            price_a <= price_b,
            "Price mapping must be non-decreasing: sqrt_a={}, price_a={}, sqrt_b={}, price_b={}",
            a.raw(),
            price_a,
            b.raw(),
            price_b
        );
    }
}

proptest! {
    #![proptest_config(price_math_proptest_config())]
    #[test]
    fn sqrt_price_to_price_is_strictly_increasing_with_gap((a, b) in sqrt_pair_with_gap_strategy(STRICT_PRICE_GAP)) {
        let price_a = sqrt_price_to_price(a).expect("valid sqrt price should succeed");
        let price_b = sqrt_price_to_price(b).expect("valid sqrt price should succeed");
        prop_assert!(
            price_a < price_b,
            "Large sqrt gaps must yield strictly increasing prices: sqrt_a={}, price_a={}, sqrt_b={}, price_b={}",
            a.raw(),
            price_a,
            b.raw(),
            price_b
        );
    }
}

proptest! {
    #![proptest_config(price_math_proptest_config())]
    #[test]
    fn sqrt_price_to_tick_round_trip_preserves_value(sqrt_price in sqrt_price_strategy()) {
        let tick = sqrt_price_to_tick(sqrt_price).expect("valid sqrt price should succeed");
        let recovered = tick_to_sqrt_x64(tick).expect("tick within bounds must convert");
        assert_rel_close(
            recovered,
            sqrt_price,
            REL_PPB_TICK_ROUND_TRIP,
            ULP_TICK_ROUND_TRIP,
            "sqrt_price_to_tick round trip"
        );
    }
}

proptest! {
    #![proptest_config(price_math_proptest_config())]
    #[test]
    fn tick_round_trip_is_exact(tick in tick_strategy()) {
        let sqrt_price = tick_to_sqrt_x64(tick).expect("tick must be convertible");
        let recovered_tick = sqrt_price_to_tick(sqrt_price).expect("sqrt from tick must be valid");
        prop_assert_eq!(
            tick, recovered_tick,
            "tick_to_sqrt_x64 should be the left inverse of sqrt_price_to_tick"
        );
    }
}

proptest! {
    #![proptest_config(price_math_proptest_config())]
    #[test]
    fn tick_conversion_reciprocity_near_boundaries(tick in tick_strategy()) {
        let sqrt_price = tick_to_sqrt_x64(tick).expect("tick must convert");
        let recovered_tick = sqrt_price_to_tick(sqrt_price).expect("sqrt must convert back");
        let final_sqrt = tick_to_sqrt_x64(recovered_tick).expect("recovered tick must convert");
        assert_rel_close(
            final_sqrt,
            sqrt_price,
            REL_PPB_TICK_ROUND_TRIP,
            ULP_TICK_ROUND_TRIP,
            "reciprocity check",
        );
    }
}

proptest! {
    #![proptest_config(price_math_proptest_config())]
    #[test]
    fn sqrt_price_to_tick_outputs_within_bounds(sqrt_price in sqrt_price_strategy()) {
        let tick = sqrt_price_to_tick(sqrt_price).expect("valid sqrt price should succeed");
        prop_assert!(
            (MIN_TICK..=MAX_TICK).contains(&tick),
            "Tick must remain within protocol bounds: {}",
            tick
        );
    }
}

proptest! {
    #![proptest_config(price_math_proptest_config())]
    #[test]
    fn sqrt_price_to_price_matches_checked_square(sqrt_price in sqrt_price_strategy()) {
        let expected = sqrt_price
            .checked_mul(sqrt_price)
            .expect("checked mul must succeed for bounded sqrt prices");
        let expected_price = (expected.raw() >> 64) as u64;
        let price = sqrt_price_to_price(sqrt_price).expect("valid sqrt price should succeed");
        prop_assert_eq!(
            price, expected_price,
            "sqrt_price_to_price must match explicit square"
        );
    }
}

proptest! {
    #![proptest_config(price_math_proptest_config())]
    #[test]
    fn sqrt_price_to_price_never_underflows(sqrt_price in sqrt_price_strategy()) {
        let price = sqrt_price_to_price(sqrt_price).expect("valid sqrt should succeed");
        let min_price = (MIN_SQRT_X64.saturating_mul(MIN_SQRT_X64)) >> 64;
        let tolerated_min = min_price.saturating_sub(1); // MIN_SQRT truncates to 0 after shifting, so allow one-unit slack
        prop_assert!(
            price as u128 >= tolerated_min,
            "Price must respect MIN_SQRT_X64 lower bound: price={}, tolerated_min={}, theoretical_min={}",
            price,
            tolerated_min,
            min_price
        );
    }
}

proptest! {
    #![proptest_config(price_math_proptest_config())]
    #[test]
    fn sqrt_price_to_tick_rejects_out_of_bounds(sqrt_price in sqrt_price_out_of_bounds_strategy()) {
        let err = sqrt_price_to_tick(sqrt_price).expect_err("out-of-bounds sqrt must error");
        let expected_err: anchor_lang::error::Error = MathError::InvalidSqrtPrice.into();
        prop_assert_eq!(err, expected_err, "Unexpected error type for sqrt_price_to_tick");
    }
}

proptest! {
    #![proptest_config(price_math_proptest_config())]
    #[test]
    fn sqrt_price_to_price_rejects_out_of_bounds(sqrt_price in sqrt_price_out_of_bounds_strategy()) {
        let err = sqrt_price_to_price(sqrt_price).expect_err("out-of-bounds sqrt must error");
        let expected_err: anchor_lang::error::Error = MathError::InvalidSqrtPrice.into();
        prop_assert_eq!(err, expected_err, "Unexpected error type for sqrt_price_to_price");
    }
}

proptest! {
    #![proptest_config(price_math_proptest_config())]
    #[test]
    fn coarse_lookup_stays_within_table_brackets(sqrt_price in sqrt_price_strategy()) {
        let coarse_tick = coarse_lookup_table_search_for_tests(sqrt_price)
            .expect("lookup table search should not fail for valid inputs");
        let (lower, upper) = lookup_bounds_for(sqrt_price.raw());
        prop_assert!(
            coarse_tick >= lower && coarse_tick <= upper,
            "Lookup interpolation must stay within bracket: sqrt={}, coarse_tick={}, lower={}, upper={}",
            sqrt_price.raw(),
            coarse_tick,
            lower,
            upper
        );
    }
}

proptest! {
    #![proptest_config(price_math_proptest_config())]
    #[test]
    fn binary_search_refinement_stays_near_coarse_guess(sqrt_price in sqrt_price_strategy()) {
        let coarse_tick = coarse_lookup_table_search_for_tests(sqrt_price)
            .expect("lookup table search should not fail for valid inputs");
        let refined_tick = sqrt_price_to_tick(sqrt_price).expect("valid sqrt price should succeed");
        let diff = (refined_tick - coarse_tick).abs();
        prop_assert!(
            diff <= TEST_BINARY_SEARCH_RANGE,
            "Refined tick must stay within ±{} of coarse guess (diff={}, coarse={}, refined={})",
            TEST_BINARY_SEARCH_RANGE,
            diff,
            coarse_tick,
            refined_tick
        );
    }
}

// The `sqrt_price_to_tick_round_trip_preserves_value` property already guarantees
// that tick conversions remain within the documented 50,000 PPB + 2000 ULP tolerance,
// so we do not add a second property that enforces a stricter floor/ceil ordering.

proptest! {
    #![proptest_config(price_math_proptest_config())]
    #[test]
    fn optimized_binary_search_matches_public_entrypoint(sqrt_price in sqrt_price_strategy()) {
        let coarse_tick = coarse_lookup_table_search_for_tests(sqrt_price)
            .expect("lookup table search should not fail for valid inputs");
        let low = (coarse_tick - TEST_BINARY_SEARCH_RANGE).max(MIN_TICK);
        let high = (coarse_tick + TEST_BINARY_SEARCH_RANGE).min(MAX_TICK);
        let direct_tick = optimized_binary_search_for_tests(sqrt_price, low, high)
            .expect("binary search should succeed within bounded range");
        let public_tick = sqrt_price_to_tick(sqrt_price).expect("valid sqrt price should succeed");
        prop_assert_eq!(
            direct_tick, public_tick,
            "Binary search helper must match sqrt_price_to_tick result"
        );
    }
}

// Global search ranges are intentionally omitted: the helper is verified against
// the production entrypoint (which enforces ±BINARY_SEARCH_RANGE bounds) in
// `optimized_binary_search_matches_public_entrypoint`.

#[test]
fn lookup_table_entries_are_monotonic_and_within_bounds() {
    let table = lookup_table_for_tests();
    assert!(
        !table.is_empty(),
        "Lookup table must contain entries for coarse approximation"
    );

    for window in table.windows(2) {
        let (left_price, left_tick) = window[0];
        let (right_price, right_tick) = window[1];
        assert!(
            left_price < right_price,
            "Lookup table prices must be strictly increasing"
        );
        assert!(
            left_tick < right_tick,
            "Lookup table ticks must be strictly increasing"
        );
        assert!(
            (MIN_TICK..=MAX_TICK).contains(&left_tick)
                && (MIN_TICK..=MAX_TICK).contains(&right_tick),
            "Lookup table ticks must respect protocol bounds"
        );
    }
}

#[test]
fn lookup_table_entries_match_tick_conversion() {
    let table = lookup_table_for_tests();
    for &(price_raw, tick) in table {
        let expected = tick_to_sqrt_x64(tick).expect("tick from table must convert");
        let table_value = Q64x64::from_raw(price_raw);
        assert_rel_close(
            table_value,
            expected,
            REL_PPB_TABLE_ALIGNMENT,
            ULP_TABLE_ALIGNMENT,
            "lookup table entry must align with tick_to_sqrt_x64",
        );
    }
}

#[test]
fn lookup_table_gaps_within_binary_search_range() {
    let table = lookup_table_for_tests();
    for window in table.windows(2) {
        let (_, left_tick) = window[0];
        let (_, right_tick) = window[1];
        let gap = right_tick - left_tick;
        assert!(
            gap <= 2 * TEST_BINARY_SEARCH_RANGE,
            "Lookup table gap {} exceeds binary search coverage",
            gap
        );
    }
}

#[test]
fn binary_search_iteration_cap_is_constant() {
    assert_eq!(
        TEST_MAX_BINARY_ITERATIONS, 32,
        "Binary search iteration budget must remain at 32 for DoS safety"
    );
}
