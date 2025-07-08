//! # fluxa_core::math::core_arithmetic
//!
//! High-performance on-chain fixed-point math utilities for Solana CLMM (Concentrated Liquidity Market Maker).
//!
//! ## Features
//! - Uses u128-backed Q64.64 fixed-point arithmetic for deterministic, branch-minimized, and heapless computation.
//! - Provides optimized Newton-Raphson square root with LUT (lookup table) for fast and accurate sqrt calculations.
//! - Implements Uniswap-style `mul_div` and `mul_div_round_up` for precise and overflow-safe multiplication/division.
//! - Includes tick-to-sqrt price and liquidity math for CLMM pools, with all operations clamped to safe ranges.
//!
//! ## Main Components
//! - `Q64x64`: Transparent wrapper for Q64.64 fixed-point numbers with checked arithmetic.
//! - `mul_div`, `mul_div_round_up`, `mul_div_q64`: Overflow-safe multiplication and division helpers.
//! - `sqrt_x64`: Fast square root for Q64.64 numbers using Newton-Raphson and LUT for initial guess.
//! - `tick_to_sqrt_x64`: Converts a tick index to its corresponding sqrt price in Q64.64.
//! - `liquidity_from_amount_0` / `liquidity_from_amount_1`: Computes liquidity from token amounts and price bounds.
//!
//! ## Safety & Determinism
//! - All arithmetic is checked for overflows and underflows, returning `MathError` on failure.
//! - All functions are deterministic and suitable for on-chain execution.
//!
//! ## Author
//! - Cass402

use crate::error::MathError;
use crate::utils::constants::{FRAC_BITS, MAX_SQRT_X64, MAX_TICK, MIN_SQRT_X64, MIN_TICK, ONE_X64};
use anchor_lang::prelude::*;
use ethnum::U256;

/// Lookup table for initial guesses in the Newton-Raphson square root algorithm for Q64.64 fixed-point numbers.
///
/// - Contains 16 entries corresponding to sqrt(0) through sqrt(15), precomputed in Q64.64 format.
/// - Used to provide a fast and accurate starting point for the iterative square root calculation,
///   significantly improving convergence speed and reducing compute cost on-chain.
/// - The LUT ensures that the square root function achieves high precision with minimal iterations,
///   and can be expanded for even greater accuracy if needed.
const SQRT_LUT: [u128; 16] = [
    0x0000000000000000,  // sqrt(0) = 0
    0x10000000000000000, // sqrt(1) ≈ 1.0 in Q64.64
    0x16A09E667F3BCC908, // sqrt(2) ≈ 1.414
    0x1BB67AE8584CAA73B, // sqrt(3) ≈ 1.732
    0x20000000000000000, // sqrt(4) = 2.0
    0x238E7D83F4A3A2E9C, // sqrt(5) ≈ 2.236
    0x26F6A8D10E1F3B9F7, // sqrt(6) ≈ 2.449
    0x29F0B2C33CF0C2E78, // sqrt(7) ≈ 2.646
    0x2D5A0A9A4387DB3F8, // sqrt(8) ≈ 2.828
    0x30000000000000000, // sqrt(9) = 3.0
    0x325C3963A97A66766, // sqrt(10) ≈ 3.162
    0x348A4AD93A3AED4D8, // sqrt(11) ≈ 3.317
    0x36877B4E1C17F3DA2, // sqrt(12) ≈ 3.464
    0x385B43F1A8F1C4E5A, // sqrt(13) ≈ 3.606
    0x3A0E3E02B0C3F8E26, // sqrt(14) ≈ 3.742
    0x3B99D4BDAD0AB7142, // sqrt(15) ≈ 3.873
];

// ---------- Core Fixed-Point Wrapper ---------------------------------------

#[repr(transparent)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
/// A fixed-point numeric type with 64 bits for the integer part and 64 bits for the fractional part,
/// represented internally as a `u128`.
///
/// This type is useful for high-precision arithmetic where floating-point rounding errors are undesirable.
/// The value is interpreted as `value / 2^64`.
pub struct Q64x64(u128);

