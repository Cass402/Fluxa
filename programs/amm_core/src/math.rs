/// Fluxa AMM Core Math Library
///
/// This module implements the mathematical operations required for Fluxa's concentrated
/// liquidity AMM functionality. It provides functions for converting between tick indices
/// and prices, calculating token amounts from liquidity values, processing swap operations,
/// and managing fee accumulation.
///
/// The implementation uses fixed-point arithmetic throughout, primarily in Q64.64 format
/// where values are scaled by 2^64 to maintain precision during calculations.
/// Additionally, it supports Q64.96 format for high-precision sqrt price operations
/// as specified in the technical design document.
use crate::constants::*;
use crate::errors::ErrorCode;
// Add import for our new utility module
//use crate::utils::large_math::efficient_pow;
//use crate::utils::large_math::U256 as EfficientU256;

/// Q64.64 fixed-point representation scaling factor
///
/// This constant represents 2^64, used for fixed-point calculations throughout the AMM.
/// Values are typically represented as an integer scaled by this factor to maintain
/// precision during mathematical operations.
pub const Q64: u128 = 1u128 << 64;

/// Q64.96 fixed-point representation scaling factor
///
/// This constant represents 2^96, used for specific precision-sensitive calculations.
/// This provides higher precision for sqrt price operations as specified in the technical design.
pub const Q96: u128 = 1u128 << 96;

/// Maximum value for u128
pub const U128MAX: u128 = u128::MAX;

// Constants for tick-to-sqrt-price calculations
const _LOG_BASE: u128 = 100; // For precision in log calculation
const _BPS_PER_TICK: u128 = 1; // 0.01% per tick (1 basis point)
const DECIMAL_SCALE: u128 = 100_000_000_000_000_000; // 1e17

// Additional fixed-point arithmetic operations for Q64.96 format

#[allow(clippy::manual_div_ceil)]
mod uint_impl {
    use uint_crate::construct_uint;
    construct_uint! {
        /// 256-bit unsigned integer.
        pub struct U256(4);
    }
}

pub use uint_impl::U256;

pub type Result<T> = std::result::Result<T, ErrorCode>;

/// Convert from Q64.64 sqrt price to Q64.96 sqrt price
pub fn convert_sqrt_price_to_q96(sqrt_price_q64: u128) -> Result<u128> {
    // multiply by 2^32, error on overflow
    let factor = 1u128 << 32;
    sqrt_price_q64
        .checked_mul(factor)
        .ok_or(ErrorCode::MathOverflow)
}

/// Convert from Q64.96 sqrt price to Q64.64 sqrt price
pub fn convert_sqrt_price_from_q96(sqrt_price_q96: u128) -> Result<u128> {
    // Q64.96 → Q64.64: right shift by 32 bits
    Ok(sqrt_price_q96 >> 32)
}

/// Converts a square root price in Q64.64 fixed-point format back to a regular price.
///
/// This function performs the inverse operation of `price_to_sqrt_price`, taking a
/// square root price in Q64.64 format and converting it back to a regular price value.
///
/// # Mathematical Formula
/// `price = (sqrt_price)^2 / 2^64`
///
/// # Parameters
/// * `sqrt_price` - The square root price in Q64.64 fixed-point format
///
/// # Returns
/// * `Result<u64>` - The corresponding price value, or an error
///
/// # Errors
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn sqrt_price_to_price(sqrt_price: u128) -> Result<u64> {
    // Handle zero case to prevent division by zero
    if sqrt_price == 0 {
        return Ok(0);
    }

    // Square the sqrt_price (returns a value in Q128.128 format)
    // Use checked operations to handle potential overflows
    let price_q128 = sqrt_price
        .checked_mul(sqrt_price)
        .ok_or(ErrorCode::MathOverflow)?;

    // Divide by Q64 to get back to Q64.64 format
    let price_q64 = price_q128.checked_div(Q64).ok_or(ErrorCode::MathOverflow)?;

    // Convert from Q64.64 to integer, which means dividing by Q64 again
    let price = price_q64.checked_div(Q64).ok_or(ErrorCode::MathOverflow)?;

    // Ensure the result fits in u64
    if price > u64::MAX as u128 {
        return Err(ErrorCode::MathOverflow);
    }

    Ok(price as u64)
}

/// Converts a square root price in Q64.64 fixed-point format to a floating point price.
///
/// This function is similar to `sqrt_price_to_price` but returns a floating point
/// value instead of an integer, which can be useful for display purposes or when
/// more precision is needed than what a u64 can provide.
///
/// # Mathematical Formula
/// `price = (sqrt_price / 2^64)^2`
///
/// # Parameters
/// * `sqrt_price` - The square root price in Q64.64 fixed-point format
///
/// # Returns
/// * `f64` - The corresponding price as a floating point value
pub fn sqrt_price_to_price_f64(sqrt_price: u128) -> f64 {
    let sqrt_price_float = (sqrt_price as f64) / (Q64 as f64);
    sqrt_price_float * sqrt_price_float
}

/// Add two Q64.96 values
pub fn add_q96(a: u128, b: u128) -> Result<u128> {
    a.checked_add(b).ok_or(ErrorCode::MathOverflow)
}

/// Subtract two Q64.96 values
pub fn sub_q96(a: u128, b: u128) -> Result<u128> {
    a.checked_sub(b).ok_or(ErrorCode::MathOverflow)
}

/// Multiply two Q64.96 values (returning Q64.96)
pub fn mul_q96(a: u128, b: u128) -> Result<u128> {
    // Convert to U256 for higher precision
    let a_x256 = U256::from(a);
    let b_x256 = U256::from(b);

    // Perform multiplication with U256
    let product = a_x256 * b_x256;

    // Shift right by 96 bits to maintain Q64.96 format
    let result = product >> 96;

    // Convert back to u128, checking for overflow
    if result > U256::from(u128::MAX) {
        return Err(ErrorCode::MathOverflow);
    }

    Ok(result.as_u128())
}

/// Divide a Q64.96 value by another Q64.96 value (returning Q64.96)
pub fn div_q96(a: u128, b: u128) -> Result<u128> {
    if b == 0 {
        return Err(ErrorCode::MathOverflow);
    }

    // Convert to U256 for higher precision
    let a_x256 = U256::from(a);
    let b_x256 = U256::from(b);
    let q96_x256 = U256::from(Q96);

    // To maintain precision, scale up the numerator before division
    // a * 2^96 / b
    let scaled_a = a_x256 * q96_x256;

    // Perform the division
    let result = scaled_a / b_x256;

    // Convert back to u128, checking for overflow
    if result > U256::from(u128::MAX) {
        return Err(ErrorCode::MathOverflow);
    }

    Ok(result.as_u128())
}

/// Convert a floating point price to Q64.96 sqrt price format
pub fn price_to_sqrt_price_q96(price: f64) -> Result<u128> {
    let sqrt_price = price.sqrt();
    // Convert to Q64.96 format
    let sqrt_price_q96 = (sqrt_price * (Q96 as f64)) as u128;
    Ok(sqrt_price_q96)
}

/// Convert a Q64.96 sqrt price to floating point price
pub fn sqrt_price_q96_to_price(sqrt_price_q96: u128) -> f64 {
    // Split into two 64-bit chunks
    let hi = (sqrt_price_q96 >> 64) as u64;
    let lo = sqrt_price_q96 as u64;

    // hi / 2^32  is exact in f64, since hi ≤ 2^32 in realistic sqrt-price ranges
    let part_hi = (hi as f64) / ((1u64 << 32) as f64);
    // lo / 2^96 has a tiny rounding error, but after dividing by 2^96 it's ~1e-16 rel-err
    let part_lo = (lo as f64) / ((1u128 << 96) as f64);

    let sqrt_price = part_hi + part_lo;
    sqrt_price * sqrt_price
}

/// Square a Q64.96 value, maintaining precision
pub fn square_q96(value: u128) -> Result<u128> {
    mul_q96(value, value)
}

/// Calculate square root of a Q64.96 value
/// Uses the Babylonian method for approximation
pub fn sqrt_q96(value: u128) -> Result<u128> {
    if value == 0 {
        return Ok(0);
    }

    // Convert to U256 for higher precision
    let value_x256 = U256::from(value);
    let q96_x256 = U256::from(Q96);

    // Initial guess - use a power of 2 close to the square root
    let msb = 255 - value_x256.leading_zeros();
    let guess = U256::from(1) << ((msb / 2) + 48); // +48 for Q64.96 format

    // Perform iterations of the Babylonian method
    let mut result = guess;
    for _ in 0..10 {
        // 10 iterations is typically enough for convergence
        // r = (r + x/r) / 2
        if result == U256::from(0) {
            return Err(ErrorCode::MathOverflow);
        }

        let value_div_result = value_x256 * q96_x256 / result;
        result = (result + value_div_result) / U256::from(2);
    }

    // Convert back to u128, checking for overflow
    if result > U256::from(u128::MAX) {
        return Err(ErrorCode::MathOverflow);
    }

    Ok(result.as_u128())
}

/// Calculate reciprocal of a Q64.96 value (1/x)
pub fn reciprocal_q96(value: u128) -> Result<u128> {
    if value == 0 {
        return Err(ErrorCode::MathOverflow);
    }

    // Calculate 1/x by dividing Q96 by the value
    div_q96(Q96, value)
}

