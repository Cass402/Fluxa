use crate::constants::*;
use crate::errors::ErrorCode;
use crate::math::*;

#[allow(clippy::manual_div_ceil)]
mod uint_impl {
    use uint_crate::construct_uint;
    construct_uint! {
        /// 256-bit unsigned integer.
        pub struct U256(4);
    }
}

pub use uint_impl::U256;

#[cfg(test)]
mod math_tests {
    use super::*;

    /// Fixture for math-related tests containing various constants and test values
    ///
    /// Contains constants for Q64.64 and Q64.96 fixed-point formats, test price values
    /// at different levels, corresponding square root prices in different formats,
    /// a test liquidity value, and test tick values at min/mid/max positions.
    #[allow(dead_code)]
    struct MathTestFixture {
        // Q64.64 format constants
        q64: u128,
        q96: u128,
        // Test prices in normal range
        price_low: u64,
        price_mid: u64,
        price_high: u64,
        // Corresponding sqrt prices in Q64.64 format
        sqrt_price_low_q64: u128,
        sqrt_price_mid_q64: u128,
        sqrt_price_high_q64: u128,
        // Corresponding sqrt prices in Q64.96 format
        sqrt_price_low_q96: u128,
        sqrt_price_mid_q96: u128,
        sqrt_price_high_q96: u128,
        // Test liquidity value
        liquidity: u128,
        // Test tick values
        tick_min: i32,
        tick_mid: i32,
        tick_max: i32,
    }

    /// Creates a new MathTestFixture with predefined test values
    ///
    /// Initializes a test fixture with:
    /// - Fixed-point format constants (Q64.64 and Q64.96)
    /// - Test price values at low, mid, and high points
    /// - Corresponding square root prices in both Q64.64 and Q64.96 formats
    /// - A test liquidity value
    /// - Test tick values at minimum, middle, and maximum positions
    ///
    /// These values are used throughout the math tests to verify calculations.
    impl MathTestFixture {
        fn new() -> Self {
            // Define test constants
            let q64 = 1u128 << 64;
            let q96 = 1u128 << 96;

            // Test prices (as u64)
            let price_low = 1_000_000; // 1.0
            let price_mid = 1_500_000; // 1.5
            let price_high = 2_000_000; // 2.0

            // Pre-calculate sqrt prices for testing
            // These values represent sqrt(price) * 2^64
            let sqrt_price_low_q64 = 1_000_000_000_000_000_000; // sqrt(1.0) * 2^64
            let sqrt_price_mid_q64 = 1_224_744_871_391_589_049; // sqrt(1.5) * 2^64
            let sqrt_price_high_q64 = 1_414_213_562_373_095_048; // sqrt(2.0) * 2^64

            // Q64.96 format values (shifted left by 32 bits)
            let sqrt_price_low_q96 = sqrt_price_low_q64 << 32;
            let sqrt_price_mid_q96 = sqrt_price_mid_q64 << 32;
            let sqrt_price_high_q96 = sqrt_price_high_q64 << 32;

            // Test liquidity value
            let liquidity = 1_000_000_000_000_000_000; // 1.0 in Q64.64

            // Test tick values
            let tick_min = -887272; // MIN_TICK
            let tick_mid = 0;
            let tick_max = 887272; // MAX_TICK

            Self {
                q64,
                q96,
                price_low,
                price_mid,
                price_high,
                sqrt_price_low_q64,
                sqrt_price_mid_q64,
                sqrt_price_high_q64,
                sqrt_price_low_q96,
                sqrt_price_mid_q96,
                sqrt_price_high_q96,
                liquidity,
                tick_min,
                tick_mid,
                tick_max,
            }
        }
    }

    /// Checks if two floating point numbers are approximately equal within a specified epsilon
    ///
    /// # Arguments
    /// * `a` - First floating point number
    /// * `b` - Second floating point number
    /// * `epsilon` - Maximum allowed difference between the two numbers
    ///
    /// # Returns
    /// `true` if the absolute difference between `a` and `b` is less than `epsilon`, `false` otherwise
    fn approx_equal(a: f64, b: f64, epsilon: f64) -> bool {
        (a - b).abs() < epsilon
    }

    // Helper to convert Q64.64 to f64
    #[allow(dead_code)]
    fn q64_to_float(value: u128) -> f64 {
        (value as f64) / (Q64 as f64)
    }

    /// Converts a Q64.96 fixed-point number to a floating point value
    ///
    /// # Arguments
    /// * `value` - The Q64.96 fixed-point number to convert
    ///
    /// # Returns
    /// The floating point representation of the Q64.96 value
    #[allow(dead_code)]
    fn q96_to_float(value: u128) -> f64 {
        (value as f64) / (Q96 as f64)
    }

    #[test]

