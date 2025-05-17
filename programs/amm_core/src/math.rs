/// Fluxa AMM Core Math Library
///
/// This module implements the mathematical operations required for Fluxa's concentrated
/// liquidity AMM functionality. It provides functions for converting between tick indices
/// and prices, calculating token amounts from liquidity values, processing swap operations,
/// and managing fee accumulation.
///
/// The implementation uses fixed-point arithmetic throughout, in Q64.64 format
/// where values are scaled by 2^64 to maintain precision during calculations.
use crate::constants::*;
use crate::errors::ErrorCode;
use anchor_lang::prelude::*;
/// # Arguments
/// * `a` - The first Q64.64 fixed-point number
/// * `b` - The second Q64.64 fixed-point number
///
/// # Returns
/// * `u128` - The product as a Q64.64 fixed-point number
///
/// # Example
///
/// ```
/// let a: u128 = 0x00000000000000010000000000000000; // 1.0 in Q64.64
/// let b: u128 = 0x00000000000000020000000000000000; // 2.0 in Q64.64
/// let result = mul_fixed(a, b); // 2.0 in Q64.64
/// assert_eq!(result, 0x00000000000000020000000000000000);
/// ```
// Use primitive type U256 for intermediary calculations to avoid overflow and keep precision
use primitive_types::U256;

#[inline(always)]
pub(crate) fn mul_fixed(a: u128, b: u128) -> u128 {
    let a_lo = a as u64 as u128; // Lower 64 bits of a
    let a_hi = (a >> 64) as u64 as u128; // Upper 64 bits of a
    let b_lo = b as u64 as u128; // Lower 64 bits of b
    let b_hi = (b >> 64) as u64 as u128; // Upper 64 bits of b

    let lo_lo = a_lo * b_lo; // Lower 64 bits of a * lower 64 bits of b
    let hi_lo = a_hi * b_lo; // Upper 64 bits of a * lower 64 bits of b
    let lo_hi = a_lo * b_hi; // Lower 64 bits of a * upper 64 bits of b
    let hi_hi = a_hi * b_hi; // Upper 64 bits of a * upper 64 bits of b

    // Carry from lower 64 bits to upper 64 bits
    let carry = lo_lo >> 64;
    let mid = hi_lo + lo_hi + carry;
    let high = hi_hi + (mid >> 64);

    // Product in Q64.64 format
    (high << 64) | (mid as u64 as u128)
}

/// Divides two Q64.64 fixed-point numbers
///
/// This function performs division of two Q64.64 fixed-point numbers
/// and returns the result as a Q64.64 fixed-point number.
///
/// # Arguments
/// * `a` - The dividend (Q64.64 fixed-point number)
/// * `b` - The divisor (Q64.64 fixed-point number)
///
/// # Returns
/// * `u128` - The quotient as a Q64.64 fixed-point number
///
/// # Panics
/// This function will panic if the divisor is zero.
///
/// # Example
///
/// ```
/// let a: u128 = 0x00000000000000020000000000000000; // 2.0 in Q64.64
/// let b: u128 = 0x00000000000000010000000000000000; // 1.0 in Q64.64
/// let result = div_fixed(a, b); // 2.0 in Q64.64
/// assert_eq!(result, 0x00000000000000020000000000000000);
/// ```
#[inline(always)]
pub(crate) fn div_fixed(a: u128, b: u128) -> u128 {
    // Check for division by zero
    debug_assert!(b != 0, "Division by zero: div_fixed() divisor is zero");

    // Scale 'a' by 2^64 using U256 to prevent overflow before division
    let a_u256 = U256::from(a) << 64;
    (a_u256 / U256::from(b)).as_u128()
}

