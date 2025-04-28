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

    /// No fees to collect from the position
    #[msg("No fees available to collect from this position")]
    NoFeesToCollect,

    /// Transfer of tokens failed
    #[msg("Token transfer operation failed")]
    TransferFailed,

    /// Invalid price range provided
    #[msg("Invalid price range: lower price must be less than upper price")]
    InvalidPriceRange,

    /// Invalid price value provided
    #[msg("Invalid price: price must be positive")]
    InvalidPrice,

    /// Invalid preset specified for price range creation
    #[msg("Invalid preset: cannot create price range from Custom preset")]
    InvalidPreset,

    /// Price range is too narrow based on tick spacing
    #[msg("Price range is too narrow for the selected fee tier's tick spacing")]
    RangeTooNarrow,

    /// Returned when a zero token reserve amount is provided where a non-zero amount is required
    ///
    /// This error prevents operations that would fail due to division by zero or other
    /// mathematical issues when calculating with reserve amounts.
    #[msg("Zero reserve amount provided")]
    ZeroReserveAmount,

    /// Oracle not initialized
    #[msg("Oracle not initialized")]
    OracleNotInitialized,

    /// Oracle timestamp invalid
    #[msg("Oracle timestamp invalid")]
    OracleInvalidTimestamp,

    /// Oracle insufficient data
    #[msg("Oracle insufficient data")]
    OracleInsufficientData,

    /// Oracle cardinality too large
    #[msg("Oracle cardinality too large")]
    OracleCardinalityTooLarge,

    /// Invalid input parameters provided
    #[msg("Invalid input parameters provided for operation")]
    InvalidInput,

    /// Invalid sqrt price limit
    #[msg("Invalid sqrt price limit")]
    InvalidSqrtPriceLimit,

    /// Returned when attempting to close a position that still has liquidity
    ///
    /// Positions can only be closed if they have zero liquidity. This error
    /// ensures users must remove all liquidity before closing a position.
    #[msg("Position still has liquidity and cannot be closed")]
    PositionNotEmpty,

    /// Returned when attempting to close a position that has uncollected fees
    ///
    /// Fees must be collected before closing a position to avoid loss of funds.
    #[msg("Position has uncollected fees that must be claimed before closing")]
    PositionFeesNotCollected,

    /// Returned when attempting to adjust position boundaries with invalid parameters
    ///
    /// Position adjustment must follow protocol rules for tick spacing and ranges.
    #[msg("Invalid position boundary adjustment parameters")]
    InvalidPositionAdjustment,

    /// Returned when attempting to add or remove zero liquidity
    ///
    /// Liquidity delta must be non-zero to avoid wasting gas on no-op transactions.
    #[msg("Liquidity delta must be non-zero")]
    ZeroLiquidityDelta,

    /// Returned when observation time delta is too large for compression
    ///
    /// The time between observations exceeds what can be stored in the compressed format.
    #[msg("Observation time delta too large for compression")]
    ObservationTimeDeltaTooLarge,

    /// Returned when observation value delta exceeds compressible range
    ///
    /// The difference between observation values is too large for the compressed format.
    #[msg("Observation delta too large for compression")]
    ObservationDeltaOverflow,

    /// Returned when a counter value decreases instead of increasing
    ///
    /// Cumulative values should only increase over time.
    #[msg("Observation value decreased unexpectedly")]
    ObservationValueDecreased,

    /// Returned when an observation timestamp is too large for compression
    ///
    /// The timestamp exceeds what can be stored in the compressed format.
    #[msg("Observation timestamp too large for compression")]
    ObservationTimestampTooLarge,

    /// Returned when an observation value is too large for compression
    ///
    /// The value exceeds what can be stored in the compressed format.
    #[msg("Observation value too large for compression")]
    ObservationValueTooLarge,

    /// Returned when trying to query an oracle with no observations
    ///
    /// Observations must be written to the oracle before querying.
    #[msg("No observations available in oracle")]
    NoObservations,

    /// Returned when an observation query hits boundary conditions
    ///
    /// The query cannot be satisfied with the available observation data.
    #[msg("Cannot interpolate observations at boundary")]
    ObservationBoundaryError,

    /// Returned when attempting to grow observations to an invalid size
    ///
    /// New observation capacity must be larger than current capacity.
    #[msg("Invalid observation growth parameters")]
    InvalidObservationGrowth,

    /// Returned when attempting to grow observations beyond maximum allowed
    ///
    /// The requested capacity exceeds implementation limits.
    #[msg("Maximum observation capacity exceeded")]
    MaxObservationsExceeded,

    /// Returned when attempting to access an observation index outside the buffer bounds
    ///
    /// The requested observation index is greater than or equal to the cardinality.
    #[msg("Observation index out of bounds")]
    ObservationIndexOutOfBounds,

    /// Returned when attempting to access an observation that is not initialized
    ///
    /// The requested observation slot exists but has not been populated with data.
    #[msg("Observation not initialized")]
    ObservationNotInitialized,

    /// Returned when the binary search for surrounding observations fails
    ///
    /// This likely indicates a problem with the observation buffer structure.
    #[msg("Observation search failed to find valid surrounding observations")]
    ObservationSearch,

    /// Returned when attempting to grow observation cardinality with invalid parameters
    ///
    /// New cardinality must be greater than current cardinality.
    #[msg("Invalid observation cardinality")]
    InvalidObservationCardinality,

    /// Returned when timestamp calculations would overflow
    ///
    /// This prevents issues when working with timestamp differences.
    #[msg("Timestamp calculation would overflow")]
    TimestampOverflow,
}
