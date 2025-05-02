//! Property-based tests for the math module
//!
//! This file contains property-based tests using the proptest framework
//! to verify that the mathematical operations in the math module satisfy
//! important invariants across a wide range of randomly generated inputs.

use crate::math::*;
use proptest::prelude::*; // Adjust the import path according to your project structure

/// Defines strategies for generating valid inputs for testing
mod strategies {
    use super::*;

    /// Strategy for generating valid sqrt prices
    pub fn sqrt_price() -> impl Strategy<Value = u128> {
        // Generate sqrt prices from very small to very large
        // but avoid zero which would cause division by zero
        1..u128::MAX
    }

    /// Strategy for generating valid liquidity values
    pub fn liquidity() -> impl Strategy<Value = u128> {
        // Generate non-zero liquidity values
        1..u128::MAX
    }

    /// Strategy for generating valid tick indices
    pub fn tick_index() -> impl Strategy<Value = i32> {
        // Generate tick indices within the valid range for Fluxa
        -887272..887272 // Adjust based on your protocol's tick range
    }

    /// Strategy for amounts (tokens)
    pub fn amount() -> impl Strategy<Value = u64> {
        // Generate token amounts from small to large
        1..u64::MAX
    }

    /// Strategy for fee percentages (in basis points)
    pub fn fee_rate() -> impl Strategy<Value = u16> {
        // Generate fee rates from 0 to 10000 (0% to 100%)
        0..10000u16
    }
}