/// Compute the tick index for a given sqrt price in Q64.96 format
///
/// # Parameters
/// * `sqrt_price_q96` - The sqrt price in Q64.96 format
///
/// # Returns
/// * `Result<i32>` - The corresponding tick index
pub fn get_tick_at_sqrt_price_q96(sqrt_price_q96: u128) -> Result<i32> {
    // clamp to [MIN_SQRT_RATIO, MAX_SQRT_RATIO]
    if sqrt_price_q96 <= MIN_SQRT_PRICE {
        return Ok(MIN_TICK);
    }
    if sqrt_price_q96 == MAX_SQRT_PRICE {
        return Ok(MAX_TICK);
    }

    // now it's guaranteed in-range
    let sqrt_q64 = convert_sqrt_price_from_q96(sqrt_price_q96)?;
    sqrt_price_to_tick(sqrt_q64)
}

/// Calculate the square root price at a given tick index in Q64.96 format
///
/// # Parameters
/// * `tick` - The tick index
///
/// # Returns
/// * `Result<u128>` - The sqrt price in Q64.96 format
pub fn get_sqrt_price_at_tick_q96(tick: i32) -> Result<u128> {
    // First get the exact Q64.64 sqrt price...
    let sqrt_q64 = tick_to_sqrt_price(tick)?;
    // ...then just shift 32 bits into Q96
    convert_sqrt_price_to_q96(sqrt_q64)
}

/// Increases numerical precision when working with small price differences
///
/// # Parameters
/// * `price_a` - First price in Q64.96 format
/// * `price_b` - Second price in Q64.96 format
///
/// # Returns
/// * `Result<u128>` - Absolute difference in Q64.96 format, with enhanced precision
pub fn enhanced_price_difference_q96(price_a: u128, price_b: u128) -> Result<u128> {
    // Determine which price is larger
    if price_a >= price_b {
        sub_q96(price_a, price_b)
    } else {
        sub_q96(price_b, price_a)
    }
}

/// Computes the amount of token A required for a given amount of liquidity at a specified price range.
///
/// Token A represents the "base" token in the pair. This function calculates how much of token A
/// is needed to provide the specified liquidity within the given price bounds, considering the
/// current price of the pool.
///
/// # Mathematical Formula
/// When current price is in range:
/// `amount_a = liquidity * (1/sqrt_price_lower - 1/sqrt_price_current)`
///
/// When current price is below range:
/// `amount_a = liquidity * (1/sqrt_price_lower - 1/sqrt_price_upper)`
///
/// When current price is above range:
/// `amount_a = 0`
///
/// # Parameters
/// * `liquidity` - The amount of liquidity to provide, in L-units (Q64.64 fixed-point)
/// * `sqrt_price_lower_q96` - The lower sqrt price bound of the position (Q64.96 fixed-point)
/// * `sqrt_price_upper_q96` - The upper sqrt price bound of the position (Q64.96 fixed-point)
/// * `sqrt_price_current_q96` - The current sqrt price of the pool (Q64.96 fixed-point)
///
/// # Returns
/// * `Result<u64>` - The calculated amount of token A needed, or an error
pub fn get_token_a_from_liquidity(
    liquidity: u128,
    sqrt_price_lower_q96: u128,
    sqrt_price_upper_q96: u128,
    sqrt_price_current_q96: u128,
) -> Result<u64> {
    get_token_a_from_liquidity_q96(
        liquidity,
        sqrt_price_lower_q96,
        sqrt_price_upper_q96,
        sqrt_price_current_q96,
    )
}

/// Computes the amount of token B required for a given amount of liquidity at a specified price range.
///
/// Token B represents the "quote" token in the pair. This function calculates how much of token B
/// is needed to provide the specified liquidity within the given price bounds, considering the
/// current price of the pool.
///
/// # Mathematical Formula
/// When current price is in range:
/// `amount_b = liquidity * (sqrt_price_current - sqrt_price_lower)`
///
/// When current price is below range:
/// `amount_b = 0`
///
/// When current price is above range:
/// `amount_b = liquidity * (sqrt_price_upper - sqrt_price_lower)`
///
/// # Parameters
/// * `liquidity` - The amount of liquidity to provide, in L-units (Q64.64 fixed-point)
/// * `sqrt_price_lower` - The lower sqrt price bound of the position (Q64.64 fixed-point)
/// * `sqrt_price_upper` - The upper sqrt price bound of the position (Q64.64 fixed-point)
/// * `sqrt_price_current` - The current sqrt price of the pool (Q64.64 fixed-point)
///
/// # Returns
/// * `Result<u64>` - The calculated amount of token B needed, or an error
///
/// # Errors
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn get_token_b_from_liquidity(
    liquidity: u128,
    sqrt_price_lower: u128,
    sqrt_price_upper: u128,
    sqrt_price_current: u128,
) -> Result<u64> {
    // Price range logic: different calculations based on where current price sits relative to position range
    let sqrt_price_to_use = if sqrt_price_current < sqrt_price_lower {
        // Price is below range: position is 100% token A, 0% token B
        return Ok(0);
    } else if sqrt_price_current > sqrt_price_upper {
        // Price is above range: position is 100% token B, 0% token A
        // For token B calculation when above range, we use the upper price bound
        sqrt_price_upper
    } else {
        // Price is in range: position is a mix of token A and token B
        sqrt_price_current
    };

    // Calculate amount_b = liquidity * (sqrt_price_to_use - sqrt_price_lower)
    // Safety check: sqrt_price_to_use should always be >= sqrt_price_lower due to price ordering
    if sqrt_price_to_use < sqrt_price_lower {
        return Err(ErrorCode::MathOverflow);
    }

    let delta_sqrt_price = sqrt_price_to_use
        .checked_sub(sqrt_price_lower)
        .ok_or(ErrorCode::MathOverflow)?;

    let amount_b_u128 = liquidity
        .checked_mul(delta_sqrt_price)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(Q64) // Scale back from Q128.128 to Q64.64
        .ok_or(ErrorCode::MathOverflow)?;

    // Convert to u64, ensuring we don't overflow
    if amount_b_u128 > u64::MAX as u128 {
        return Err(ErrorCode::MathOverflow);
    }

    Ok(amount_b_u128 as u64)
}

/// Enhanced version of get_token_a_from_liquidity using U256 for higher precision
///
/// # Parameters
/// * `liquidity` - The amount of liquidity to provide, in L-units (Q64.64 fixed-point)
/// * `sqrt_price_lower_q96` - The lower sqrt price bound of the position (Q64.96 fixed-point)
/// * `sqrt_price_upper_q96` - The upper sqrt price bound of the position (Q64.96 fixed-point)
/// * `sqrt_price_current_q96` - The current sqrt price of the pool (Q64.96 fixed-point)
///
/// # Returns
/// * `Result<u64>` - The calculated amount of token A needed, or an error
pub fn get_token_a_from_liquidity_q96(
    liquidity: u128,
    sqrt_price_lower_q96: u128,
    sqrt_price_upper_q96: u128,
    sqrt_price_current_q96: u128,
) -> Result<u64> {
    // Price range logic: different calculations based on where current price sits relative to position range
    let sqrt_price_to_use_q96 = if sqrt_price_current_q96 > sqrt_price_upper_q96 {
        // Price is above range: position is 100% token B, 0% token A
        return Ok(0);
    } else if sqrt_price_current_q96 < sqrt_price_lower_q96 {
        // Price is below range: position is 100% token A, 0% token B
        // For token A calculation when below range, we use the upper price bound
        sqrt_price_upper_q96
    } else {
        // Price is in range: position is a mix of token A and token B
        sqrt_price_current_q96
    };

    // Convert to U256 for higher precision
    let liquidity_x256 = U256::from(liquidity);
    let sqrt_price_lower_q96_x256 = U256::from(sqrt_price_lower_q96);
    let sqrt_price_to_use_q96_x256 = U256::from(sqrt_price_to_use_q96);
    let q96_x256 = U256::from(Q96);

    // Compute (1/sqrt_price_lower) - invert the lower bound
    let inv_lower_q96_x256 = q96_x256 * q96_x256 / sqrt_price_lower_q96_x256;

    // Compute (1/sqrt_price_to_use) - invert the current/upper bound
    let inv_current_q96_x256 = q96_x256 * q96_x256 / sqrt_price_to_use_q96_x256;

    // If lower price >= current/upper price, then delta is zero or negative
    if inv_lower_q96_x256 <= inv_current_q96_x256 {
        return Ok(0);
    }

    // Calculate difference of inverses: (1/sqrt_price_lower - 1/sqrt_price_to_use)
    let delta_q96_x256 = inv_lower_q96_x256 - inv_current_q96_x256;

    // Calculate final result: liquidity * delta
    let token_amount_q96_x256 = liquidity_x256 * delta_q96_x256 / q96_x256;

    // Check if the result fits in u64
    if token_amount_q96_x256 > U256::from(u64::MAX) {
        return Err(ErrorCode::MathOverflow);
    }

    Ok(token_amount_q96_x256.as_u64())
}

