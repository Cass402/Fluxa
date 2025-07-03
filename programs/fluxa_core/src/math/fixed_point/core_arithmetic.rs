use anchor_lang::prelude::*;

use core::ops::Mul;
//use core::ops::{Add, Div, Mul, Sub};

use crate::error::MathError::*;
// use crate::utils::constants::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fixed<const PRECISION: u8> {
    value: u128,
}

impl<const PRECISION: u8> Fixed<PRECISION> {
    #[inline]
    pub fn new(value: u128) -> Result<Self> {
        if value > Self::MAX_SAFE_VALUE {
            return Err(ValueTooLarge.into());
        }
        Ok(Self { value })
    }

    #[inline]
    pub fn from_integer(int_value: u64) -> Result<Self> {
        let shifted = (int_value as u128) << PRECISION;
        Self::new(shifted)
    }

    pub const MAX_SAFE_VALUE: u128 = u128::MAX >> PRECISION;
    pub const ONE: Self = Self {
        value: 1u128 << PRECISION,
    };
    pub const ZERO: Self = Self { value: 0 };

    #[inline]
    pub const fn raw_value(self) -> u128 {
        self.value
    }

    // #[cfg(feature = "std")]
    // pub fn to_f64(self) -> f64 {
    //     self.value as f64 / (1u128 << PRECISION) as f64
    // }
}

impl<const PRECISION: u8> Mul for Fixed<PRECISION> {
    type Output = Result<Self>;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        let a = self.value;
        let b = rhs.value;

        if a > 0 && b > u128::MAX / a {
            return Err(Overflow.into());
        }

        let product = a * b;

        let result = product >> PRECISION;

        if result > Self::MAX_SAFE_VALUE {
            return Err(Overflow.into());
        }

        Ok(Self { value: result })
    }
}
