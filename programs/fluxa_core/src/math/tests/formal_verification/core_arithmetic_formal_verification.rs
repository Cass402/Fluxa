//! Formal verification specs for core_arithmetic.rs using Prusti
//! This module contains formal contracts to prove correctness of CLMM math operations

#[cfg(feature = "verification")]
use crate::math::core_arithmetic::*;
#[cfg(feature = "verification")]
use anchor_lang::prelude::*;
#[cfg(feature = "verification")]
use prusti_contracts::*;

// ---------- Mathematical Constants for Verification ------------------------

#[cfg(feature = "verification")]
const SQRT_EPSILON: u128 = 1000; // ~5.4e-17 in Q64.64, negligible for CLMM

#[cfg(feature = "verification")]
const MAX_SAFE_MUL: u128 = 0xFFFFFFFFFFFFFFFF; // 2^64 - 1

// ---------- Helper Predicates -----------------------------------------------

#[cfg(feature = "verification")]
#[pure]
#[ensures(result == (x <= MAX_SAFE))]
pub fn is_safe_value(x: u128) -> bool {
    x <= MAX_SAFE
}

#[cfg(feature = "verification")]
#[pure]
pub fn mul_will_not_overflow(a: u128, b: u128) -> bool {
    // Conservative check: if either operand > sqrt(u128::MAX), multiplication might overflow
    a <= MAX_SAFE_MUL && b <= MAX_SAFE_MUL
}

#[cfg(feature = "verification")]
#[pure]
#[ensures(result == (tick >= MIN_TICK && tick <= MAX_TICK))]
pub fn is_valid_tick(tick: i32) -> bool {
    tick >= MIN_TICK && tick <= MAX_TICK
}

#[cfg(feature = "verification")]
#[pure]
#[ensures(result == (sqrt_price >= MIN_SQRT_X64 && sqrt_price <= MAX_SQRT_X64))]
pub fn is_valid_sqrt_price(sqrt_price: u128) -> bool {
    sqrt_price >= MIN_SQRT_X64 && sqrt_price <= MAX_SQRT_X64
}

// ---------- Q64x64 Operation Specifications --------------------------------

#[cfg(feature = "verification")]
impl Q64x64 {
    #[pure]
    #[ensures(result.raw() == x)]
    pub fn verified_from_raw(x: u128) -> Self {
        Self::from_raw(x)
    }

    #[pure]
    #[requires(x <= u64::MAX)]
    #[requires(x >= 1)] // Ensure result >= ONE_X64
    #[ensures(result.raw() == (x as u128) << FRAC_BITS)]
    #[ensures(result.raw() >= ONE_X64)]
    pub fn verified_from_int(x: u64) -> Self {
        Self::from_int(x)
    }

    #[requires(is_safe_value(self.raw()))]
    #[requires(is_safe_value(rhs.raw()))]
    #[requires(mul_will_not_overflow(self.raw(), rhs.raw()))]
    #[ensures(result.is_ok() ==> is_safe_value(result.unwrap().raw()))]
    pub fn verified_checked_mul(self, rhs: Self) -> Result<Self> {
        self.checked_mul(rhs)
    }

    #[requires(rhs.raw() != 0)]
    #[requires(is_safe_value(self.raw()))]
    #[requires(is_safe_value(rhs.raw()))]
    #[ensures(result.is_ok() ==> is_safe_value(result.unwrap().raw()))]
    pub fn verified_checked_div(self, rhs: Self) -> Result<Self> {
        self.checked_div(rhs)
    }

    #[requires(is_safe_value(self.raw()))]
    #[requires(is_safe_value(rhs.raw()))]
    #[requires(self.raw() <= u128::MAX - rhs.raw())] // Prevent overflow
    #[ensures(result.is_ok() ==> result.unwrap().raw() == self.raw() + rhs.raw())]
    #[ensures(result.is_ok() ==> result.unwrap().raw() >= self.raw())]
    #[ensures(result.is_ok() ==> result.unwrap().raw() >= rhs.raw())]
    pub fn verified_checked_add(self, rhs: Self) -> Result<Self> {
        self.checked_add(rhs)
    }

    #[requires(is_safe_value(self.raw()))]
    #[requires(is_safe_value(rhs.raw()))]
    #[requires(self.raw() >= rhs.raw())] // Prevent underflow
    #[ensures(result.is_ok() ==> result.unwrap().raw() == self.raw() - rhs.raw())]
    #[ensures(result.is_ok() ==> result.unwrap().raw() <= self.raw())]
    pub fn verified_checked_sub(self, rhs: Self) -> Result<Self> {
        self.checked_sub(rhs)
    }
}

// ---------- Core Arithmetic Function Specifications ------------------------

#[cfg(feature = "verification")]
#[requires(c != 0)]
#[requires(mul_will_not_overflow(a, b))]
#[ensures(result.is_ok() ==> result.unwrap() <= a.max(b))] // Sanity bound
pub fn verified_mul_div(a: u128, b: u128, c: u128) -> Result<u128> {
    mul_div(a, b, c)
}