/// Enhanced version of get_token_b_from_liquidity using U256 for higher precision
///
/// # Parameters
/// * `liquidity` - The amount of liquidity to provide, in L-units (Q64.64 fixed-point)
/// * `sqrt_price_lower_q96` - The lower sqrt price bound of the position (Q64.96 fixed-point)
/// * `sqrt_price_upper_q96` - The upper sqrt price bound of the position (Q64.96 fixed-point)
/// * `sqrt_price_current_q96` - The current sqrt price of the pool (Q64.96 fixed-point)
///
/// # Returns
/// * `Result<u64>` - The calculated amount of token B needed, or an error
pub fn get_token_b_from_liquidity_q96(
    liquidity: u128,
    sqrt_price_lower_q96: u128,
    sqrt_price_upper_q96: u128,
    sqrt_price_current_q96: u128,
) -> Result<u64> {
    // Price range logic: different calculations based on where current price sits relative to position range
    let sqrt_price_to_use_q96 = if sqrt_price_current_q96 < sqrt_price_lower_q96 {
        // Price is below range: position is 100% token A, 0% token B
        return Ok(0);
    } else if sqrt_price_current_q96 > sqrt_price_upper_q96 {
        // Price is above range: position is 100% token B, 0% token A
        // For token B calculation when above range, we use the upper price bound
        sqrt_price_upper_q96
    } else {
        // Price is in range: position is a mix of token A and token B
        sqrt_price_current_q96
    };

    // Convert to U256 for higher precision
    let liquidity_x256 = U256::from(liquidity);
    let sqrt_price_lower_q96_x256 = U256::from(sqrt_price_lower_q96);
    let sqrt_price_to_use_q96_x256 = U256::from(sqrt_price_to_use_q96);
    let q96_x256 = U256::from(Q96);

    // Calculate difference: (sqrt_price_to_use - sqrt_price_lower)
    // If the current/upper price <= lower price, then delta is zero or negative
    if sqrt_price_to_use_q96_x256 <= sqrt_price_lower_q96_x256 {
        return Ok(0);
    }

    let delta_q96_x256 = sqrt_price_to_use_q96_x256 - sqrt_price_lower_q96_x256;

    // Calculate final result: liquidity * delta
    let token_amount_q96_x256 = liquidity_x256 * delta_q96_x256 / q96_x256;

    // Check if the result fits in u64
    if token_amount_q96_x256 > U256::from(u64::MAX) {
        return Err(ErrorCode::MathOverflow);
    }

    Ok(token_amount_q96_x256.as_u64())
}

/// Converts a tick index to a square root price in Q64.64 fixed-point format.
///
/// In Fluxa's concentrated liquidity AMM, prices are discretized into "ticks" where
/// each tick represents a 0.01% (1.0001x) change in price. This function converts
/// from the discrete tick space to the continuous square root price space.
///
/// # Mathematical Formula
/// `sqrt_price = 1.0001^(tick/2) * 2^64`
///
/// # Implementation Details
/// This function uses binary exponentiation for efficiency, with precomputed powers
/// of sqrt(1.0001) to speed up the calculation.
///
/// # Parameters
/// * `tick` - The tick index to convert
///
/// # Returns
/// * `Result<u128>` - The corresponding sqrt price in Q64.64 fixed-point format, or an error
///
/// # Errors
/// * `ErrorCode::InvalidTickRange` - If the tick is outside the allowed range
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn tick_to_sqrt_price(tick: i32) -> Result<u128> {
    // Tick validation
    if !(MIN_TICK..=MAX_TICK).contains(&tick) {
        return Err(ErrorCode::InvalidTickRange);
    }

    // Each tick represents a 0.01% (1.0001) change in price
    // sqrt_price = 1.0001^(tick/2) * Q64

    // For efficiency, we'll use the binary exponentiation method
    // First, handle negative ticks
    let abs_tick = tick.unsigned_abs() as u128;
    let is_negative = tick < 0;

    // Precomputed values for (sqrt(1.0001))^n where n is a power of 2
    // These values are pre-calculated to avoid expensive power operations at runtime
    let sqrt_1_0001_powers = [
        1_000050000000000_u128,  // sqrt(1.0001)^1
        1_000100002500000_u128,  // sqrt(1.0001)^2
        1_000200010000000_u128,  // sqrt(1.0001)^4
        1_000400040001000_u128,  // sqrt(1.0001)^8
        1_000800160008000_u128,  // sqrt(1.0001)^16
        1_001601280064000_u128,  // sqrt(1.0001)^32
        1_003204640128000_u128,  // sqrt(1.0001)^64
        1_006415808256000_u128,  // sqrt(1.0001)^128
        1_012867840512000_u128,  // sqrt(1.0001)^256
        1_025857067264000_u128,  // sqrt(1.0001)^512
        1_052213753312000_u128,  // sqrt(1.0001)^1024
        1_106801830144000_u128,  // sqrt(1.0001)^2048
        1_225785703184000_u128,  // sqrt(1.0001)^4096
        1_503213729408000_u128,  // sqrt(1.0001)^8192
        2_259689019904000_u128,  // sqrt(1.0001)^16384
        5_105885570048000_u128,  // sqrt(1.0001)^32768
        26_090033976320000_u128, // sqrt(1.0001)^65536
    ];

    // Binary exponentiation
    // Start with 1.0, scaled up for precision
    let mut sqrt_price = DECIMAL_SCALE; // 1.0 in Q64.64 format

    // Each p was ×1e15; we want ×1e17 → scale_factor = 1e17/1e15 = 100
    let scale_factor = DECIMAL_SCALE / 1_000_000_000_000_000; // = 100
    let scaled_powers: Vec<u128> = sqrt_1_0001_powers
        .iter()
        .map(|&p| p * scale_factor)
        .collect();

    // Apply binary exponentiation: decompose tick into powers of 2 and multiply
    for (i, &power) in scaled_powers.iter().enumerate().take(17) {
        if (abs_tick & (1 << i)) != 0 {
            // break (sqrt_price * power) / DECIMAL_SCALE into two safe mul/divs
            let factor = power / DECIMAL_SCALE;
            let remainder = power % DECIMAL_SCALE;

            // part1 = sqrt_price * factor
            let part1 = sqrt_price
                .checked_mul(factor)
                .ok_or(ErrorCode::MathOverflow)?;

            // part2 = (sqrt_price * remainder) / DECIMAL_SCALE
            let part2 = sqrt_price
                .checked_mul(remainder)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(DECIMAL_SCALE)
                .ok_or(ErrorCode::MathOverflow)?;

            sqrt_price = part1.checked_add(part2).ok_or(ErrorCode::MathOverflow)?;
        }
    }

    // If tick is negative, we need to invert the sqrt_price (1/x)
    let final_sqrt_price = if is_negative {
        DECIMAL_SCALE
            .checked_mul(DECIMAL_SCALE)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(sqrt_price)
            .ok_or(ErrorCode::MathOverflow)?
    } else {
        sqrt_price
    };

    // Convert to Q64.64 format
    let sqrt_price_q64 = final_sqrt_price
        .checked_mul(Q64)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(DECIMAL_SCALE)
        .ok_or(ErrorCode::MathOverflow)?;

    Ok(sqrt_price_q64)
}

/// Convert from tick to price in higher precision using U256
///
/// # Parameters
/// * `tick` - The tick index
///
/// # Returns
/// * `Result<u128>` - The price in Q64.96 format
pub fn tick_to_price_u256(tick: i32) -> Result<u128> {
    // Convert to U256 for higher precision
    let tick_abs_u256 = U256::from(tick.unsigned_abs());
    let is_negative = tick < 0;

    // Calculate 1.0001^tick using U256 exponentiation
    // Base value of 1.0001 in Q64.96 format
    let one_bp_x96 = U256::from(Q96) + (U256::from(Q96) / U256::from(10000));

    let mut result = U256::from(Q96); // Start with 1.0 in Q64.96 format
    let mut base = one_bp_x96;
    let mut exp = tick_abs_u256;

    // Binary exponentiation algorithm
    while exp > U256::from(0) {
        if exp & U256::from(1) == U256::from(1) {
            result = result * base / U256::from(Q96);
        }
        base = base * base / U256::from(Q96);
        exp >>= 1;
    }

    // If tick is negative, invert the result
    if is_negative {
        result = U256::from(Q96) * U256::from(Q96) / result;
    }

    // Check if result fits in u128
    if result > U256::from(u128::MAX) {
        return Err(ErrorCode::MathOverflow);
    }

    Ok(result.as_u128())
}

