use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("The provided tick range is invalid")]
    InvalidTickRange,
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
    #[msg("Insufficient liquidity available")]
    InsufficientLiquidity,
    #[msg("Invalid tick spacing")]
    InvalidTickSpacing,
    #[msg("Price limit reached")]
    PriceLimitReached,
    #[msg("Insufficient input amount")]
    InsufficientInputAmount,
    #[msg("Calculation resulted in zero output")]
    ZeroOutputAmount,
    #[msg("Position does not have enough liquidity")]
    PositionLiquidityTooLow,
    #[msg("Price must be within specified range")]
    PriceOutOfRange,
    #[msg("Operation would result in math overflow")]
    MathOverflow,
}