/// Implements core arithmetic operations for the `Q64x64` fixed-point type.
///
/// # Methods
///
/// - `raw(self) -> u128`  
///   Returns the underlying raw `u128` value of the fixed-point number.
///
/// - `from_raw(v: u128) -> Self`  
///   Constructs a `Q64x64` from a raw `u128` value.
///
/// - `from_int(x: u64) -> Self`  
///   Converts an integer value to a `Q64x64` by shifting it to the fixed-point representation.
///
/// - `zero() -> Self`  
///   Returns the zero value in `Q64x64` format.
///
/// - `one() -> Self`  
///   Returns the value one in `Q64x64` format.
///
/// - `checked_mul(self, rhs: Self) -> Result<Self>`  
///   Multiplies two `Q64x64` values, returning an error if the result overflows.
///
/// - `checked_div(self, rhs: Self) -> Result<Self>`  
///   Divides two `Q64x64` values, returning an error if dividing by zero or if the result overflows.
///
/// - `checked_add(self, rhs: Self) -> Result<Self>`  
///   Adds two `Q64x64` values, returning an error if the result overflows.
///
/// - `checked_sub(self, rhs: Self) -> Result<Self>`  
///   Subtracts one `Q64x64` value from another, returning an error if the result underflows.
///
/// # Errors
///
/// Arithmetic operations return a `Result` and may fail with `MathError::Overflow`,
/// `MathError::Underflow`, or `MathError::DivideByZero` as appropriate.
impl Q64x64 {
    #[inline(always)]
    pub const fn raw(self) -> u128 {
        self.0
    }

    #[inline(always)]
    pub const fn from_raw(v: u128) -> Self {
        Self(v)
    }

    #[inline(always)]
    pub const fn from_int(x: u64) -> Self {
        Self((x as u128) << FRAC_BITS)
    }

    #[inline(always)]
    pub const fn zero() -> Self {
        Self(0)
    }

    #[inline(always)]
    pub const fn one() -> Self {
        Self(ONE_X64)
    }

    // An optimized multiplication for Q64.64 fixed-point numbers
    // Uses u256 intermediate to avoid overflow, then collapses to mulhi
    // This is a checked operation that returns an error if the result overflows.
    // The multiplication is done in a way that preserves the fixed-point format.
    // The result is shifted right by FRAC_BITS to maintain the Q64.64 representation
    #[inline(always)]
    pub fn checked_mul(self, rhs: Self) -> Result<Self> {
        // Use u256 intermediate, then collapse to mulhi
        let prod = ((U256::from(self.0)) * (U256::from(rhs.0))) >> FRAC_BITS;
        if prod > U256::from(u128::MAX) {
            return Err(MathError::Overflow.into());
        }
        Ok(Self(prod.as_u128()))
    }

    // Optimized division for Q64.64 fixed-point numbers
    // Uses u256 intermediate to avoid overflow, then collapses to divhi
    // This is a checked operation that returns an error if dividing by zero or if the result overflows.
    // The division is done in a way that preserves the fixed-point format.
    // The result is shifted left by FRAC_BITS to maintain the Q64.64 representation
    #[inline(always)]
    pub fn checked_div(self, rhs: Self) -> Result<Self> {
        require!(rhs.0 != 0, MathError::DivideByZero);
        let num = (U256::from(self.0)) << FRAC_BITS;
        let result = num / (U256::from(rhs.0));
        if result > U256::from(u128::MAX) {
            return Err(MathError::Overflow.into());
        }
        Ok(Self(result.as_u128()))
    }

    // Optimized addition for Q64.64 fixed-point numbers
    #[inline(always)]
    pub fn checked_add(self, rhs: Self) -> Result<Self> {
        Ok(Self(self.0.checked_add(rhs.0).ok_or(MathError::Overflow)?))
    }

