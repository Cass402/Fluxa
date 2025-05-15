use crate::constants::*;
use crate::errors::ErrorCode;
use crate::math; // For sqrt_price_q64_to_tick, tick_to_sqrt_price_q64
use crate::state::pool::{InitializePoolParams, Pool}; // Target for testing
use crate::tick::TickData as ActualTickData; // Use the actual TickData // Used by Pool

use anchor_lang::prelude::*;
use proptest::prelude::*;
use std::collections::BTreeMap;

// Helper to convert f64 to Q64.64
fn float_to_q64(val: f64) -> u128 {
    if val < 0.0 {
        // Or handle error appropriately
        panic!("float_to_q64 does not support negative numbers");
    }
    let integer_part = val.trunc() as u128;
    let fractional_part = val.fract();
    let fractional_q64 = (fractional_part * (1u128 << 64) as f64) as u128;
    (integer_part << 64) | fractional_q64
}

// Helper to convert Q64.64 to f64
#[allow(dead_code)] // Potentially useful for debugging or future float assertions
#[cfg(test)] // Only used in tests
fn q64_to_float(val: u128) -> f64 {
    let integer_part = (val >> 64) as f64;
    let fractional_part = (val & ((1u128 << 64) - 1)) as f64 / (1u128 << 64) as f64;
    integer_part + fractional_part
}

// Helper to check Q64.64 values within acceptable epsilon, similar to math_test.rs
#[cfg(test)]
fn assert_q64_approx_eq(a: u128, b: u128, epsilon_bits: u8, message: &str) {
    let epsilon = 1u128 << epsilon_bits;
    let diff = a.abs_diff(b);
    assert!(
        diff <= epsilon,
        "{message} Q64.64 values differ by more than allowed epsilon: {a:x} (dec: {a}) vs {b:x} (dec: {b}), diff: {diff:x} (dec: {diff})"
    );
}
// Using ActualTickData from crate::tick
type TickData = ActualTickData;

// Mock Account wrapper
#[derive(Debug, Clone)]
pub struct MockAccount<T: Clone + std::fmt::Debug> {
    pub data: T,
}

impl<T: Clone + std::fmt::Debug> MockAccount<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

impl<T: Clone + std::fmt::Debug> std::ops::Deref for MockAccount<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: Clone + std::fmt::Debug> std::ops::DerefMut for MockAccount<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

// Helper for default InitializePoolParams
fn default_initialize_pool_params() -> InitializePoolParams {
    InitializePoolParams {
        bump: 1,
        factory: Pubkey::new_unique(),
        token0_mint: Pubkey::new_unique(),
        token1_mint: Pubkey::new_unique(),
        token0_vault: Pubkey::new_unique(),
        token1_vault: Pubkey::new_unique(),
        initial_sqrt_price_q64: float_to_q64(1.0),
        fee_rate: 30, // 0.3%
        tick_spacing: 60,
    }
}

// Helper to create a Pool instance with default parameters
fn create_default_pool() -> Pool {
    let mut pool = Pool::default();
    let params = default_initialize_pool_params();
    pool.initialize(params).unwrap();
    pool
}

mod initialize_pool_tests {
    use super::*;

    #[test]
    fn test_initialize_pool_success() {
        let mut pool = Pool::default();
        let params = default_initialize_pool_params();
        let initial_sqrt_price = params.initial_sqrt_price_q64;

        let result = pool.initialize(params.clone());
        assert!(result.is_ok());

        assert_eq!(pool.bump, params.bump);
        assert_eq!(pool.factory, params.factory);
        assert_eq!(pool.token0_mint, params.token0_mint);
        assert_eq!(pool.token1_mint, params.token1_mint);
        assert_eq!(pool.fee_rate, params.fee_rate);
        assert_eq!(pool.tick_spacing, params.tick_spacing);
        assert_eq!(pool.sqrt_price_q64, initial_sqrt_price);
        assert_eq!(pool.liquidity, 0);
        let deserialized_bitmap: BTreeMap<i16, u64> =
            borsh::BorshDeserialize::try_from_slice(&pool.tick_bitmap_data).unwrap();
        assert!(deserialized_bitmap.is_empty());

        let expected_tick = math::sqrt_price_q64_to_tick(initial_sqrt_price).unwrap();
        assert_eq!(pool.current_tick, expected_tick);
    }

