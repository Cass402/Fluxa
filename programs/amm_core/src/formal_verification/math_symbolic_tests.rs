//! Symbolic execution tests for the math module
//!
//! This file contains formal verification tests using the Kani Rust Verifier
//! to prove mathematical correctness and absence of runtime errors like
//! integer overflow, division by zero, and other undefined behaviors.
//!
//! To run these tests:
//! 1. Install Kani: https://github.com/model-checking/kani
//! 2. Run: `cargo kani --harness <harness_name>`
//!
//! Example: `cargo kani --harness verify_sqrt_price_to_price`

#![cfg(kani)]

use crate::math::*;

// Configuration macros for Kani
#[allow(unused_imports)]
use kani::*;

/// Verifies that sqrt_price_to_price and price_to_sqrt_price functions are inverses of each other
/// within acceptable rounding error
#[kani::proof]
#[kani::unwind(4)]
pub fn verify_sqrt_price_to_price() {
    // Create a symbolic u128 value for sqrt_price
    #[cfg(kani)]
    let sqrt_price = kani::any();

    // For non-kani environments, use a fixed test value
    #[cfg(not(kani))]
    let sqrt_price = 1_000_000_000_000_000_000u128;

    // Constrain the symbolic value to avoid extreme values that might cause overflow
    kani::assume(sqrt_price > 0 && sqrt_price < (u128::MAX >> 2));

    // Calculate price from sqrt_price
    let price = sqrt_price_to_price(sqrt_price).unwrap();

    // Calculate sqrt_price from price
    let sqrt_price_roundtrip = price_to_sqrt_price(price).unwrap();

    // Calculate the error (should be very small due to rounding)
    let error = if sqrt_price > sqrt_price_roundtrip {
        sqrt_price - sqrt_price_roundtrip
    } else {
        sqrt_price_roundtrip - sqrt_price
    };

    // Define acceptable error threshold (allowing for rounding)
    // Note: The exact threshold may need adjustment based on your protocol's precision requirements
    let max_relative_error = sqrt_price / 1_000_000; // 0.0001%

    // Verify that the error is within acceptable bounds
    #[cfg(kani)]
    kani::assert(
        error <= max_relative_error,
        "Sqrt price to price conversion error. More than 0.0001% difference",
    );

    // For non-kani environments, just print the result
    #[cfg(not(kani))]
    println!("Error: {}, Max allowed: {}", error, max_relative_error);
}

/// Verifies that tick_index_to_sqrt_price and sqrt_price_to_tick_index are inverses
/// within acceptable rounding error (typically ±1 tick)
#[kani::proof]
#[kani::unwind(4)]
fn verify_tick_index_to_sqrt_price_roundtrip() {
    // Create a symbolic i32 value for tick_index
    let tick_index = kani::any();

    // Constrain the symbolic value to a valid tick range
    // Adjust these bounds based on your protocol's tick range
    #[cfg(kani)]
    kani::assume(tick_index >= -887272 && tick_index <= 887272);

    // Convert tick_index to sqrt_price
    let sqrt_price = tick_to_sqrt_price(tick_index).unwrap();

    // Convert sqrt_price back to tick_index
    let tick_index_roundtrip = sqrt_price_to_tick(sqrt_price).unwrap();

    // Calculate the error (should be at most 1 tick due to rounding)
    let error = (tick_index - tick_index_roundtrip).abs();

    // Verify that the error is at most 1 tick
    kani::assert(
        error <= 1,
        "Tick index conversion error. More than 1 tick difference",
    );
}

/// Verifies that adding token0 (base token) decreases the sqrt price
#[kani::proof]
#[kani::unwind(4)]
fn verify_token0_decreases_price() {
    // Create symbolic values
    #[cfg(kani)]
    let sqrt_price = kani::any();
    #[cfg(kani)]
    let liquidity = kani::any();
    #[cfg(kani)]
    let amount = kani::any();

    // Constrain the symbolic values to avoid extreme values
    #[cfg(kani)]
    kani::assume(sqrt_price > 0 && sqrt_price < (u128::MAX >> 3));
    #[cfg(kani)]
    kani::assume(liquidity > 0 && liquidity < (u128::MAX >> 3));
    #[cfg(kani)]
    kani::assume(amount > 0 && amount < (u64::MAX >> 2));

    // Calculate next sqrt price after adding token0
    let next_sqrt_price =
        get_next_sqrt_price_from_amount0_exact_in(sqrt_price, liquidity, amount, true);

    // Verify that adding token0 decreases the price
    #[cfg(kani)]
    kani::assert(
        next_sqrt_price < sqrt_price,
        "Next sqrt price should be less than the current sqrt price",
    );
}

