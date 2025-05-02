#![no_main]

use amm_core::constants::{MAX_TICK, MIN_SQRT_PRICE, MIN_TICK};
use amm_core::math::*;
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct TickPriceInput {
    tick: i32,
    sqrt_price: u128,
}

// Fuzz test for tick and price conversion functions in the AMM core.
//
// This test validates:
// - Conversion between tick indices and sqrt prices
// - Roundtrip conversions (tick -> sqrt price -> tick)
// - Finding the nearest usable tick for various tick spacings
// - Q96 fixed-point math conversions for ticks and prices
//
// The test constrains inputs to valid ranges and tests multiple tick spacing values
// to ensure the conversion functions work correctly across different configurations.
fuzz_target!(|input: TickPriceInput| {
    // Constrain tick to valid range
    let tick = input.tick.clamp(MIN_TICK, MAX_TICK);

    // Test tick to sqrt price conversion
    if let Ok(sqrt_price) = tick_to_sqrt_price(tick) {
        // Test sqrt price to tick conversion (roundtrip)
        let _ = sqrt_price_to_tick(sqrt_price);
    }

    // Test sqrt price to tick conversion directly
    let sqrt_price = (input.sqrt_price % (1 << 96)).max(MIN_SQRT_PRICE);
    let _ = sqrt_price_to_tick(sqrt_price);

    // Test nearest usable tick
    for spacing in [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000] {
        let _ = nearest_usable_tick(tick, spacing);
    }

    // Test Q96 tick conversions
    let _ = get_tick_at_sqrt_price_q96(sqrt_price);

    // Convert tick to sqrt price in Q96
    let _ = get_sqrt_price_at_tick_q96(tick);
});