/// Inverts a Q64.64 fixed-point number
///
/// This function calculates the reciprocal (1/x) of a Q64.64 fixed-point number
/// and returns the result as a Q64.64 fixed-point number.
///
/// # Arguments
/// * `x` - The Q64.64 fixed-point number to invert
///
/// # Returns
/// * `u128` - The reciprocal as a Q64.64 fixed-point number
///
/// # Panics
/// This function will panic if the input is zero.
///
/// # Example
///
/// ```
/// let x: u128 = 0x00000000000000020000000000000000; // 2.0 in Q64.64
/// let result = invert_fixed(x); // 0.5 in Q64.64
/// ```
#[inline(always)]
pub(crate) fn invert_fixed(x: u128) -> u128 {
    // 1.0 / x
    div_fixed(Q64, x)
}

/// Performs binary exponentiation using a precomputed table
///
/// This function calculates the result of raising a value to a power using
/// the binary exponentiation algorithm and a precomputed table of powers.
///
/// # Arguments
/// * `table` - A precomputed table of powers
/// * `exp` - The exponent to raise the base to
///
/// # Returns
/// * `u128` - The result of the exponentiation in fixed-point format
///
/// # Panics
/// This function will panic if the exponent is greater than the length of the table.
///
/// # Example
///
/// ```
/// let table: [u128; 64] = [0; 64]; // Precomputed table of powers
/// let exp: u32 = 5; // Exponent
/// let result = binary_pow(&table, exp); // Result of exponentiation
/// ```
#[inline(always)]
pub(crate) fn binary_pow(table: &[u128], mut exp: u32) -> u128 {
    // The original debug_assert was: exp < table.len(). This is incorrect.
    // `exp` is the exponent itself, `i` is the index into the table.

    let mut result = Q64;
    let mut i = 0;

    if exp == 0 {
        return Q64; // base^0 = 1.0
    }

    while exp > 0 {
        if i >= table.len() {
            panic!(
                "Exponent too large for POWERS table in binary_pow: exp={}, i={}, table_len={}",
                exp,
                i,
                table.len()
            );
        }
        if exp & 1 == 1 {
            result = mul_fixed(result, table[i]);
        }
        exp >>= 1;
        i += 1;
    }
    result
}

/// Calculates the square root of a fixed-point number using the Babylonian method
///
/// This function implements the Babylonian method (also known as Newton's method)
/// to calculate the square root of a fixed-point number.
///
/// # Arguments
/// * `x` - The fixed-point number to calculate the square root of
///
/// # Returns
/// * `u128` - The square root of the input in fixed-point format
///
/// # Panics
/// This function will panic if input exceed MAX_Q64_UNIT.
///
/// # Example
///
/// ```
/// let x: u128 = 0x00000000000000040000000000000000; // 4.0 in Q64.64
/// let result = babylonian_sqrt(x); // 2.0 in Q64.64
/// ```
#[inline(always)]
#[allow(dead_code)]
pub(crate) fn babylonian_sqrt(x: u128) -> u128 {
    if x == 0 {
        return 0;
    }

    // Initial guess. Q64 (1.0) is a common starting point.
    // A better guess can speed up convergence but adds complexity.
    // If x is very small, Q64 might be too large.
    let mut res_q64 = if x == 0 {
        0
    } else if x < Q64 {
        // x_val < 1.0
        x // For small x, x itself is a better start than 1.0
    } else {
        // For x_val >= 1.0, try to get a more reasonable starting point than 1.0
        // A simple approach: if x is very large, its sqrt will also be large.
        // Roughly, sqrt(N*2^k) = sqrt(N)*2^(k/2).
        // If x is, say, (Y * 2^64), then sqrt(x) is approx sqrt(Y) * 2^32.
        // A simple guess: take the integer part of x (x >> 64),
        // then find its integer sqrt, then shift back.
        // Or, just use x itself as a starting point if it's large, or a bit-shifted version.
        // Using x itself as a guess if x > Q64. Or Q64. Let's stick to Q64 for now and rely on iterations.
        // A common robust guess is 2^floor(log2(sqrt(x))).
        // For simplicity, let's use Q64 if x >= Q64, and x if x < Q64.
        // The previous logic was: Q64 for x >= Q64, and x for 0 < x < Q64. This is reasonable.
        // The issue might be the number of iterations for extreme values.
        Q64
    };
    if res_q64 == 0 && x > 0 {
        // Ensure guess is not zero if x is not zero
        res_q64 = 1; // Smallest representable positive Q64.64 fraction
    }

    // Fixed number of iterations for on-chain determinism and gas control.
    // 6-10 iterations are often sufficient for Q64.64 precision.
    const ITERATIONS: usize = 30; // Increased iterations
    for _ in 0..ITERATIONS {
        if res_q64 == 0 {
            break;
        } // Avoid division by zero if guess collapses
        let term_q64 = div_fixed(x, res_q64);
        // Average: (res + x/res) / 2, using U256 for the sum to prevent overflow
        res_q64 = ((U256::from(res_q64) + U256::from(term_q64)) >> 1).as_u128();
    }
    res_q64
}

