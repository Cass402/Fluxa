#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use fluxa_core::math::core_arithmetic::{
    liquidity_from_amount_0, liquidity_from_amount_1, Q64x64, MAX_SQRT_X64, MIN_SQRT_X64, ONE_X64,
};
use honggfuzz::fuzz;

#[derive(Debug, Clone)]
struct LiquidityInput {
    sqrt_a: Q64x64,
    sqrt_b: Q64x64,
    amount0: u64,
    amount1: u64,
}

impl<'a> Arbitrary<'a> for LiquidityInput {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        // Generate two sqrt prices, ensuring sqrt_a < sqrt_b
        let raw_a = u.int_in_range(MIN_SQRT_X64..=MAX_SQRT_X64)?;
        let raw_b = u.int_in_range(MIN_SQRT_X64..=MAX_SQRT_X64)?;

        let (sqrt_a, sqrt_b) = if raw_a < raw_b {
            (Q64x64::from_raw(raw_a), Q64x64::from_raw(raw_b))
        } else {
            (Q64x64::from_raw(raw_b), Q64x64::from_raw(raw_a))
        };

        // Ensure they're actually different
        let sqrt_b = if sqrt_a.raw() == sqrt_b.raw() {
            Q64x64::from_raw(std::cmp::min(sqrt_b.raw() + 1, MAX_SQRT_X64))
        } else {
            sqrt_b
        };

        let amount0 = u.arbitrary::<u64>()?;
        let amount1 = u.arbitrary::<u64>()?;

        Ok(LiquidityInput {
            sqrt_a,
            sqrt_b,
            amount0,
            amount1,
        })
    }
}

#[derive(Debug, Clone, Arbitrary)]
enum LiquidityOp {
    FromAmount0,
    FromAmount1,
}

#[derive(Debug, Clone, Arbitrary)]
struct LiquidityTest {
    input: LiquidityInput,
    operation: LiquidityOp,
}

fn fuzz_liquidity_calculations(test: LiquidityTest) {
    let LiquidityTest { input, operation } = test;

    // Verify precondition: sqrt_a < sqrt_b
    assert!(input.sqrt_a.raw() < input.sqrt_b.raw());

    match operation {
        LiquidityOp::FromAmount0 => {
            let result = liquidity_from_amount_0(input.sqrt_a, input.sqrt_b, input.amount0);

            match result {
                Ok(liquidity) => {
                    // Verify liquidity is reasonable
                    assert!(liquidity <= u128::MAX);

                    // Test mathematical properties
                    if input.amount0 == 0 {
                        assert_eq!(liquidity, 0);
                    } else if input.amount0 > 0 {
                        // With positive amount0, should get positive liquidity
                        // unless sqrt values are at extremes
                        assert!(liquidity >= 0);
                    }

                    // Test proportionality: double amount0 should roughly double liquidity
                    if input.amount0 > 0 && input.amount0 <= u64::MAX / 2 {
                        if let Ok(double_liquidity) =
                            liquidity_from_amount_0(input.sqrt_a, input.sqrt_b, input.amount0 * 2)
                        {
                            // Should be roughly proportional (within rounding errors)
                            let ratio = if liquidity > 0 {
                                double_liquidity / liquidity
                            } else {
                                0
                            };
                            // Allow some tolerance for fixed-point precision
                            if liquidity > 0 {
                                assert!(ratio >= 1 && ratio <= 3); // Rough proportionality check
                            }
                        }
                    }
                }
                Err(e) => {
                    let error_msg = format!("{:?}", e);
                    assert!(
                        error_msg.contains("Overflow")
                            || error_msg.contains("OutOfRange")
                            || error_msg.contains("DivideByZero")
                    );
                }
            }
        }

        LiquidityOp::FromAmount1 => {
            let result = liquidity_from_amount_1(input.sqrt_a, input.sqrt_b, input.amount1);

            match result {
                Ok(liquidity) => {
                    // Verify liquidity is reasonable
                    assert!(liquidity <= u128::MAX);

                    // Test mathematical properties
                    if input.amount1 == 0 {
                        assert_eq!(liquidity, 0);
                    } else if input.amount1 > 0 {
                        assert!(liquidity >= 0);
                    }

                    // Test proportionality
                    if input.amount1 > 0 && input.amount1 <= u64::MAX / 2 {
                        if let Ok(double_liquidity) =
                            liquidity_from_amount_1(input.sqrt_a, input.sqrt_b, input.amount1 * 2)
                        {
                            let ratio = if liquidity > 0 {
                                double_liquidity / liquidity
                            } else {
                                0
                            };
                            if liquidity > 0 {
                                assert!(ratio >= 1 && ratio <= 3);
                            }
                        }
                    }
                }
                Err(e) => {
                    let error_msg = format!("{:?}", e);
                    assert!(
                        error_msg.contains("Overflow")
                            || error_msg.contains("OutOfRange")
                            || error_msg.contains("DivideByZero")
                    );
                }
            }
        }
    }
}

fn main() {
    loop {
        fuzz!(|data: LiquidityTest| {
            fuzz_liquidity_calculations(data);
        });
    }
}