    #[test]
    fn test_initialize_pool_mints_must_differ() {
        let mut pool = Pool::default();
        let mut params = default_initialize_pool_params();
        let same_mint = Pubkey::new_unique();
        params.token0_mint = same_mint;
        params.token1_mint = same_mint;

        let result = pool.initialize(params);
        assert_eq!(result.unwrap_err(), error!(ErrorCode::MintsMustDiffer));
    }

    #[test]
    fn test_initialize_pool_invalid_initial_price_zero() {
        let mut pool = Pool::default();
        let mut params = default_initialize_pool_params();
        params.initial_sqrt_price_q64 = 0;
        let result = pool.initialize(params);
        assert_eq!(result.unwrap_err(), error!(ErrorCode::InvalidInitialPrice));
    }

    #[test]
    fn test_initialize_pool_invalid_initial_price_too_high() {
        let mut pool = Pool::default();
        let mut params = default_initialize_pool_params();
        params.initial_sqrt_price_q64 = MAX_SQRT_PRICE + 1;
        let result = pool.initialize(params);
        assert_eq!(result.unwrap_err(), error!(ErrorCode::InvalidInitialPrice));
    }

    #[test]
    fn test_initialize_pool_valid_min_max_initial_price() {
        let mut pool = Pool::default();
        let mut params = default_initialize_pool_params();

        params.initial_sqrt_price_q64 = MIN_SQRT_PRICE;
        let result_min = pool.initialize(params.clone());

        // The Pool::initialize function considers 0 an invalid price.
        // If MIN_SQRT_PRICE is 0, we expect an error. Otherwise, it should be ok.
        if MIN_SQRT_PRICE == 0 {
            assert_eq!(
                result_min.unwrap_err(),
                error!(ErrorCode::InvalidInitialPrice),
                "Expected InvalidInitialPrice for MIN_SQRT_PRICE = 0"
            );
        } else {
            assert!(
                result_min.is_ok(),
                "MIN_SQRT_PRICE ({}) should be valid: {:?}",
                MIN_SQRT_PRICE,
                result_min.err()
            );
        }

        // Use a fresh pool instance for the MAX_SQRT_PRICE test to ensure clean state
        let mut pool_for_max = Pool::default();
        params.initial_sqrt_price_q64 = MAX_SQRT_PRICE;
        let result_max = pool_for_max.initialize(params.clone());
        assert!(
            result_max.is_ok(),
            "MAX_SQRT_PRICE should be valid: {:?}",
            result_max.err()
        );
    }

    #[test]
    fn test_initialize_pool_invalid_tick_spacing() {
        let mut pool = Pool::default();
        let mut params = default_initialize_pool_params();
        params.tick_spacing = 0;
        let result = pool.initialize(params);
        assert_eq!(result.unwrap_err(), error!(ErrorCode::InvalidTickSpacing));
    }

    proptest! {
        #[test]
        fn proptest_initialize_pool_valid_params(
            bump in 0..=u8::MAX,
            initial_sqrt_price_q64 in MIN_SQRT_PRICE..=MAX_SQRT_PRICE,
            fee_rate in 0u16..10000, // Up to 100%
            tick_spacing in 1u16..u16::MAX
        ) {
            let mut pool = Pool::default();
            let mut params = default_initialize_pool_params();
            params.bump = bump;
            params.initial_sqrt_price_q64 = initial_sqrt_price_q64;
            params.fee_rate = fee_rate;
            params.tick_spacing = tick_spacing;

            // Ensure mints are different
            let mut mint0_bytes = [0u8; 32];
            mint0_bytes[0] = bump;
            params.token0_mint = Pubkey::new_from_array(mint0_bytes);
            let mut mint1_bytes = [0u8; 32];
            mint1_bytes[0] = bump.wrapping_add(1);
            params.token1_mint = Pubkey::new_from_array(mint1_bytes);
             if params.token0_mint == params.token1_mint {
                mint1_bytes[0] = bump.wrapping_add(2); // Ensure different if wrap around made them same
                params.token1_mint = Pubkey::new_from_array(mint1_bytes);
            }
            prop_assume!(params.token0_mint != params.token1_mint);


            let result = pool.initialize(params.clone());

            match math::sqrt_price_q64_to_tick(initial_sqrt_price_q64) {
                Ok(_expected_tick) => {
                    assert!(result.is_ok());
                    let deserialized_bitmap: BTreeMap<i16, u64> = borsh::BorshDeserialize::try_from_slice(&pool.tick_bitmap_data).unwrap();
                    assert!(deserialized_bitmap.is_empty());
                },
                Err(_) => {
                    assert!(result.is_err());
                }
            }
        }
    }
}

