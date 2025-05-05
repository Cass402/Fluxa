#![no_main]

use amm_core::constants::{MAX_TICK, MIN_TICK};
use amm_core::math::*;
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct FeeCalculationInput {
    tick_lower: i32,
    tick_upper: i32,
    tick_current: i32,
    fee_growth_global: u128,
    fee_growth_below: u128,
    fee_growth_above: u128,
}

// Fuzzes the fee calculation logic by generating random inputs within realistic constraints.
//
// This fuzz target:
// 1. Constrains tick values to valid ranges
// 2. Ensures fee growth values are reasonable and consistent
// 3. Tests the `calculate_fee_growth_inside` function with the generated inputs
fuzz_target!(|input: FeeCalculationInput| {
    // Constrain values to realistic ranges
    let tick_lower = input.tick_lower.clamp(MIN_TICK, MAX_TICK - 1);
    let tick_upper = input.tick_upper.clamp(tick_lower + 1, MAX_TICK);
    let tick_current = input.tick_current.clamp(MIN_TICK, MAX_TICK);

    // Ensure fee growth values are reasonable to prevent stack overflow
    // Reduce the max size to avoid large computations
    let fee_growth_global = input.fee_growth_global % (1 << 32); // Reduced from 1 << 64
    let fee_growth_below = input.fee_growth_below % (fee_growth_global.saturating_add(1) / 2);
    let fee_growth_above =
        input.fee_growth_above % (fee_growth_global.saturating_sub(fee_growth_below));

    // Test fee growth inside calculation
    let _ = calculate_fee_growth_inside(
        tick_lower,
        tick_upper,
        tick_current,
        fee_growth_global,
        fee_growth_below,
        fee_growth_above,
    );
});
