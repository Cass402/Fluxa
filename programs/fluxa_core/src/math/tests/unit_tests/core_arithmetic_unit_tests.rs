#[cfg(test)]
mod tests {
    use crate::math::core_arithmetic::*;
    use crate::utils::constants::*;
    use anchor_lang::prelude::*;

    // Helper function for approximate equality testing with tolerance
    fn assert_q64_approx_eq(a: Q64x64, b: Q64x64, tolerance: u128) {
        let diff = if a.raw() > b.raw() {
            a.raw() - b.raw()
        } else {
            b.raw() - a.raw()
        };
        assert!(
            diff <= tolerance,
            "Values not approximately equal: {} vs {}, diff: {}",
            a.raw(),
            b.raw(),
            diff
        );
    }

    // Helper to assert that an error occurred
    fn assert_is_error<T>(result: Result<T>) {
        assert!(result.is_err(), "Expected error but got Ok");
    }

    #[test]
    fn test_q64x64_construction() {
        // Test basic construction
        assert_eq!(Q64x64::zero().raw(), 0);
        assert_eq!(Q64x64::one().raw(), ONE_X64);
        assert_eq!(Q64x64::from_int(5).raw(), 5u128 << FRAC_BITS);
        assert_eq!(Q64x64::from_raw(12345).raw(), 12345);
    }

    #[test]
    fn test_q64x64_constants() {
        // Verify constants behave correctly
        assert_eq!(ONE_X64, 1u128 << 64);
        assert_eq!(FRAC_BITS, 64);

        // ONE should behave as multiplicative identity
        let test_val = Q64x64::from_int(42);
        let result = test_val.checked_mul(Q64x64::one()).unwrap();
        assert_eq!(result, test_val);
    }

    #[test]
    fn test_q64x64_addition() {
        // Basic addition
        let a = Q64x64::from_int(5);
        let b = Q64x64::from_int(3);
        let result = a.checked_add(b).unwrap();
        assert_eq!(result, Q64x64::from_int(8));

        // Addition with zero
        let result = a.checked_add(Q64x64::zero()).unwrap();
        assert_eq!(result, a);

        // Addition overflow
        let max_val = Q64x64::from_raw(u128::MAX);
        let one = Q64x64::from_raw(1);
        assert!(max_val.checked_add(one).is_err());
    }

    #[test]
    fn test_q64x64_subtraction() {
        // Basic subtraction
        let a = Q64x64::from_int(8);
        let b = Q64x64::from_int(3);
        let result = a.checked_sub(b).unwrap();
        assert_eq!(result, Q64x64::from_int(5));

        // Subtraction with zero
        let result = a.checked_sub(Q64x64::zero()).unwrap();
        assert_eq!(result, a);

        // Subtraction underflow
        let small = Q64x64::from_raw(5);
        let large = Q64x64::from_raw(10);
        assert!(small.checked_sub(large).is_err());
    }

    #[test]
    fn test_q64x64_multiplication() {
        // Basic multiplication
        let a = Q64x64::from_int(4);
        let b = Q64x64::from_int(3);
        let result = a.checked_mul(b).unwrap();
        assert_eq!(result, Q64x64::from_int(12));

        // Multiplication by zero
        let result = a.checked_mul(Q64x64::zero()).unwrap();
        assert_eq!(result, Q64x64::zero());

        // Multiplication by one
        let result = a.checked_mul(Q64x64::one()).unwrap();
        assert_eq!(result, a);

        // Fractional multiplication (0.5 * 0.5 = 0.25)
        let half = Q64x64::from_raw(ONE_X64 / 2);
        let result = half.checked_mul(half).unwrap();
        let quarter = Q64x64::from_raw(ONE_X64 / 4);
        assert_eq!(result, quarter);

        // Multiplication overflow
        let large = Q64x64::from_raw(u128::MAX / 2);
        let result = large.checked_mul(Q64x64::from_int(3));
        assert!(result.is_err());
    }

    #[test]
    fn test_q64x64_division() {
        // Basic division
        let a = Q64x64::from_int(12);
        let b = Q64x64::from_int(3);
        let result = a.checked_div(b).unwrap();
        assert_eq!(result, Q64x64::from_int(4));

        // Division by one
        let result = a.checked_div(Q64x64::one()).unwrap();
        assert_eq!(result, a);

        // Fractional division (1.0 / 2.0 = 0.5)
        let result = Q64x64::one().checked_div(Q64x64::from_int(2)).unwrap();
        let half = Q64x64::from_raw(ONE_X64 / 2);
        assert_eq!(result, half);

        // Division by zero
        let result = a.checked_div(Q64x64::zero());
        assert_is_error(result);

        // Division overflow (very small divisor)
        let large = Q64x64::from_raw(u128::MAX / 2);
        let tiny = Q64x64::from_raw(1);
        let result = large.checked_div(tiny);
        assert!(result.is_err());
    }

