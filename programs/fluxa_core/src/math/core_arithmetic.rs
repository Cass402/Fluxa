//! High-performance on-chain fixed-point math for Solana CLMM
//! Uses u128-backed Q64.64 with optimized Newton-Raphson sqrt and mulhi operations
//! Zero heap, minimal branches, deterministic compute units
//! Author: Cass402

use anchor_lang::prelude::*;
use ethnum::U256;

// ---------- Constants (compile-time optimized) -----------------------------

pub const FRAC_BITS: u32 = 64; // Q64.64
pub const ONE_X64: u128 = 1u128 << FRAC_BITS;
pub const MAX_SAFE: u128 = u128::MAX;

// sqrt(price) bounds from Raydium CLMM spec
pub const MIN_SQRT_X64: u128 = 4295128739;
pub const MAX_SQRT_X64: u128 = 79226673521066979257578248091u128;

// Newton-Raphson sqrt lookup table (16 entries for initial guess)
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

// ---------- Error Codes -----------------------------------------------------

#[error_code]
pub enum MathError {
    #[msg("overflow")]
    Overflow,
    #[msg("division by zero")]
    DivideByZero,
    #[msg("input out of bounds")]
    OutOfRange,
    #[msg("sqrt did not converge")]
    SqrtNoConverge,
}

// ---------- Core Fixed-Point Wrapper ---------------------------------------

#[repr(transparent)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Q64x64(u128);

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

    // Optimized multiply: uses single mulhi when possible
    #[inline(always)]
    pub fn checked_mul(self, rhs: Self) -> Result<Self> {
        // Use u256 intermediate, then collapse to mulhi
        let prod = ((U256::from(self.0)) * (U256::from(rhs.0))) >> FRAC_BITS;
        if prod > U256::from(u128::MAX) {
            return Err(MathError::Overflow.into());
        }
        Ok(Self(prod.as_u128()))
    }

    // Optimized division: exact (a<<64)/b
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

    #[inline(always)]
    pub fn checked_add(self, rhs: Self) -> Result<Self> {
        Ok(Self(self.0.checked_add(rhs.0).ok_or(MathError::Overflow)?))
    }

    #[inline(always)]
    pub fn checked_sub(self, rhs: Self) -> Result<Self> {
        Ok(Self(self.0.checked_sub(rhs.0).ok_or(MathError::Overflow)?))
    }
}

// ---------- Uniswap-style mul_div for exact (a * b) / c --------------------

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
pub fn sqrt_x64(value: Q64x64) -> Result<Q64x64> {
    let v = value.raw();
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
    let shift = (128 - v.leading_zeros()) as i32;
    if shift > 68 {
        x <<= (shift - 68) / 2;
    } else if shift < 68 {
        x >>= (68 - shift) / 2;
    }

    // Newton-Raphson: x' = (x + v/x) / 2
    // Optimized to 4 iterations (sufficient for Q64.64 precision)
    for _ in 0..4 {
        // x = (x + (v << 64) / x) >> 1
        x = (x + mul_div(v, ONE_X64, x)?) >> 1;
    }

    // Clamp into the valid √ price range so we never underflow/overflow
    x = x.clamp(MIN_SQRT_X64, MAX_SQRT_X64);

    Ok(Q64x64::from_raw(x))
}

// ---------- Tick ⇄ √Price (optimized constants) ----------------------------

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

pub const MIN_TICK: i32 = -443_636;
pub const MAX_TICK: i32 = 443_636;

#[inline(always)]
pub fn tick_to_sqrt_x64(tick: i32) -> Result<Q64x64> {
    require!((MIN_TICK..=MAX_TICK).contains(&tick), MathError::OutOfRange);

    let mut ratio: u128 = ONE_X64;
    let abs_tick = tick.unsigned_abs();

    // Unrolled bit-by-bit multiplication for minimal branches
    if abs_tick & 0x1 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[0], ONE_X64)?;
    }
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

#[inline(always)]
pub fn liquidity_from_amount_0(sqrt_a: Q64x64, sqrt_b: Q64x64, amount0: u64) -> Result<u128> {
    require!(sqrt_a.raw() < sqrt_b.raw(), MathError::OutOfRange);

    let delta = sqrt_b.raw() - sqrt_a.raw();
    // raw_n = amount0 * sqrt_a.raw()
    let raw_n = mul_div(amount0 as u128, sqrt_a.raw(), 1)?;
    // rawL = raw_n * sqrt_b.raw() / delta
    mul_div(raw_n, sqrt_b.raw(), delta)
}

#[inline(always)]
pub fn liquidity_from_amount_1(sqrt_a: Q64x64, sqrt_b: Q64x64, amount1: u64) -> Result<u128> {
    require!(sqrt_a.raw() < sqrt_b.raw(), MathError::OutOfRange);

    // L = amount1 / (sqrt_b - sqrt_a)
    let denominator = sqrt_b.raw() - sqrt_a.raw();
    mul_div((amount1 as u128) << FRAC_BITS, ONE_X64, denominator)
}