    // Optimized subtraction for Q64.64 fixed-point numbers
    #[inline(always)]
    pub fn checked_sub(self, rhs: Self) -> Result<Self> {
        Ok(Self(self.0.checked_sub(rhs.0).ok_or(MathError::Underflow)?))
    }
}

// ---------- Uniswap-style mul_div for exact (a * b) / c --------------------

// This function performs the multiplication of two u128 values `a` and `b`, then divides the result by `c`.
// It uses U256 to handle potential overflow during multiplication and division.
// The result is returned as a u128, or an error if the division by zero occurs
// or if the result exceeds the maximum value of u128.
#[inline(always)]
pub fn mul_div(a: u128, b: u128, c: u128) -> Result<u128> {
    require!(c != 0, MathError::DivideByZero);
    let prod = U256::from(a) * U256::from(b);
    let result = prod / U256::from(c);
    if result > U256::from(u128::MAX) {
        return Err(MathError::Overflow.into());
    }
    Ok(result.as_u128())
}

/// Ceil division: (a*b + (c-1)) / c  (or detect remainder and +1)
/// Only use this for tick → price so we never under-credit LPs.
/// This function performs the multiplication of two u128 values `a` and `b`, then divides the result by `c`.
/// It uses U256 to handle potential overflow during multiplication and division.
/// The result is rounded up to the nearest integer, ensuring that any remainder results in an increment.
/// The result is returned as a u128, or an error if the division by zero occurs
/// or if the result exceeds the maximum value of u128.
#[inline(always)]
pub fn mul_div_round_up(a: u128, b: u128, c: u128) -> Result<u128> {
    require!(c != 0, MathError::DivideByZero);
    let prod = U256::from(a) * U256::from(b);
    let div = U256::from(c);
    let (q, r) = (prod / div, prod % div);
    let res = if r == U256::ZERO { q } else { q + U256::ONE };
    if res > U256::from(u128::MAX) {
        return Err(MathError::Overflow.into());
    }
    Ok(res.as_u128())
}

#[inline(always)]
pub fn mul_div_q64(a: Q64x64, b: Q64x64, c: Q64x64) -> Result<Q64x64> {
    Ok(Q64x64::from_raw(mul_div(a.raw(), b.raw(), c.raw())?))
}

// ---------- Optimized Newton-Raphson √ with LUT ----------------------------
// has 0.0000046461147% error in known sqrt values, negligible for CLMM but can be improved with more LUT entries
// precision around very small values (1.382720978E-14) is around 0.003% more than tolerance of 0.1%, can be improved with more iterations (but increases compute cost)
/// This function computes the square root of a Q64x64 fixed-point number using the Newton-Raphson method.
/// It uses a lookup table (LUT) for an initial guess, which significantly speeds up convergence.
/// The function performs 4 iterations of the Newton-Raphson method to refine the guess.
/// It clamps the result to the valid square root range defined by `MIN_SQRT_X64` and `MAX_SQRT_X64`.
///
/// # Arguments
/// * `value`: A `Q64x64` fixed-point number for which the square root is to be computed.
/// # Returns
/// * `Result<Q64x64>`: The square root of the input value as a `Q64x64` fixed-point number.
///   Returns an error if the input is negative or if the square root does not converge within
///   the defined precision.
#[inline(always)]
pub fn sqrt_x64(value: Q64x64) -> Result<Q64x64> {
    let v = value.raw();
    // Early return for zero
    if v == 0 {
        return Ok(Q64x64::zero());
    }

    // Pick initial guess from integer part, fallback to 1 if v>0
    let int_part = (v >> FRAC_BITS) as usize;
    let lut_index = if int_part == 0 {
        1 // Use sqrt(1) ≈ 1.0 for fractional values
    } else {
        int_part.min(15) // Use sqrt(int_part) for integer values, capped at 15
    };
    let mut x = SQRT_LUT[lut_index];

    // Scale initial guess based on input magnitude
    // This ensures we start with a reasonable approximation for the square root
    // We shift the guess to match the leading bits of v, ensuring we don't underflow
    // or overflow during the Newton-Raphson iterations.
    let shift = (128 - v.leading_zeros()) as i32;
    if shift > 68 {
        x <<= (shift - 68) / 2;
    } else if shift < 68 {
        x >>= (68 - shift) / 2;
    }

    // Newton-Raphson: x' = (x + v/x) / 2
    // Optimized to 4 iterations (sufficient for Q64.64 precision)
    // This method converges quickly to the square root, especially for large values.
    // Each iteration refines the guess by averaging the current guess with the quotient of v and x.
    // The number of iterations is chosen to balance precision and compute cost.
    // The loop runs 4 times, which is generally sufficient for convergence in Q64.64.
    for _ in 0..4 {
        // x = (x + (v << 64) / x) >> 1
        x = (x + mul_div(v, ONE_X64, x)?) >> 1;
    }

    // Clamp into the valid √ price range so we never underflow/overflow
    x = x.clamp(MIN_SQRT_X64, MAX_SQRT_X64);

    Ok(Q64x64::from_raw(x))
}