    #[test]
    fn test_mul_div_precision() {
        // Test that mul_div preserves precision
        let a = 1000u128;
        let b = 2000u128;
        let c = 500u128;
        let result = mul_div(a, b, c).unwrap();
        assert_eq!(result, 4000u128);

        // Test edge case with large numbers
        let result = mul_div(u128::MAX / 4, 2, u128::MAX / 2).unwrap();
        assert_eq!(result, 0);

        // Test division by zero
        assert!(mul_div(100, 200, 0).is_err());

        // Test overflow
        assert!(mul_div(u128::MAX, u128::MAX, 1).is_err());
    }

    #[test]
    fn test_mul_div_q64() {
        let a = Q64x64::from_int(10);
        let b = Q64x64::from_int(20);
        let c = Q64x64::from_int(5);
        let result = mul_div_q64(a, b, c).unwrap();
        assert_eq!(result, Q64x64::from_int(40));

        // Division by zero
        let result = mul_div_q64(a, b, Q64x64::zero());
        assert!(result.is_err());
    }

    #[test]
    fn test_sqrt_x64_basic() {
        // sqrt(0) = 0
        let result = sqrt_x64(Q64x64::zero()).unwrap();
        assert_eq!(result, Q64x64::zero());

        // sqrt(1) = 1
        let result = sqrt_x64(Q64x64::one()).unwrap();
        assert_q64_approx_eq(result, Q64x64::one(), ONE_X64 / 1000);

        // sqrt(4) = 2
        let four = Q64x64::from_int(4);
        let result = sqrt_x64(four).unwrap();
        assert_q64_approx_eq(result, Q64x64::from_int(2), ONE_X64 / 1000);

        // sqrt(9) = 3
        let nine = Q64x64::from_int(9);
        let result = sqrt_x64(nine).unwrap();
        assert_q64_approx_eq(result, Q64x64::from_int(3), ONE_X64 / 1000);
    }

    #[test]
    fn test_sqrt_x64_precision() {
        // Test sqrt(2) ≈ 1.414213562373095
        let two = Q64x64::from_int(2);
        let result = sqrt_x64(two).unwrap();
        let expected = Q64x64::from_raw(26087635650665564424); // ≈ 1.414... in Q64.64
        assert_q64_approx_eq(result, expected, ONE_X64 / 10000);

        // Test sqrt(0.25) = 0.5
        let quarter = Q64x64::from_raw(ONE_X64 / 4);
        let result = sqrt_x64(quarter).unwrap();
        let half = Q64x64::from_raw(ONE_X64 / 2);
        assert_q64_approx_eq(result, half, ONE_X64 / 1000);
    }