/// Verifies that adding token1 (quote token) increases the sqrt price
#[kani::proof]
#[kani::unwind(4)]
fn verify_token1_increases_price() {
    // Create symbolic values
    #[cfg(kani)]
    let sqrt_price = kani::any();
    #[cfg(kani)]
    let liquidity = kani::any();
    #[cfg(kani)]
    let amount = kani::any();

    // Constrain the symbolic values to avoid extreme values
    #[cfg(kani)]
    kani::assume(sqrt_price > 0 && sqrt_price < (u128::MAX >> 3));
    #[cfg(kani)]
    kani::assume(liquidity > 0 && liquidity < (u128::MAX >> 3));
    #[cfg(kani)]
    kani::assume(amount > 0 && amount < (u64::MAX >> 2));

    // Calculate next sqrt price after adding token1
    let next_sqrt_price =
        get_next_sqrt_price_from_amount1_exact_in(sqrt_price, liquidity, amount, true);

    // Verify that adding token1 increases the price
    #[cfg(kani)]
    kani::assert(
        next_sqrt_price > sqrt_price,
        "Next sqrt price should be greater than the current sqrt price",
    );
}

/// Verifies the liquidity conservation property when converting between
/// token amounts and liquidity
#[kani::proof]
#[kani::unwind(6)]
fn verify_liquidity_conservation() {
    // Create symbolic values
    #[cfg(kani)]
    let sqrt_price_lower = kani::any();
    #[cfg(kani)]
    let sqrt_price_upper = kani::any();
    #[cfg(kani)]
    let liquidity = kani::any();

    // Constrain the symbolic values
    #[cfg(kani)]
    kani::assume(sqrt_price_lower > 0 && sqrt_price_lower < (u128::MAX >> 4));
    #[cfg(kani)]
    kani::assume(sqrt_price_upper > sqrt_price_lower && sqrt_price_upper < (u128::MAX >> 4));
    #[cfg(kani)]
    kani::assume(liquidity > 0 && liquidity < (u128::MAX >> 4));

    // Calculate token amounts needed for the given liquidity
    let amount0 = get_amount0_delta(sqrt_price_lower, sqrt_price_upper, liquidity, true);

    let amount1 = get_amount1_delta(sqrt_price_lower, sqrt_price_upper, liquidity, true);

    // Calculate the liquidity from the token amounts
    let liquidity_from_amounts = get_liquidity_from_amounts(
        sqrt_price_lower,
        sqrt_price_upper,
        amount0 as u128,
        amount1 as u128,
    );

    // Calculate the relative error
    let error = if liquidity > liquidity_from_amounts {
        liquidity - liquidity_from_amounts
    } else {
        liquidity_from_amounts - liquidity
    };

    // Define acceptable error threshold
    let max_relative_error = liquidity / 1_000_000; // 0.0001%

    // Verify that the error is within acceptable bounds
    #[cfg(kani)]
    kani::assert(
        error <= max_relative_error,
        "Liquidity conservation error exceeds acceptable bounds",
    );
}

/// Verifies that fee calculation never exceeds the amount
#[kani::proof]
#[kani::unwind(4)]
fn verify_fee_calculation() {
    // Create symbolic values
    #[cfg(kani)]
    let amount = kani::any();
    #[cfg(kani)]
    let fee_rate = kani::any();

    // Constrain the symbolic values
    #[cfg(kani)]
    kani::assume(amount < (u64::MAX >> 2));
    #[cfg(kani)]
    kani::assume(fee_rate <= 10000); // Max 100% in basis points

    // Calculate fee
    let fee_amount = calculate_fee(amount, fee_rate);

    // Verify that fee never exceeds amount
    #[cfg(kani)]
    kani::assert(
        fee_amount <= amount,
        "Fee amount exceeds the original amount",
    );

    // Verify that fee is correctly calculated
    let expected_fee = (amount as u128) * (fee_rate as u128) / 10000u128;
    let difference = if expected_fee > (fee_amount as u128) {
        expected_fee - (fee_amount as u128)
    } else {
        (fee_amount as u128) - expected_fee
    };

    // Allow for a small rounding error
    #[cfg(kani)]
    kani::assert(difference <= 1, "Fee calculation error exceeds 1 unit");
}