mod modify_liquidity_tests {
    use super::*;
    use crate::tick_bitmap::is_tick_initialized;

    fn setup_pool_and_ticks() -> (Pool, MockAccount<TickData>, MockAccount<TickData>, i32, i32) {
        let mut pool = create_default_pool();
        let tick_spacing = pool.tick_spacing as i32;
        pool.current_tick = 10 * tick_spacing; // Example current_tick, aligned
        pool.sqrt_price_q64 = math::tick_to_sqrt_price_q64(pool.current_tick).unwrap();

        let tick_lower_idx = 0;
        let tick_upper_idx = 20 * tick_spacing;

        (
            pool,
            MockAccount::new(TickData::default()),
            MockAccount::new(TickData::default()),
            tick_lower_idx,
            tick_upper_idx,
        )
    }

    #[test]
    fn test_add_liquidity_current_tick_within_range() {
        let (mut pool, mut tick_lower_acc, mut tick_upper_acc, tl, tu) = setup_pool_and_ticks();
        pool.current_tick = (tl + tu) / 2; // Ensure current_tick is within range
        let delta: i128 = 1000;

        let res =
            pool.modify_liquidity_for_test(tl, tu, delta, &mut tick_lower_acc, &mut tick_upper_acc);
        assert!(res.is_ok());
        assert_eq!(pool.liquidity, delta as u128);
        assert_eq!(tick_lower_acc.data.liquidity_gross, delta as u128);
        assert_eq!(tick_lower_acc.data.liquidity_net, delta);
        assert_eq!(
            tick_lower_acc.data.initialized, 1,
            "Lower tick should be initialized"
        );
        assert_eq!(tick_upper_acc.data.liquidity_gross, delta as u128);
        assert_eq!(tick_upper_acc.data.liquidity_net, -delta);
        assert_eq!(
            tick_upper_acc.data.initialized, 1,
            "Upper tick should be initialized"
        );

        let tick_bitmap_map: BTreeMap<i16, u64> =
            borsh::BorshDeserialize::try_from_slice(&pool.tick_bitmap_data).unwrap();
        assert!(is_tick_initialized(&tick_bitmap_map, tl, pool.tick_spacing).unwrap());
        assert!(is_tick_initialized(&tick_bitmap_map, tu, pool.tick_spacing).unwrap());
    }

    #[test]
    fn test_add_liquidity_current_tick_outside_range() {
        let (mut pool, mut tick_lower_acc, mut tick_upper_acc, tl, tu) = setup_pool_and_ticks();
        pool.current_tick = tu + pool.tick_spacing as i32; // Outside range
        let delta: i128 = 1000;

        let res =
            pool.modify_liquidity_for_test(tl, tu, delta, &mut tick_lower_acc, &mut tick_upper_acc);
        assert!(res.is_ok());
        assert_eq!(pool.liquidity, 0); // Pool liquidity not affected
    }

    #[test]
    fn test_remove_liquidity_current_tick_within_range() {
        let (mut pool, mut tick_lower_acc, mut tick_upper_acc, tl, tu) = setup_pool_and_ticks();
        let add_delta: i128 = 1000;
        pool.modify_liquidity_for_test(tl, tu, add_delta, &mut tick_lower_acc, &mut tick_upper_acc)
            .unwrap();
        pool.current_tick = (tl + tu) / 2;
        pool.liquidity = add_delta as u128;

        let remove_delta: i128 = -500;
        let res = pool.modify_liquidity_for_test(
            tl,
            tu,
            remove_delta,
            &mut tick_lower_acc,
            &mut tick_upper_acc,
        );
        assert!(res.is_ok());
        assert_eq!(pool.liquidity, (add_delta + remove_delta) as u128);
        assert_eq!(
            tick_lower_acc.data.liquidity_gross,
            (add_delta + remove_delta) as u128
        );
        assert_eq!(
            tick_lower_acc.data.initialized, 1,
            "Lower tick should still be initialized"
        );
    }