/// Performs integer division with rounding up
///
/// This function divides `a` by `b` and rounds up the result to the nearest integer.
/// It's useful when you need ceiling division rather than the default floor division.
///
/// # Arguments
/// * `a` - The dividend (numerator)
/// * `b` - The divisor (denominator)
///
/// # Returns
/// * `u128` - The result of dividing `a` by `b`, rounded up
///
/// # Panics
/// This function will panic if `b` is zero.
///
/// # Example
///
/// ```
/// let a: u128 = 10; // Dividend
/// let b: u128 = 3; // Divisor
/// let result = round_up_div(a, b); // Result of division rounded up (4)
/// ```
#[inline(always)]
#[allow(dead_code)]
pub(crate) fn round_up_div(a: u128, b: u128) -> u128 {
    debug_assert!(b != 0, "Division by zero: round_up_div() divisor is zero");

    let (q, r) = (a / b, a % b);

    if r == 0 {
        q
    } else {
        q + 1
    }
}

/// Clamps a u128 value between a minimum and maximum value
///
/// This function ensures that the input value `x` is within the range [min, max].
/// If `x` is less than `min`, it returns `min`.
/// If `x` is greater than `max`, it returns `max`.
/// Otherwise, it returns `x` unchanged.
///
/// # Arguments
/// * `x` - The value to clamp
/// * `min` - The minimum allowed value
/// * `max` - The maximum allowed value
///
/// # Returns
/// * `u128` - The clamped value
///
/// # Panics
/// This function will panic if `min` is greater than `max`.
///
/// # Example
///
/// ```
/// let x: u128 = 10; // Value to clamp
/// let min: u128 = 5; // Minimum allowed value
/// let max: u128 = 15; // Maximum allowed value
/// let result = clamp_u128(x, min, max); // Result of clamping (10)
#[inline(always)]
#[allow(dead_code)]
pub(crate) fn clamp_u128(x: u128, min: u128, max: u128) -> u128 {
    debug_assert!(min <= max, "Clamp error: min is greater than max");

    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}

/// Converts a u64 integer to a Q64.64 fixed-point number
///
/// This function converts a u64 integer to a Q64.64 fixed-point number
/// by left-shifting by 64 bits.
///
/// # Arguments
/// * `amount` - The u64 integer to convert
///
/// # Returns
/// * `u128` - The Q64.64 fixed-point representation of the input
#[inline(always)]
#[allow(dead_code)]
pub(crate) fn to_q64(amount: u64) -> u128 {
    (amount as u128) << 64
}

/// Converts a Q64.64 fixed-point number to a u64 integer
///
/// This function extracts the integer part of a Q64.64 fixed-point number
/// by right-shifting by 64 bits and truncating to u64.
///
/// # Arguments
/// * `x` - The Q64.64 fixed-point number to convert
///
/// # Returns
/// * `u64` - The integer part of the fixed-point number
#[inline(always)]
#[allow(dead_code)]
pub(crate) fn from_q64(x: u128) -> u64 {
    (x >> 64) as u64
}

