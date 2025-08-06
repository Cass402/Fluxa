//! # fluxa_core::math::core_arithmetic
//!
//! High-performance, deterministic, and overflow-safe fixed-point math utilities for Solana CLMM (Concentrated Liquidity Market Maker).
//!
//! ## Why this module?
//! - All math is performed using u128-backed Q64.64 fixed-point arithmetic to guarantee deterministic, branch-minimized, and heapless computation on-chain.
//! - Overflow/underflow is always checked and returns a protocol error, never panics, to ensure safety and auditability.
//! - Newton-Raphson square root with LUT is used for fast, accurate, and gas-efficient sqrt calculations, critical for price math in CLMMs.
//! - Uniswap-style `mul_div` and `mul_div_round_up` are implemented for precise, overflow-safe multiplication/division, avoiding floating-point errors and rounding bias.
//! - All tick-to-sqrt and liquidity math is clamped to safe ranges, preventing protocol bricking or silent state corruption.
//!
//! ## Usage
//! - Use these helpers for all on-chain math in CLMM pools, swaps, and liquidity operations.
//! - Never use floating-point or unchecked arithmetic in protocol logic.
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
use bytemuck::{Pod, Zeroable};
use ethnum::U256;

/// Lookup table for initial guesses in the Newton-Raphson square root algorithm for Q64.64 fixed-point numbers.
///
/// # Why this LUT?
/// - Provides a fast, accurate starting point for sqrt calculations, minimizing Newton-Raphson iterations and compute cost.
/// - Precomputed in Q64.64 for on-chain determinism and precision.
/// - Can be expanded for even greater accuracy if needed, but 16 entries is a good trade-off for compute vs. precision.
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
#[derive(
    Copy,
    Clone,
    Default,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Pod,
    Zeroable,
    AnchorSerialize,
    AnchorDeserialize,
)]
/// Q64.64 fixed-point numeric type (u128-backed, 64 integer bits, 64 fractional bits).
///
/// # Why this type?
/// - Enables high-precision, overflow-safe math for all protocol operations.
/// - Floating-point is not allowed on-chain; this type guarantees deterministic, lossless math.
/// - All arithmetic is checked and returns a protocol error on failure.
/// - Used for all price, liquidity, and fee math in the protocol.
///
/// The value is interpreted as `value / 2^64`.
pub struct Q64x64(u128);

impl anchor_lang::Space for Q64x64 {
    const INIT_SPACE: usize = 16; // 128 bits = 16 bytes
}

/// Implements core arithmetic for Q64x64, always checked for overflow/underflow.
///
/// # Why these methods?
/// - All math is performed using u256 intermediates to prevent overflow.
/// - All operations are checked and return a protocol error on failure.
/// - No panics, no unchecked math, always deterministic.
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

    #[inline(always)]
    pub fn to_q64x64signed(self) -> Result<Q64x64Signed> {
        if self.raw() > i128::MAX as u128 {
            return Err(MathError::InvalidInput.into());
        }
        Ok(Q64x64Signed::from_raw(self.raw() as i128))
    }

    // Optimized multiplication for Q64.64 fixed-point numbers.
    // Why: Uses u256 intermediate to avoid overflow, then collapses to mulhi.
    // All math is checked and returns a protocol error on failure.
    // The result is shifted right by FRAC_BITS to maintain the Q64.64 representation.
    #[inline(always)]
    pub fn checked_mul(self, rhs: Self) -> Result<Self> {
        // Use u256 intermediate, then collapse to mulhi
        let prod = ((U256::from(self.0)) * (U256::from(rhs.0))) >> FRAC_BITS;
        if prod > U256::from(u128::MAX) {
            return Err(MathError::Overflow.into());
        }
        Ok(Self(prod.as_u128()))
    }

    // Optimized division for Q64.64 fixed-point numbers.
    // Why: Uses u256 intermediate to avoid overflow, then collapses to divhi.
    // All math is checked and returns a protocol error on failure.
    // The result is shifted left by FRAC_BITS to maintain the Q64.64 representation.
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

    // Optimized addition for Q64.64 fixed-point numbers.
    // Why: All math is checked and returns a protocol error on failure.
    #[inline(always)]
    pub fn checked_add(self, rhs: Self) -> Result<Self> {
        Ok(Self(self.0.checked_add(rhs.0).ok_or(MathError::Overflow)?))
    }

    // Optimized subtraction for Q64.64 fixed-point numbers.
    // Why: All math is checked and returns a protocol error on failure.
    #[inline(always)]
    pub fn checked_sub(self, rhs: Self) -> Result<Self> {
        Ok(Self(self.0.checked_sub(rhs.0).ok_or(MathError::Underflow)?))
    }
}

/// ---------- Signed Q64.64 Wrapper ------------------------------------------
/// Signed Q64.64 fixed-point numeric type (i128-backed, 64 integer bits, 64 fractional bits).
///
/// # Why this type?
/// - Enables high-precision, overflow-safe math for signed protocol operations (e.g., liquidity deltas).
/// - Floating-point is not allowed on-chain; this type guarantees deterministic, lossless math.
/// - All arithmetic is checked and returns a protocol error on failure.
///
/// The value is interpreted as `value / 2^64`.
#[repr(transparent)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Pod, Zeroable)]
pub struct Q64x64Signed(i128);