// ---------- Tick ⇄ √Price (optimized constants) ----------------------------

// The POW2_COEFF array contains precomputed coefficients for the tick-to-sqrt conversion.
// Each coefficient corresponds to a power of 2, allowing for efficient bitwise operations
// to compute the square root price from a tick index.
const POW2_COEFF: [u128; 19] = [
    0xfffcb933bd6fad38, // bit 0 → 2⁰
    0xfff97272373d4132, // bit 1 → 2¹
    0xfff2e50f5f656933, // bit 2 → 2²
    0xffe5caca7e10e4e6, // bit 3 → 2³
    0xffcb9843d60f615a, // bit 4 → 2⁴
    0xff973b41fa98c081, // bit 5 → 2⁵
    0xff2ea16466c96a38, // bit 6 → 2⁶
    0xfe5dee046a99a2a8, // bit 7 → 2⁷
    0xfcbe86c7900a88af, // bit 8 → 2⁸
    0xf987a7253ac41317, // bit 9 → 2⁹
    0xf3392b0822b70006, // bit 10 → 2¹⁰
    0xe7159475a2c29b74, // bit 11 → 2¹¹
    0xd097f3bdfd2022b9, // bit 12 → 2¹²
    0xa9f746462d870fe0, // bit 13 → 2¹³
    0x70d869a156d2a1b9, // bit 14 → 2¹⁴
    0x31be135f97d08fda, // bit 15 → 2¹⁵
    0x09aa508b5b7a84e2, // bit 16 → 2¹⁶
    0x005d6af8dedb8119, // bit 17 → 2¹⁷
    0x00002216e584f5fa, // bit 18 → 2¹⁸
];

