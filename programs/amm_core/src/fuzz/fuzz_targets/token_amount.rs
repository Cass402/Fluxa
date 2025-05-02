#![no_main]

use amm_core::constants::MIN_SQRT_PRICE;
use amm_core::math::*;
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

/// Custom struct for fuzzing token amount calculations
#[derive(Arbitrary, Debug)]
struct TokenAmountInput {
    liquidity: u128,
    sqrt_price_lower: u128,
    sqrt_price_upper: u128,
    sqrt_price_current: u128,
}

// Fuzzes token amount calculations to ensure they handle various inputs correctly.
//
// This fuzz target tests:
// 1. Token amount calculations from liquidity (both regular and Q96 versions)
// 2. Amount delta calculations for price ranges
//
// The test caps input values to avoid excessive computation time and ensures
// that sqrt price values are within valid ranges.
fuzz_target!(|input: TokenAmountInput| {
    // Cap values to avoid excessive computation time
    let liquidity = input.liquidity % (1 << 96);
    let sqrt_price_lower = (input.sqrt_price_lower % (1 << 96)).max(MIN_SQRT_PRICE);
    let sqrt_price_upper = (input.sqrt_price_upper % (1 << 96)).max(sqrt_price_lower + 1);
    let sqrt_price_current = (input.sqrt_price_current % (1 << 96)).max(MIN_SQRT_PRICE);

    // Test token amount calculations
    let _ = get_token_a_from_liquidity(
        liquidity,
        sqrt_price_lower,
        sqrt_price_upper,
        sqrt_price_current,
    );

    let _ = get_token_b_from_liquidity(
        liquidity,
        sqrt_price_lower,
        sqrt_price_upper,
        sqrt_price_current,
    );

    // Test Q96 versions as well
    let sqrt_price_lower_q96 = match convert_sqrt_price_to_q96(sqrt_price_lower) {
        Ok(v) => v,
        Err(_) => return,
    };

    let sqrt_price_upper_q96 = match convert_sqrt_price_to_q96(sqrt_price_upper) {
        Ok(v) => v,
        Err(_) => return,
    };

    let sqrt_price_current_q96 = match convert_sqrt_price_to_q96(sqrt_price_current) {
        Ok(v) => v,
        Err(_) => return,
    };

    let _ = get_token_a_from_liquidity_q96(
        liquidity,
        sqrt_price_lower_q96,
        sqrt_price_upper_q96,
        sqrt_price_current_q96,
    );

    let _ = get_token_b_from_liquidity_q96(
        liquidity,
        sqrt_price_lower_q96,
        sqrt_price_upper_q96,
        sqrt_price_current_q96,
    );

    // Test amount delta calculations
    let _ =
        get_amount_a_delta_for_price_range(liquidity, sqrt_price_lower, sqrt_price_upper, false);

    let _ = get_amount_a_delta_for_price_range(liquidity, sqrt_price_lower, sqrt_price_upper, true);

    let _ =
        get_amount_b_delta_for_price_range(liquidity, sqrt_price_lower, sqrt_price_upper, false);

    let _ = get_amount_b_delta_for_price_range(liquidity, sqrt_price_lower, sqrt_price_upper, true);
});