/// Verifies that tick spacing calculations maintain correct ordering
#[kani::proof]
#[kani::unwind(4)]
fn verify_tick_spacing_consistency() {
    // Create symbolic values
    #[cfg(kani)]
    let tick_index = kani::any();
    #[cfg(kani)]
    let tick_spacing = kani::any();

    // Constrain the symbolic values
    #[cfg(kani)]
    kani::assume(tick_index >= -887272 && tick_index <= 887272);
    #[cfg(kani)]
    kani::assume(tick_spacing > 0 && tick_spacing <= 100);

    // Calculate the nearest spaced tick
    let nearest_tick = nearest_usable_tick(tick_index, tick_spacing);

    // Verify that the nearest tick is properly spaced
    #[cfg(kani)]
    kani::assert(
        nearest_tick % tick_spacing == 0,
        "Nearest tick is not spaced correctly",
    );

    // Verify that the nearest tick is within half a spacing of the original tick
    let distance = (tick_index - nearest_tick).abs();
    #[cfg(kani)]
    kani::assert(
        distance <= tick_spacing / 2,
        "Distance exceeds half of tick spacing",
    );
}

/// Verifies get_amount_delta functions with zero liquidity
#[kani::proof]
#[kani::unwind(4)]
fn verify_zero_liquidity_returns_zero() {
    // Create symbolic values
    #[cfg(kani)]
    let sqrt_price_a = kani::any();
    #[cfg(kani)]
    let sqrt_price_b = kani::any();

    // Constrain the symbolic values
    #[cfg(kani)]
    kani::assume(sqrt_price_a > 0 && sqrt_price_a < (u128::MAX >> 4));
    #[cfg(kani)]
    kani::assume(sqrt_price_b > 0 && sqrt_price_b < (u128::MAX >> 4));
    #[cfg(kani)]
    kani::assume(sqrt_price_a != sqrt_price_b);

    // Get amount deltas with zero liquidity
    let amount0 = get_amount0_delta(sqrt_price_a, sqrt_price_b, 0, true);

    let amount1 = get_amount1_delta(sqrt_price_a, sqrt_price_b, 0, true);

    // Verify that both amounts are zero
    #[cfg(kani)]
    kani::assert(amount0 == 0, "Amount0 should be zero");
    #[cfg(kani)]
    kani::assert(amount1 == 0, "Amount1 should be zero");
}

/// Verifies monotonicity of sqrt price and tick index relationships
#[kani::proof]
#[kani::unwind(4)]
fn verify_sqrt_price_tick_monotonicity() {
    // Create symbolic values
    #[cfg(kani)]
    let sqrt_price1 = kani::any();
    #[cfg(kani)]
    let sqrt_price2 = kani::any();

    // Constrain the symbolic values
    #[cfg(kani)]
    kani::assume(sqrt_price1 > 0 && sqrt_price1 < (u128::MAX >> 4));
    #[cfg(kani)]
    kani::assume(sqrt_price2 > 0 && sqrt_price2 < (u128::MAX >> 4));
    #[cfg(kani)]
    kani::assume(sqrt_price1 < sqrt_price2);

    // Convert to tick indices
    let tick1 = sqrt_price_to_tick(sqrt_price1).unwrap();
    let tick2 = sqrt_price_to_tick(sqrt_price2).unwrap();

    // Verify monotonicity: if sqrt_price1 < sqrt_price2, then tick1 <= tick2
    #[cfg(kani)]
    kani::assert(
        tick1 <= tick2,
        "Tick index is not monotonic with sqrt price",
    );
}

