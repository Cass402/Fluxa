use anchor_lang::prelude::*;

#[error_code]
pub enum MathError {
    #[msg("Arithmetic overflow detected")]
    Overflow = 9000,

    #[msg("Arithmetic underflow detected")]
    Underflow = 9001,

    #[msg("Division by zero attempted")]
    DivisionByZero = 9002,

    #[msg("Input value exceeds safe limits")]
    ValueTooLarge = 9003,

    #[msg("Square root computation failed validation")]
    InvalidSqrtResult = 9004,

    #[msg("Tick value outside valid range [-887272, 887272]")]
    TickOutOfBounds = 9005,

    #[msg("Invalid sqrt price range: lower >= upper")]
    InvalidSqrtPriceRange = 9006,

    #[msg("Computed sqrt price outside valid bounds")]
    InvalidSqrtPriceResult = 9007,

    #[msg("Token amounts cannot be zero")]
    ZeroAmount = 9008,

    #[msg("Precision loss exceeds acceptable threshold")]
    ExcessivePrecisionLoss = 9009,
}