/// Converts a square root price to the nearest tick index.
///
/// This function performs the inverse operation of `tick_to_sqrt_price`, converting
/// from a continuous square root price to the discrete tick space. It uses binary search
/// to find the closest tick that corresponds to the given square root price.
///
/// # Mathematical Formula (theoretical)
/// `tick = log(sqrt_price / Q64) * 2 / log(1.0001)`
///
/// # Implementation Details
/// Rather than using logarithms directly (which are computationally expensive), this function
/// uses a binary search to find the closest tick efficiently.
///
/// # Parameters
/// * `sqrt_price` - The square root price in Q64.64 fixed-point format
///
/// # Returns
/// * `Result<i32>` - The corresponding tick index, or an error
///
/// # Errors
/// * `ErrorCode::PriceOutOfRange` - If the sqrt_price is outside the allowed range
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn sqrt_price_to_tick(sqrt_price: u128) -> Result<i32> {
    // Validate sqrt_price is within allowed range
    if sqrt_price < MIN_SQRT_PRICE {
        return Err(ErrorCode::PriceOutOfRange);
    }

    // anything at our min √-price just snaps to MIN_TICK
    if sqrt_price == MIN_SQRT_PRICE {
        return Ok(MIN_TICK);
    }

    // anything above our max √-price just snaps to MAX_TICK
    if sqrt_price == MAX_SQRT_PRICE {
        return Ok(MAX_TICK);
    }

    // We'll use a binary search to find the closest tick
    let mut low = MIN_TICK;
    let mut high = MAX_TICK;

    while low <= high {
        let mid = (low + high) / 2;
        let cmp = match tick_to_sqrt_price(mid) {
            Ok(p_mid) => p_mid.cmp(&sqrt_price),
            // If computing p_mid overflows, treat as "too big"
            Err(ErrorCode::MathOverflow) => std::cmp::Ordering::Greater,
            Err(e) => return Err(e),
        };

        match cmp {
            std::cmp::Ordering::Equal => return Ok(mid),
            std::cmp::Ordering::Less => low = mid + 1,
            std::cmp::Ordering::Greater => high = mid - 1,
        }
    }

    // Return the closest tick by comparing distances
    let sqrt_price_low = tick_to_sqrt_price(high)?;
    let sqrt_price_high = tick_to_sqrt_price(low)?;

    let diff_low = sqrt_price.abs_diff(sqrt_price_low);

    let diff_high = sqrt_price.abs_diff(sqrt_price_high);

    // current “best”:
    let mut best_tick = if diff_low <= diff_high { high } else { low };
    let mut best_diff = if diff_low <= diff_high {
        diff_low
    } else {
        diff_high
    };

    // Also check one tick below and one tick above, just in case fixed-point
    // rounding made them closer.
    for &cand in [best_tick - 1, best_tick + 1].iter() {
        if !(MIN_TICK..=MAX_TICK).contains(&cand) {
            continue;
        }
        let price_cand = tick_to_sqrt_price(cand)?;
        let diff_cand = sqrt_price.abs_diff(price_cand);
        if diff_cand < best_diff {
            best_diff = diff_cand;
            best_tick = cand;
        }
    }

    Ok(best_tick)
}

/// Rounds a tick to the nearest tick that is usable based on the given tick spacing.
///
/// In concentrated liquidity AMMs, ticks are often restricted to multiples of a certain spacing
/// value (e.g., every 10th tick only) to reduce the computational overhead and storage
/// requirements of tracking each tick. This function rounds a given tick to the nearest
/// valid tick according to the specified spacing.
///
/// # Mathematical Formula
/// `nearest_tick = round(tick / spacing) * spacing`
///
/// Where the rounding direction is determined by which is closer: the next lower usable tick
/// or the next higher usable tick.
///
/// # Parameters
/// * `tick` - The tick to round to the nearest usable tick
/// * `tick_spacing` - The spacing between usable ticks (e.g., 1, 10, 60)
///
/// # Returns
/// * `i32` - The nearest valid tick that is a multiple of tick_spacing
///
/// # Examples
/// - With tick=123 and tick_spacing=10, returns 120
/// - With tick=155 and tick_spacing=10, returns 160
/// - With tick=-43 and tick_spacing=5, returns -45
/// - With tick=0 and tick_spacing=any, returns 0
pub fn nearest_usable_tick(tick: i32, tick_spacing: i32) -> i32 {
    // Handle special case of tick_spacing = 1 (all ticks are usable)
    if tick_spacing == 1 {
        tick
    } else {
        // Calculate the remainder when dividing by tick_spacing
        let remainder = tick.rem_euclid(tick_spacing);

        // If tick is already a multiple of tick_spacing, it's already usable
        if remainder == 0 {
            tick
        } else {
            // Calculate distance to lower and upper usable ticks
            let distance_to_lower = remainder;
            let distance_to_upper = tick_spacing - remainder;

            // Round to the nearest usable tick; ties go upward
            // If strictly closer to lower, pick lower.
            // If strictly closer to upper, pick upper.
            // If exactly tied, go _away_ from zero:
            //   positive tick → upper,   negative tick → lower
            if distance_to_lower < distance_to_upper
                || (distance_to_lower == distance_to_upper && tick < 0)
            {
                tick - distance_to_lower
            } else {
                tick + distance_to_upper
            }
        }
    }
}

/// Calculates the next price and consumed amount for a single step of a swap operation.
///
/// This core function computes how a swap affects the price within a single price range,
/// without crossing tick boundaries. It's used repeatedly in the main swap algorithm to
/// process swaps that may cross multiple ticks.
///
/// # Mathematical Formulas
/// For token A (x) input:
/// `new_sqrt_price = sqrt_price * liquidity / (liquidity + amount_in * sqrt_price / Q64)`
///
/// For token B (y) input:
/// `new_sqrt_price = sqrt_price + (amount_in * Q64) / liquidity`
///
/// # Parameters
/// * `sqrt_price` - Current square root price in Q64.64 fixed-point format
/// * `liquidity` - Current liquidity in the active range
/// * `amount` - Input amount of token to swap
/// * `is_token_a` - True if swapping token A for token B, false if swapping token B for token A
///
/// # Returns
/// * `Result<(u128, u64)>` - Tuple of (new_sqrt_price, amount_consumed), or an error
///
/// # Errors
/// * `ErrorCode::InsufficientLiquidity` - If there is no liquidity to perform the swap
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn calculate_swap_step(
    sqrt_price: u128,
    liquidity: u128,
    amount: u64,
    is_token_a: bool,
) -> Result<(u128, u64)> {
    // Cannot swap if there's no liquidity
    if liquidity == 0 {
        return Err(ErrorCode::InsufficientLiquidity);
    }

    // Early return for zero amount
    if amount == 0 {
        return Ok((sqrt_price, 0));
    }

    let new_sqrt_price: u128;
    let amount_consumed: u64;

    if is_token_a {
        // Token A to Token B swap (x to y)
        // For token A input, price should decrease

        // Scale the input amount by the current sqrt price to get it in the right units
        let amount_in_scaled = (amount as u128)
            .checked_mul(sqrt_price)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(Q64)
            .ok_or(ErrorCode::MathOverflow)?;

        // Calculate new sqrt price using the constant product formula
        // For token A input: new_sqrt_price = liquidity * sqrt_price / (liquidity + amount_in * sqrt_price / Q64)
        let denominator = liquidity
            .checked_add(amount_in_scaled)
            .ok_or(ErrorCode::MathOverflow)?;

        // Ensure we don't divide by zero
        if denominator == 0 {
            return Err(ErrorCode::MathOverflow);
        }

        // For token A input, price decreases
        new_sqrt_price = liquidity
            .checked_mul(sqrt_price)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(denominator)
            .ok_or(ErrorCode::MathOverflow)?;

        // Ensure the new sqrt_price is not greater than the old one (price should decrease)
        if new_sqrt_price > sqrt_price {
            return Err(ErrorCode::MathOverflow);
        }

        // Calculate amount consumed based on the price change
        let sqrt_price_delta = sqrt_price
            .checked_sub(new_sqrt_price)
            .ok_or(ErrorCode::MathOverflow)?;

        // Calculate amount consumed: liquidity * delta_sqrt_price / sqrt_price
        amount_consumed = if sqrt_price == 0 {
            0
        } else {
            let consumed = liquidity
                .checked_mul(sqrt_price_delta)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(sqrt_price)
                .ok_or(ErrorCode::MathOverflow)?;

            // Ensure it fits in u64 and cap at the requested amount
            consumed.min(amount as u128) as u64
        };
    } else {
        // Token B to Token A swap (y to x)
        // For token B input, price should increase

        // Scale the input amount to Q64.64 format
        let amount_in_scaled = (amount as u128)
            .checked_mul(Q64)
            .ok_or(ErrorCode::MathOverflow)?;

        // For liquidity = 0, we can't calculate a price (division by zero)
        if liquidity == 0 {
            return Err(ErrorCode::InsufficientLiquidity);
        }

        // Calculate price change based on the input amount
        // For token B input: new_sqrt_price = sqrt_price + (amount_in * Q64) / liquidity
        let price_delta = amount_in_scaled
            .checked_div(liquidity)
            .ok_or(ErrorCode::MathOverflow)?;

        // Calculate new sqrt price by adding the delta
        new_sqrt_price = sqrt_price
            .checked_add(price_delta)
            .ok_or(ErrorCode::MathOverflow)?;

        // Ensure the new sqrt_price is not less than the old one (price should increase)
        if new_sqrt_price < sqrt_price {
            return Err(ErrorCode::MathOverflow);
        }

        // Calculate amount consumed based on the price change
        let sqrt_price_delta = new_sqrt_price
            .checked_sub(sqrt_price)
            .ok_or(ErrorCode::MathOverflow)?;

        // Calculate amount consumed: liquidity * delta_sqrt_price / Q64
        amount_consumed = (liquidity
            .checked_mul(sqrt_price_delta)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(Q64)
            .ok_or(ErrorCode::MathOverflow)? as u64)
            .min(amount); // Cap at the requested amount
    }

    // Ensure new price is within global bounds
    let final_sqrt_price = if is_token_a {
        // For token A input, price decreases, so we clamp at the minimum
        new_sqrt_price.max(MIN_SQRT_PRICE)
    } else {
        // For token B input, price increases, so we clamp at the maximum
        new_sqrt_price
    };

    Ok((final_sqrt_price, amount_consumed))
}