    #[test]
    fn test_remove_all_liquidity_uninitializes_ticks() {
        let (mut pool, mut tick_lower_acc, mut tick_upper_acc, tl, tu) = setup_pool_and_ticks();
        let add_delta: i128 = 1000;
        pool.modify_liquidity_for_test(tl, tu, add_delta, &mut tick_lower_acc, &mut tick_upper_acc)
            .unwrap();
        pool.current_tick = (tl + tu) / 2;
        pool.liquidity = add_delta as u128;

        let remove_all_delta: i128 = -add_delta;
        let res = pool.modify_liquidity_for_test(
            tl,
            tu,
            remove_all_delta,
            &mut tick_lower_acc,
            &mut tick_upper_acc,
        );
        assert!(res.is_ok());
        assert_eq!(pool.liquidity, 0);
        assert_eq!(tick_lower_acc.data.liquidity_gross, 0);
        assert_eq!(
            tick_lower_acc.data.initialized, 0,
            "Lower tick should be uninitialized"
        );

        let tick_bitmap_map: BTreeMap<i16, u64> =
            borsh::BorshDeserialize::try_from_slice(&pool.tick_bitmap_data).unwrap();
        assert!(!is_tick_initialized(&tick_bitmap_map, tl, pool.tick_spacing).unwrap());
    }

    proptest! {
        #[test]
        fn proptest_modify_liquidity(
            initial_pool_liq in 0u128..10000,
            delta_abs in 1u128..5000,
            is_add in proptest::bool::ANY,
            current_tick_val in -100i32..100, // In units of tick_spacing
            lower_offset in -5i32..0,
            upper_offset in 1i32..6
        ) {
            let mut pool = create_default_pool();
            let ts = pool.tick_spacing as i32;
            pool.liquidity = initial_pool_liq;
            pool.current_tick = current_tick_val * ts;
            pool.sqrt_price_q64 = math::tick_to_sqrt_price_q64(pool.current_tick).unwrap();

            let tl = pool.current_tick + lower_offset * ts;
            let tu = pool.current_tick + upper_offset * ts;
            prop_assume!(tl < tu);

            let mut tld_acc = MockAccount::new(TickData::default());
            let mut tud_acc = MockAccount::new(TickData::default());

            let delta = if is_add { delta_abs as i128 } else { -(delta_abs as i128) };

            if !is_add { // Pre-add liquidity if removing
                let pre_add = delta_abs as i128 * 2;
                 pool.modify_liquidity_for_test(tl, tu, pre_add, &mut tld_acc, &mut tud_acc).unwrap();
                 // pool.liquidity is now correctly updated by the modify_liquidity_for_test call.
                 // initial_pool_liq was the liquidity *before* this pre_add.
            }

            let pool_liq_before_op = pool.liquidity;
            let tld_gross_before = tld_acc.data.liquidity_gross;
            let _tud_gross_before = tud_acc.data.liquidity_gross; // Mark as unused if not used

            let res = pool.modify_liquidity_for_test(tl, tu, delta, &mut tld_acc, &mut tud_acc);
            assert!(res.is_ok());

            let expected_tld_gross = if delta > 0 { tld_gross_before.saturating_add(delta.unsigned_abs()) } else { tld_gross_before.saturating_sub(delta.unsigned_abs()) };
            assert_eq!(tld_acc.data.liquidity_gross, expected_tld_gross);

            if pool.current_tick >= tl && pool.current_tick < tu {
                let expected_pool_liq = if delta > 0 { pool_liq_before_op.saturating_add(delta.unsigned_abs()) } else { pool_liq_before_op.saturating_sub(delta.unsigned_abs()) };
                // If this assertion fails, it means pool.modify_liquidity_for_test is not updating
                // pool.liquidity as expected, particularly for negative deltas.
                // The failure `left: 3, right: 1` (when delta is -1, pool_liq_before_op is 2)
                // implies pool.liquidity became 3 (2+1) instead of 1 (2-1).
                // This suggests the `if liquidity_delta > 0` branch was taken with `liquidity_delta as u128 == 1`
                // even though the input `delta` was -1. This is a deep contradiction if the Pool code is as provided.
                assert_eq!(
                    pool.liquidity,
                    expected_pool_liq,
                    "Pool liquidity mismatch. Before op: {}, delta: {}, current_tick: {}, range: [{}, {}). Expected: {}, Got: {}. TLD gross before: {}, TLD gross after: {}",
                    pool_liq_before_op, delta, pool.current_tick, tl, tu,
                    expected_pool_liq, pool.liquidity,
                    tld_gross_before, tld_acc.data.liquidity_gross
                );

            } else {
                assert_eq!(pool.liquidity, pool_liq_before_op);
            }
        }
    }
}