#[cfg(feature = "verification")]
#[requires(c != 0)]
#[requires(mul_will_not_overflow(a, b))]
#[ensures(result.is_ok() ==> {
    // Round up result should be >= floor result
    let floor_result = mul_div(a, b, c);
    floor_result.is_ok() ==> result.unwrap() >= floor_result.unwrap()
})]
#[ensures(result.is_ok() ==> {
    // Round up result should be <= floor result + 1
    let floor_result = mul_div(a, b, c);
    floor_result.is_ok() ==> result.unwrap() <= floor_result.unwrap() + 1
})]
pub fn verified_mul_div_round_up(a: u128, b: u128, c: u128) -> Result<u128> {
    mul_div_round_up(a, b, c)
}

// ---------- Square Root Verification ----------------------------------------

#[cfg(feature = "verification")]
#[requires(is_safe_value(value.raw()))]
#[ensures(result.is_ok() ==> is_valid_sqrt_price(result.unwrap().raw()))]
#[ensures(value.raw() == 0 ==> result.is_ok() && result.unwrap().raw() == 0)]
pub fn verified_sqrt_x64(value: Q64x64) -> Result<Q64x64> {
    sqrt_x64(value)
}

// ---------- Tick to Price Conversion Verification ---------------------------

#[cfg(feature = "verification")]
#[requires(is_valid_tick(tick))]
#[ensures(result.is_ok() ==> is_valid_sqrt_price(result.unwrap().raw()))]
pub fn verified_tick_to_sqrt_x64(tick: i32) -> Result<Q64x64> {
    tick_to_sqrt_x64(tick)
}

// ---------- Liquidity Formula Verification ----------------------------------

#[cfg(feature = "verification")]
#[requires(is_valid_sqrt_price(sqrt_a.raw()))]
#[requires(is_valid_sqrt_price(sqrt_b.raw()))]
#[requires(sqrt_a.raw() < sqrt_b.raw())]
#[requires(amount0 > 0)]
#[ensures(result.is_ok() ==> result.unwrap() > 0)]
#[ensures(result.is_ok() ==> result.unwrap() < u128::MAX)]
pub fn verified_liquidity_from_amount_0(
    sqrt_a: Q64x64,
    sqrt_b: Q64x64,
    amount0: u64,
) -> Result<u128> {
    liquidity_from_amount_0(sqrt_a, sqrt_b, amount0)
}

#[cfg(feature = "verification")]
#[requires(is_valid_sqrt_price(sqrt_a.raw()))]
#[requires(is_valid_sqrt_price(sqrt_b.raw()))]
#[requires(sqrt_a.raw() < sqrt_b.raw())]
#[requires(amount1 > 0)]
#[ensures(result.is_ok() ==> result.unwrap() > 0)]
#[ensures(result.is_ok() ==> result.unwrap() < u128::MAX)]
pub fn verified_liquidity_from_amount_1(
    sqrt_a: Q64x64,
    sqrt_b: Q64x64,
    amount1: u64,
) -> Result<u128> {
    liquidity_from_amount_1(sqrt_a, sqrt_b, amount1)
}

// ---------- Safety Lemmas ---------------------------------------------------

#[cfg(feature = "verification")]
#[pure]
#[ensures(is_valid_sqrt_price(result))]
pub fn clamp_preserves_validity(value: u128) -> u128 {
    value.clamp(MIN_SQRT_X64, MAX_SQRT_X64)
}

#[cfg(feature = "verification")]
#[pure]
#[ensures(result == true)]
pub fn constants_are_valid() -> bool {
    MIN_SQRT_X64 < MAX_SQRT_X64 && MIN_TICK < MAX_TICK && ONE_X64 > 0 && FRAC_BITS == 64
}

// ---------- Test Module -----------------------------------------------------

#[cfg(all(test, feature = "verification"))]
mod tests {
    use super::*;

    #[test]
    fn test_constants_validity() {
        assert!(constants_are_valid());
    }

    #[test]
    fn test_sqrt_on_perfect_squares() {
        let four = Q64x64::from_int(4);
        let sqrt_four = verified_sqrt_x64(four).unwrap();
        let two = Q64x64::from_int(2);

        // sqrt(4) should be very close to 2
        let diff = if sqrt_four.raw() > two.raw() {
            sqrt_four.raw() - two.raw()
        } else {
            two.raw() - sqrt_four.raw()
        };

        assert!(diff < SQRT_EPSILON);
    }

    #[test]
    fn test_mul_div_identity() {
        let a = ONE_X64;
        let b = ONE_X64 * 2;
        let c = ONE_X64;

        let result = verified_mul_div(a, b, c).unwrap();
        assert_eq!(result, b);
    }

    #[test]
    fn test_tick_bounds() {
        let min_sqrt = verified_tick_to_sqrt_x64(MIN_TICK).unwrap();
        let max_sqrt = verified_tick_to_sqrt_x64(MAX_TICK).unwrap();

        assert!(is_valid_sqrt_price(min_sqrt.raw()));
        assert!(is_valid_sqrt_price(max_sqrt.raw()));
    }
}