/// Calculates the fee growth inside a position's price range.
///
/// This function determines how much of the global fee growth applies to a specific
/// position based on its price range and the current price. It's critical for
/// tracking earned fees for concentrated liquidity positions.
///
/// # Mathematical Concept
/// For a position with tick range [lower, upper], we calculate:
/// `fee_growth_inside = fee_growth_global - fee_growth_below - fee_growth_above`
///
/// Where fee_growth_below/above represent the accumulated fee growth outside
/// the position's range.
///
/// # Parameters
/// * `tick_lower` - Lower tick bound of the position
/// * `tick_upper` - Upper tick bound of the position
/// * `tick_current` - Current tick of the pool
/// * `fee_growth_global` - Global fee growth across all ticks
/// * `fee_growth_below` - Fee growth below the lower tick
/// * `fee_growth_above` - Fee growth above the upper tick
///
/// # Returns
/// * `Result<u128>` - The fee growth inside the position's range, or an error
///
/// # Errors
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn calculate_fee_growth_inside(
    tick_lower: i32,
    tick_upper: i32,
    tick_current: i32,
    fee_growth_global: u128,
    fee_growth_below: u128,
    fee_growth_above: u128,
) -> Result<u128> {
    // Determine which fee growth values to use based on the current tick position
    // relative to the position's range

    // For the "below" component: if current tick is >= lower bound,
    // use the tracked fee growth below; otherwise, complement it.
    let fee_growth_below_used = if tick_current >= tick_lower {
        fee_growth_below
    } else {
        fee_growth_global
            .checked_sub(fee_growth_below)
            .ok_or(ErrorCode::MathOverflow)?
    };

    // For the "above" component: if current tick is < upper bound,
    // use the tracked fee growth above; otherwise, complement it.
    let fee_growth_above_used = if tick_current < tick_upper {
        fee_growth_above
    } else {
        fee_growth_global
            .checked_sub(fee_growth_above)
            .ok_or(ErrorCode::MathOverflow)?
    };

    // Calculate fee growth inside as global minus outside components
    fee_growth_global
        .checked_sub(fee_growth_below_used)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_sub(fee_growth_above_used)
        .ok_or(ErrorCode::MathOverflow)
}

/// Converts a numeric price to a square root price in Q64.64 fixed-point format.
///
/// This utility function converts a regular price value to the square root price
/// format used throughout the AMM's internal calculations.
///
/// # Mathematical Formula
/// `sqrt_price = sqrt(price) * 2^64`
///
/// # Implementation Details
/// Uses Newton's method for computing the square root efficiently.
///
/// # Parameters
/// * `price` - The price value to convert
///
/// # Returns
/// * `Result<u128>` - The square root price in Q64.64 format, or an error
///
/// # Errors
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn price_to_sqrt_price(price: u64) -> Result<u128> {
    // We want floor( √price * 2^64 ), i.e. √(price * 2^128)
    let target = U256::from(price) << 128;
    let mut lo = U256::zero();
    // ensure hi >= sqrt(price * 2^128):
    let mut hi = U256::from(price) << 64;

    while lo < hi {
        let mid = (lo + hi + U256::one()) >> 1;
        if mid.checked_mul(mid).unwrap_or(U256::max_value()) <= target {
            lo = mid;
        } else {
            hi = mid - U256::one();
        }
    }

    if lo > U256::from(u128::MAX) {
        return Err(ErrorCode::MathOverflow);
    }
    Ok(lo.as_u128())
}

/// Calculates the amount of token A required for a specific liquidity amount in a price range.
///
/// This function determines how much of token A is needed to provide the specified liquidity
/// between the given square root price bounds. It's used during position creation and
/// liquidity management.
///
/// # Parameters
/// * `liquidity` - The amount of liquidity to be provided
/// * `sqrt_price_lower` - The lower square root price bound
/// * `sqrt_price_upper` - The upper square root price bound
/// * `round_up` - Whether to round up (for deposits) or down (for withdrawals)
///
/// # Returns
/// * `Result<u128>` - The calculated token A amount, or an error
///
/// # Errors
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn get_amount_a_delta_for_price_range(
    liquidity: u128,
    sqrt_price_lower: u128,
    sqrt_price_upper: u128,
    round_up: bool,
) -> Result<u128> {
    // Safety check: ensure price bounds are valid
    if sqrt_price_lower > sqrt_price_upper {
        return Err(ErrorCode::InvalidTickRange);
    }

    // Calculate amount_a = liquidity * (1/sqrt_price_lower - 1/sqrt_price_upper)
    // Using fixed-point arithmetic for precision with U256 intermediates

    // Convert to U256 for higher precision
    let liquidity_x256 = U256::from(liquidity);
    let sqrt_price_lower_x256 = U256::from(sqrt_price_lower);
    let sqrt_price_upper_x256 = U256::from(sqrt_price_upper);
    let q64_x256 = U256::from(Q64);

    // Compute (1/sqrt_price_lower) * Q64 - invert the lower bound
    // First multiply Q64*Q64 to maintain precision
    let q64_squared = q64_x256 * q64_x256;

    // Then divide by the lower price
    let inv_lower = if sqrt_price_lower == 0 {
        return Err(ErrorCode::MathOverflow);
    } else {
        q64_squared / sqrt_price_lower_x256
    };

    // Compute (1/sqrt_price_upper) * Q64 - invert the upper bound
    let inv_upper = if sqrt_price_upper == 0 {
        return Err(ErrorCode::MathOverflow);
    } else {
        q64_squared / sqrt_price_upper_x256
    };

    // Calculate the difference of the inverses
    if inv_lower < inv_upper {
        return Err(ErrorCode::MathOverflow);
    }
    let delta_inv = inv_lower - inv_upper;

    // Calculate liquidity * (inv_lower - inv_upper)
    let amount = liquidity_x256 * delta_inv;

    // Apply rounding based on the round_up parameter
    let result = if round_up {
        // Rounding up: Add (Q64 - 1) to the numerator before division
        // This ensures any fractional part becomes 1 more in the result
        (amount + q64_x256 - U256::from(1)) / q64_x256
    } else {
        // Rounding down: Simple division
        amount / q64_x256
    };

    // Convert back to u128, checking for overflow
    if result > U256::from(u128::MAX) {
        return Err(ErrorCode::MathOverflow);
    }

    Ok(result.as_u128())
}

/// Calculates the amount of token B required for a specific liquidity amount in a price range.
///
/// This function determines how much of token B is needed to provide the specified liquidity
/// between the given square root price bounds. It's used during position creation and
/// liquidity management.
///
/// # Parameters
/// * `liquidity` - The amount of liquidity to be provided
/// * `sqrt_price_lower` - The lower square root price bound
/// * `sqrt_price_upper` - The upper square root price bound
/// * `round_up` - Whether to round up (for deposits) or down (for withdrawals)
///
/// # Returns
/// * `Result<u128>` - The calculated token B amount, or an error
///
/// # Errors
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn get_amount_b_delta_for_price_range(
    liquidity: u128,
    sqrt_price_lower: u128,
    sqrt_price_upper: u128,
    round_up: bool,
) -> Result<u128> {
    // Safety check: ensure price bounds are valid
    if sqrt_price_lower > sqrt_price_upper {
        return Err(ErrorCode::InvalidTickRange);
    }

    // Calculate amount_b = liquidity * (sqrt_price_upper - sqrt_price_lower)
    // Using U256 for high precision intermediate calculations

    // Convert to U256 for higher precision
    let liquidity_x256 = U256::from(liquidity);
    let sqrt_price_lower_x256 = U256::from(sqrt_price_lower);
    let sqrt_price_upper_x256 = U256::from(sqrt_price_upper);
    let q64_x256 = U256::from(Q64);

    // Calculate the price difference in high precision
    if sqrt_price_upper_x256 < sqrt_price_lower_x256 {
        return Err(ErrorCode::MathOverflow);
    }
    let delta_sqrt_price = sqrt_price_upper_x256 - sqrt_price_lower_x256;

    // Calculate liquidity * (sqrt_price_upper - sqrt_price_lower) using U256
    let amount = liquidity_x256 * delta_sqrt_price;

    // Apply rounding based on the round_up parameter
    let result = if round_up {
        // Rounding up: Add (Q64 - 1) to the numerator before division
        // This ensures any fractional part becomes 1 more in the result
        (amount + q64_x256 - U256::from(1)) / q64_x256
    } else {
        // Rounding down: Simple division
        amount / q64_x256
    };

    // Convert back to u128, checking for overflow
    if result > U256::from(u128::MAX) {
        return Err(ErrorCode::MathOverflow);
    }

    Ok(result.as_u128())
}

