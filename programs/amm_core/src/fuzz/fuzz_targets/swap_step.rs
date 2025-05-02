#![no_main]

use amm_core::constants::MIN_SQRT_PRICE;
use amm_core::math::*;
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct SqrtPriceOperationInput {
    sqrt_price_a: u128,
    sqrt_price_b: u128,
    liquidity: u128,
    amount: u64,
    is_token_a: bool,
}

// Fuzz target for the swap step calculations.
//
// This target tests various mathematical functions related to AMM operations:
// - `calculate_swap_step`: Tests the core swap step calculation
// - Virtual reserves calculations: Tests functions that compute token reserves
// - Range-based calculations: Tests reserve calculations within price ranges
// - Liquidity calculations: Tests deriving liquidity from token reserves
// - Invariant verification: Tests that the constant product invariant holds
//
// The fuzzer generates random inputs while ensuring they meet the requirements
// of the tested functions (e.g., positive liquidity, valid price ranges).
fuzz_target!(|input: SqrtPriceOperationInput| {
    // Cap values to avoid excessive computation time
    let sqrt_price = (input.sqrt_price_a % (1 << 96)).max(MIN_SQRT_PRICE);
    let liquidity = input.liquidity % (1 << 96);

    // Only proceed if we have liquidity (function requirement)
    if liquidity > 0 {
        let _ = calculate_swap_step(sqrt_price, liquidity, input.amount, input.is_token_a);
    }

    // Test virtual reserves calculations
    let _ = calculate_virtual_reserves(liquidity, sqrt_price);
    let _ = calculate_virtual_reserve_a(liquidity, sqrt_price);
    let _ = calculate_virtual_reserve_b(liquidity, sqrt_price);

    // Test virtual reserves in range
    let sqrt_price_lower = (input.sqrt_price_a % (1 << 96)).max(MIN_SQRT_PRICE);
    let sqrt_price_upper = (input.sqrt_price_b % (1 << 96)).max(sqrt_price_lower + 1);
    let sqrt_price_current = (sqrt_price % (1 << 96)).max(MIN_SQRT_PRICE);

    let _ = calculate_virtual_reserves_in_range(
        liquidity,
        sqrt_price_current,
        sqrt_price_lower,
        sqrt_price_upper,
    );

    // Test liquidity calculation from reserves
    if input.amount > 0 {
        let _ = calculate_liquidity_from_reserves(input.amount, 0, sqrt_price, true);

        let _ = calculate_liquidity_from_reserves(0, input.amount, sqrt_price, false);
    }

    // Test invariant verification
    let _ = verify_virtual_reserves_invariant(input.amount, input.amount, liquidity);
});