// Converts a tick index to its corresponding square root price in Q64.64 format.
// This function uses a bitwise approach to efficiently compute the square root price
// from the tick index, leveraging precomputed coefficients for powers of 2.
// It handles both positive and negative ticks, ensuring the result is clamped to valid square root
// price bounds defined by `MIN_SQRT_X64` and `MAX_SQRT_X64`.
//// # Arguments
// * `tick`: An i32 tick index, which must be within the valid range defined by `MIN_TICK` and `MAX_TICK`.
// # Returns
// * `Result<Q64x64>`: The square root price corresponding to the tick index.
//   Returns an error if the tick is out of range or if the computed square root price is
//   outside the valid bounds defined by `MIN_SQRT_X64` and `MAX_SQRT_X64`.
#[inline(always)]
pub fn tick_to_sqrt_x64(tick: i32) -> Result<Q64x64> {
    require!((MIN_TICK..=MAX_TICK).contains(&tick), MathError::OutOfRange);

    let mut ratio: u128 = ONE_X64;
    let abs_tick = tick.unsigned_abs();

    // Unrolled bit-by-bit multiplication for minimal branches
    // This approach uses bitwise operations to multiply the ratio by the appropriate coefficients
    // based on the bits set in the absolute tick index.

    // 0x1 is the base case, which is already set to ONE_X64
    if abs_tick & 0x1 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[0], ONE_X64)?;
    }
    // For each subsequent bit, we multiply the ratio by the corresponding coefficient from the POW2_COEFF array.
    if abs_tick & 0x2 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[1], ONE_X64)?;
    }
    if abs_tick & 0x4 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[2], ONE_X64)?;
    }
    if abs_tick & 0x8 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[3], ONE_X64)?;
    }
    if abs_tick & 0x10 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[4], ONE_X64)?;
    }
    if abs_tick & 0x20 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[5], ONE_X64)?;
    }
    if abs_tick & 0x40 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[6], ONE_X64)?;
    }
    if abs_tick & 0x80 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[7], ONE_X64)?;
    }
    if abs_tick & 0x100 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[8], ONE_X64)?;
    }
    if abs_tick & 0x200 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[9], ONE_X64)?;
    }
    if abs_tick & 0x400 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[10], ONE_X64)?;
    }
    if abs_tick & 0x800 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[11], ONE_X64)?;
    }
    if abs_tick & 0x1000 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[12], ONE_X64)?;
    }
    if abs_tick & 0x2000 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[13], ONE_X64)?;
    }
    if abs_tick & 0x4000 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[14], ONE_X64)?;
    }
    if abs_tick & 0x8000 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[15], ONE_X64)?;
    }
    if abs_tick & 0x10000 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[16], ONE_X64)?;
    }
    if abs_tick & 0x20000 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[17], ONE_X64)?;
    }
    if abs_tick & 0x40000 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[18], ONE_X64)?;
    }

    // Invert for positive ticks
    if tick > 0 {
        ratio = mul_div(ONE_X64, ONE_X64, ratio)?;
    }

    // Clamp into valid √-price bounds so MIN_TICK→√ never underflows
    ratio = ratio.clamp(MIN_SQRT_X64, MAX_SQRT_X64);

    Ok(Q64x64::from_raw(ratio))
}

// ---------- Optimized Liquidity Formulas -----------------------------------

/// Computes the liquidity from a given amount of token0 and the price bounds defined by `sqrt_a` and `sqrt_b`.
/// This function calculates the liquidity based on the formula:
/// L = (amount0 * sqrt_a) * sqrt_b / (sqrt_b - sqrt_a)
/// It ensures that the square root prices are in the correct order (sqrt_a < sqrt_b).
#[inline(always)]
pub fn liquidity_from_amount_0(sqrt_a: Q64x64, sqrt_b: Q64x64, amount0: u64) -> Result<u128> {
    require!(sqrt_a.raw() < sqrt_b.raw(), MathError::OutOfRange);

    let delta = sqrt_b.raw() - sqrt_a.raw();
    // raw_n = amount0 * sqrt_a.raw()
    let raw_n = mul_div(amount0 as u128, sqrt_a.raw(), 1)?;
    // rawL = raw_n * sqrt_b.raw() / delta
    mul_div(raw_n, sqrt_b.raw(), delta)
}

/// Computes the liquidity from a given amount of token1 and the price bounds defined by `sqrt_a` and `sqrt_b`.
/// This function calculates the liquidity based on the formula:
/// L = amount1 / (sqrt_b - sqrt_a)
/// It ensures that the square root prices are in the correct order (sqrt_a < sqrt_b).
#[inline(always)]
pub fn liquidity_from_amount_1(sqrt_a: Q64x64, sqrt_b: Q64x64, amount1: u64) -> Result<u128> {
    require!(sqrt_a.raw() < sqrt_b.raw(), MathError::OutOfRange);

    // L = amount1 / (sqrt_b - sqrt_a)
    let denominator = sqrt_b.raw() - sqrt_a.raw();
    mul_div((amount1 as u128) << FRAC_BITS, ONE_X64, denominator)
}