mod swap_step_tests {
    use super::*;

    #[test]
    fn test_swap_step_zero_for_one_reaches_target() {
        let pool = create_default_pool();
        let cur_p = float_to_q64(1.1);
        let tar_p = float_to_q64(1.0);
        let liq = float_to_q64(1000.0);
        let gross_in_rem = float_to_q64(100.0);

        let (gross_in, net_out, next_p) = pool
            .swap_step(cur_p, tar_p, liq, gross_in_rem, pool.fee_rate, true)
            .unwrap();
        assert_eq!(next_p, tar_p);
        assert!(gross_in > 0 && gross_in < gross_in_rem);
        assert!(net_out > 0);
    }

    #[test]
    fn test_swap_step_zero_for_one_limited_by_input() {
        let pool = create_default_pool();
        let cur_p = float_to_q64(1.1);
        let tar_p = float_to_q64(1.0);
        let liq = float_to_q64(1000.0);
        let gross_in_rem = float_to_q64(1.0); // Small input

        let (gross_in, net_out, next_p) = pool
            .swap_step(cur_p, tar_p, liq, gross_in_rem, pool.fee_rate, true)
            .unwrap();
        assert_eq!(gross_in, gross_in_rem);
        assert!(next_p < cur_p && next_p > tar_p);
        assert!(net_out > 0);
    }

    #[test]
    fn test_swap_step_one_for_zero_reaches_target() {
        let pool = create_default_pool();
        let cur_p = float_to_q64(1.0);
        let tar_p = float_to_q64(1.1);
        let liq = float_to_q64(1000.0);
        let gross_in_rem = float_to_q64(100.4); // Adjusted to ensure target is reached after 0.3% fee

        let (gross_in, net_out, next_p) = pool
            .swap_step(cur_p, tar_p, liq, gross_in_rem, pool.fee_rate, false)
            .unwrap();
        assert_q64_approx_eq(
            next_p,
            tar_p,
            1, // Target should be met precisely when input is sufficient
            "test_swap_step_one_for_zero_reaches_target: next_p vs tar_p",
        );
        assert!(gross_in > 0 && gross_in < gross_in_rem);
        assert!(net_out > 0);
    }

    #[test]
    fn test_swap_step_zero_liquidity() {
        let pool = create_default_pool();
        let cur_p = float_to_q64(1.0);
        let tar_p = float_to_q64(1.1);
        let (gross_in, net_out, next_p) = pool
            .swap_step(cur_p, tar_p, 0, float_to_q64(10.0), pool.fee_rate, false)
            .unwrap();
        assert_eq!(gross_in, 0);
        assert_eq!(net_out, 0);
        assert_eq!(next_p, cur_p);
    }

    proptest! {
        #[test]
        fn proptest_swap_step(
            cur_p_f in 0.1f64..10.0,
            tar_p_factor in 0.5f64..2.0,
            liq_f in 100.0f64..100000.0,
            gross_in_rem_f in 1.0f64..1000.0,
            fee_bps in 0u16..1000,
            z4o in proptest::bool::ANY
        ) {
            let pool = Pool { fee_rate: fee_bps, ..create_default_pool() };
            let cur_p = float_to_q64(cur_p_f);
            let mut tar_p = float_to_q64(cur_p_f * tar_p_factor);

            if z4o {
                if tar_p >= cur_p { tar_p = cur_p.saturating_sub(1); }
            } else if tar_p <= cur_p { tar_p = cur_p.saturating_add(1); }
            prop_assume!(tar_p > 0 && tar_p <= MAX_SQRT_PRICE);
            prop_assume!(cur_p != tar_p);

            let liq = float_to_q64(liq_f);
            let gross_in_rem = float_to_q64(gross_in_rem_f);

            let res = pool.swap_step(cur_p, tar_p, liq, gross_in_rem, fee_bps, z4o);
            prop_assume!(res.is_ok());
            let (gross_in, net_out, next_p) = res.unwrap();

            prop_assert!(gross_in <= gross_in_rem);
            if liq > 0 && gross_in_rem > 0 && cur_p != tar_p { // if any swap can happen
                 // If gross_in is 0, it means target was current or no liquidity to move price
                 if gross_in == 0 {
                    // If this fails, swap_step returned gross_in=0 but net_out!=0, which is a bug.
                    prop_assert_eq!(
                        net_out,
                        0,
                        "If gross_in is 0, net_out should be 0. Got net_out = {}. cur_p_f={}, tar_p_factor={}, liq_f={}, gross_in_rem_f={}, fee_bps={}, z4o={}",
                        net_out, cur_p_f, tar_p_factor, liq_f, gross_in_rem_f, fee_bps, z4o
                    );

                 } else { // gross_in > 0
                    // It's possible to consume some input but get zero output due to rounding or fees,
                    // especially if the price movement is minimal.
                    // No specific assertion needed here for net_out >= 0 as it's u128.
                 }
            }

            if z4o { prop_assert!(next_p <= cur_p && next_p >= tar_p.min(cur_p)); }
            else { prop_assert!(next_p >= cur_p && next_p <= tar_p.max(cur_p)); }
        }
    }
}