/// Converts a tick index to its corresponding sqrt price in Q64.64 fixed-point format
///
/// The function calculates the square root of the price corresponding to a given tick index
/// using the formula: sqrt(price) = 1.0001^(tick/2)
///
/// # Arguments
/// * `tick` - The tick index to convert
///
/// # Returns
/// * `Result<u128, ProgramError>` - The sqrt price in Q64.64 format or an error
///
/// # Panics
/// This function will panic if the tick index is out of bounds.
///
/// # Example
/// ```
/// let tick: i32 = 100; // Tick index
/// let result = tick_to_sqrt_price_q64(tick); // Resulting sqrt price in Q64.64 format
/// ```
pub fn tick_to_sqrt_price_q64(tick: i32) -> Result<u128> {
    if !(MIN_TICK..=MAX_TICK).contains(&tick) {
        return Err(ErrorCode::InvalidTickRange.into());
    }

    let abs_tick = tick.unsigned_abs();

    // The POWERS table in constants.rs stores (sqrt(1.0001))^(2^i).
    // binary_pow computes (sqrt(1.0001))^abs_tick.
    // Max index `i` accessed in binary_pow is floor(log2(abs_tick)).
    // If abs_tick is MAX_TICK (887272), i_max is 19. POWERS table has length 20 (indices 0-19).
    // The panic inside binary_pow will handle if abs_tick is unexpectedly too large for the table.

    let sqrt_price_abs_tick = binary_pow(&POWERS, abs_tick);

    let final_sqrt_price = if tick < 0 {
        invert_fixed(sqrt_price_abs_tick)
    } else {
        sqrt_price_abs_tick
    };

    // Ensure the result is within theoretical Q64.64 bounds if necessary,
    // though tick limits should prevent extreme values that overflow u128 itself.
    // MIN_SQRT_PRICE and MAX_SQRT_PRICE from constants.rs are based on these tick limits.
    // The calculation should naturally stay within these if POWERS table is correct.
    Ok(final_sqrt_price)
}

/// Converts a sqrt price in Q64.64 fixed-point format to its corresponding tick index
///
/// The function calculates the tick index corresponding to a given sqrt price
/// using the inverse of the formula: sqrt(price) = 1.0001^(tick/2)
///
/// # Arguments
/// * `sqrt_price` - The sqrt price in Q64.64 format to convert
///
/// # Returns
/// * `Result<i32, ProgramError>` - The tick index or an error
///
/// # Example
///
/// let sqrt_price: u128 = ...; // Sqrt price in Q64.64 format
/// let result = sqrt_price_q64_to_tick(sqrt_price); // Resulting tick index
///
pub fn sqrt_price_q64_to_tick(sqrt_price_q64: u128) -> Result<i32> {
    // Handle edge cases for sqrt_price_q64
    // If sqrt_price is 0, log is undefined. Price 0 implies tick is -infinity.
    if sqrt_price_q64 == 0 {
        // This case needs careful consideration based on protocol design.
        // Typically, price shouldn't be zero. If it can be, map to MIN_TICK or error.
        return Ok(MIN_TICK); // Or Err(ErrorCode::PriceOutOfRange.into())
    }

    // Based on constants.rs, MIN_SQRT_PRICE is 0, MAX_SQRT_PRICE is large.
    // Clamping/checking against these might be useful if sqrt_price_q64 can be outside them.
    // However, valid sqrt_price_q64 should correspond to a tick within [MIN_TICK, MAX_TICK].

    if sqrt_price_q64 == Q64 {
        // 1.0
        return Ok(0);
    }

    // Binary search for the tick `i` such that `tick_to_sqrt_price_q64(i)` is closest to `sqrt_price_q64`.
    // We want the largest tick `i` such that `sqrtP(i) <= sqrt_price_q64`.
    let mut low = MIN_TICK;
    let mut high = MAX_TICK;
    let mut ans = MIN_TICK; // Default to MIN_TICK

    while low <= high {
        // Calculate mid carefully to avoid overflow with i32
        let mid = low + (high - low) / 2;

        let mid_sqrt_price = tick_to_sqrt_price_q64(mid)?;

        if mid_sqrt_price <= sqrt_price_q64 {
            ans = mid; // mid is a potential candidate
            low = mid.checked_add(1).ok_or(ErrorCode::MathOverflow)?;
        } else {
            high = mid.checked_sub(1).ok_or(ErrorCode::MathOverflow)?;
        }
    }

    // ans should be the floor tick. Clamp to be absolutely sure, though binary search should maintain bounds.
    Ok(ans.clamp(MIN_TICK, MAX_TICK))
}