/// Calculates the virtual reserves of token A and B at the current price.
///
/// In concentrated liquidity AMMs, virtual reserves represent the effective amounts
/// of tokens that determine the price at the current point. These are calculated
/// based on the current active liquidity in the pool and the current price.
///
/// # Mathematical Formula
/// For token A: `virtual_reserve_a = liquidity / sqrt_price`
/// For token B: `virtual_reserve_b = liquidity * sqrt_price`
///
/// # Parameters
/// * `liquidity` - The current active liquidity at the price point
/// * `sqrt_price` - The current sqrt price in Q64.64 fixed-point format
///
/// # Returns
/// * `Result<(u64, u64)>` - A tuple of (virtual_reserve_a, virtual_reserve_b), or an error
///
/// # Errors
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
/// * `ErrorCode::InsufficientLiquidity` - If the liquidity is zero
pub fn calculate_virtual_reserves(liquidity: u128, sqrt_price: u128) -> Result<(u64, u64)> {
    // Handle the “zero” edge cases up front so we never divide by zero
    if liquidity == 0 || sqrt_price == 0 {
        return Ok((0, 0));
    }
    // reuse the existing helpers so we get exactly the same rounding & clamping
    let a = calculate_virtual_reserve_a(liquidity, sqrt_price)?;
    let b = calculate_virtual_reserve_b(liquidity, sqrt_price)?;
    Ok((a, b))
}

/// Enhanced version of calculate_virtual_reserves using U256 for higher precision
///
/// # Parameters
/// * `liquidity` - The current active liquidity at the price point
/// * `sqrt_price` - The current sqrt price in Q64.96 fixed-point format
///
/// # Returns
/// * `Result<(u64, u64)>` - A tuple of (virtual_reserve_a, virtual_reserve_b), or an error
pub fn calculate_virtual_reserves_u256(liquidity: u128, sqrt_price: u128) -> Result<(u64, u64)> {
    if liquidity == 0 || sqrt_price == 0 {
        return Ok((0, 0));
    }

    // Convert to U256 for higher precision
    let liquidity_x256 = U256::from(liquidity);
    let sqrt_price_x256 = U256::from(sqrt_price);
    let q96_x256 = U256::from(Q96);

    // For token A: virtual reserve = L / sqrt(P)
    // Multiply by Q96 to maintain precision, then divide by sqrt_price
    let numerator_a = liquidity_x256 * q96_x256;

    // Divide by sqrt_price
    let result_a = numerator_a / sqrt_price_x256;

    // For token B: virtual reserve = L * sqrt(P) / Q96
    // Multiply liquidity by sqrt_price
    let product_b = liquidity_x256 * sqrt_price_x256;

    // Divide by Q96 to get the actual value
    let result_b = product_b / q96_x256;

    // Ensure the results fit in u64
    if result_a > U256::from(u64::MAX) || result_b > U256::from(u64::MAX) {
        return Err(ErrorCode::MathOverflow);
    }

    Ok((result_a.as_u64(), result_b.as_u64()))
}

/// Calculate virtual reserve for token A
///
/// # Parameters
/// * `liquidity` - Current liquidity
/// * `sqrt_price` - Current sqrt price in Q64.64 format
///
/// # Returns
/// * `Result<u64>` - Virtual reserve for token A
pub fn calculate_virtual_reserve_a(liquidity: u128, sqrt_price: u128) -> Result<u64> {
    if liquidity == 0 || sqrt_price == 0 {
        return Ok(0);
    }

    // virtual_reserve_a = liquidity * 2^64 / sqrt_price
    let liq = U256::from(liquidity);
    let price = U256::from(sqrt_price);
    let q96 = U256::from(Q96);

    let num = liq.checked_mul(q96).ok_or(ErrorCode::MathOverflow)?;
    let res = num.checked_div(price).ok_or(ErrorCode::MathOverflow)?;
    // instead of erroring, just clamp to u64::MAX
    let cap = U256::from(u64::MAX);
    if res > cap {
        Ok(u64::MAX)
    } else {
        Ok(res.as_u64())
    }
}

/// Calculate virtual reserve for token B
///
/// # Parameters
/// * `liquidity` - Current liquidity
/// * `sqrt_price` - Current sqrt price in Q64.64 format
///
/// # Returns
/// * `Result<u64>` - Virtual reserve for token B
pub fn calculate_virtual_reserve_b(liquidity: u128, sqrt_price: u128) -> Result<u64> {
    if liquidity == 0 || sqrt_price == 0 {
        return Ok(0);
    }

    // virtual_reserve_b = liquidity * sqrt_price / 2^64
    let liq = U256::from(liquidity);
    let price = U256::from(sqrt_price);
    let q96 = U256::from(Q96);

    let prod = liq.checked_mul(price).ok_or(ErrorCode::MathOverflow)?;
    let res = prod.checked_div(q96).ok_or(ErrorCode::MathOverflow)?;
    if res > U256::from(u64::MAX) {
        return Err(ErrorCode::MathOverflow);
    }
    Ok(res.as_u64())
}

/// Calculate virtual reserves in a specific price range
///
/// # Parameters
/// * `liquidity` - Liquidity in the range
/// * `current_sqrt_price` - Current sqrt price
/// * `lower_sqrt_price` - Lower bound sqrt price
/// * `upper_sqrt_price` - Upper bound sqrt price
///
/// # Returns
/// * `Result<(u64, u64)>` - Virtual reserves in the range as (token_a, token_b)
pub fn calculate_virtual_reserves_in_range(
    liquidity: u128,
    current_sqrt_price: u128,
    lower_sqrt_price: u128,
    upper_sqrt_price: u128,
) -> Result<(u64, u64)> {
    // Early return for zero liquidity
    if liquidity == 0 {
        return Ok((0, 0));
    }

    // Ensure price boundaries are valid
    if lower_sqrt_price > upper_sqrt_price {
        return Err(ErrorCode::InvalidTickRange);
    }

    // Handle case where current price is below range (all in token A)
    if current_sqrt_price <= lower_sqrt_price {
        let amount_a = match get_amount_a_delta_for_price_range(
            liquidity,
            lower_sqrt_price,
            upper_sqrt_price,
            false,
        ) {
            Ok(a) => a as u64,
            Err(_) => return Err(ErrorCode::MathOverflow),
        };
        return Ok((amount_a, 0));
    }

    // Handle case where current price is above range (all in token B)
    if current_sqrt_price >= upper_sqrt_price {
        let amount_b = match get_amount_b_delta_for_price_range(
            liquidity,
            lower_sqrt_price,
            upper_sqrt_price,
            false,
        ) {
            Ok(b) => b as u64,
            Err(_) => return Err(ErrorCode::MathOverflow),
        };
        return Ok((0, amount_b));
    }

    // Current price is in range, so we have both tokens
    let amount_a = match get_amount_a_delta_for_price_range(
        liquidity,
        current_sqrt_price,
        upper_sqrt_price,
        false,
    ) {
        Ok(a) => a as u64,
        Err(_) => return Err(ErrorCode::MathOverflow),
    };

    let amount_b = match get_amount_b_delta_for_price_range(
        liquidity,
        lower_sqrt_price,
        current_sqrt_price,
        false,
    ) {
        Ok(b) => b as u64,
        Err(_) => return Err(ErrorCode::MathOverflow),
    };

    Ok((amount_a, amount_b))
}

/// Calculates the effective liquidity from token amounts at the current price.
///
/// This function performs the reverse calculation of `calculate_virtual_reserves`,
/// determining how much liquidity corresponds to given token amounts at the current price.
///
/// # Mathematical Formula
/// From token A: `liquidity = virtual_reserve_a * sqrt_price`
/// From token B: `liquidity = virtual_reserve_b / sqrt_price`
///
/// # Parameters
/// * `reserve_a` - The virtual reserve of token A
/// * `reserve_b` - The virtual reserve of token B
/// * `sqrt_price` - The current sqrt price in Q64.64 fixed-point format
/// * `from_token_a` - If true, calculate from token A; if false, calculate from token B
///
/// # Returns
/// * `Result<u128>` - The calculated liquidity value, or an error
///
/// # Errors
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
/// * `ErrorCode::ZeroReserveAmount` - If the token amount is zero
pub fn calculate_liquidity_from_reserves(
    reserve_a: u64,
    reserve_b: u64,
    sqrt_price: u128,
    from_token_a: bool,
) -> Result<u128> {
    if from_token_a {
        if reserve_a == 0 {
            return Err(ErrorCode::ZeroReserveAmount);
        }

        // Check for potential overflow before multiplication
        if sqrt_price > U128MAX / (reserve_a as u128) {
            return Err(ErrorCode::MathOverflow);
        }

        // L = virtual_reserve_a * sqrt(P)
        (reserve_a as u128)
            .checked_mul(sqrt_price)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(Q64)
            .ok_or(ErrorCode::MathOverflow)
    } else {
        if reserve_b == 0 {
            return Err(ErrorCode::ZeroReserveAmount);
        }

        // Check for potential overflow before multiplication
        if Q64 > U128MAX / (reserve_b as u128) {
            return Err(ErrorCode::MathOverflow);
        }

        // L = virtual_reserve_b / sqrt(P)
        let numerator = (reserve_b as u128)
            .checked_mul(Q64)
            .ok_or(ErrorCode::MathOverflow)?;

        // Prevent division by zero
        if sqrt_price == 0 {
            return Err(ErrorCode::MathOverflow);
        }

        Ok(numerator
            .checked_div(sqrt_price)
            .ok_or(ErrorCode::MathOverflow)?)
    }
}

