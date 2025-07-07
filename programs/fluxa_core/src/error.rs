use anchor_lang::prelude::*;

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