/// Verifies that swap calculations maintain correct pricing direction
#[kani::proof]
#[cfg_attr(kani, kani::unwind(5))]
fn verify_swap_price_direction() {
    // Create symbolic values
    #[cfg(kani)]
    let sqrt_price = kani::any();
    #[cfg(kani)]
    let liquidity = kani::any();
    #[cfg(kani)]
    let amount_in = kani::any();

    // Constrain the symbolic values
    #[cfg(kani)]
    kani::assume(sqrt_price > 0 && sqrt_price < (u128::MAX >> 4));
    #[cfg(kani)]
    kani::assume(liquidity > 0 && liquidity < (u128::MAX >> 4));
    #[cfg(kani)]
    kani::assume(amount_in > 0 && amount_in < (u64::MAX >> 2));

    // Calculate next price after token0 in (price should decrease)
    let next_price_0_in =
        get_next_sqrt_price_from_amount0_exact_in(sqrt_price, liquidity, amount_in, true);

    // Calculate next price after token1 in (price should increase)
    let next_price_1_in =
        get_next_sqrt_price_from_amount1_exact_in(sqrt_price, liquidity, amount_in, true);

    // Verify that adding token0 decreases the price
    #[cfg(kani)]
    kani::assert(
        next_price_0_in < sqrt_price,
        "Next price after token0 in should be less than the current price",
    );

    // Verify that adding token1 increases the price
    #[cfg(kani)]
    kani::assert(
        next_price_1_in > sqrt_price,
        "Next price after token1 in should be greater than the current price",
    );
}

/// Verifies that get_amount0_delta and get_amount1_delta handle price ordering correctly
#[kani::proof]
#[kani::unwind(4)]
fn verify_get_amount_delta_price_ordering() {
    // Create symbolic values
    #[cfg(kani)]
    let sqrt_price_a = kani::any();
    #[cfg(kani)]
    let sqrt_price_b = kani::any();
    #[cfg(kani)]
    let liquidity = kani::any();

    // Constrain the symbolic values
    #[cfg(kani)]
    kani::assume(sqrt_price_a > 0 && sqrt_price_a < (u128::MAX >> 4));
    #[cfg(kani)]
    kani::assume(sqrt_price_b > 0 && sqrt_price_b < (u128::MAX >> 4));
    #[cfg(kani)]
    kani::assume(sqrt_price_a != sqrt_price_b);
    #[cfg(kani)]
    kani::assume(liquidity > 0 && liquidity < (u128::MAX >> 4));

    // Calculate amount deltas
    let amount0_ab = get_amount0_delta(sqrt_price_a, sqrt_price_b, liquidity, true);
    let amount0_ba = get_amount0_delta(sqrt_price_b, sqrt_price_a, liquidity, true);

    let amount1_ab = get_amount1_delta(sqrt_price_a, sqrt_price_b, liquidity, true);
    let amount1_ba = get_amount1_delta(sqrt_price_b, sqrt_price_a, liquidity, true);

    // Verify that the results are the same regardless of price ordering
    #[cfg(kani)]
    kani::assert(
        amount0_ab == amount0_ba,
        "Amount0 delta should be the same regardless of price ordering",
    );
    #[cfg(kani)]
    kani::assert(
        amount1_ab == amount1_ba,
        "Amount1 delta should be the same regardless of price ordering",
    );
}

/// Verifies that calculate_fee never returns more than the input amount
#[kani::proof]
#[kani::unwind(4)]
fn verify_calculate_fee_bounds() {
    // Create symbolic values
    #[cfg(kani)]
    let amount = kani::any();
    #[cfg(kani)]
    let fee_rate = kani::any();

    // Constrain fee_rate to valid basis points (0-10000)
    #[cfg(kani)]
    kani::assume(fee_rate <= 10000);

    // Calculate fee
    let fee = calculate_fee(amount, fee_rate);

    // Verify that fee is not more than amount
    #[cfg(kani)]
    kani::assert(fee <= amount, "Fee should not exceed the original amount");

    // For 100% fee (10000 basis points), fee should equal amount
    if fee_rate == 10000 {
        #[cfg(kani)]
        kani::assert(
            fee == amount,
            "For 100% fee, fee should equal the original amount",
        );
    }

    // For 0% fee, fee should be 0
    if fee_rate == 0 {
        #[cfg(kani)]
        kani::assert(fee == 0, "For 0% fee, fee should be zero");
    }
}