/// Calculates the amount of token 0 corresponding to a price range and liquidity
///
/// This function computes the amount of token 0 based on a price range defined by
/// sqrt_price_lower_q64 and sqrt_price_upper_q64, and the provided liquidity.
///
/// # Arguments
/// * `sqrt_price_lower_q64` - The lower sqrt price bound in Q64.64 format
/// * `sqrt_price_upper_q64` - The upper sqrt price bound in Q64.64 format
/// * `liquidity` - The amount of liquidity
/// * `round_up` - Whether to round up the result
///
/// # Returns
/// * `Result<u128, ProgramError>` - The calculated amount of token 0 or an error
///
/// # Example
/// ```
/// let sqrt_price_lower_q64: u128 = ...; // Lower sqrt price bound in Q64.64 format
/// let sqrt_price_upper_q64: u128 = ...; // Upper sqrt price bound in Q64.64 format
/// let liquidity: u128 = ...; // Amount of liquidity
/// let round_up: bool = ...; // Whether to round up the result
/// let result = get_amount_0_delta(sqrt_price_lower_q64, sqrt_price_upper_q64, liquidity, round_up);
/// ```
pub fn get_amount_0_delta(
    sqrt_price_lower_q64: u128,
    sqrt_price_upper_q64: u128,
    liquidity: u128,
    round_up: bool,
) -> Result<u128> {
    if sqrt_price_lower_q64 > sqrt_price_upper_q64 {
        return Err(ErrorCode::InvalidPriceRange.into());
    }
    if sqrt_price_lower_q64 == sqrt_price_upper_q64 {
        return Ok(0);
    }

    // Formula: ΔX = L * (1/sqrt_P_lower - 1/sqrt_P_upper)
    let inv_sqrt_lower_q64 = invert_fixed(sqrt_price_lower_q64);
    let inv_sqrt_upper_q64 = invert_fixed(sqrt_price_upper_q64);

    // (1/sqrt_P_lower - 1/sqrt_P_upper) can be negative if order is wrong, but we checked.
    let diff_inv_sqrt_q64 = inv_sqrt_lower_q64
        .checked_sub(inv_sqrt_upper_q64)
        .ok_or(ErrorCode::MathOverflow)?;

    let amount0_raw_u256 = U256::from(liquidity) * U256::from(diff_inv_sqrt_q64);
    let mut amount0_u256 = amount0_raw_u256 >> 64;
    let remainder_u256 = amount0_raw_u256 & (U256::from(Q64) - U256::one());

    if round_up && !remainder_u256.is_zero() {
        amount0_u256 = amount0_u256
            .checked_add(U256::one())
            .ok_or(ErrorCode::MathOverflow)?;
    }

    Ok(amount0_u256.as_u128())
}

