/// Fluxa AMM Core Error Definitions
///
/// This module defines a comprehensive set of error codes used throughout the Fluxa AMM,
/// providing clear and specific feedback when operations fail. Each error includes
/// a human-readable message to assist with debugging and client-side error handling.
///
/// These errors are mapped to unique codes by the Anchor framework and are exposed
/// through the program interface to clients.
use anchor_lang::prelude::*;

/// Core error codes for the Fluxa AMM
///
/// These errors represent all possible failure modes in the protocol's
/// core operations, including parameter validations, mathematical constraints,
/// and liquidity conditions.
#[error_code]
pub enum ErrorCode {
    /// Returned when a tick range is out of bounds or improperly formatted
    ///
    /// This error occurs when:
    /// - Position boundaries are outside the MIN_TICK/MAX_TICK range
    /// - Lower tick is greater than or equal to upper tick
    /// - Ticks do not align with the required tick spacing
    #[msg("The provided tick range is invalid")]
    InvalidTickRange,

    /// Returned when a swap would exceed the user's specified slippage tolerance
    ///
    /// This protects users from price movements between transaction submission
    /// and execution, particularly in volatile markets or during network congestion.
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,

    /// Returned when there is not enough liquidity to fulfill a swap request
    ///
    /// This can occur when:
    /// - The pool has too little total liquidity
    /// - No liquidity exists in the current price range
    /// - The swap amount is too large relative to available liquidity
    #[msg("Insufficient liquidity available")]
    InsufficientLiquidity,

    /// Returned when attempting to create a pool with invalid tick spacing
    ///
    /// Each fee tier has a corresponding tick spacing, and this error occurs when
    /// the provided tick spacing doesn't match an allowed value.
    #[msg("Invalid tick spacing")]
    InvalidTickSpacing,

    /// Returned when a swap reaches the specified price limit
    ///
    /// This happens when:
    /// - A swap with a price limit is executed
    /// - The price limit would be reached before consuming the entire input amount
    /// - The swap execution stops at the limit price
    #[msg("Price limit reached")]
    PriceLimitReached,

    /// Returned when the input amount for a swap is insufficient
    ///
    /// This could happen when:
    /// - The input amount is too small to produce any output due to rounding
    /// - The input amount doesn't cover the minimum gas costs for the swap
    #[msg("Insufficient input amount")]
    InsufficientInputAmount,

    /// Returned when a swap calculation results in zero output tokens
    ///
    /// This can occur when:
    /// - The swap amount is too small relative to the price
    /// - Rounding errors would result in zero output
    /// - Fees would consume the entire output
    #[msg("Calculation resulted in zero output")]
    ZeroOutputAmount,

    /// Returned when attempting to remove more liquidity than exists in a position
    ///
    /// This error protects against underflow in liquidity accounting and ensures
    /// position tracking remains consistent.
    #[msg("Position does not have enough liquidity")]
    PositionLiquidityTooLow,

    /// Returned when a price is outside the valid range for the protocol
    ///
    /// This prevents operations with prices that could cause computational issues
    /// or overflow in the protocol's fixed-point arithmetic.
    #[msg("Price must be within specified range")]
    PriceOutOfRange,

    /// Returned when a mathematical operation would result in overflow
    ///
    /// This is a safety mechanism to prevent computational errors in fixed-point math
    /// operations, which could potentially lead to economic vulnerabilities.
    #[msg("Operation would result in math overflow")]
    MathOverflow,

    /// Returned when an unauthorized account attempts to perform a restricted action
    ///
    /// This error occurs when an account other than the authorized one attempts
    /// to modify a position, collect fees, or perform other owner-only operations.
    #[msg("Unauthorized access attempted")]
    UnauthorizedAccess,

    /// Returned when a position references an invalid or mismatched pool
    ///
    /// This error occurs when the pool referenced by a position does not match
    /// the pool provided in the instruction context.
    #[msg("Invalid pool reference")]
    InvalidPool,

    /// Returned when a token vault does not match the expected vault for a pool
    ///
    /// This error prevents manipulation of the protocol by ensuring that only
    /// the correct token vaults can be used in operations with a specific pool.
    #[msg("Invalid token vault")]
    InvalidVault,

    /// Returned when attempting to initialize a pool with both token mints being the same
    ///
    /// A valid pool must have two different token mints. This error prevents creation of
    /// nonsensical pools with the same token on both sides.
    #[msg("Token mints must be different")]
    MintsMustDiffer,

    /// Returned when attempting to initialize a pool with an invalid initial price
    ///
    /// The initial price must be within the valid range for the protocol to ensure
    /// calculations remain within the bounds of the fixed-point math.
    #[msg("Initial price is outside acceptable range")]
    InvalidInitialPrice,

    /// Returned when attempting to set up token vaults with invalid configuration
    ///
    /// Token vaults must be properly initialized and associated with the correct token mint.
    #[msg("Token vault setup failed")]
    VaultSetupFailed,

    /// Returned when attempting to reference a tick that doesn't exist or has no references
    ///
    /// This error prevents operations on uninitialized ticks or attempting to remove
    /// liquidity from a tick that has no references.
    #[msg("Invalid tick reference")]
    InvalidTickReference,
}