/// Verifies that get_liquidity_from_amounts handles various edge cases correctly
#[kani::proof]
#[kani::unwind(5)]
fn verify_get_liquidity_from_amounts_edge_cases() {
    // Create symbolic values
    #[cfg(kani)]
    let sqrt_price_lower = kani::any();
    #[cfg(kani)]
    let sqrt_price_upper = kani::any();
    #[cfg(kani)]
    let amount0 = kani::any();
    #[cfg(kani)]
    let amount1 = kani::any();

    // Constrain the symbolic values
    #[cfg(kani)]
    kani::assume(sqrt_price_lower > 0 && sqrt_price_lower < (u128::MAX >> 4));
    #[cfg(kani)]
    kani::assume(sqrt_price_upper > sqrt_price_lower && sqrt_price_upper < (u128::MAX >> 4));

    // Case 1: both amounts are zero
    let liquidity_zero = get_liquidity_from_amounts(sqrt_price_lower, sqrt_price_upper, 0, 0);

    // Verify that zero amounts result in zero liquidity
    #[cfg(kani)]
    kani::assert(
        liquidity_zero == 0,
        "Liquidity should be zero when both amounts are zero",
    );

    // Case 2: only amount0 is provided
    let liquidity_only0 =
        get_liquidity_from_amounts(sqrt_price_lower, sqrt_price_upper, amount0, 0);

    // If amount0 is non-zero, liquidity should be non-zero
    if amount0 > 0 {
        #[cfg(kani)]
        kani::assert(
            liquidity_only0 > 0,
            "Liquidity should be non-zero when amount0 is provided",
        );
    } else {
        #[cfg(kani)]
        kani::assert(
            liquidity_only0 == 0,
            "Liquidity should be zero when amount0 is zero",
        );
    }

    // Case 3: only amount1 is provided
    let liquidity_only1 =
        get_liquidity_from_amounts(sqrt_price_lower, sqrt_price_upper, 0, amount1);

    // If amount1 is non-zero, liquidity should be non-zero
    if amount1 > 0 {
        #[cfg(kani)]
        kani::assert(
            liquidity_only1 > 0,
            "Liquidity should be non-zero when amount1 is provided",
        );
    } else {
        #[cfg(kani)]
        kani::assert(
            liquidity_only1 == 0,
            "Liquidity should be zero when amount1 is zero",
        );
    }

    // Case 4: both amounts are provided
    let liquidity_both =
        get_liquidity_from_amounts(sqrt_price_lower, sqrt_price_upper, amount0, amount1);
    // If both amounts are non-zero, liquidity should be non-zero
    if amount0 > 0 && amount1 > 0 {
        #[cfg(kani)]
        kani::assert(
            liquidity_both > 0,
            "Liquidity should be non-zero when both amounts are provided",
        );
    } else {
        #[cfg(kani)]
        kani::assert(
            liquidity_both == 0,
            "Liquidity should be zero when at least one amount is zero",
        );
    }
}

/// Verifies that tick arithmetic behaves correctly for extreme values
#[kani::proof]
#[kani::unwind(4)]
fn verify_tick_arithmetic_extremes() {
    // Test minimum and maximum ticks
    // Adjust these values according to your protocol's configuration
    let min_tick = -887272;
    let max_tick = 887272;

    // Calculate sqrt prices for extreme ticks
    let min_sqrt_price = tick_to_sqrt_price(min_tick).unwrap();
    let max_sqrt_price = tick_to_sqrt_price(max_tick).unwrap();

    // Verify that the sqrt prices are valid (non-zero, finite)
    #[cfg(kani)]
    kani::assert(min_sqrt_price > 0, "Min sqrt price should be positive");
    #[cfg(kani)]
    kani::assert(max_sqrt_price > 0, "Max sqrt price should be positive");
    #[cfg(kani)]
    kani::assert(
        max_sqrt_price > min_sqrt_price,
        "Max sqrt price should be greater than min sqrt price",
    );

    // Verify that converting back to ticks gives values close to the originals
    let min_tick_roundtrip = sqrt_price_to_tick(min_sqrt_price).unwrap();
    let max_tick_roundtrip = sqrt_price_to_tick(max_sqrt_price).unwrap();

    // Allow for at most 1 tick rounding error
    #[cfg(kani)]
    kani::assert(
        (min_tick - min_tick_roundtrip).abs() <= 1,
        "Min tick conversion error exceeds 1 tick",
    );
    #[cfg(kani)]
    kani::assert(
        (max_tick - max_tick_roundtrip).abs() <= 1,
        "Max tick conversion error exceeds 1 tick",
    );
}