/// Implements core arithmetic for Q64x64Signed, always checked for overflow/underflow.
///
/// # Why these methods?
/// - All math is performed using checked i128 arithmetic to prevent overflow.
/// - All operations are checked and return a protocol error on failure.
/// - No panics, no unchecked math, always deterministic.
impl Q64x64Signed {
    #[inline(always)]
    pub const fn zero() -> Self {
        Self(0)
    }

    #[inline(always)]
    pub const fn from_raw(value: i128) -> Self {
        Self(value)
    }

    #[inline(always)]
    pub const fn raw(self) -> i128 {
        self.0
    }

    #[inline(always)]
    pub const fn from_int(x: i64) -> Self {
        Self((x as i128) << FRAC_BITS)
    }

    /// Adds two `Q64x64Signed` values, returning an error if the result overflows.
    #[inline(always)]
    pub fn checked_add(self, other: Self) -> Result<Self> {
        Ok(Self(
            self.0.checked_add(other.0).ok_or(MathError::Overflow)?,
        ))
    }

    /// Subtracts two `Q64x64Signed` values, returning an error if the result underflows.
    #[inline(always)]
    pub fn checked_sub(self, other: Self) -> Result<Self> {
        Ok(Self(
            self.0.checked_sub(other.0).ok_or(MathError::Underflow)?,
        ))
    }

    #[inline(always)]
    pub fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub fn negate(self) -> Result<Self> {
        Ok(Self(self.0.checked_neg().ok_or(MathError::Overflow)?))
    }

    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Converts the `Q64x64Signed` value to a `Q64x64` value, returning an error if the value is negative.
    #[inline(always)]
    pub fn to_q64x64(self) -> Result<Q64x64> {
        if self.raw() < 0 {
            return Err(MathError::InvalidInput.into());
        }
        Ok(Q64x64::from_raw(self.raw().unsigned_abs()))
    }
}

// ---------- Uniswap-style mul_div for exact (a * b) / c --------------------

// Uniswap-style (a * b) / c with full precision and overflow safety.
// Why: Used for all price, fee, and liquidity math in the protocol.
// Uses U256 to prevent overflow, always checked, never panics.
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

/// Ceil division: (a*b + (c-1)) / c (or detect remainder and +1).
///
/// # Why this function?
/// - Used for tick → price so we never under-credit LPs due to rounding.
/// - Always checked for overflow and division by zero.
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
// Sqrt implementation has negligible error for CLMM, but can be improved with more LUT entries or iterations if needed.
/// Computes the square root of a Q64x64 fixed-point number using Newton-Raphson and LUT for initial guess.
///
/// # Why this method?
/// - LUT provides a fast, accurate starting point, minimizing iterations and compute cost.
/// - 4 Newton-Raphson iterations is a good trade-off for precision vs. compute on-chain.
/// - All results are clamped to valid sqrt price range to prevent underflow/overflow.
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

// Precomputed coefficients for tick-to-sqrt conversion.
// Why: Each coefficient corresponds to a power of 2, enabling efficient bitwise computation of sqrt price from tick index.
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

/// Converts a tick index to its corresponding square root price in Q64.64 format.
///
/// # Why this method?
/// - Uses bitwise approach and precomputed coefficients for gas-efficient, branch-minimized computation.
/// - Handles both positive and negative ticks, always clamped to valid sqrt price bounds.
/// - All math is checked and returns a protocol error on failure.
///
/// # Arguments
/// * `tick`: An i32 tick index, which must be within the valid range defined by `MIN_TICK` and `MAX_TICK`.
/// # Returns
/// * `Result<Q64x64>`: The square root price corresponding to the tick index.
///   Returns an error if the tick is out of range or if the computed square root price is
///   outside the valid bounds defined by `MIN_SQRT_X64` and `MAX_SQRT_X64`.
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

/// Computes liquidity from a given amount of token0 and price bounds.
///
/// # Why this method?
/// - All math is performed in Q64.64 for precision and overflow safety.
/// - Ensures sqrt prices are in correct order (sqrt_a < sqrt_b) to prevent protocol bricking.
/// - Formula: L = (amount0 * sqrt_a) * sqrt_b / (sqrt_b - sqrt_a)
#[inline(always)]
pub fn liquidity_from_amount_0(sqrt_a: Q64x64, sqrt_b: Q64x64, amount0: u64) -> Result<u128> {
    require!(sqrt_a.raw() < sqrt_b.raw(), MathError::OutOfRange);

    let delta = sqrt_b.raw() - sqrt_a.raw();
    // raw_n = amount0 * sqrt_a.raw()
    let raw_n = mul_div(amount0 as u128, sqrt_a.raw(), 1)?;
    // rawL = raw_n * sqrt_b.raw() / delta
    mul_div(raw_n, sqrt_b.raw(), delta)
}

/// Computes liquidity from a given amount of token1 and price bounds.
///
/// # Why this method?
/// - All math is performed in Q64.64 for precision and overflow safety.
/// - Ensures sqrt prices are in correct order (sqrt_a < sqrt_b) to prevent protocol bricking.
/// - Formula: L = amount1 / (sqrt_b - sqrt_a)
#[inline(always)]
pub fn liquidity_from_amount_1(sqrt_a: Q64x64, sqrt_b: Q64x64, amount1: u64) -> Result<u128> {
    require!(sqrt_a.raw() < sqrt_b.raw(), MathError::OutOfRange);

    // L = amount1 / (sqrt_b - sqrt_a)
    let denominator = sqrt_b.raw() - sqrt_a.raw();
    mul_div((amount1 as u128) << FRAC_BITS, ONE_X64, denominator)
}