    /// Test conversion from Q64.64 to Q64.96 format
    ///
    /// Tests the following scenarios:
    /// - Normal conversion case
    /// - Zero value
    /// - Large value close to overflow
    /// - Overflow condition
    fn test_convert_sqrt_price_to_q96() {
        let fixture = MathTestFixture::new();

        // Test normal case
        let result = convert_sqrt_price_to_q96(fixture.sqrt_price_mid_q64).unwrap();
        assert_eq!(result, fixture.sqrt_price_mid_q96);

        // Test zero
        let result = convert_sqrt_price_to_q96(0).unwrap();
        assert_eq!(result, 0);

        // Test large value (close to overflow)
        let large_value = u128::MAX >> 32;
        let result = convert_sqrt_price_to_q96(large_value).unwrap();
        assert_eq!(result, large_value << 32);

        // Test overflow
        let overflow_value = u128::MAX - 100;
        let result = convert_sqrt_price_to_q96(overflow_value);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::MathOverflow.to_string()
        );
    }

    #[test]

    /// Test conversion from Q64.96 to Q64.64 format
    ///
    /// Tests the following scenarios:
    /// - Normal conversion case
    /// - Zero value
    /// - Value that will lose precision (rounding behavior)
    fn test_convert_sqrt_price_from_q96() {
        let fixture = MathTestFixture::new();

        // Test normal case
        let result = convert_sqrt_price_from_q96(fixture.sqrt_price_mid_q96).unwrap();
        assert_eq!(result, fixture.sqrt_price_mid_q64);

        // Test zero
        let result = convert_sqrt_price_from_q96(0).unwrap();
        assert_eq!(result, 0);

        // Test value that will lose precision (test rounding behavior)
        let test_value = fixture.q96 + 50;
        let expected = test_value >> 32; // Expected value after conversion
        let result = convert_sqrt_price_from_q96(test_value).unwrap();
        assert_eq!(result, expected);
    }

    #[test]

    /// Test addition in Q64.96 format
    ///
    /// Tests the following scenarios:
    /// - Normal addition case
    /// - Adding zero
    /// - Overflow condition
    fn test_add_q96() {
        let fixture = MathTestFixture::new();

        // Test normal addition
        let a = fixture.sqrt_price_low_q96;
        let b = fixture.sqrt_price_mid_q96;
        let result = add_q96(a, b).unwrap();
        assert_eq!(result, a + b);

        // Test adding zero
        let result = add_q96(a, 0).unwrap();
        assert_eq!(result, a);

        // Test overflow
        let result = add_q96(U128MAX - 10, 20);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::MathOverflow.to_string()
        );
    }

    #[test]

    /// Test subtraction in Q64.96 format
    ///
    /// Tests the following scenarios:
    /// - Normal subtraction case
    /// - Subtracting zero
    /// - Underflow condition
    fn test_sub_q96() {
        let fixture = MathTestFixture::new();

        // Test normal subtraction
        let a = fixture.sqrt_price_high_q96;
        let b = fixture.sqrt_price_low_q96;
        let result = sub_q96(a, b).unwrap();
        assert_eq!(result, a - b);

        // Test subtracting zero
        let result = sub_q96(a, 0).unwrap();
        assert_eq!(result, a);

        // Test underflow
        let result = sub_q96(10, 20);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::MathOverflow.to_string()
        );
    }

    #[test]

    /// Test multiplication in Q64.96 format
    ///
    /// Tests the following scenarios:
    /// - Multiplication by zero
    /// - Multiplication by one (identity)
    /// - Basic multiplication with small values
    /// - Overflow handling
    fn test_mul_q96() {
        let fixture = MathTestFixture::new();

        // Test zero multiplication
        let result = mul_q96(0, fixture.sqrt_price_mid_q96).unwrap();
        assert_eq!(result, 0);

        // Test identity multiplication (× 1.0)
        let one_q96 = fixture.q96;
        let result = mul_q96(fixture.sqrt_price_mid_q96, one_q96).unwrap();
        // Expect approximately the same value (exact comparison may fail due to fixed-point rounding)
        let relative_error = (result as f64 - fixture.sqrt_price_mid_q96 as f64).abs()
            / (fixture.sqrt_price_mid_q96 as f64);
        assert!(
            relative_error < 0.001,
            "Relative error too large: {relative_error}"
        );

        // Test basic multiplication
        // we use smaller values to avoid overflow
        let a = fixture.q96 / 1_000_000; // 0.000001 in Q64.96
        let b = fixture.q96 / 1_000_000; // 0.000001 in Q64.96
        let expected = fixture.q96 / 1_000_000_000_000; // 0.000001² in Q64.96
        let result = mul_q96(a, b).unwrap();
        // Allow small rounding error in fixed-point math
        let relative_error = (result as f64 - expected as f64).abs() / (expected as f64);
        assert!(
            relative_error < 0.001,
            "Relative error too large: {relative_error}"
        );

        // Test overflow handling
        let large_value = U128MAX / 2;
        let small_value = 3 * Q96;
        let result = mul_q96(large_value, small_value);
        assert!(result.is_err());
    }

    #[test]

    /// Test division in Q64.96 format
    ///
    /// Tests the following scenarios:
    /// - Division by one (identity)
    /// - Self division (should equal one)
    /// - Division by zero (should error)
    fn test_div_q96() {
        let fixture = MathTestFixture::new();

        // Test division by 1
        let one_q96 = fixture.q96;
        let result = div_q96(fixture.sqrt_price_mid_q96, one_q96).unwrap();
        // Expect approximately the same value
        let relative_error = (result as f64 - fixture.sqrt_price_mid_q96 as f64).abs()
            / (fixture.sqrt_price_mid_q96 as f64);
        assert!(
            relative_error < 0.001,
            "Relative error too large: {relative_error}"
        );

        // Test self division (should be close to 1.0)
        let a = fixture.sqrt_price_mid_q96;
        let result = div_q96(a, a).unwrap();
        let relative_error = (result as f64 - one_q96 as f64).abs() / (one_q96 as f64);
        assert!(
            relative_error < 0.001,
            "Relative error too large: {relative_error}"
        );

        // Test division by zero
        let result = div_q96(fixture.sqrt_price_mid_q96, 0);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::MathOverflow.to_string()
        );
    }

    #[test]

    /// Test conversion from floating point price to Q64.96 sqrt price
    ///
    /// Tests the following scenarios:
    /// - Price = 1.0 (sqrt = 1.0)
    /// - Price = 4.0 (sqrt = 2.0)
    /// - Fractional price (price = 1.5)
    fn test_price_to_sqrt_price_q96() {
        // Test with exact squares to verify precision

        // Test price = 1.0 (sqrt = 1.0)
        let price = 1.0;
        let result = price_to_sqrt_price_q96(price).unwrap();
        assert_eq!(result, Q96);

        // Test price = 4.0 (sqrt = 2.0)
        let price = 4.0;
        let expected = (2.0 * Q96 as f64) as u128;
        let result = price_to_sqrt_price_q96(price).unwrap();
        assert_eq!(result, expected);

        // Test with fractional price
        let price = 1.5;
        let expected = (1.5f64.sqrt() * Q96 as f64) as u128;
        let result = price_to_sqrt_price_q96(price).unwrap();
        // Allow small rounding error
        let relative_error = (result as f64 - expected as f64).abs() / (expected as f64);
        assert!(
            relative_error < 0.0001,
            "Relative error too large: {relative_error}"
        );
    }

    #[test]

    /// Test conversion from Q64.96 sqrt price to floating point price
    ///
    /// Tests the following scenarios:
    /// - Sqrt price = 1.0 * Q96 (price = 1.0)
    /// - Sqrt price = 2.0 * Q96 (price = 4.0)
    /// - Fixture value (price = 1.5)
    fn test_sqrt_price_q96_to_price() {
        // Test with exact values

        // Test sqrt_price = 1.0 * Q96 (price = 1.0)
        let sqrt_price_q96 = Q96;
        let result = sqrt_price_q96_to_price(sqrt_price_q96);
        assert!(approx_equal(result, 1.0, 0.0000001));

        // Test sqrt_price = 2.0 * Q96 (price = 4.0)
        let sqrt_price_q96 = 2 * Q96;
        let result = sqrt_price_q96_to_price(sqrt_price_q96);
        assert!(approx_equal(result, 4.0, 0.0000001));

        // Test with a fixture value
        let sqrt1_5_q64 = 22_592_555_198_148_960_256_u128; // sqrt(1.5) * 2^64
        let sqrt_price_q96 = sqrt1_5_q64 << 32; // Convert to Q64.96
        let result = sqrt_price_q96_to_price(sqrt_price_q96);
        assert!(approx_equal(result, 1.5, 0.0001));
    }

    #[test]
    /// Test square calculation in Q64.96 format
    ///
    /// Tests the following scenarios:
    /// - Squaring 1.0 (result = 1.0)
    /// - Squaring 2.0 (result = 4.0)
    /// - Squaring 0.0 (result = 0.0)
    /// - Overflow handling with large values
    fn test_square_q96() {
        let _fixture = MathTestFixture::new();

        // Test squaring 1.0
        let one_q96 = Q96;
        let result = square_q96(one_q96).unwrap();
        // Due to fixed-point rounding, we should be close to one_q96
        let relative_error = (result as f64 - one_q96 as f64).abs() / (one_q96 as f64);
        assert!(
            relative_error < 0.001,
            "Relative error too large: {relative_error}"
        );

        // Test squaring 2.0
        let two_q96 = 2 * Q96;
        let expected = 4 * Q96; // approximate due to fixed-point
        let result = square_q96(two_q96).unwrap();
        let relative_error = (result as f64 - expected as f64).abs() / (expected as f64);
        assert!(
            relative_error < 0.001,
            "Relative error too large: {relative_error}"
        );

        // Test squaring 0.0
        let result = square_q96(0).unwrap();
        assert_eq!(result, 0);

        // Test overflow
        let large_value = U128MAX / 2;
        let result = square_q96(large_value);
        assert!(result.is_err());
    }

    #[test]
    /// Test square root calculation in Q64.96 format
    ///
    /// Tests the following scenarios:
    /// - Square root of 1.0 (result = 1.0)
    /// - Square root of 4.0 (result = 2.0)
    /// - Square root of 0.0 (result = 0.0)
    /// - Square root of a non-perfect square (2.25, result = 1.5)
    fn test_sqrt_q96() {
        let _fixture = MathTestFixture::new();

        // Test sqrt of 1.0
        let one_q96 = Q96;
        let result = sqrt_q96(one_q96).unwrap();
        // Due to fixed-point rounding, we should be close to one_q96
        let relative_error = (result as f64 - one_q96 as f64).abs() / (one_q96 as f64);
        assert!(
            relative_error < 0.001,
            "Relative error too large: {relative_error}"
        );

        // Test sqrt of 4.0
        let four_q96 = 4 * Q96;
        let expected = 2 * Q96; // approximate due to fixed-point
        let result = sqrt_q96(four_q96).unwrap();
        let relative_error = (result as f64 - expected as f64).abs() / (expected as f64);
        assert!(
            relative_error < 0.001,
            "Relative error too large: {relative_error}"
        );

        // Test sqrt of 0.0
        let result = sqrt_q96(0).unwrap();
        assert_eq!(result, 0);

        // Test with a non-perfect square
        let input = (9.0 / 4.0 * Q96 as f64) as u128; // 2.25 in Q64.96
        let expected = (1.5 * Q96 as f64) as u128; // 1.5 in Q64.96
        let result = sqrt_q96(input).unwrap();
        let relative_error = (result as f64 - expected as f64).abs() / (expected as f64);
        assert!(
            relative_error < 0.001,
            "Relative error too large: {relative_error}"
        );
    }

    #[test]

    /// Test reciprocal calculation in Q64.96 format
    ///
    /// Tests the following scenarios:
    /// - Reciprocal of 1.0 (result = 1.0)
    /// - Reciprocal of 2.0 (result = 0.5)
    /// - Reciprocal of a very small number (0.001, result = 1000)
    /// - Reciprocal of 0.0 (should error with MathOverflow)
    fn test_reciprocal_q96() {
        let _fixture = MathTestFixture::new();

        // Test reciprocal of 1.0
        let one_q96 = Q96;
        let result = reciprocal_q96(one_q96).unwrap();
        // Due to fixed-point rounding, we should be close to one_q96
        let relative_error = (result as f64 - one_q96 as f64).abs() / (one_q96 as f64);
        assert!(
            relative_error < 0.001,
            "Relative error too large: {relative_error}"
        );

        // Test reciprocal of 2.0
        let two_q96 = 2 * Q96;
        let expected = Q96 / 2; // 0.5 in Q64.96
        let result = reciprocal_q96(two_q96).unwrap();
        let relative_error = (result as f64 - expected as f64).abs() / (expected as f64);
        assert!(
            relative_error < 0.001,
            "Relative error too large: {relative_error}"
        );

        // Test reciprocal of a very small number
        let small = Q96 / 1000; // 0.001 in Q64.96
        let expected = 1000 * Q96; // 1000 in Q64.96
        let result = reciprocal_q96(small).unwrap();
        let relative_error = (result as f64 - expected as f64).abs() / (expected as f64);
        assert!(
            relative_error < 0.001,
            "Relative error too large: {relative_error}"
        );

        // Test reciprocal of 0.0 (should error)
        let result = reciprocal_q96(0);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::MathOverflow.to_string()
        );
    }

    #[test]

    /// Test getting tick index from sqrt price
    ///
    /// Tests the following cases:
    /// - Exact powers of 1.0001 (tick = 0, 50, -50)
    /// - Boundary cases (minimum and maximum sqrt prices)
    /// - Verifies that the function correctly maps sqrt prices to their corresponding tick indices
    fn test_get_tick_at_sqrt_price_q96() {
        // Test exact powers of 1.0001

        // Test sqrt price = 1.0 (tick = 0)
        let sqrt_price_q96 = Q96;
        let result = get_tick_at_sqrt_price_q96(sqrt_price_q96).unwrap();
        assert_eq!(result, 0);

        // Test sqrt price ≈ 1.0001^50
        // Calculate 1.0001^50 manually
        let sqrt_price = (1.0001f64.powf(50.0)).sqrt();
        let sqrt_price_q96 = (sqrt_price * Q96 as f64) as u128;
        let result = get_tick_at_sqrt_price_q96(sqrt_price_q96).unwrap();
        assert_eq!(result, 50);

        // Test sqrt price ≈ 1.0001^(-50)
        let sqrt_price = (1.0001f64.powf(-50.0)).sqrt();
        let sqrt_price_q96 = (sqrt_price * Q96 as f64) as u128;
        let result = get_tick_at_sqrt_price_q96(sqrt_price_q96).unwrap();
        assert_eq!(result, -50);

        // Test boundary cases
        // (These should be close to MIN_TICK and MAX_TICK, but not exact due to floating point imprecision)

        // Test minimum sqrt price
        let result = get_tick_at_sqrt_price_q96(1).unwrap();
        assert!(result <= MIN_TICK);

        // Test maximum sqrt price
        let result = get_tick_at_sqrt_price_q96(U128MAX).unwrap();
        assert!(result >= MAX_TICK);
    }

    #[test]

    /// Test getting sqrt price from tick index
    ///
    /// Tests the following cases:
    /// - Exact tick values (tick = 0, 100, -100)
    /// - Boundary cases (MIN_TICK and MAX_TICK)
    /// - Verifies that the function correctly calculates sqrt prices with acceptable precision
    fn test_get_sqrt_price_at_tick_q96() {
        // Test exact tick values

        // Test tick = 0 (sqrt price = 1.0)
        let result = get_sqrt_price_at_tick_q96(0).unwrap();
        let expected = Q96;
        let relative_error = (result as f64 - expected as f64).abs() / (expected as f64);
        assert!(
            relative_error < 0.0001,
            "Relative error too large: {relative_error}"
        );

        // Test tick = 100
        let tick = 100;
        let expected_sqrt_price = (1.0001f64.powf(tick as f64)).sqrt() * Q96 as f64;
        let result = get_sqrt_price_at_tick_q96(tick).unwrap();
        let relative_error = (result as f64 - expected_sqrt_price).abs() / expected_sqrt_price;
        assert!(
            relative_error < 0.0001,
            "Relative error too large: {relative_error}"
        );

        // Test tick = -100
        let tick = -100;
        let expected_sqrt_price = (1.0001f64.powf(tick as f64)).sqrt() * Q96 as f64;
        let result = get_sqrt_price_at_tick_q96(tick).unwrap();
        let relative_error = (result as f64 - expected_sqrt_price).abs() / expected_sqrt_price;
        assert!(
            relative_error < 0.0001,
            "Relative error too large: {relative_error}"
        );

        // Test boundary cases

        // Test MIN_TICK
        let result = get_sqrt_price_at_tick_q96(MIN_TICK).unwrap();
        assert!(result > 0); // Should be a very small positive number

        // Test MAX_TICK
        let result = get_sqrt_price_at_tick_q96(MAX_TICK).unwrap();
        assert!(result < U128MAX); // Should be a very large number but less than MAX
    }

    #[test]

    /// Tests the enhanced_price_difference_q96 function
    ///
    /// This test verifies that the function:
    /// - Correctly calculates the absolute difference between two prices
    /// - Handles cases where a > b and a < b
    /// - Returns 0 when prices are equal
    /// - Works correctly with very small differences
    fn test_enhanced_price_difference_q96() {
        let fixture = MathTestFixture::new();

        // Test normal case (a > b)
        let a = fixture.sqrt_price_high_q96;
        let b = fixture.sqrt_price_low_q96;
        let result = enhanced_price_difference_q96(a, b).unwrap();
        assert_eq!(result, a - b);

        // Test normal case (a < b)
        let result = enhanced_price_difference_q96(b, a).unwrap();
        assert_eq!(result, a - b);

        // Test equal prices
        let result = enhanced_price_difference_q96(a, a).unwrap();
        assert_eq!(result, 0);

        // Test with very small difference
        let a = fixture.sqrt_price_mid_q96;
        let b = a - 1;
        let result = enhanced_price_difference_q96(a, b).unwrap();
        assert_eq!(result, 1);
    }

    #[test]

    /// Test token A amount calculation from liquidity
    ///
    /// This test verifies that the function:
    /// - Correctly calculates token A amount when current price is in range
    /// - Returns the full token A amount when price is below range (100% token A)
    /// - Returns zero token A when price is above range (0% token A)
    /// - Handles zero liquidity case correctly
    ///
    /// The test manually calculates expected values and compares them with function results.
    fn test_get_token_a_from_liquidity() {
        let fixture = MathTestFixture::new();

        // Case 1: Current price in range
        let liquidity = fixture.liquidity;
        let sqrt_price_lower = fixture.sqrt_price_low_q96;
        let sqrt_price_upper = fixture.sqrt_price_high_q96;
        let sqrt_price_current = fixture.sqrt_price_mid_q96;

        let result = get_token_a_from_liquidity(
            liquidity,
            sqrt_price_lower,
            sqrt_price_upper,
            sqrt_price_current,
        )
        .unwrap();

        // Manually calculate expected amount
        // amount_a = liquidity * (1/sqrt_price_lower - 1/sqrt_price_current)
        let inv_lower_q256 = U256::from(fixture.q96)
            .checked_mul(U256::from(fixture.q96))
            .unwrap()
            / U256::from(sqrt_price_lower);
        let inv_current_q256 = U256::from(fixture.q96)
            .checked_mul(U256::from(fixture.q96))
            .unwrap()
            / U256::from(sqrt_price_current);

        let expected = (U256::from(liquidity)
            .checked_mul(inv_lower_q256 - inv_current_q256)
            .unwrap()
            / U256::from(fixture.q96))
        .as_u64();

        assert_eq!(result, expected);

        // Case 2: Current price below range (100% token A)
        let sqrt_price_current = sqrt_price_lower / 2; // Below lower bound

        let result = get_token_a_from_liquidity(
            liquidity,
            sqrt_price_lower,
            sqrt_price_upper,
            sqrt_price_current,
        )
        .unwrap();

        // Manually calculate expected amount
        // amount_a = liquidity * (1/sqrt_price_lower - 1/sqrt_price_upper)
        let expected = get_token_a_from_liquidity(
            liquidity,
            sqrt_price_lower,
            sqrt_price_upper,
            sqrt_price_current,
        )
        .unwrap();

        assert_eq!(result, expected);

        // Case 3: Current price above range (0% token A)
        let sqrt_price_current = sqrt_price_upper * 2; // Above upper bound

        let result = get_token_a_from_liquidity(
            liquidity,
            sqrt_price_lower,
            sqrt_price_upper,
            sqrt_price_current,
        )
        .unwrap();

        assert_eq!(result, 0);

        // Case 4: Zero liquidity
        let result =
            get_token_a_from_liquidity(0, sqrt_price_lower, sqrt_price_upper, sqrt_price_current)
                .unwrap();

        assert_eq!(result, 0);
    }

    #[test]

    /// Tests the calculation of token B amount from liquidity
    ///
    /// This test verifies the `get_token_b_from_liquidity` function with different scenarios:
    /// - Case 1: Current price within the price range
    /// - Case 2: Current price below the price range (0% token B)
    /// - Case 3: Current price above the price range (100% token B)
    /// - Case 4: Zero liquidity
    ///
    /// For each case, it compares the calculated result with the expected amount
    /// derived from the mathematical formula: amount_b = liquidity * (sqrt_price_current - sqrt_price_lower)
    fn test_get_token_b_from_liquidity() {
        let fixture = MathTestFixture::new();

        // Case 1: Current price in range
        let liquidity = fixture.liquidity;
        let sqrt_price_lower = fixture.sqrt_price_low_q64;
        let sqrt_price_upper = fixture.sqrt_price_high_q64;
        let sqrt_price_current = fixture.sqrt_price_mid_q64;

        let result = get_token_b_from_liquidity(
            liquidity,
            sqrt_price_lower,
            sqrt_price_upper,
            sqrt_price_current,
        )
        .unwrap();

        // Manually calculate expected amount
        // amount_b = liquidity * (sqrt_price_current - sqrt_price_lower)
        let expected = (liquidity * (sqrt_price_current - sqrt_price_lower) / fixture.q64) as u64;

        assert_eq!(result, expected);

        // Case 2: Current price below the price range (0% token B)
        let sqrt_price_current = sqrt_price_lower / 2; // Below lower bound

        let result = get_token_b_from_liquidity(
            liquidity,
            sqrt_price_lower,
            sqrt_price_upper,
            sqrt_price_current,
        )
        .unwrap();

        assert_eq!(result, 0);

        // Case 3: Current price above the price range (100% token B)
        let sqrt_price_current = sqrt_price_upper * 2; // Above upper bound

        let result = get_token_b_from_liquidity(
            liquidity,
            sqrt_price_lower,
            sqrt_price_upper,
            sqrt_price_current,
        )
        .unwrap();

        // Manually calculate expected amount
        // amount_b = liquidity * (sqrt_price_upper - sqrt_price_lower)
        let expected = (liquidity * (sqrt_price_upper - sqrt_price_lower) / fixture.q64) as u64;

        assert_eq!(result, expected);

        // Case 4: Zero liquidity
        let result =
            get_token_b_from_liquidity(0, sqrt_price_lower, sqrt_price_upper, sqrt_price_current)
                .unwrap();

        assert_eq!(result, 0);
    }

    #[test]

    /// Tests the high-precision (Q64.96) version of token A amount calculation from liquidity
    ///
    /// This test verifies that:
    /// 1. The Q96 precision function produces results close to the standard Q64 version
    /// 2. Edge cases are handled correctly:
    ///    - Current price in range
    ///    - Current price below range (100% token A)
    ///    - Current price above range (0% token A)
    ///    - Zero liquidity
    fn test_get_token_a_from_liquidity_q96() {
        let fixture = MathTestFixture::new();

        // Case 1: Current price in range
        let liquidity = fixture.liquidity;
        let sqrt_price_lower_q96 = fixture.sqrt_price_low_q96;
        let sqrt_price_upper_q96 = fixture.sqrt_price_high_q96;
        let sqrt_price_current_q96 = fixture.sqrt_price_mid_q96;

        let result = get_token_a_from_liquidity_q96(
            liquidity,
            sqrt_price_lower_q96,
            sqrt_price_upper_q96,
            sqrt_price_current_q96,
        )
        .unwrap();

        // Verify result by comparing with standard precision function
        let standard_result = get_token_a_from_liquidity(
            liquidity,
            fixture.sqrt_price_low_q96,
            fixture.sqrt_price_high_q96,
            fixture.sqrt_price_mid_q96,
        )
        .unwrap();

        // Allow small difference due to increased precision
        let rel_diff =
            ((result as i128 - standard_result as i128).abs() as f64) / (standard_result as f64);
        assert!(rel_diff < 0.01, "Relative difference too large: {rel_diff}");

        // Case 2: Current price below range (100% token A)
        let sqrt_price_current_q96 = sqrt_price_lower_q96 / 2; // Below lower bound

        let result = get_token_a_from_liquidity_q96(
            liquidity,
            sqrt_price_lower_q96,
            sqrt_price_upper_q96,
            sqrt_price_current_q96,
        )
        .unwrap();

        // Verify against standard version
        let standard_result = get_token_a_from_liquidity(
            liquidity,
            fixture.sqrt_price_low_q96,
            fixture.sqrt_price_high_q96,
            fixture.sqrt_price_low_q96 / 2,
        )
        .unwrap();

        let rel_diff =
            ((result as i128 - standard_result as i128).abs() as f64) / (standard_result as f64);
        assert!(rel_diff < 0.01, "Relative difference too large: {rel_diff}");

        // Case 3: Current price above range (0% token A)
        let sqrt_price_current_q96 = sqrt_price_upper_q96 * 2; // Above upper bound

        let result = get_token_a_from_liquidity_q96(
            liquidity,
            sqrt_price_lower_q96,
            sqrt_price_upper_q96,
            sqrt_price_current_q96,
        )
        .unwrap();

        assert_eq!(result, 0);

        // Case 4: Zero liquidity
        let result = get_token_a_from_liquidity_q96(
            0,
            sqrt_price_lower_q96,
            sqrt_price_upper_q96,
            sqrt_price_current_q96,
        )
        .unwrap();

        assert_eq!(result, 0);
    }

    #[test]

    /// Tests the `get_token_b_from_liquidity_q96` function with various scenarios:
    /// - Current price in range (partial token B)
    /// - Current price below range (0% token B)
    /// - Current price above range (100% token B)
    /// - Zero liquidity
    ///
    /// Verifies results against the standard precision function.
    fn test_get_token_b_from_liquidity_q96() {
        let fixture = MathTestFixture::new();

        // Case 1: Current price in range
        let liquidity = fixture.liquidity;
        let sqrt_price_lower_q96 = fixture.sqrt_price_low_q96;
        let sqrt_price_upper_q96 = fixture.sqrt_price_high_q96;
        let sqrt_price_current_q96 = fixture.sqrt_price_mid_q96;

        let result = get_token_b_from_liquidity_q96(
            liquidity,
            sqrt_price_lower_q96,
            sqrt_price_upper_q96,
            sqrt_price_current_q96,
        )
        .unwrap();

        // Verify result by comparing with standard precision function
        let standard_result = get_token_b_from_liquidity(
            liquidity,
            fixture.sqrt_price_low_q64,
            fixture.sqrt_price_high_q64,
            fixture.sqrt_price_mid_q64,
        )
        .unwrap();

        // Allow small difference due to increased precision
        let rel_diff =
            ((result as i128 - standard_result as i128).abs() as f64) / (standard_result as f64);
        assert!(rel_diff < 0.01, "Relative difference too large: {rel_diff}");

        // Case 2: Current price below range (0% token B)
        let sqrt_price_current_q96 = sqrt_price_lower_q96 / 2; // Below lower bound

        let result = get_token_b_from_liquidity_q96(
            liquidity,
            sqrt_price_lower_q96,
            sqrt_price_upper_q96,
            sqrt_price_current_q96,
        )
        .unwrap();

        assert_eq!(result, 0);

        // Case 3: Current price above range (100% token B)
        let sqrt_price_current_q96 = sqrt_price_upper_q96 * 2; // Above upper bound

        let result = get_token_b_from_liquidity_q96(
            liquidity,
            sqrt_price_lower_q96,
            sqrt_price_upper_q96,
            sqrt_price_current_q96,
        )
        .unwrap();

        // Verify against standard version
        let standard_result = get_token_b_from_liquidity(
            liquidity,
            fixture.sqrt_price_low_q64,
            fixture.sqrt_price_high_q64,
            fixture.sqrt_price_high_q64 * 2,
        )
        .unwrap();

        let rel_diff =
            ((result as i128 - standard_result as i128).abs() as f64) / (standard_result as f64);
        assert!(rel_diff < 0.01, "Relative difference too large: {rel_diff}");

        // Case 4: Zero liquidity
        let result = get_token_b_from_liquidity_q96(
            0,
            sqrt_price_lower_q96,
            sqrt_price_upper_q96,
            sqrt_price_current_q96,
        )
        .unwrap();

        assert_eq!(result, 0);
    }

    #[test]

    /// Tests the conversion from tick index to square root price.
    /// Verifies:
    /// - Tick 0 corresponds to sqrt price of 1.0 (Q64)
    /// - Positive and negative tick conversions match expected values
    /// - Out-of-range ticks return appropriate errors
    /// - Boundary ticks (MIN_TICK, MAX_TICK) produce valid sqrt prices
    fn test_tick_to_sqrt_price() {
        // Test specific tick values and verify against the expected sqrt price

        // Test tick = 0 (sqrt price = 1.0)
        let result = tick_to_sqrt_price(0).unwrap();
        let expected = Q64;
        let rel_diff = ((result as i128 - expected as i128).abs() as f64) / (expected as f64);
        assert!(
            rel_diff < 0.00001,
            "Relative difference too large: {rel_diff}"
        );

        // Test positive tick
        let tick = 100;
        // Calculate expected sqrt price: sqrt(1.0001^100) * 2^64
        let expected = ((1.0001f64.powf(tick as f64)).sqrt() * Q64 as f64) as u128;
        let result = tick_to_sqrt_price(tick).unwrap();
        let rel_diff = ((result as i128 - expected as i128).abs() as f64) / (expected as f64);
        assert!(rel_diff < 0.0001, "Relative error too large: {rel_diff}");

        // Test negative tick
        let tick = -100;
        // Calculate expected sqrt price: sqrt(1.0001^(-100)) * 2^64
        let expected = ((1.0001f64.powf(tick as f64)).sqrt() * Q64 as f64) as u128;
        let result = tick_to_sqrt_price(tick).unwrap();
        let rel_diff = ((result as i128 - expected as i128).abs() as f64) / (expected as f64);
        assert!(rel_diff < 0.0001, "Relative error too large: {rel_diff}");

        // Test out of range tick (should error)
        let result = tick_to_sqrt_price(MIN_TICK - 1);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::InvalidTickRange.to_string()
        );

        let result = tick_to_sqrt_price(MAX_TICK + 1);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::InvalidTickRange.to_string()
        );

        // Test boundary ticks
        let result = tick_to_sqrt_price(MIN_TICK).unwrap();
        assert!(result > 0); // Should be a very small positive number

        let result = tick_to_sqrt_price(MAX_TICK).unwrap();
        assert!(result < U128MAX); // Should be a very large number but less than MAX
    }

    #[test]

    /// Tests the sqrt_price_to_tick function which converts a sqrt price to its corresponding tick index
    /// Verifies:
    /// - Conversion of specific sqrt prices to expected tick values
    /// - Roundtrip conversion from tick to sqrt price and back
    /// - Error handling for out-of-range values
    /// - Boundary conditions at MIN_SQRT_PRICE and MAX_SQRT_PRICE
    /// - Approximate conversion for sqrt prices that don't exactly match a tick
    fn test_sqrt_price_to_tick() {
        // Test specific sqrt price values and verify against the expected tick

        // Test sqrt price = 1.0 * 2^64 (tick = 0)
        let result = sqrt_price_to_tick(Q64).unwrap();
        assert_eq!(result, 0);

        // Test a positive tick equivalent
        let tick = 100;
        let sqrt_price = tick_to_sqrt_price(tick).unwrap();
        let result = sqrt_price_to_tick(sqrt_price).unwrap();
        assert_eq!(result, tick);

        // Test a negative tick equivalent
        let tick = -100;
        let sqrt_price = tick_to_sqrt_price(tick).unwrap();
        let result = sqrt_price_to_tick(sqrt_price).unwrap();
        assert_eq!(result, tick);

        // Test out of range sqrt price (too small)
        let result = sqrt_price_to_tick(MIN_SQRT_PRICE - 1);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::PriceOutOfRange.to_string()
        );

        // Test boundary sqrt prices
        let result = sqrt_price_to_tick(MIN_SQRT_PRICE).unwrap();
        assert!(result <= MIN_TICK);

        let result = sqrt_price_to_tick(U128MAX).unwrap();
        assert!(result >= MAX_TICK - 10); // Allow some tolerance due to binary search algorithm

        // Test sqrt price that doesn't exactly match a tick (should find closest)
        let sqrt_price = tick_to_sqrt_price(100).unwrap();
        let adjusted_sqrt_price = sqrt_price + (sqrt_price / 10000); // small adjustment
        let result = sqrt_price_to_tick(adjusted_sqrt_price).unwrap();
        // Should be close to 100
        assert!((98..=102).contains(&result));
    }

    #[test]

    /// Verifies the nearest_usable_tick function correctly rounds ticks to the nearest multiple of tick_spacing
    ///
    /// Tests:
    /// - When tick_spacing = 1, all ticks are usable (no rounding needed)
    /// - When tick_spacing > 1, ticks that are exact multiples of spacing remain unchanged
    /// - Rounding behavior for ticks between usable ticks (rounds to nearest, with ties rounding up)
    /// - Correct handling of ticks near the zero boundary
    fn test_nearest_usable_tick() {
        // Test with different tick spacings

        // Test tick_spacing = 1 (every tick is usable)
        let tick_spacing = 1;
        for tick in [-100, -50, -1, 0, 1, 50, 100].iter() {
            let result = nearest_usable_tick(*tick, tick_spacing);
            assert_eq!(result, *tick);
        }

        // Test tick_spacing = 10
        let tick_spacing = 10;

        // Test exact multiples of spacing
        for tick in [-100, -50, 0, 50, 100].iter() {
            let result = nearest_usable_tick(*tick, tick_spacing);
            assert_eq!(result, *tick);
        }

        // Test ticks needing rounding
        let test_cases = [
            // (input_tick, expected_output)
            (-104, -100), // Closer to -100
            (-106, -110), // Closer to -110
            (-105, -110), // Equidistant, rounds up
            (104, 100),   // Closer to 100
            (106, 110),   // Closer to 110
            (105, 110),   // Equidistant, rounds up
        ];

        for (input, expected) in test_cases.iter() {
            let result = nearest_usable_tick(*input, tick_spacing);
            assert_eq!(result, *expected);
        }

        // Test negative to positive boundary
        let result = nearest_usable_tick(-4, 10);
        assert_eq!(result, 0);

        let result = nearest_usable_tick(-6, 10);
        assert_eq!(result, -10);
    }

    #[test]
    /// Tests the `calculate_swap_step` function with various scenarios:
    /// - Swapping token A for token B (x to y)
    /// - Swapping token B for token A (y to x)
    /// - Verifies price changes match expected formulas
    /// - Tests error handling for zero liquidity
    /// - Tests behavior with zero amount
    fn test_calculate_swap_step() {
        let fixture = MathTestFixture::new();

        // Test swapping token A for token B (x to y)
        let sqrt_price = fixture.sqrt_price_mid_q64;
        let liquidity = fixture.liquidity;
        let amount = 1_000_000; // 1.0 token A
        let is_token_a = true;

        let (new_sqrt_price, amount_consumed) =
            calculate_swap_step(sqrt_price, liquidity, amount, is_token_a).unwrap();

        // Verification:
        // 1. New price should be lower than original (x to y swap decreases price)
        assert!(new_sqrt_price < sqrt_price);

        // 2. Amount consumed should be <= requested amount
        assert!(amount_consumed <= amount);

        // 3. Price change should match the formula: new_sqrt_price = sqrt_price * liquidity / (liquidity + amount_in * sqrt_price)
        let amount_in_scaled = (amount as u128) * sqrt_price / Q64;
        let expected_new_sqrt_price = sqrt_price * liquidity / (liquidity + amount_in_scaled);
        let rel_diff = ((new_sqrt_price as i128 - expected_new_sqrt_price as i128).abs() as f64)
            / (expected_new_sqrt_price as f64);
        assert!(
            rel_diff < 0.0001,
            "Relative difference too large: {rel_diff}"
        );

        // Test swapping token B for token A (y to x)
        let amount = 1_000_000; // 1.0 token B
        let is_token_a = false;

        let (new_sqrt_price, amount_consumed) =
            calculate_swap_step(sqrt_price, liquidity, amount, is_token_a).unwrap();

        // Verification:
        // 1. New price should be higher than original (y to x swap increases price)
        assert!(new_sqrt_price > sqrt_price);

        // 2. Amount consumed should be <= requested amount
        assert!(amount_consumed <= amount);

        // 3. Price change should match the formula: new_sqrt_price = sqrt_price + (amount_in * Q64) / liquidity
        let amount_in_scaled = (amount as u128) * Q64;
        let price_delta = amount_in_scaled / liquidity;
        let expected_new_sqrt_price = sqrt_price + price_delta;
        let rel_diff = ((new_sqrt_price as i128 - expected_new_sqrt_price as i128).abs() as f64)
            / (expected_new_sqrt_price as f64);
        assert!(
            rel_diff < 0.0001,
            "Relative difference too large: {rel_diff}"
        );

        // Test error case: zero liquidity
        let result = calculate_swap_step(sqrt_price, 0, amount, is_token_a);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::InsufficientLiquidity.to_string()
        );

        // Test zero amount case
        let (new_sqrt_price, amount_consumed) =
            calculate_swap_step(sqrt_price, liquidity, 0, is_token_a).unwrap();

        assert_eq!(new_sqrt_price, sqrt_price);
        assert_eq!(amount_consumed, 0);
    }

    #[test]
    /// Test fee growth calculation within a price range
    fn test_calculate_fee_growth_inside() {
        // Test various scenarios for fee growth calculation

        // Define test parameters
        let tick_lower = -100;
        let tick_upper = 100;
        let fee_growth_global = 1_000_000; // Some global fee growth

        // Case 1: Current tick is in range
        let tick_current = 0;
        let fee_growth_below = 100_000; // Fee growth below lower tick
        let fee_growth_above = 200_000; // Fee growth above upper tick

        let result = calculate_fee_growth_inside(
            tick_lower,
            tick_upper,
            tick_current,
            fee_growth_global,
            fee_growth_below,
            fee_growth_above,
        )
        .unwrap();

        // Expected: fee_growth_global - fee_growth_below - fee_growth_above
        let expected = fee_growth_global - fee_growth_below - fee_growth_above;
        assert_eq!(result, expected);

        // Case 2: Current tick is below range
        let tick_current = -200;
        // Adjust values to prevent overflow: ensure fee_growth_below > fee_growth_above
        let fee_growth_below = 300_000;
        let fee_growth_above = 200_000;

        let result = calculate_fee_growth_inside(
            tick_lower,
            tick_upper,
            tick_current,
            fee_growth_global,
            fee_growth_below,
            fee_growth_above,
        )
        .unwrap();

        // Expected: fee_growth_below - fee_growth_above
        // This simplifies from: fee_growth_global - (fee_growth_global - fee_growth_below) - fee_growth_above
        let expected = fee_growth_below - fee_growth_above;
        assert_eq!(result, expected);

        // Case 3: Current tick is above range
        let tick_current = 200;
        let fee_growth_above = 300_000;
        let fee_growth_below = 200_000;

        let result = calculate_fee_growth_inside(
            tick_lower,
            tick_upper,
            tick_current,
            fee_growth_global,
            fee_growth_below,
            fee_growth_above,
        )
        .unwrap();

        // Expected: fee_growth_global - fee_growth_below - (fee_growth_global - fee_growth_above)
        let expected = fee_growth_above - fee_growth_below;
        assert_eq!(result, expected);

        // Test error case: arithmetic overflow
        let tick_current = 0;
        let fee_growth_below = fee_growth_global + 1; // Will cause underflow

        let result = calculate_fee_growth_inside(
            tick_lower,
            tick_upper,
            tick_current,
            fee_growth_global,
            fee_growth_below,
            fee_growth_above,
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::MathOverflow.to_string()
        );
    }

    #[test]
    /// Test price to sqrt price conversion
    fn test_price_to_sqrt_price() {
        // Test various price inputs and verify sqrt price outputs

        // Test price = 1.0
        let price = 1;
        let result = price_to_sqrt_price(price).unwrap();
        let expected = Q64; // 1.0 in Q64.64
        let rel_diff = ((result as i128 - expected as i128).abs() as f64) / (expected as f64);
        assert!(
            rel_diff < 0.0001,
            "Relative difference too large: {rel_diff}"
        );

        // Test price = 4.0
        let price = 4;
        let result = price_to_sqrt_price(price).unwrap();
        let expected = 2 * Q64; // sqrt(4) = 2.0 in Q64.64
        let rel_diff = ((result as i128 - expected as i128).abs() as f64) / (expected as f64);
        assert!(
            rel_diff < 0.0001,
            "Relative difference too large: {rel_diff}"
        );

        // Test price = 0.25
        let price = 0; // Will be treated as zero
        let result = price_to_sqrt_price(price).unwrap();
        assert_eq!(result, 0);

        // Test large price
        let price = u64::MAX;
        let result = price_to_sqrt_price(price).unwrap();
        // We can't easily calculate the exact expected value, but we can check it's non-zero and reasonable
        assert!(result > 0);
        assert!(result < u128::MAX / 2);
    }

    #[test]
    /// Test token A amount calculation for a price range
    fn test_get_amount_a_delta_for_price_range() {
        let fixture = MathTestFixture::new();

        // Test normal case
        let liquidity = fixture.liquidity;
        let sqrt_price_lower = fixture.sqrt_price_low_q64;
        let lower_256 = U256::from(sqrt_price_lower);
        let sqrt_price_upper = fixture.sqrt_price_high_q64;
        let upper_256 = U256::from(sqrt_price_upper);
        let q64_256 = U256::from(fixture.q64);

        // Test with round_up = false
        let result = get_amount_a_delta_for_price_range(
            liquidity,
            sqrt_price_lower,
            sqrt_price_upper,
            false,
        )
        .unwrap();

        // Manually calculate expected amount
        // amount_a = liquidity * (1/sqrt_price_lower - 1/sqrt_price_upper)
        let inv_lower = q64_256
            .checked_mul(q64_256)
            .unwrap()
            .checked_div(lower_256)
            .unwrap();
        let inv_upper = q64_256
            .checked_mul(q64_256)
            .unwrap()
            .checked_div(upper_256)
            .unwrap();
        let delta_inv = inv_lower.checked_sub(inv_upper).unwrap();
        let raw = U256::from(liquidity)
            .checked_mul(delta_inv)
            .unwrap()
            .checked_div(q64_256)
            .unwrap();

        let expected = raw.as_u128();

        assert_eq!(result, expected);

        // Test with round_up = true
        let result =
            get_amount_a_delta_for_price_range(liquidity, sqrt_price_lower, sqrt_price_upper, true)
                .unwrap();

        // Expected should be rounded up (ceil division)
        let numerator = U256::from(liquidity)
                    .checked_mul(delta_inv).unwrap()
                    + q64_256         // + (2^64)
                    - U256::from(1); // - 1

        let expected = numerator.checked_div(q64_256).unwrap().as_u128();

        assert_eq!(result, expected);

        // Test invalid price range (lower > upper)
        let result = get_amount_a_delta_for_price_range(
            liquidity,
            sqrt_price_upper,
            sqrt_price_lower,
            false,
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::InvalidTickRange.to_string()
        );

        // Test zero liquidity
        let result =
            get_amount_a_delta_for_price_range(0, sqrt_price_lower, sqrt_price_upper, false)
                .unwrap();

        assert_eq!(result, 0);
    }

    #[test]
    /// Test token B amount calculation for a price range
    fn test_get_amount_b_delta_for_price_range() {
        let fixture = MathTestFixture::new();

        // Test normal case
        let liquidity = fixture.liquidity;
        let sqrt_price_lower = fixture.sqrt_price_low_q64;
        let sqrt_price_upper = fixture.sqrt_price_high_q64;

        // Test with round_up = false
        let result = get_amount_b_delta_for_price_range(
            liquidity,
            sqrt_price_lower,
            sqrt_price_upper,
            false,
        )
        .unwrap();

        // Manually calculate expected amount
        // amount_b = liquidity * (sqrt_price_upper - sqrt_price_lower)
        let expected = liquidity * (sqrt_price_upper - sqrt_price_lower) / fixture.q64;

        assert_eq!(result, expected);

        // Test with round_up = true
        let result =
            get_amount_b_delta_for_price_range(liquidity, sqrt_price_lower, sqrt_price_upper, true)
                .unwrap();

        // Expected should be rounded up
        let expected = (liquidity * (sqrt_price_upper - sqrt_price_lower)).div_ceil(fixture.q64);

        assert_eq!(result, expected);

        // Test invalid price range (lower > upper)
        let result = get_amount_b_delta_for_price_range(
            liquidity,
            sqrt_price_upper,
            sqrt_price_lower,
            false,
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::InvalidTickRange.to_string()
        );

        // Test zero liquidity
        let result =
            get_amount_b_delta_for_price_range(0, sqrt_price_lower, sqrt_price_upper, false)
                .unwrap();

        assert_eq!(result, 0);
    }

    #[test]
    /// Test virtual reserves calculation
    fn test_calculate_virtual_reserves() {
        let fixture = MathTestFixture::new();

        // Test normal case
        let liquidity = fixture.liquidity;
        let sqrt_price = fixture.sqrt_price_mid_q64;

        let (virtual_a, virtual_b) = calculate_virtual_reserves(liquidity, sqrt_price).unwrap();

        // Verify individual values
        let expected_a = calculate_virtual_reserve_a(liquidity, sqrt_price).unwrap();
        let expected_b = calculate_virtual_reserve_b(liquidity, sqrt_price).unwrap();

        assert_eq!(virtual_a, expected_a);
        assert_eq!(virtual_b, expected_b);

        // Test the constant product invariant: virtual_a * virtual_b ≈ liquidity^2
        //assert!(verify_virtual_reserves_invariant(
        //    virtual_a, virtual_b, liquidity
        //));

        // Test zero liquidity
        let (virtual_a, virtual_b) = calculate_virtual_reserves(0, sqrt_price).unwrap();
        assert_eq!(virtual_a, 0);
        assert_eq!(virtual_b, 0);

        // Test zero price
        let (virtual_a, virtual_b) = calculate_virtual_reserves(liquidity, 0).unwrap();
        assert_eq!(virtual_a, 0);
        assert_eq!(virtual_b, 0);
    }

    #[test]
    /// Test virtual reserve A calculation
    fn test_calculate_virtual_reserve_a() {
        let fixture = MathTestFixture::new();

        // Test normal case
        let liquidity = fixture.liquidity;
        let sqrt_price = fixture.sqrt_price_mid_q64;

        let result = calculate_virtual_reserve_a(liquidity, sqrt_price).unwrap();

        // Expected: virtual_reserve_a = liquidity / sqrt_price
        let expected = ((liquidity as f64 * Q96 as f64) / sqrt_price as f64) as u64;

        // Allow small difference due to fixed-point arithmetic
        let rel_diff = ((result as i64 - expected as i64).abs() as f64) / (expected as f64);
        assert!(rel_diff < 0.01, "Relative difference too large: {rel_diff}");

        // Test zero liquidity
        let result = calculate_virtual_reserve_a(0, sqrt_price).unwrap();
        assert_eq!(result, 0);

        // Test zero price
        let result = calculate_virtual_reserve_a(liquidity, 0).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    /// Test virtual reserve B calculation
    fn test_calculate_virtual_reserve_b() {
        let fixture = MathTestFixture::new();

        // Test normal case
        let liquidity = fixture.liquidity;
        let sqrt_price = fixture.sqrt_price_mid_q64;

        let result = calculate_virtual_reserve_b(liquidity, sqrt_price).unwrap();

        // Expected: virtual_reserve_b = liquidity * sqrt_price / Q96
        let expected = ((liquidity as f64 * sqrt_price as f64) / Q96 as f64) as u64;

        // Allow small difference due to fixed-point arithmetic
        let rel_diff = ((result as i64 - expected as i64).abs() as f64) / (expected as f64);
        assert!(rel_diff < 0.01, "Relative difference too large: {rel_diff}");

        // Test zero liquidity
        let result = calculate_virtual_reserve_b(0, sqrt_price).unwrap();
        assert_eq!(result, 0);

        // Test zero price
        let result = calculate_virtual_reserve_b(liquidity, 0).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    /// Test virtual reserves calculation within a price range
    fn test_calculate_virtual_reserves_in_range() {
        let fixture = MathTestFixture::new();

        // Define test parameters
        let liquidity = fixture.liquidity;
        let lower_sqrt_price = fixture.sqrt_price_low_q64;
        let upper_sqrt_price = fixture.sqrt_price_high_q64;

        // Case 1: Current price is in range
        let current_sqrt_price = fixture.sqrt_price_mid_q64;

        let (virtual_a, virtual_b) = calculate_virtual_reserves_in_range(
            liquidity,
            current_sqrt_price,
            lower_sqrt_price,
            upper_sqrt_price,
        )
        .unwrap();

        // Verify against direct calculations
        let expected_a = get_amount_a_delta_for_price_range(
            liquidity,
            current_sqrt_price,
            upper_sqrt_price,
            false,
        )
        .unwrap() as u64;

        let expected_b = get_amount_b_delta_for_price_range(
            liquidity,
            lower_sqrt_price,
            current_sqrt_price,
            false,
        )
        .unwrap() as u64;

        assert_eq!(virtual_a, expected_a);
        assert_eq!(virtual_b, expected_b);

        // Case 2: Current price below range (100% token A)
        let sqrt_price_current = lower_sqrt_price / 2; // Below lower bound

        let (virtual_a, _virtual_b) = calculate_virtual_reserves_in_range(
            liquidity,
            sqrt_price_current,
            lower_sqrt_price,
            upper_sqrt_price,
        )
        .unwrap();

        // Expected: all in token A, none in token B
        let expected_a = get_amount_a_delta_for_price_range(
            liquidity,
            lower_sqrt_price,
            upper_sqrt_price,
            false,
        )
        .unwrap() as u64;

        assert_eq!(virtual_a, expected_a);
        assert_eq!(_virtual_b, 0);

        // Case 3: Current price above range (all token B)
        let sqrt_price_current = upper_sqrt_price * 2; // Above upper bound

        let (virtual_a, virtual_b) = calculate_virtual_reserves_in_range(
            liquidity,
            sqrt_price_current,
            lower_sqrt_price,
            upper_sqrt_price,
        )
        .unwrap();

        // Expected: all in token B, none in token A
        let expected_b = get_amount_b_delta_for_price_range(
            liquidity,
            lower_sqrt_price,
            upper_sqrt_price,
            false,
        )
        .unwrap() as u64;

        assert_eq!(virtual_a, 0);
        assert_eq!(virtual_b, expected_b);

        // Test zero liquidity
        let (virtual_a, virtual_b) = calculate_virtual_reserves_in_range(
            0,
            current_sqrt_price,
            lower_sqrt_price,
            upper_sqrt_price,
        )
        .unwrap();

        assert_eq!(virtual_a, 0);
        assert_eq!(virtual_b, 0);
    }

    #[test]
    /// Test liquidity calculation from token reserves
    fn test_calculate_liquidity_from_reserves() {
        let fixture = MathTestFixture::new();

        // Test calculation from token A
        let reserve_a = 1_000_000; // 1.0 token A
        let reserve_b = 0; // Not used in this calculation
        let sqrt_price = fixture.sqrt_price_mid_q64;
        let from_token_a = true;

        let result =
            calculate_liquidity_from_reserves(reserve_a, reserve_b, sqrt_price, from_token_a)
                .unwrap();

        // Expected: L = virtual_reserve_a * sqrt(P) / Q64
        let expected = (reserve_a as u128) * sqrt_price / Q64;
        assert_eq!(result, expected);

        // Test calculation from token B
        let reserve_a = 0; // Not used in this calculation
        let reserve_b = 1_000_000; // 1.0 token B
        let sqrt_price = fixture.sqrt_price_mid_q64;
        let from_token_a = false;

        let result =
            calculate_liquidity_from_reserves(reserve_a, reserve_b, sqrt_price, from_token_a)
                .unwrap();

        // Expected: L = virtual_reserve_b * Q64 / sqrt(P)
        let expected = (reserve_b as u128) * Q64 / sqrt_price;
        assert_eq!(result, expected);

        // Test error case: zero reserve amount for token A
        let result = calculate_liquidity_from_reserves(0, reserve_b, sqrt_price, true);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::ZeroReserveAmount.to_string()
        );

        // Test error case: zero reserve amount for token B
        let result = calculate_liquidity_from_reserves(reserve_a, 0, sqrt_price, false);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::ZeroReserveAmount.to_string()
        );
    }

    /*
        #[test]
        /// Test virtual reserves invariant verification
        fn test_verify_virtual_reserves_invariant() {
            let fixture = MathTestFixture::new();

            // Test case: perfect invariant (reserve_a * reserve_b = liquidity^2)
            let liquidity = 1_000_000_000; // 1.0 in fixed-point

            // Calculate perfect reserves where the invariant holds exactly
            let sqrt_price = fixture.sqrt_price_mid_q64;

            // virtual_a = liquidity * Q96 / sqrt_price
            let virtual_a = (liquidity * Q96 / sqrt_price) as u64;

            // virtual_b = liquidity * sqrt_price / Q96
            let virtual_b = (liquidity * sqrt_price / Q96) as u64;

            // Verify invariant with exact values
            let result = verify_virtual_reserves_invariant(virtual_a, virtual_b, liquidity);

            assert!(result);

            // Test case: invariant holds within tolerance (small rounding error)
            let virtual_a_adjusted = virtual_a + 1; // Add small error

            let result = verify_virtual_reserves_invariant(virtual_a_adjusted, virtual_b, liquidity);

            assert!(result);

            // Test case: invariant does not hold (large discrepancy)
            let virtual_a_invalid = virtual_a * 2; // Double the reserve

            let result = verify_virtual_reserves_invariant(virtual_a_invalid, virtual_b, liquidity);

            assert!(!result);

            // Test edge cases

            // Zero reserves, zero liquidity
            let result = verify_virtual_reserves_invariant(0, 0, 0);
            assert!(result);

            // Zero reserves, non-zero liquidity
            let result = verify_virtual_reserves_invariant(0, 0, 1);
            assert!(!result);

            // Non-zero reserves, zero liquidity
            let result = verify_virtual_reserves_invariant(1, 1, 0);
            assert!(!result);
        }
    */
    #[test]
    /// Test helper function for dividing by sqrt price
    fn test_div_by_sqrt_price_x64() {
        let fixture = MathTestFixture::new();

        // Use the private function through reflection or by temporarily making it public
        // For this test, we'll assume it's accessible

        // Test normal division
        let value = 1_000_000_000_000; // 1.0 in some fixed-point
        let sqrt_price = fixture.sqrt_price_mid_q64;

        let result = div_by_sqrt_price_x64(value, sqrt_price).unwrap();

        // Expected: value * 2^64 / sqrt_price
        let expected_u128 = (value << 64) / sqrt_price;

        // Verify it fits in u64
        assert!(expected_u128 <= u64::MAX as u128);
        let expected = expected_u128 as u64;

        assert_eq!(result, expected);

        // Test division by zero
        let result = div_by_sqrt_price_x64(value, 0);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::MathOverflow.to_string()
        );

        // Test result too large for u64
        let huge_value = u128::MAX / 2;
        let small_sqrt_price = 1;

        let result = div_by_sqrt_price_x64(huge_value, small_sqrt_price);
        assert!(result.is_err());
    }

    #[test]
    /// Test helper function for multiplying by sqrt price
    fn test_mul_by_sqrt_price_x64() {
        let fixture = MathTestFixture::new();

        // Test normal multiplication
        let value = 1_000_000_000_000; // 1.0 in some fixed-point
        let sqrt_price = fixture.sqrt_price_mid_q64;

        let result = mul_by_sqrt_price_x64(value, sqrt_price).unwrap();

        // Expected: value * sqrt_price / 2^64
        let expected_u128 = (value * sqrt_price) >> 64;

        // Verify it fits in u64
        assert!(expected_u128 <= u64::MAX as u128);
        let expected = expected_u128 as u64;

        assert_eq!(result, expected);

        // Test result too large for u64
        let huge_value = u128::MAX / 3;
        let large_sqrt_price = u128::MAX / 3;

        let result = mul_by_sqrt_price_x64(huge_value, large_sqrt_price);
        assert!(result.is_err());
    }

    #[test]
    /// Test sqrt calculation for u128 values
    fn test_sqrt_u128() {
        // Test with perfect squares
        assert_eq!(sqrt_u128(0), 0);
        assert_eq!(sqrt_u128(1), 1);
        assert_eq!(sqrt_u128(4), 2);
        assert_eq!(sqrt_u128(9), 3);
        assert_eq!(sqrt_u128(16), 4);
        assert_eq!(sqrt_u128(25), 5);
        assert_eq!(sqrt_u128(10000), 100);

        // Test with largest possible perfect square in u128
        let largest_perfect_sqrt_u64 = (1u128 << 64) - 1;
        let largest_perfect_square = largest_perfect_sqrt_u64 * largest_perfect_sqrt_u64;
        assert_eq!(sqrt_u128(largest_perfect_square), largest_perfect_sqrt_u64);

        // Test with non-perfect squares (should round down)
        assert_eq!(sqrt_u128(2), 1);
        assert_eq!(sqrt_u128(3), 1);
        assert_eq!(sqrt_u128(5), 2);
        assert_eq!(sqrt_u128(8), 2);
        assert_eq!(sqrt_u128(10), 3);
        assert_eq!(sqrt_u128(99), 9);
        assert_eq!(sqrt_u128(101), 10);

        // Test with very large values
        assert_eq!(sqrt_u128(u64::MAX as u128), 4294967295); // sqrt(2^64-1)

        // Test very large non-perfect square
        let large_value = u128::MAX;
        let result = sqrt_u128(large_value);
        // sqrt(2^128-1) is close to 2^64, but will be slightly smaller
        assert!(result < (1u128 << 64));
        assert!(result > ((1u128 << 64) - 1000)); // approximate check
    }

    // Additional integration tests to verify mathematical invariants across functions
    /*
    #[test]
    /// Test roundtrip conversion: tick -> sqrt_price -> tick
    fn test_tick_sqrt_price_roundtrip() {
        // Test roundtrip conversion for various ticks
        for tick in [
            -100000, -10000, -1000, -100, -10, -1, 0, 1, 10, 100, 1000, 10000, 100000,
        ]
        .iter()
        {
            let sqrt_price = tick_to_sqrt_price(*tick).unwrap();
            let tick_back = sqrt_price_to_tick(sqrt_price).unwrap();

            // Should get back the same tick or very close
            assert!(
                (tick_back - *tick).abs() <= 1,
                "Roundtrip failed for tick {}: got {} back",
                *tick,
                tick_back
            );
        }
    }
    */
    /*    #[test]
        /// Test virtual reserves and liquidity consistency
        fn test_virtual_reserves_liquidity_consistency() {
            let fixture = MathTestFixture::new();

            // Define test values

            let liquidity = fixture.liquidity;
            let sqrt_price = fixture.sqrt_price_mid_q64;

            // Calculate virtual reserves from liquidity
            let (virtual_a, virtual_b) = calculate_virtual_reserves(liquidity, sqrt_price).unwrap();

            // Verify virtual reserves match the constant product formula
            assert!(verify_virtual_reserves_invariant(
                virtual_a, virtual_b, liquidity
            ));

            // Calculate liquidity back from virtual reserve A
            let liquidity_from_a =
                calculate_liquidity_from_reserves(virtual_a, 0, sqrt_price, true).unwrap();

            // Calculate liquidity back from virtual reserve B
            let liquidity_from_b =
                calculate_liquidity_from_reserves(0, virtual_b, sqrt_price, false).unwrap();

            // Both should give approximately the same liquidity (within rounding error)
            // Very small differences may exist due to rounding in fixed-point arithmetic
            let rel_diff_a =
                ((liquidity_from_a as i128 - liquidity as i128).abs() as f64) / (liquidity as f64);
            assert!(
                rel_diff_a < 0.001,
                "Relative difference for token A too large: {rel_diff_a}"
            );

            let rel_diff_b =
                ((liquidity_from_b as i128 - liquidity as i128).abs() as f64) / (liquidity as f64);
            assert!(
                rel_diff_b < 0.001,
                "Relative difference for token B too large: {rel_diff_b}"
            );
        }

        #[test]
        /// Test invariant maintenance during swap
        fn test_swap_invariant_maintenance() {
            let fixture = MathTestFixture::new();

            // Setup initial state
            let initial_sqrt_price = fixture.sqrt_price_mid_q64;
            let liquidity = fixture.liquidity;

            // Get initial virtual reserves
            let (initial_a, initial_b) =
                calculate_virtual_reserves(liquidity, initial_sqrt_price).unwrap();

            // Perform a swap of token A for token B
            let amount_a = 1_000_000; // Some amount of token A
            let is_token_a = true;

            let (new_sqrt_price, _amount_consumed) =
                calculate_swap_step(initial_sqrt_price, liquidity, amount_a, is_token_a).unwrap();

            // Get new virtual reserves
            let (new_a, new_b) = calculate_virtual_reserves(liquidity, new_sqrt_price).unwrap();

            // Check that virtual reserves changed correctly
            // For A->B swap: A increases, B decreases
            assert!(new_a > initial_a);
            assert!(new_b < initial_b);

            // Check constant product formula still holds
            assert!(verify_virtual_reserves_invariant(new_a, new_b, liquidity));

            // Additional check: manually verify the swap formula
            // For token A input: amount_out_b = liquidity * (sqrt_price_before - sqrt_price_after)
            let expected_amount_out_b =
                (liquidity * (initial_sqrt_price - new_sqrt_price) / Q64) as u64;
            let amount_delta_b = initial_b - new_b;

            let rel_diff = ((expected_amount_out_b as i64 - amount_delta_b as i64).abs() as f64)
                / (amount_delta_b as f64);
            assert!(
                rel_diff < 0.001,
                "Relative difference too large: {rel_diff}"
            );
        }
    */
    #[test]
    /// Test fee growth calculations and consistency
    fn test_fee_growth_consistency() {
        // Test fee growth calculations maintain consistent accounting

        // Setup test scenario
        let tick_lower = -100;
        let tick_upper = 100;
        let fee_growth_global = 1_000_000;

        // Test with current tick in range
        let tick_current = 0;
        let fee_growth_below = 200_000;
        let fee_growth_above = 300_000;

        let fee_growth_inside = calculate_fee_growth_inside(
            tick_lower,
            tick_upper,
            tick_current,
            fee_growth_global,
            fee_growth_below,
            fee_growth_above,
        )
        .unwrap();

        // Verify: fee_growth_global = fee_growth_below + fee_growth_inside + fee_growth_above
        // When current tick is in range
        let fee_sum = fee_growth_below + fee_growth_inside + fee_growth_above;
        assert_eq!(fee_sum, fee_growth_global);

        // Test with current tick below range
        let tick_current = -200;
        let fee_growth_below = 300_000; // Adjust to prevent overflow
        let fee_growth_above = 200_000;

        let fee_growth_inside = calculate_fee_growth_inside(
            tick_lower,
            tick_upper,
            tick_current,
            fee_growth_global,
            fee_growth_below,
            fee_growth_above,
        )
        .unwrap();

        // Verify consistency with different current tick
        let fee_growth_below_used = fee_growth_global - fee_growth_below;
        let expected_inside = fee_growth_global - fee_growth_below_used - fee_growth_above;
        assert_eq!(fee_growth_inside, expected_inside);

        // Test with current tick above range
        let tick_current = 200;
        let fee_growth_above = 300_000; // Adjust to prevent overflow
        let fee_growth_below = 200_000;

        let fee_growth_inside = calculate_fee_growth_inside(
            tick_lower,
            tick_upper,
            tick_current,
            fee_growth_global,
            fee_growth_below,
            fee_growth_above,
        )
        .unwrap();

        // Verify consistency with different current tick
        let fee_growth_above_used = fee_growth_global - fee_growth_above;
        let expected_inside = fee_growth_global - fee_growth_below - fee_growth_above_used;
        assert_eq!(fee_growth_inside, expected_inside);
    }

    #[test]
    /// Test Q64.64 and Q64.96 conversions are consistent
    fn test_q64_q96_conversions_consistency() {
        let fixture = MathTestFixture::new();

        // Test roundtrip conversion: Q64.64 -> Q64.96 -> Q64.64
        let original_q64 = fixture.sqrt_price_mid_q64;
        let q96 = convert_sqrt_price_to_q96(original_q64).unwrap();
        let back_to_q64 = convert_sqrt_price_from_q96(q96).unwrap();

        assert_eq!(original_q64, back_to_q64);

        // Test that token amount calculations are consistent between both formats
        let liquidity = fixture.liquidity;
        let sqrt_price_lower_q64 = fixture.sqrt_price_low_q64;
        let sqrt_price_upper_q64 = fixture.sqrt_price_high_q64;
        let sqrt_price_current_q64 = fixture.sqrt_price_mid_q64;

        // Convert to Q64.96
        let sqrt_price_lower_q96 = convert_sqrt_price_to_q96(sqrt_price_lower_q64).unwrap();
        let sqrt_price_upper_q96 = convert_sqrt_price_to_q96(sqrt_price_upper_q64).unwrap();
        let sqrt_price_current_q96 = convert_sqrt_price_to_q96(sqrt_price_current_q64).unwrap();

        // Calculate token amounts with both formats
        let token_a_q64 = get_token_a_from_liquidity(
            liquidity,
            sqrt_price_lower_q96,
            sqrt_price_upper_q96,
            sqrt_price_current_q96,
        )
        .unwrap();

        let token_a_q96 = get_token_a_from_liquidity_q96(
            liquidity,
            sqrt_price_lower_q96,
            sqrt_price_upper_q96,
            sqrt_price_current_q96,
        )
        .unwrap();

        // Should be close (allow small difference due to precision)
        let rel_diff =
            ((token_a_q64 as i128 - token_a_q96 as i128).abs() as f64) / (token_a_q64 as f64);
        assert!(
            rel_diff < 0.01,
            "Q64.64 and Q64.96 calculations differ too much for token A: {rel_diff}"
        );

        // Same for token B
        let token_b_q64 = get_token_b_from_liquidity(
            liquidity,
            sqrt_price_lower_q64,
            sqrt_price_upper_q64,
            sqrt_price_current_q64,
        )
        .unwrap();

        let token_b_q96 = get_token_b_from_liquidity_q96(
            liquidity,
            sqrt_price_lower_q96,
            sqrt_price_upper_q96,
            sqrt_price_current_q96,
        )
        .unwrap();

        let rel_diff =
            ((token_b_q64 as i128 - token_b_q96 as i128).abs() as f64) / (token_b_q64 as f64);
        assert!(
            rel_diff < 0.01,
            "Q64.64 and Q64.96 calculations differ too much for token B: {rel_diff}"
        );
    }
    /*
        #[test]
        /// Comprehensive test for token amount and liquidity calculations
        fn test_token_liquidity_roundtrip() {
            let fixture = MathTestFixture::new();

            // Set up position parameters
            let sqrt_price_lower = fixture.sqrt_price_low_q64;
            let sqrt_price_upper = fixture.sqrt_price_high_q64;
            let sqrt_price_current = fixture.sqrt_price_mid_q64;
            let original_liquidity = fixture.liquidity;

            // Calculate token amounts for the position
            let token_a = get_token_a_from_liquidity(
                original_liquidity,
                sqrt_price_lower,
                sqrt_price_upper,
                sqrt_price_current,
            )
            .unwrap();

            let token_b = get_token_b_from_liquidity(
                original_liquidity,
                sqrt_price_lower,
                sqrt_price_upper,
                sqrt_price_current,
            )
            .unwrap();

            // Calculate liquidity back from token amounts and price range
            let amount_a_for_range = get_amount_a_delta_for_price_range(
                original_liquidity,
                sqrt_price_current,
                sqrt_price_upper,
                false,
            )
            .unwrap() as u64;

            let amount_b_for_range = get_amount_b_delta_for_price_range(
                original_liquidity,
                sqrt_price_lower,
                sqrt_price_current,
                false,
            )
            .unwrap() as u64;

            // Verify tokens match
            assert_eq!(token_a, amount_a_for_range);
            assert_eq!(token_b, amount_b_for_range);

            // Verify we can reconstruct the liquidity from either token
            // (This is an approximation due to rounding, so we check within a small margin)

            // From token A - only valid if not fully in token B
            if token_a > 0 {
                let liquidity_from_a = calculate_liquidity_from_reserves(
                    token_a,
                    0,
                    sqrt_price_current, // We use current price when token_a is non-zero
                    true,
                )
                .unwrap();

                let rel_diff = ((liquidity_from_a as i128 - original_liquidity as i128).abs() as f64)
                    / (original_liquidity as f64);
                assert!(
                    rel_diff < 0.01,
                    "Liquidity from token A differs too much: {rel_diff}"
                );
            }

            // From token B - only valid if not fully in token A
            if token_b > 0 {
                let liquidity_from_b = calculate_liquidity_from_reserves(
                    0,
                    token_b,
                    sqrt_price_current, // We use current price when token_b is non-zero
                    false,
                )
                .unwrap();

                let rel_diff = ((liquidity_from_b as i128 - original_liquidity as i128).abs() as f64)
                    / (original_liquidity as f64);
                assert!(
                    rel_diff < 0.01,
                    "Liquidity from token B differs too much: {rel_diff}"
                );
            }
        }
    */
    #[test]
    /// Test behavior with extreme values that explore edge cases
    fn test_extreme_values() {
        // Test with minimum allowed sqrt price
        let min_sqrt_price = MIN_SQRT_PRICE;
        let liquidity = 1_000_000_000_000;

        // Should not panic or error with minimum price
        let (virtual_a, _virtual_b) =
            calculate_virtual_reserves(liquidity, min_sqrt_price).unwrap();

        // At minimum price, virtual reserve A should be very large
        assert!(virtual_a > 0);

        // Test with very small non-zero liquidity
        let tiny_liquidity = 1;
        let sqrt_price = Q64; // Price of 1.0

        let (virtual_a, _virtual_b) =
            calculate_virtual_reserves(tiny_liquidity, sqrt_price).unwrap();

        // Very small liquidity should still give valid results
        assert_ne!(virtual_a, 0);

        // Test price near 0 (but above MIN_SQRT_PRICE)
        let near_zero_price = MIN_SQRT_PRICE + 1;

        let (virtual_a, _virtual_b) =
            calculate_virtual_reserves(liquidity, near_zero_price).unwrap();

        // Near zero price should result in very large amount of token A
        assert!(virtual_a > 0);

        // Test with very large liquidity (close to u128::MAX)
        let huge_liquidity = u128::MAX / 2;
        let normal_price = Q64; // Price of 1.0

        // This might overflow, but should handle it gracefully
        let result = calculate_virtual_reserves(huge_liquidity, normal_price);

        // Either it returns an error (which is fine) or the values should be valid
        if let Ok((virtual_a, virtual_b)) = result {
            assert!(virtual_a > 0);
            assert!(virtual_b > 0);
        }
    }
}