/// Verifies that get_next_sqrt_price functions handle zero amount correctly
#[kani::proof]
#[kani::unwind(4)]
fn verify_next_sqrt_price_zero_amount() {
    // Create symbolic values
    #[cfg(kani)]
    let sqrt_price = kani::any();
    #[cfg(kani)]
    let liquidity = kani::any();

    // Constrain the symbolic values
    #[cfg(kani)]
    kani::assume(sqrt_price > 0 && sqrt_price < (u128::MAX >> 4));
    #[cfg(kani)]
    kani::assume(liquidity > 0 && liquidity < (u128::MAX >> 4));

    // Calculate next sqrt prices with zero amount
    let next_sqrt_price0 =
        get_next_sqrt_price_from_amount0_exact_in(sqrt_price, liquidity, 0, true);

    let next_sqrt_price1 =
        get_next_sqrt_price_from_amount1_exact_in(sqrt_price, liquidity, 0, true);

    // Verify that prices don't change with zero amount
    #[cfg(kani)]
    kani::assert(
        next_sqrt_price0 == sqrt_price,
        "Next sqrt price should be equal to the current sqrt price when amount is zero",
    );
    #[cfg(kani)]
    kani::assert(
        next_sqrt_price1 == sqrt_price,
        "Next sqrt price should be equal to the current sqrt price when amount is zero",
    );
}

/// Verifies that get_next_sqrt_price functions handle zero liquidity correctly
#[kani::proof]
#[kani::unwind(4)]
fn verify_next_sqrt_price_zero_liquidity() {
    // Create symbolic values
    #[cfg(kani)]
    let sqrt_price = kani::any();
    #[cfg(kani)]
    let amount = kani::any();

    // Constrain the symbolic values
    #[cfg(kani)]
    kani::assume(sqrt_price > 0 && sqrt_price < (u128::MAX >> 4));
    #[cfg(kani)]
    kani::assume(amount > 0 && amount < (u64::MAX >> 2));

    // Note: These functions should handle division by zero properly
    // Either by returning the original price or by having proper checks

    // Calculate next sqrt prices with zero liquidity
    let next_sqrt_price0 = get_next_sqrt_price_from_amount0_exact_in(sqrt_price, 0, amount, true);

    let next_sqrt_price1 = get_next_sqrt_price_from_amount1_exact_in(sqrt_price, 0, amount, true);

    // We assume the implementation handles this by returning some valid value
    // (specific behavior depends on implementation, but should not panic)
    #[cfg(kani)]
    kani::assert(next_sqrt_price0 > 0, "Next sqrt price should be valid");
    #[cfg(kani)]
    kani::assert(next_sqrt_price1 > 0, "Next sqrt price should be valid");
}

/// Main entry point for running all symbolic verification tests
/// This function can be used as a wrapper to run all verifications
#[kani::proof]
#[kani::unwind(6)]
fn verify_all_math_properties() {
    // Individual verification functions can be called here
    // Or they can be run separately with specific unwinding bounds

    // Core arithmetic verifications
    verify_sqrt_price_to_price();
    verify_tick_index_to_sqrt_price_roundtrip();

    // Swap direction verifications
    verify_token0_decreases_price();
    verify_token1_increases_price();

    // Liquidity conservation verification
    verify_liquidity_conservation();

    // Fee calculation verifications
    verify_fee_calculation();
    verify_calculate_fee_bounds();

    // Tick spacing verification
    verify_tick_spacing_consistency();

    // Zero value handling verifications
    verify_zero_liquidity_returns_zero();
    verify_next_sqrt_price_zero_amount();

    // Price and tick relationship verifications
    verify_sqrt_price_tick_monotonicity();
    verify_tick_arithmetic_extremes();

    // Get amount delta verifications
    verify_get_amount_delta_price_ordering();
    verify_get_liquidity_from_amounts_edge_cases();

    // Swap calculation verifications
    verify_swap_price_direction();
}

/// Helper functions for verifications
/// Checks if two numbers are approximately equal within a relative error
/// Used for floating-point-like comparisons with fixed-point math
fn approximately_equal(a: u128, b: u128, max_relative_error: u128) -> bool {
    if a == b {
        return true;
    }

    let abs_diff = if a > b { a - b } else { b - a };

    // Calculate relative error to larger value to avoid division by zero
    let larger = if a > b { a } else { b };

    // If larger is 0, they can only be equal if both are 0
    if larger == 0 {
        return false;
    }

    // Calculate if the relative error is acceptable
    abs_diff <= (larger * max_relative_error) / 1_000_000
}