/// Calculates the amount of token 1 corresponding to a price range and liquidity
///
/// This function computes the amount of token 1 based on a price range defined by
/// sqrt_price_lower_q64 and sqrt_price_upper_q64, and the provided liquidity.
///
/// # Arguments
/// * `sqrt_price_lower_q64` - The lower sqrt price bound in Q64.64 format
/// * `sqrt_price_upper_q64` - The upper sqrt price bound in Q64.64 format
/// * `liquidity` - The amount of liquidity
/// * `round_up` - Whether to round up the result
///
/// # Returns
/// * `Result<u128, ProgramError>` - The calculated amount of token 1 or an error
///
/// # Example
///
/// let sqrt_price_lower_q64: u128 = ...; // Lower sqrt price bound in Q64.64 format
/// let sqrt_price_upper_q64: u128 = ...; // Upper sqrt price bound in Q64.64 format
/// let liquidity: u128 = ...; // Amount of liquidity
/// let round_up: bool = ...; // Whether to round up the result
/// let result = get_amount_1_delta(sqrt_price_lower_q64, sqrt_price_upper_q64, liquidity, round_up);
///
pub fn get_amount_1_delta(
    sqrt_price_lower_q64: u128,
    sqrt_price_upper_q64: u128,
    liquidity: u128,
    round_up: bool,
) -> Result<u128> {
    if sqrt_price_lower_q64 > sqrt_price_upper_q64 {
        return Err(ErrorCode::InvalidPriceRange.into());
    }
    if sqrt_price_lower_q64 == sqrt_price_upper_q64 {
        return Ok(0);
    }

    // Formula: ΔY = L * (sqrt_P_upper - sqrt_P_lower)
    let diff_sqrt_q64 = sqrt_price_upper_q64
        .checked_sub(sqrt_price_lower_q64)
        .ok_or(ErrorCode::MathOverflow)?;

    let amount1_raw_u256 = U256::from(liquidity) * U256::from(diff_sqrt_q64);
    let mut amount1_u256 = amount1_raw_u256 >> 64;
    let remainder_u256 = amount1_raw_u256 & (U256::from(Q64) - U256::one());

    if round_up && !remainder_u256.is_zero() {
        amount1_u256 = amount1_u256
            .checked_add(U256::one())
            .ok_or(ErrorCode::MathOverflow)?;
    }

    Ok(amount1_u256.as_u128())
}

/// Calculates the liquidity amount for a given amount of token 0
///
/// This function computes the liquidity based on a price range defined by
/// sqrt_price_lower_q64 and sqrt_price_upper_q64, and the provided amount of token 0.
///
/// # Arguments
/// * `sqrt_price_lower_q64` - The lower sqrt price bound in Q64.64 format
/// * `sqrt_price_upper_q64` - The upper sqrt price bound in Q64.64 format
/// * `amount_0` - The amount of token 0
///
/// # Returns
/// * `Result<u128, ProgramError>` - The calculated liquidity amount or an error
///
/// # Example
///
/// let sqrt_price_lower_q64: u128 = ...; // Lower sqrt price bound in Q64.64 format
/// let sqrt_price_upper_q64: u128 = ...; // Upper sqrt price bound in Q64.64 format
/// let amount_0: u128 = ...; // Amount of token 0
/// let result = get_liquidity_for_amount0(sqrt_price_lower_q64, sqrt_price_upper_q64, amount_0);
///
pub fn get_liquidity_for_amount0(
    sqrt_price_lower_q64: u128,
    sqrt_price_upper_q64: u128,
    amount_0: u128,
) -> Result<u128> {
    if sqrt_price_lower_q64 > sqrt_price_upper_q64 {
        return Err(ErrorCode::InvalidPriceRange.into());
    }
    if sqrt_price_lower_q64 == sqrt_price_upper_q64 {
        // If amount_0 is > 0, this implies infinite liquidity, or an error.
        return if amount_0 == 0 {
            Ok(0)
        } else {
            Err(ErrorCode::ZeroOutputAmount.into()) // Or a more specific "DivisionByZero" if you add it
        };
    }

    // Formula: L = amount0 / (1/sqrt_P_lower - 1/sqrt_P_upper)
    let inv_sqrt_lower_q64 = invert_fixed(sqrt_price_lower_q64);
    let inv_sqrt_upper_q64 = invert_fixed(sqrt_price_upper_q64);
    let diff_inv_sqrt_q64 = inv_sqrt_lower_q64
        .checked_sub(inv_sqrt_upper_q64)
        .ok_or(ErrorCode::MathOverflow)?;

    if diff_inv_sqrt_q64 == 0 {
        // Should be caught by price check, but defensive
        return if amount_0 == 0 {
            Ok(0)
        } else {
            Err(ErrorCode::ZeroOutputAmount.into()) // Or a more specific "DivisionByZero"
        };
    }

    let liquidity_u256 = (U256::from(amount_0) << 64) / U256::from(diff_inv_sqrt_q64);
    Ok(liquidity_u256.as_u128())
}

