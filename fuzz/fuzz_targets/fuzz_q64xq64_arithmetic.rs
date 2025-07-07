#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use fluxa_core::math::core_arithmetic::{MathError, Q64x64};
use honggfuzz::fuzz;

#[derive(Debug, Clone)]
struct Q64x64Pair {
    a: Q64x64,
    b: Q64x64,
}

impl<'a> Arbitrary<'a> for Q64x64Pair {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let a_raw = u.arbitrary::<u128>()?;
        let b_raw = u.arbitrary::<u128>()?;

        Ok(Q64x64Pair {
            a: Q64x64::from_raw(a_raw),
            b: Q64x64::from_raw(b_raw),
        })
    }
}

#[derive(Debug, Clone, Arbitrary)]
enum ArithmeticOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Arbitrary)]
struct ArithmeticInput {
    pair: Q64x64Pair,
    operation: ArithmeticOp,
}

fn fuzz_arithmetic(input: ArithmeticInput) {
    let ArithmeticInput { pair, operation } = input;

    match operation {
        ArithmeticOp::Add => {
            let result = pair.a.checked_add(pair.b);
            match result {
                Ok(sum) => {
                    // Verify no overflow occurred
                    assert!(sum.raw() >= pair.a.raw() || sum.raw() >= pair.b.raw());
                }
                Err(e) => {
                    // Should only fail on overflow
                    assert!(format!("{:?}", e).contains("Overflow"));
                }
            }
        }

        ArithmeticOp::Sub => {
            let result = pair.a.checked_sub(pair.b);
            match result {
                Ok(diff) => {
                    // Verify subtraction is correct when no underflow
                    assert!(diff.raw() <= pair.a.raw());
                }
                Err(e) => {
                    // Should only fail on underflow (wrapped as overflow in checked_sub)
                    assert!(format!("{:?}", e).contains("Overflow"));
                }
            }
        }

        ArithmeticOp::Mul => {
            let result = pair.a.checked_mul(pair.b);
            match result {
                Ok(_product) => {
                    // If multiplication succeeds, verify no panic occurred
                    // For very small values, product might be smaller than inputs
                }
                Err(e) => {
                    // Should only fail on overflow
                    assert!(format!("{:?}", e).contains("Overflow"));
                }
            }
        }

        ArithmeticOp::Div => {
            let result = pair.a.checked_div(pair.b);
            match result {
                Ok(quotient) => {
                    // Division succeeded, verify b was not zero
                    assert!(pair.b.raw() != 0);
                    // For division, result should be reasonable
                    if pair.a.raw() >= pair.b.raw() && pair.b.raw() > 0 {
                        assert!(quotient.raw() > 0);
                    }
                }
                Err(e) => {
                    let error_msg = format!("{:?}", e);
                    // Should fail only on division by zero or overflow
                    assert!(error_msg.contains("DivideByZero") || error_msg.contains("Overflow"));
                    if error_msg.contains("DivideByZero") {
                        assert_eq!(pair.b.raw(), 0);
                    }
                }
            }
        }
    }
}

fn main() {
    loop {
        fuzz!(|data: ArithmeticInput| {
            fuzz_arithmetic(data);
        });
    }
}