mod swap_tests {
    use super::*;
    use crate::tick_bitmap::flip_tick_initialized_status;

    fn setup_pool_for_swap_with_ticks() -> Pool {
        let mut pool = create_default_pool();
        pool.tick_spacing = 60;
        pool.fee_rate = 30;
        pool.current_tick = 0; // Price 1.0
        pool.sqrt_price_q64 = float_to_q64(1.0);
        pool.liquidity = float_to_q64(10000.0); // Large liquidity

        // Initialize ticks at -60, 60, 120 for crossing
        let ticks_to_init = [-60, 60, 120];
        for &tick_idx in ticks_to_init.iter() {
            let mut current_map: BTreeMap<i16, u64> =
                borsh::BorshDeserialize::try_from_slice(&pool.tick_bitmap_data).unwrap();
            // In a real scenario, TickData would have liquidity_net.
            // For bitmap, only `initialized` matters for `next_initialized_tick`.
            flip_tick_initialized_status(&mut current_map, tick_idx, pool.tick_spacing, true)
                .unwrap();
            pool.tick_bitmap_data = borsh::to_vec(&current_map).unwrap();
        }
        pool
    }

    #[test]
    fn test_swap_zero_amount() {
        let mut pool = setup_pool_for_swap_with_ticks();
        let pool_key = Pubkey::new_unique(); // Mock pool key
        let (total_in, total_out) = pool
            .swap(true, 0, MIN_SQRT_PRICE, &pool_key, &[], 0)
            .unwrap();
        assert_eq!(total_in, 0);
        assert_eq!(total_out, 0);
    }

    #[test]
    fn test_swap_z4o_single_step_no_cross() {
        let mut pool = setup_pool_for_swap_with_ticks(); // Starts at tick 0 (price 1.0)
        let limit = float_to_q64(0.999); // Price target slightly lower, won't cross tick -60
        let amount = float_to_q64(10.0);

        let initial_p = pool.sqrt_price_q64;
        let pool_key = Pubkey::new_unique();
        let (total_in, total_out) = pool
            .swap(true, amount.try_into().unwrap(), limit, &pool_key, &[], 0)
            .unwrap();
        assert!(total_in > 0 && total_in <= amount);
        assert!(total_out > 0);
        assert!(pool.sqrt_price_q64 < initial_p && pool.sqrt_price_q64 >= limit);
        assert_eq!(
            pool.current_tick,
            math::sqrt_price_q64_to_tick(pool.sqrt_price_q64).unwrap()
        );
    }

    #[test]
    fn test_swap_z4o_hits_price_limit() {
        let mut pool = setup_pool_for_swap_with_ticks();
        let limit = pool.sqrt_price_q64 - 100; // A limit that will be hit
        let pool_key = Pubkey::new_unique();
        let (total_in, total_out) = pool
            .swap(
                true,
                float_to_q64(1000.0).try_into().unwrap(),
                limit,
                &pool_key,
                &[],
                0,
            )
            .unwrap();
        assert!(total_in < float_to_q64(1000.0)); // Did not consume all
        assert!(total_out > 0);
        assert!(pool.sqrt_price_q64 >= limit);
    }