proptest! {
    // Price calculation invariants

    #[test]
    fn test_sqrt_price_to_price_roundtrip(sqrt_price in strategies::sqrt_price()) {
        // Skip extremely large values that might cause overflow
        prop_assume!(sqrt_price < (u128::MAX >> 2));

        // Calculate price from sqrt_price
        let price = sqrt_price_to_price(sqrt_price).unwrap();

        // Calculate sqrt_price from price
        let sqrt_price_roundtrip = price_to_sqrt_price(price).unwrap();

        // Due to potential rounding errors, we check that the values are close
        // The relative error should be very small
        let difference = sqrt_price.abs_diff(sqrt_price_roundtrip);

        // Allow for a small relative error (e.g., 0.0001%)
        let max_allowed_error = sqrt_price / 1_000_000;
        prop_assert!(difference <= max_allowed_error);
    }

    #[test]
    fn test_get_next_sqrt_price_from_amount0_exact_in(
        sqrt_price in strategies::sqrt_price(),
        liquidity in strategies::liquidity(),
        amount in strategies::amount()
    ) {
        // Skip extreme values that might cause overflow
        prop_assume!(sqrt_price < (u128::MAX >> 3));
        prop_assume!(liquidity < (u128::MAX >> 3));
        prop_assume!(amount < (u64::MAX >> 2));

        // Get next sqrt price after adding token0
        let next_sqrt_price = get_next_sqrt_price_from_amount0_exact_in(
            sqrt_price,
            liquidity,
            amount,
            true
        );

        // The next sqrt price must be less than or equal to the current sqrt price
        // (adding token0 decreases the price)
        prop_assert!(next_sqrt_price <= sqrt_price);

        // For non-zero inputs, the next sqrt price should be strictly less
        if amount > 0 && liquidity > 0 {
            prop_assert!(next_sqrt_price < sqrt_price);
        }
    }

    #[test]
    fn test_get_next_sqrt_price_from_amount1_exact_in(
        sqrt_price in strategies::sqrt_price(),
        liquidity in strategies::liquidity(),
        amount in strategies::amount()
    ) {
        // Skip extreme values that might cause overflow
        prop_assume!(sqrt_price < (u128::MAX >> 3));
        prop_assume!(liquidity < (u128::MAX >> 3));
        prop_assume!(amount < (u64::MAX >> 2));

        // Get next sqrt price after adding token1
        let next_sqrt_price = get_next_sqrt_price_from_amount1_exact_in(
            sqrt_price,
            liquidity,
            amount,
            true
        );

        // The next sqrt price must be greater than or equal to the current sqrt price
        // (adding token1 increases the price)
        prop_assert!(next_sqrt_price >= sqrt_price);

        // For non-zero inputs, the next sqrt price should be strictly greater
        if amount > 0 && liquidity > 0 {
            prop_assert!(next_sqrt_price > sqrt_price);
        }
    }

    #[test]
    fn test_get_amount_delta_liquidity_relationship(
        sqrt_price_lower in strategies::sqrt_price(),
        sqrt_price_upper in strategies::sqrt_price(),
        liquidity in strategies::liquidity()
    ) {
        // Ensure lower < upper for valid price range
        prop_assume!(sqrt_price_lower < sqrt_price_upper);
        // Avoid extremely large values to prevent overflow
        prop_assume!(sqrt_price_upper < (u128::MAX >> 4));
        prop_assume!(liquidity < (u128::MAX >> 4));

        // Calculate token amounts needed for the given liquidity
        let amount0 = get_amount0_delta(
            sqrt_price_lower,
            sqrt_price_upper,
            liquidity,
            true
        );

        let amount1 = get_amount1_delta(
            sqrt_price_lower,
            sqrt_price_upper,
            liquidity,
            true
        );

        // Calculate the liquidity from the token amounts
        let liquidity_from_amounts = get_liquidity_from_amounts(
            sqrt_price_lower,
            sqrt_price_upper,
            amount0 as u128,
            amount1 as u128
        );

        // The liquidity calculated from amounts should be close to the original liquidity
        // Allow for a small relative error due to rounding
        let max_allowed_error = liquidity / 1_000_000;  // 0.0001%
        let difference = liquidity.abs_diff(liquidity_from_amounts);

        prop_assert!(difference <= max_allowed_error);
    }

    #[test]
    fn test_tick_index_to_sqrt_price_roundtrip(tick_index in strategies::tick_index()) {
        // Convert tick index to sqrt price
        let sqrt_price = tick_to_sqrt_price(tick_index).unwrap();

        // Convert sqrt price back to tick index
        let tick_index_roundtrip = sqrt_price_to_tick(sqrt_price).unwrap();

        // The roundtrip should result in the same tick index
        // or at most differ by 1 due to rounding
        let difference = (tick_index - tick_index_roundtrip).abs();
        prop_assert!(difference <= 1);
    }

    #[test]
    fn test_fee_calculation_invariants(
        amount in strategies::amount(),
        fee_rate in strategies::fee_rate()
    ) {
        // Skip extremely large values to prevent overflow
        prop_assume!(amount < (u64::MAX >> 2));

        // Calculate fee
        let fee_amount = calculate_fee(amount, fee_rate);

        // The fee amount should not exceed the original amount
        prop_assert!(fee_amount <= amount);

        // The fee amount should be proportional to the fee rate
        // 10000 basis points = 100%
        let expected_fee = (amount as u128) * (fee_rate as u128) / 10000u128;
        let difference = expected_fee.abs_diff(fee_amount as u128);

        // Allow for a small rounding error
        prop_assert!(difference <= 1);
    }

    #[test]
    fn test_get_amount_delta_with_zero_liquidity(
        sqrt_price_lower in strategies::sqrt_price(),
        sqrt_price_upper in strategies::sqrt_price()
    ) {
        // Ensure lower < upper for valid price range
        prop_assume!(sqrt_price_lower < sqrt_price_upper);

        // With zero liquidity, both token amounts should be zero
        let amount0 = get_amount0_delta(sqrt_price_lower, sqrt_price_upper, 0, true);
        let amount1 = get_amount1_delta(sqrt_price_lower, sqrt_price_upper, 0, true);

        prop_assert_eq!(amount0, 0);
        prop_assert_eq!(amount1, 0);
    }

    #[test]
    fn test_monotonicity_of_sqrt_price_to_tick(sqrt_price1 in strategies::sqrt_price(), sqrt_price2 in strategies::sqrt_price()) {
        // Skip extremely large values
        prop_assume!(sqrt_price1 < (u128::MAX >> 4));
        prop_assume!(sqrt_price2 < (u128::MAX >> 4));

        let tick1 = sqrt_price_to_tick(sqrt_price1).unwrap();
        let tick2 = sqrt_price_to_tick(sqrt_price2).unwrap();

        // If sqrt_price1 < sqrt_price2, then tick1 <= tick2
        if sqrt_price1 < sqrt_price2 {
            prop_assert!(tick1 <= tick2);
        }
        // If sqrt_price1 > sqrt_price2, then tick1 >= tick2
        else if sqrt_price1 > sqrt_price2 {
            prop_assert!(tick1 >= tick2);
        }
    }
}
/// Contains property tests for swap calculations
#[cfg(test)]
mod swap_tests {
    use super::*;

