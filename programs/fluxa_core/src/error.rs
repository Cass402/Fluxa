use anchor_lang::prelude::*;

// The MathError enum defines various error codes that can occur during mathematical operations in the Fluxa protocol.
#[error_code]
pub enum MathError {
    #[msg("overflow")]
    Overflow,
    #[msg("underflow")]
    Underflow,
    #[msg("division by zero")]
    DivideByZero,
    #[msg("input out of bounds")]
    OutOfRange,
    #[msg("sqrt did not converge")]
    SqrtNoConverge,
    #[msg("Invalid Price Range")]
    InvalidPriceRange,
    #[msg("Invalid input")]
    InvalidInput,
    #[msg("Excessive Token Amount")]
    ExcessiveTokenAmount,
    #[msg("Invalid Liquidity")]
    InvalidLiquidity,
}