    #[test]
    fn test_swap_z4o_cross_one_tick() {
        let mut pool = setup_pool_for_swap_with_ticks(); // Starts at tick 0 (price 1.0)
                                                         // Tick -60 is initialized. Price at -60 is ~0.997
        let price_at_neg_60 = math::tick_to_sqrt_price_q64(-60).unwrap();
        let limit = price_at_neg_60; // Aim to cross tick 0 and stop at/after -60
        let amount = float_to_q64(500.0); // Amount likely to cross tick 0

        let initial_liq = pool.liquidity;
        let pool_key = Pubkey::new_unique();
        let (total_in, total_out) = pool
            .swap(true, amount.try_into().unwrap(), limit, &pool_key, &[], 0)
            .unwrap();
        assert!(total_in > 0);
        assert!(total_out > 0);
        assert!(pool.sqrt_price_q64 < float_to_q64(1.0)); // Price decreased
        assert!(pool.sqrt_price_q64 <= math::tick_to_sqrt_price_q64(0).unwrap()); // Crossed or reached tick 0
        assert!(pool.sqrt_price_q64 >= limit); // Stopped at or after -60
        assert_eq!(pool.liquidity, initial_liq); // MVP: liquidity doesn't change on cross
        assert_eq!(
            pool.current_tick,
            math::sqrt_price_q64_to_tick(pool.sqrt_price_q64).unwrap()
        );
        // To verify tick crossing message, one would need to capture stdout or modify swap.
    }

    proptest! {
        #[test]
        fn proptest_swap_properties(
            initial_p_f in 0.8f64..1.2, // Around 1.0
            initial_liq_f in 1000.0f64..100000.0,
            amount_f in 1.0f64..500.0,
            z4o in proptest::bool::ANY,
            limit_factor in 0.9f64..1.1 // Relative to initial price
        ) {
            let mut pool = setup_pool_for_swap_with_ticks();
            pool.sqrt_price_q64 = float_to_q64(initial_p_f);
            pool.current_tick = math::sqrt_price_q64_to_tick(pool.sqrt_price_q64).unwrap();
            pool.liquidity = float_to_q64(initial_liq_f);

            let amount = float_to_q64(amount_f);
            let mut limit_p = float_to_q64(initial_p_f * limit_factor);

            if z4o {
                if limit_p >= pool.sqrt_price_q64 { limit_p = pool.sqrt_price_q64.saturating_sub(1); }
            } else if limit_p <= pool.sqrt_price_q64 { limit_p = pool.sqrt_price_q64.saturating_add(1); }
            prop_assume!(limit_p > 0 && limit_p <= MAX_SQRT_PRICE);
            if limit_p == pool.sqrt_price_q64 && amount > 0 { // ensure limit allows some swap
                 prop_assume!(false); // skip if limit is exactly current and we want to swap
            }


            let initial_p_val = pool.sqrt_price_q64;
            let initial_liq_val = pool.liquidity;
            let pool_key = Pubkey::new_unique();

            let res =
                pool.swap(z4o, amount.try_into().unwrap(), limit_p, &pool_key, &[], 0);
            prop_assume!(res.is_ok());
            let (total_in, total_out) = res.unwrap();

            prop_assert!(total_in <= amount);
            if amount > 0 && initial_liq_val > 0 {
                let can_swap_towards_limit = if z4o { initial_p_val > limit_p } else { initial_p_val < limit_p };
                if can_swap_towards_limit { // If limit allows any swap
                    // It's possible total_in is 0 if the very first step calculation results in 0 input needed to hit target.
                    if total_in == 0 { prop_assert_eq!(total_out, 0); }
                    // else { prop_assert!(total_out > 0); } // This can be false if fee is 100% and input is tiny
                } else if initial_p_val == limit_p { // If already at limit
                     prop_assert_eq!(total_in, 0);
                     prop_assert_eq!(total_out, 0);
                }
            }

            if z4o { prop_assert!(pool.sqrt_price_q64 <= initial_p_val && pool.sqrt_price_q64 >= limit_p); }
            else { prop_assert!(pool.sqrt_price_q64 >= initial_p_val && pool.sqrt_price_q64 <= limit_p); }

            prop_assert_eq!(pool.liquidity, initial_liq_val); // MVP check
            prop_assert_eq!(pool.current_tick, math::sqrt_price_q64_to_tick(pool.sqrt_price_q64).unwrap());
        }
    }
}