/// Calculates the liquidity amount for a given amount of token 1
///
/// This function computes the liquidity based on a price range defined by
/// sqrt_price_lower_q64 and sqrt_price_upper_q64, and the provided amount of token 1.
///
/// # Arguments
/// * `sqrt_price_lower_q64` - The lower sqrt price bound in Q64.64 format
/// * `sqrt_price_upper_q64` - The upper sqrt price bound in Q64.64 format
/// * `amount_1` - The amount of token 1
///
/// # Returns
/// * `Result<u128, ProgramError>` - The calculated liquidity amount or an error
///
/// # Example
///
/// let sqrt_price_lower_q64: u128 = ...; // Lower sqrt price bound in Q64.64 format
/// let sqrt_price_upper_q64: u128 = ...; // Upper sqrt price bound in Q64.64 format
/// let amount_1: u128 = ...; // Amount of token 1
/// let result = get_liquidity_for_amount1(sqrt_price_lower_q64, sqrt_price_upper_q64, amount_1);
///
pub fn get_liquidity_for_amount1(
    sqrt_price_lower_q64: u128,
    sqrt_price_upper_q64: u128,
    amount_1: u128,
) -> Result<u128> {
    if sqrt_price_lower_q64 > sqrt_price_upper_q64 {
        return Err(ErrorCode::InvalidPriceRange.into());
    }
    if sqrt_price_lower_q64 == sqrt_price_upper_q64 {
        return if amount_1 == 0 {
            Ok(0)
        } else {
            Err(ErrorCode::ZeroOutputAmount.into()) // Or a more specific "DivisionByZero"
        };
    }

    // Formula: L = amount1 / (sqrt_P_upper - sqrt_P_lower)
    let diff_sqrt_q64 = sqrt_price_upper_q64
        .checked_sub(sqrt_price_lower_q64)
        .ok_or(ErrorCode::MathOverflow)?;

    if diff_sqrt_q64 == 0 {
        return if amount_1 == 0 {
            Ok(0)
        } else {
            Err(ErrorCode::ZeroOutputAmount.into()) // Or a more specific "DivisionByZero"
        };
    }

    let liquidity_u256 = (U256::from(amount_1) << 64) / U256::from(diff_sqrt_q64);
    Ok(liquidity_u256.as_u128())
}