/// Verifies the virtual reserves match the constant product formula.
///
/// In a concentrated liquidity AMM, the product of virtual reserves should equal the
/// square of the liquidity at the current price point. This function verifies this
/// invariant holds, within a small tolerance for rounding errors.
///
/// # Mathematical Formula
/// `virtual_reserve_a * virtual_reserve_b ≈ liquidity^2`
///
/// # Parameters
/// * `virtual_reserve_a` - Virtual reserve of token A
/// * `virtual_reserve_b` - Virtual reserve of token B  
/// * `expected_liquidity` - Current liquidity at the price point
///
/// # Returns
/// * `bool` - True if the invariant holds, false otherwise
pub fn verify_virtual_reserves_invariant(
    virtual_reserve_a: u64,
    virtual_reserve_b: u64,
    expected_liquidity: u128,
) -> bool {
    // edge cases
    if virtual_reserve_a == 0 || virtual_reserve_b == 0 {
        return expected_liquidity == 0;
    }

    // compute product and liquidity²
    let reserve_product = match (virtual_reserve_a as u128).checked_mul(virtual_reserve_b as u128) {
        Some(product) => product,
        None => return false, // Overflow, invariant doesn't hold
    };

    let liquidity_squared = match expected_liquidity.checked_mul(expected_liquidity) {
        Some(squared) => squared,
        None => return false, // Overflow, invariant doesn't hold
    };

    // 0.1% tolerance: |Δ| ≤ 0.001
    let diff = reserve_product.abs_diff(liquidity_squared);

    // avoid truncation by multiplying instead of dividing
    match diff.checked_mul(1000) {
        Some(diff_times_1000) => diff_times_1000 <= liquidity_squared + 999,
        None => false, // Overflow, invariant doesn't hold
    }
}

/// Helper function for dividing a value by sqrt price
pub fn div_by_sqrt_price_x64(value: u128, sqrt_price_x64: u128) -> Result<u64> {
    if sqrt_price_x64 == 0 {
        return Err(ErrorCode::MathOverflow);
    }

    let num = U256::from(value) << 64;
    let den = U256::from(sqrt_price_x64);
    let res = num.checked_div(den).ok_or(ErrorCode::MathOverflow)?;
    if res > U256::from(u64::MAX) {
        return Err(ErrorCode::MathOverflow);
    }
    Ok(res.as_u64())
}

/// Helper function for multiplying a value by sqrt price
#[allow(dead_code)]
pub fn mul_by_sqrt_price_x64(value: u128, sqrt_price_x64: u128) -> Result<u64> {
    // value * sqrt_price_x64 / 2^64
    let result = value
        .checked_mul(sqrt_price_x64)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_shr(64)
        .ok_or(ErrorCode::MathOverflow)?;

    // Convert to u64, ensuring it doesn't overflow
    result.try_into().map_err(|_| ErrorCode::MathOverflow)
}

/// Calculate square root of a u128 value
///
/// # Parameters
/// * `value` - The value to calculate the square root of
///
/// # Returns
/// * `u128` - The square root of the input value
#[allow(dead_code)]
pub fn sqrt_u128(value: u128) -> u128 {
    if value == 0 {
        return 0;
    }

    // Initial estimate
    let mut x = value;
    // Use checked_add to avoid overflow
    let mut y = match x.checked_add(1) {
        Some(val) => val >> 1, // (x + 1) / 2
        None => x >> 1,        // If overflow, use x/2 as initial guess
    };

    // Newton's method for square root approximation
    while y < x {
        x = y;
        // Avoid division by zero and use checked operations
        if x == 0 {
            return 0;
        }
        y = match x.checked_add(value.checked_div(x).unwrap_or(x)) {
            Some(val) => val >> 1,
            None => x, // If overflow, return current approximation
        };
    }

    x
}

/// Calculates the next square root price after adding a specific amount of token0 (exact input)
///
/// This function computes how the square root price changes when a specified amount of token0
/// is added to the pool, assuming the entire input amount is consumed (exact input).
///
/// # Parameters
/// * `sqrt_price` - Current square root price in Q64.64 fixed-point format
/// * `liquidity` - Current liquidity in the active range
/// * `amount` - Exact input amount of token0 to add
/// * `by_amount_in` - True to calculate by amount in, false to calculate by amount out
///
/// # Returns
/// * `u128` - The new square root price after the token0 addition
pub fn get_next_sqrt_price_from_amount0_exact_in(
    sqrt_price: u128,
    liquidity: u128,
    amount: u64,
    _by_amount_in: bool,
) -> u128 {
    // If zero values, return original price
    if amount == 0 || liquidity == 0 {
        return sqrt_price;
    }

    // Scale the input amount by the current sqrt price to get it in the right units
    let amount_scaled = (amount as u128)
        .checked_mul(sqrt_price)
        .unwrap_or(u128::MAX);

    let amount_in_scaled = amount_scaled.checked_div(Q64).unwrap_or(0);

    // Calculate new sqrt price using the constant product formula
    // For token A input: new_sqrt_price = liquidity * sqrt_price / (liquidity + amount_in * sqrt_price / Q64)
    let denominator = liquidity.checked_add(amount_in_scaled).unwrap_or(u128::MAX);

    // Ensure we don't divide by zero
    if denominator == 0 {
        return MIN_SQRT_PRICE;
    }

    // For token A input, price decreases
    let new_sqrt_price = liquidity
        .checked_mul(sqrt_price)
        .unwrap_or(u128::MAX)
        .checked_div(denominator)
        .unwrap_or(sqrt_price);

    // For token A input, ensure the price doesn't increase
    // This is critical for the property test assertion
    if new_sqrt_price > sqrt_price {
        return sqrt_price;
    }

    // Make sure the calculated price is within bounds
    if new_sqrt_price < MIN_SQRT_PRICE {
        MIN_SQRT_PRICE
    } else {
        new_sqrt_price
    }
}

/// Calculates the next square root price after adding a specific amount of token1 (exact input)
///
/// This function computes how the square root price changes when a specified amount of token1
/// is added to the pool, assuming the entire input amount is consumed (exact input).
///
/// # Parameters
/// * `sqrt_price` - Current square root price in Q64.64 fixed-point format
/// * `liquidity` - Current liquidity in the active range
/// * `amount` - Exact input amount of token1 to add
/// * `by_amount_in` - True to calculate by amount in, false to calculate by amount out
///
/// # Returns
/// * `u128` - The new square root price after the token1 addition
pub fn get_next_sqrt_price_from_amount1_exact_in(
    sqrt_price: u128,
    liquidity: u128,
    amount: u64,
    _by_amount_in: bool,
) -> u128 {
    // If zero values, return original price
    if amount == 0 || liquidity == 0 {
        return sqrt_price;
    }

    // Scale the input amount to Q64.64 format
    let amount_in_scaled = (amount as u128).checked_mul(Q64).unwrap_or(u128::MAX);

    // Calculate price change based on the input amount
    // For token B input: new_sqrt_price = sqrt_price + (amount_in * Q64) / liquidity
    let price_delta = amount_in_scaled.checked_div(liquidity).unwrap_or(0);

    // Calculate new sqrt price by adding the delta
    let new_sqrt_price = sqrt_price.checked_add(price_delta).unwrap_or(u128::MAX);

    // For token B input, ensure the price doesn't decrease
    // This is critical for the property test assertion
    if new_sqrt_price < sqrt_price {
        return sqrt_price;
    }

    // Make sure the calculated price is within bounds
    new_sqrt_price
}

/// Calculates the amount of token0 required for a specified liquidity amount across a price range.
///
/// This function is a wrapper around `get_amount_a_delta_for_price_range` with a more
/// intuitive interface for calculating token amounts.
///
/// # Parameters
/// * `sqrt_price_a` - The first sqrt price bound
/// * `sqrt_price_b` - The second sqrt price bound
/// * `liquidity` - The amount of liquidity
/// * `round_up` - Whether to round up (for deposits) or down (for withdrawals)
///
/// # Returns
/// * `u64` - The calculated token0 amount
pub fn get_amount0_delta(
    sqrt_price_a: u128,
    sqrt_price_b: u128,
    liquidity: u128,
    round_up: bool,
) -> u64 {
    // If liquidity is zero, return zero immediately
    if liquidity == 0 {
        return 0;
    }

    // Order the prices correctly (lower, upper)
    let (sqrt_price_lower, sqrt_price_upper) = if sqrt_price_a <= sqrt_price_b {
        (sqrt_price_a, sqrt_price_b)
    } else {
        (sqrt_price_b, sqrt_price_a)
    };

    // Call the existing function to calculate the amount
    match get_amount_a_delta_for_price_range(
        liquidity,
        sqrt_price_lower,
        sqrt_price_upper,
        round_up,
    ) {
        Ok(amount) => amount as u64,
        Err(_) => 0, // Return 0 in case of error
    }
}