    #[test]
    fn test_sqrt_x64_bounds() {
        // Test minimum valid sqrt value
        let min_val = Q64x64::from_raw((MIN_SQRT_X64 * MIN_SQRT_X64) >> FRAC_BITS);
        let result = sqrt_x64(min_val);
        assert!(result.is_ok());

        // Test maximum valid sqrt value
        let max_val = Q64x64::from_raw(MAX_SQRT_X64);
        let max_input = max_val.checked_mul(max_val).unwrap();
        let result = sqrt_x64(max_input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tick_to_sqrt_x64_bounds() {
        // Test minimum tick
        let result = tick_to_sqrt_x64(MIN_TICK).unwrap();
        assert!(result.raw() >= MIN_SQRT_X64);

        // Test maximum tick
        let result = tick_to_sqrt_x64(MAX_TICK).unwrap();
        assert!(result.raw() <= MAX_SQRT_X64);

        // Test tick = 0 (should be approximately 1.0)
        let result = tick_to_sqrt_x64(0).unwrap();
        assert_q64_approx_eq(result, Q64x64::one(), ONE_X64 / 1000);

        // Test out of bounds - below minimum
        let result = tick_to_sqrt_x64(MIN_TICK - 1);
        assert!(result.is_err());

        // Test out of bounds - above maximum
        let result = tick_to_sqrt_x64(MAX_TICK + 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_tick_to_sqrt_x64_symmetry() {
        // Test that positive and negative ticks are reciprocals
        let tick = 1000;
        let pos_result = tick_to_sqrt_x64(tick).unwrap();
        let neg_result = tick_to_sqrt_x64(-tick).unwrap();

        let product = pos_result.checked_mul(neg_result).unwrap();
        assert_q64_approx_eq(product, Q64x64::one(), ONE_X64 / 10000);
    }

    #[test]
    fn test_tick_to_sqrt_x64_small_values() {
        // Test small tick values for precision
        for tick in -10..=10 {
            let result = tick_to_sqrt_x64(tick);
            assert!(result.is_ok(), "Failed for tick: {tick}");
        }

        // Test that tick 1 gives approximately 1.0001^0.5
        let result = tick_to_sqrt_x64(1).unwrap();
        // 1.0001^0.5 ≈ 1.00005 in Q64.64
        let expected = Q64x64::from_raw(ONE_X64 + (ONE_X64 / 20000));
        assert_q64_approx_eq(result, expected, ONE_X64 / (100000));
    }

    #[test]
    fn test_liquidity_from_amount_0_basic() {
        // Test basic case: sqrt_a < sqrt_b
        let sqrt_a = Q64x64::from_raw(ONE_X64); // 1.0
        let sqrt_b = Q64x64::from_raw(ONE_X64 * 2); // 2.0
        let amount0 = 1000u64;

        let result = liquidity_from_amount_0(sqrt_a, sqrt_b, amount0).unwrap();

        // L = amount0 * sqrt_a * sqrt_b / (sqrt_b - sqrt_a)
        // L = 1000 * 1.0 * 2.0 / (2.0 - 1.0) = 2000
        let expected = 2000u128 << FRAC_BITS;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_liquidity_from_amount_0_edge_cases() {
        let sqrt_a = Q64x64::from_raw(ONE_X64);
        let sqrt_b = Q64x64::from_raw(ONE_X64 * 2);

        // Test with zero amount
        let result = liquidity_from_amount_0(sqrt_a, sqrt_b, 0).unwrap();
        assert_eq!(result, 0);

        // Test with equal sqrt values (should fail)
        let result = liquidity_from_amount_0(sqrt_a, sqrt_a, 1000);
        assert!(result.is_err());

        // Test with reversed order (sqrt_a > sqrt_b, should fail)
        let result = liquidity_from_amount_0(sqrt_b, sqrt_a, 1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_liquidity_from_amount_1_basic() {
        // Test basic case: sqrt_a < sqrt_b
        let sqrt_a = Q64x64::from_raw(ONE_X64); // 1.0
        let sqrt_b = Q64x64::from_raw(ONE_X64 * 2); // 2.0
        let amount1 = 1000u64;

        let result = liquidity_from_amount_1(sqrt_a, sqrt_b, amount1).unwrap();

        // L = amount1 / (sqrt_b - sqrt_a)
        // L = 1000 / (2.0 - 1.0) = 1000
        let expected = 1000u128 << FRAC_BITS;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_liquidity_from_amount_1_edge_cases() {
        let sqrt_a = Q64x64::from_raw(ONE_X64);
        let sqrt_b = Q64x64::from_raw(ONE_X64 * 2);

        // Test with zero amount
        let result = liquidity_from_amount_1(sqrt_a, sqrt_b, 0).unwrap();
        assert_eq!(result, 0);

        // Test with equal sqrt values (should fail)
        let result = liquidity_from_amount_1(sqrt_a, sqrt_a, 1000);
        assert!(result.is_err());

        // Test with reversed order (sqrt_a > sqrt_b, should fail)
        let result = liquidity_from_amount_1(sqrt_b, sqrt_a, 1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_liquidity_functions_consistency() {
        // Test that both liquidity functions work with realistic tick values
        let tick_lower = tick_to_sqrt_x64(-1000).unwrap();
        let tick_upper = tick_to_sqrt_x64(1000).unwrap();

        let amount0 = 1_000_000u64;
        let amount1 = 2_000_000u64;

        let liq0 = liquidity_from_amount_0(tick_lower, tick_upper, amount0);
        let liq1 = liquidity_from_amount_1(tick_lower, tick_upper, amount1);

        assert!(liq0.is_ok());
        assert!(liq1.is_ok());

        // Both should produce positive liquidity
        assert!(liq0.unwrap() > 0);
        assert!(liq1.unwrap() > 0);
    }

    #[test]
    fn test_precision_preservation() {
        // Test that operations maintain expected precision
        let val = Q64x64::from_raw(ONE_X64 / 3); // 1/3
        let three = Q64x64::from_int(3);

        // (1/3) * 3 should be close to 1
        let result = val.checked_mul(three).unwrap();
        assert_q64_approx_eq(result, Q64x64::one(), ONE_X64 / 10000);

        // Test division precision
        let result = Q64x64::one().checked_div(three).unwrap();
        let back = result.checked_mul(three).unwrap();
        assert_q64_approx_eq(back, Q64x64::one(), ONE_X64 / 10000);
    }

    #[test]
    fn test_large_number_handling() {
        // Test operations with large numbers near the limit
        let large = Q64x64::from_raw(u128::MAX / 4);
        let small = Q64x64::from_raw(5 << 64);

        // Division should work
        let result = large.checked_div(small).unwrap();
        assert!(result.raw() > 0);

        // Multiplication should overflow
        let overflow_result = large.checked_mul(small);
        assert!(overflow_result.is_err());
    }

    #[test]
    fn test_error_conditions() {
        // Test various error conditions systematically

        // Overflow in addition
        let max_val = Q64x64::from_raw(u128::MAX);
        let one = Q64x64::from_raw(1);
        assert_is_error(max_val.checked_add(one));

        // Underflow in subtraction
        let zero = Q64x64::zero();
        assert_is_error(zero.checked_sub(one));

        // Division by zero
        assert_is_error(one.checked_div(zero));

        // Out of range tick
        let result = tick_to_sqrt_x64(MAX_TICK + 1);
        assert!(result.is_err());
    }
}