/// Calculates the next sqrt price after adding a specified amount of token 0 to the pool
///
/// This function computes the next sqrt price based on the current sqrt price,
/// the current liquidity, and the amount of token 0 being added to the pool.
///
/// # Arguments
/// * `sqrt_price_current_q64` - The current sqrt price in Q64.64 format
/// * `liquidity` - The current liquidity in the pool
/// * `amount_0_in` - The amount of token 0 being added to the pool
///
/// # Returns
/// * `Result<u128, ProgramError>` - The calculated next sqrt price or an error
///
/// # Example
///
/// let sqrt_price_current_q64: u128 = ...; // Current sqrt price in Q64.64 format
/// let liquidity: u128 = ...; // Current liquidity
/// let amount_0_in: u128 = ...; // Amount of token 0 to add
/// let result = compute_next_sqrt_price_from_amount0_in(sqrt_price_current_q64, liquidity, amount_0_in);
///
pub fn compute_next_sqrt_price_from_amount0_in(
    sqrt_price_current_q64: u128,
    liquidity: u128,
    amount_0_in: u128,
) -> Result<u128> {
    if liquidity == 0 {
        // Or handle based on how zero liquidity swaps are defined.
        // Often, this means price moves infinitely, or it's an error.
        return Err(ErrorCode::InsufficientLiquidity.into()); // Using existing InsufficientLiquidity
    }
    if amount_0_in == 0 {
        return Ok(sqrt_price_current_q64);
    }

    // Formula: sqrt_P_next = (L * sqrt_P_curr) / (L + amount_in * sqrt_P_curr)
    // To implement with Q64.64 and u128 for L and amount_in:
    // sqrt_P_next_q64 = ( (L_int * sqrtP_q64_val) << 64 ) / ( (L_int << 64) + (amount_in_int * sqrtP_q64_val) )
    let num_term_u256 = U256::from(liquidity) * U256::from(sqrt_price_current_q64); // L_int * (sqrtP_val * 2^64)
    let den_term1_u256 = U256::from(liquidity) << 64; // L_int * 2^64
    let den_term2_u256 = U256::from(amount_0_in) * U256::from(sqrt_price_current_q64); // amount_in_int * (sqrtP_val * 2^64)
    let den_sum_u256 = den_term1_u256
        .checked_add(den_term2_u256)
        .ok_or(ErrorCode::MathOverflow)?;

    if den_sum_u256.is_zero() {
        return Err(ErrorCode::ZeroOutputAmount.into()); // Or a more specific "DivisionByZero"
    }

    let next_sqrt_price_q64 = ((num_term_u256 << 64) / den_sum_u256).as_u128();
    Ok(next_sqrt_price_q64)
}

/// Calculates the next sqrt price after adding a specified amount of token 1 to the pool
///
/// This function computes the next sqrt price based on the current sqrt price,
/// the current liquidity, and the amount of token 1 being added to the pool.
///
/// # Arguments
/// * `sqrt_price_current_q64` - The current sqrt price in Q64.64 format
/// * `liquidity` - The current liquidity in the pool
/// * `amount_1_in` - The amount of token 1 being added to the pool
///
/// # Returns
/// * `Result<u128, ProgramError>` - The calculated next sqrt price or an error
///
/// # Example
///
/// let sqrt_price_current_q64: u128 = ...; // Current sqrt price in Q64.64 format
/// let liquidity: u128 = ...; // Current liquidity
/// let amount_1_in: u128 = ...; // Amount of token 1 to add
/// let result = compute_next_sqrt_price_from_amount1_in(sqrt_price_current_q64, liquidity, amount_1_in);
///
pub fn compute_next_sqrt_price_from_amount1_in(
    sqrt_price_current_q64: u128,
    liquidity: u128,
    amount_1_in: u128,
) -> Result<u128> {
    if liquidity == 0 {
        return Err(ErrorCode::InsufficientLiquidity.into()); // Using existing InsufficientLiquidity
    }
    if amount_1_in == 0 {
        return Ok(sqrt_price_current_q64);
    }

    // Formula: sqrt_P_next = sqrt_P_current + amount1_in / L
    // amount1_in / L needs to be converted to Q64.64
    // term_q64 = (amount1_in_int * 2^64) / L_int
    let term_q64_u256 = (U256::from(amount_1_in) << 64) / U256::from(liquidity);

    let next_sqrt_price_q64 = sqrt_price_current_q64
        .checked_add(term_q64_u256.as_u128())
        .ok_or(ErrorCode::MathOverflow)?;

    Ok(next_sqrt_price_q64)
}