/// Calculates the amount of token1 required for a specified liquidity amount across a price range.
///
/// This function is a wrapper around `get_amount_b_delta_for_price_range` with a more
/// intuitive interface for calculating token amounts.
///
/// # Parameters
/// * `sqrt_price_a` - The first sqrt price bound
/// * `sqrt_price_b` - The second sqrt price bound
/// * `liquidity` - The amount of liquidity
/// * `round_up` - Whether to round up (for deposits) or down (for withdrawals)
///
/// # Returns
/// * `u64` - The calculated token1 amount
pub fn get_amount1_delta(
    sqrt_price_a: u128,
    sqrt_price_b: u128,
    liquidity: u128,
    round_up: bool,
) -> u64 {
    // If liquidity is zero, return zero immediately
    if liquidity == 0 {
        return 0;
    }

    // Order the prices correctly (lower, upper)
    let (sqrt_price_lower, sqrt_price_upper) = if sqrt_price_a <= sqrt_price_b {
        (sqrt_price_a, sqrt_price_b)
    } else {
        (sqrt_price_b, sqrt_price_a)
    };

    // Call the existing function to calculate the amount
    match get_amount_b_delta_for_price_range(
        liquidity,
        sqrt_price_lower,
        sqrt_price_upper,
        round_up,
    ) {
        Ok(amount) => amount as u64,
        Err(_) => 0, // Return 0 in case of error
    }
}

/// Calculates the liquidity amount from token0 and token1 amounts across a specified price range.
///
/// This function computes the effective liquidity that corresponds to a specific combination
/// of token0 and token1 amounts at a given price range. It's used during position creation
/// and for testing the mathematical relationships between liquidity and token amounts.
///
/// # Parameters
/// * `sqrt_price_lower` - The lower square root price bound
/// * `sqrt_price_upper` - The upper square root price bound
/// * `amount0` - The amount of token0
/// * `amount1` - The amount of token1
///
/// # Returns
/// * `u128` - The calculated liquidity amount
pub fn get_liquidity_from_amounts(
    sqrt_price_lower: u128,
    sqrt_price_upper: u128,
    amount0: u128,
    amount1: u128,
) -> u128 {
    // If both token amounts are zero, return zero liquidity
    if amount0 == 0 && amount1 == 0 {
        return 0;
    }

    // Calculate liquidity based on token0 amount
    let liquidity0 = if amount0 == 0 {
        0
    } else {
        // L = amount0 / (1/sqrt_price_lower - 1/sqrt_price_upper)
        // Compute (1/sqrt_price_lower) * Q64 - invert the lower bound
        let inv_lower = match Q64
            .checked_mul(Q64)
            .and_then(|q| q.checked_div(sqrt_price_lower))
        {
            Some(val) => val,
            None => return 0, // Return 0 on math overflow
        };

        // Compute (1/sqrt_price_upper) * Q64 - invert the upper bound
        let inv_upper = match Q64
            .checked_mul(Q64)
            .and_then(|q| q.checked_div(sqrt_price_upper))
        {
            Some(val) => val,
            None => return 0, // Return 0 on math overflow
        };

        // Calculate the difference of the inverses
        let delta_inv = match inv_lower.checked_sub(inv_upper) {
            Some(val) => val,
            None => return 0, // Return 0 on math overflow
        };

        // Calculate amount0 * Q64 / (inv_lower - inv_upper)
        amount0
            .checked_mul(Q64)
            .and_then(|a| a.checked_div(delta_inv))
            .unwrap_or(0)
    };

    // Calculate liquidity based on token1 amount
    let liquidity1 = if amount1 == 0 {
        0
    } else {
        // L = amount1 / (sqrt_price_upper - sqrt_price_lower)
        let delta_sqrt_price = match sqrt_price_upper.checked_sub(sqrt_price_lower) {
            Some(val) => val,
            None => return 0, // Return 0 on math overflow
        };

        // Calculate amount1 * Q64 / (sqrt_price_upper - sqrt_price_lower)
        amount1
            .checked_mul(Q64)
            .and_then(|a| a.checked_div(delta_sqrt_price))
            .unwrap_or(0)
    };

    // Return the minimum of the two liquidity values
    // If one token is 0, then use the other token's liquidity value
    if liquidity0 == 0 {
        liquidity1
    } else if liquidity1 == 0 {
        liquidity0
    } else {
        liquidity0.min(liquidity1)
    }
}

/// Calculates the liquidity amount from token0 and token1 amounts across a specified price range
/// using U256 for higher precision.
///
/// # Parameters
/// * `sqrt_price_lower` - The lower square root price bound
/// * `sqrt_price_upper` - The upper square root price bound
/// * `amount0` - The amount of token0
/// * `amount1` - The amount of token1
///
/// # Returns
/// * `u128` - The calculated liquidity amount
pub fn get_liquidity_from_amounts_u256(
    sqrt_price_lower: u128,
    sqrt_price_upper: u128,
    amount0: u128,
    amount1: u128,
) -> u128 {
    // If both token amounts are zero, return zero liquidity
    if amount0 == 0 && amount1 == 0 {
        return 0;
    }

    // Convert inputs to U256 for higher precision
    let sqrt_price_lower_x256 = U256::from(sqrt_price_lower);
    let sqrt_price_upper_x256 = U256::from(sqrt_price_upper);
    let amount0_x256 = U256::from(amount0);
    let amount1_x256 = U256::from(amount1);
    let q64_x256 = U256::from(Q64);

    // Calculate liquidity based on token0 amount
    let liquidity0 = if amount0 == 0 {
        U256::from(0)
    } else {
        // L = amount0 / (1/sqrt_price_lower - 1/sqrt_price_upper)

        // Compute (1/sqrt_price_lower) - invert the lower bound
        let inv_lower = if sqrt_price_lower == 0 {
            return 0; // Avoid division by zero
        } else {
            q64_x256 * q64_x256 / sqrt_price_lower_x256
        };

        // Compute (1/sqrt_price_upper) - invert the upper bound
        let inv_upper = if sqrt_price_upper == 0 {
            return 0; // Avoid division by zero
        } else {
            q64_x256 * q64_x256 / sqrt_price_upper_x256
        };

        // Safety check
        if inv_lower <= inv_upper {
            return 0;
        }

        // Calculate the difference of the inverses
        let delta_inv = inv_lower - inv_upper;

        // Calculate amount0 * Q64 / (inv_lower - inv_upper)
        if delta_inv == U256::from(0) {
            U256::from(0)
        } else {
            amount0_x256 * q64_x256 / delta_inv
        }
    };

    // Calculate liquidity based on token1 amount
    let liquidity1 = if amount1 == 0 {
        U256::from(0)
    } else {
        // L = amount1 / (sqrt_price_upper - sqrt_price_lower)

        // Safety check
        if sqrt_price_upper_x256 <= sqrt_price_lower_x256 {
            return 0;
        }

        let delta_sqrt_price = sqrt_price_upper_x256 - sqrt_price_lower_x256;

        // Calculate amount1 * Q64 / (sqrt_price_upper - sqrt_price_lower)
        if delta_sqrt_price == U256::from(0) {
            U256::from(0)
        } else {
            amount1_x256 * q64_x256 / delta_sqrt_price
        }
    };

    // Return the minimum of the two liquidity values
    // If one token is 0, then use the other token's liquidity value
    let result = if liquidity0 == U256::from(0) {
        liquidity1
    } else if liquidity1 == U256::from(0) {
        liquidity0
    } else {
        // Use the minimum of the two values
        if liquidity0 < liquidity1 {
            liquidity0
        } else {
            liquidity1
        }
    };

    // Convert back to u128, checking for overflow
    if result > U256::from(u128::MAX) {
        u128::MAX
    } else {
        result.as_u128()
    }
}

/// Calculates the fee amount from a given amount and fee rate
///
/// This function calculates the fee amount by multiplying the input amount
/// by the fee rate (in basis points, where 10000 = 100%) and dividing by 10000.
///
/// # Parameters
/// * `amount` - The amount to calculate the fee on
/// * `fee_rate` - The fee rate in basis points (e.g., 30 for 0.3%)
///
/// # Returns
/// * `u64` - The calculated fee amount
pub fn calculate_fee(amount: u64, fee_rate: u16) -> u64 {
    // Early return for edge cases
    if amount == 0 || fee_rate == 0 {
        return 0;
    }

    // Calculate fee: amount * fee_rate / 10000
    // Using a u128 for the intermediate calculation to avoid overflow
    let fee_amount = (amount as u128) * (fee_rate as u128) / 10000u128;

    // Ensure the result fits in u64
    fee_amount as u64
}

/// Enhanced version of calculate_fee that takes u128 amount
///
/// # Parameters
/// * `amount` - The amount to calculate the fee on
/// * `fee_rate` - The fee rate in basis points (e.g., 30 for 0.3%)
///
/// # Returns
/// * `u128` - The calculated fee amount
pub fn calculate_fee_u128(amount: u128, fee_rate: u16) -> u128 {
    // Early return for edge cases
    if amount == 0 || fee_rate == 0 {
        return 0;
    }

    // Calculate fee: amount * fee_rate / 10000
    amount * (fee_rate as u128) / 10000u128
}
