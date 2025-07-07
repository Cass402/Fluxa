#![no_main]

use arbitrary::Arbitrary;
use fluxa_core::math::core_arithmetic::{mul_div, mul_div_round_up, MathError};
use honggfuzz::fuzz;

#[derive(Debug, Clone, Arbitrary)]
struct MulDivInput {
    a: u128,
    b: u128,
    c: u128,
}

#[derive(Debug, Clone, Arbitrary)]
enum MulDivOp {
    Normal,
    RoundUp,
}

#[derive(Debug, Clone, Arbitrary)]
struct MulDivTest {
    input: MulDivInput,
    operation: MulDivOp,
}

fn fuzz_mul_div_operations(test: MulDivTest) {
    let MulDivTest { input, operation } = test;
    let MulDivInput { a, b, c } = input;

    match operation {
        MulDivOp::Normal => {
            let result = mul_div(a, b, c);
            match result {
                Ok(value) => {
                    // Verify c was not zero
                    assert!(c != 0);
                    // Result should be within u128 bounds
                    assert!(value <= u128::MAX);

                    // Test some mathematical properties
                    if a == 0 || b == 0 {
                        assert_eq!(value, 0);
                    }
                    if c == 1 {
                        // (a * b) / 1 = a * b (if no overflow)
                        if let Some(expected) = a.checked_mul(b) {
                            assert_eq!(value, expected);
                        }
                    }
                }
                Err(e) => {
                    let error_msg = format!("{:?}", e);
                    assert!(error_msg.contains("DivideByZero") || error_msg.contains("Overflow"));
                    if error_msg.contains("DivideByZero") {
                        assert_eq!(c, 0);
                    }
                }
            }
        }

        MulDivOp::RoundUp => {
            let result = mul_div_round_up(a, b, c);
            match result {
                Ok(value) => {
                    // Verify c was not zero
                    assert!(c != 0);

                    // Compare with normal mul_div - round_up should be >= normal
                    if let Ok(normal_result) = mul_div(a, b, c) {
                        assert!(value >= normal_result);
                        // Should be at most 1 greater than normal result
                        assert!(value <= normal_result + 1);
                    }

                    // Test edge cases
                    if a == 0 || b == 0 {
                        assert_eq!(value, 0);
                    }
                }
                Err(e) => {
                    let error_msg = format!("{:?}", e);
                    assert!(error_msg.contains("DivideByZero") || error_msg.contains("Overflow"));
                    if error_msg.contains("DivideByZero") {
                        assert_eq!(c, 0);
                    }
                }
            }
        }
    }
}

fn main() {
    loop {
        fuzz!(|data: MulDivTest| {
            fuzz_mul_div_operations(data);
        });
    }
}