    proptest! {
        #[test]
        fn test_swap_exact_in_invariants(
            sqrt_price in strategies::sqrt_price(),
            liquidity in strategies::liquidity(),
            amount_in in strategies::amount(),
            zero_for_one in proptest::bool::ANY
        ) {
            // Skip extreme values
            prop_assume!(sqrt_price < (u128::MAX >> 4));
            prop_assume!(liquidity < (u128::MAX >> 4));
            prop_assume!(amount_in < (u64::MAX >> 2));
            prop_assume!(liquidity > 0);

            // Perform swap calculation
            let (next_sqrt_price, amount_in_used, _amount_out) = if zero_for_one {
                // Token0 -> Token1 (price decreases)
                let next_price = get_next_sqrt_price_from_amount0_exact_in(
                    sqrt_price,
                    liquidity,
                    amount_in,
                    true
                );

                let amount0_used = get_amount0_delta(
                    next_price,
                    sqrt_price,
                    liquidity,
                    true
                );

                let amount1_out = get_amount1_delta(
                    next_price,
                    sqrt_price,
                    liquidity,
                    false
                );

                (next_price, amount0_used, amount1_out)
            } else {
                // Token1 -> Token0 (price increases)
                let next_price = get_next_sqrt_price_from_amount1_exact_in(
                    sqrt_price,
                    liquidity,
                    amount_in,
                    true
                );

                let amount1_used = get_amount1_delta(
                    sqrt_price,
                    next_price,
                    liquidity,
                    true
                );

                let amount0_out = get_amount0_delta(
                    sqrt_price,
                    next_price,
                    liquidity,
                    false
                );

                (next_price, amount1_used, amount0_out)
            };

            // The amount used should not exceed the amount in
            prop_assert!(amount_in_used <= amount_in);

            // If we used all the input, then the next price should have moved
            if amount_in_used == amount_in && amount_in > 0 {
                prop_assert!(next_sqrt_price != sqrt_price);
            }

            // If zero_for_one (selling token0 for token1), price should decrease
            if zero_for_one && amount_in_used > 0 {
                prop_assert!(next_sqrt_price < sqrt_price);
            }
            // If one_for_zero (selling token1 for token0), price should increase
            else if !zero_for_one && amount_in_used > 0 {
                prop_assert!(next_sqrt_price > sqrt_price);
            }
        }
    }
}

/// Contains property tests for concentrated liquidity math
#[cfg(test)]
mod concentrated_liquidity_tests {
    use super::*;

    proptest! {
        #[test]
        fn test_liquidity_conservation_invariant(
            sqrt_price in strategies::sqrt_price(),
            liquidity in strategies::liquidity(),
            sqrt_price_target in strategies::sqrt_price()
        ) {
            // Skip extreme values
            prop_assume!(sqrt_price < (u128::MAX >> 4));
            prop_assume!(sqrt_price_target < (u128::MAX >> 4));
            prop_assume!(liquidity < (u128::MAX >> 4));
            prop_assume!(liquidity > 0);

            // Ensure sqrt prices are different
            prop_assume!(sqrt_price != sqrt_price_target);

            let (smaller_sqrt_price, larger_sqrt_price) = if sqrt_price < sqrt_price_target {
                (sqrt_price, sqrt_price_target)
            } else {
                (sqrt_price_target, sqrt_price)
            };

            // Calculate the amount of token0 and token1 needed
            let amount0 = get_amount0_delta(
                smaller_sqrt_price,
                larger_sqrt_price,
                liquidity,
                true
            );

            let amount1 = get_amount1_delta(
                smaller_sqrt_price,
                larger_sqrt_price,
                liquidity,
                true
            );

            // Calculate the liquidity from the token amounts
            let calculated_liquidity = get_liquidity_from_amounts(
                smaller_sqrt_price,
                larger_sqrt_price,
                amount0 as u128,
                amount1 as u128
            );

            // The liquidity should be conserved (allowing for minimal rounding error)
            let max_allowed_error = liquidity / 1_000_000;  // 0.0001%
            let difference = liquidity.abs_diff(calculated_liquidity);

            prop_assert!(difference <= max_allowed_error);
        }
    }
}
