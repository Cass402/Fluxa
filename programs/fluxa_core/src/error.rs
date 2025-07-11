use anchor_lang::prelude::*;

// The MathError enum defines various error codes that can occur during mathematical operations in the Fluxa protocol.
#[error_code]
pub enum MathError {
    // Error Overflow occurs when a mathematical operation exceeds the maximum limit of the data type.
    #[msg("overflow")]
    Overflow,
    // Error Underflow occurs when a mathematical operation results in a value that is too small to represent.
    #[msg("underflow")]
    Underflow,
    // Error DivideByZero occurs when there is an attempt to divide a number by zero.
    #[msg("division by zero")]
    DivideByZero,
    // Error OutOfRange occurs when an input value is outside the acceptable range for a given operation.
    #[msg("input out of bounds")]
    OutOfRange,
    // Error SqrtNoConverge occurs when the square root calculation does not converge to a solution.
    #[msg("sqrt did not converge")]
    SqrtNoConverge,
    // Error InvalidPriceRange occurs when the provided price range is not valid for the operation being performed.
    #[msg("Invalid Price Range")]
    InvalidPriceRange,
    // Error InvalidInput occurs when the input provided to a function is not valid or does not meet the expected criteria.
    #[msg("Invalid input")]
    InvalidInput,
    // Error ExcessiveTokenAmount occurs when the amount of tokens involved in a transaction exceeds the allowed limit.
    #[msg("Excessive Token Amount")]
    ExcessiveTokenAmount,
    // Error InvalidLiquidity occurs when the liquidity provided is not valid for the operation being performed.
    #[msg("Invalid Liquidity")]
    InvalidLiquidity,
    // Error InvalidSqrtPrice occurs when the provided square root price is not valid for the operation being performed.
    #[msg("Invalid Sqrt Price")]
    InvalidSqrtPrice,
    // Error InvalidPrice occurs when the provided price is not valid for the operation being performed.
    #[msg("Invalid Price")]
    InvalidPrice,
}

#[error_code]
pub enum PdaSecurityAuthorityError {
    // Error InvalidBumpSeed occurs when the bump seed used in PDA derivation is invalid.
    #[msg("Invalid PDA bump seed")]
    InvalidBumpSeed,
    #[msg("Authority change already in progress")]
    AuthorityChangeInProgress,
    #[msg("Unauthorized authority change")]
    Unauthorized,
    #[msg("No pending authority change")]
    NoAuthorityChangeRequested,
    #[msg("Invalid signature threshold for multisig")]
    InvalidSignatureThreshold,
    #[msg("Insufficient multisig signatures")]
    InsufficientSignatures,
    #[msg("Emergency contact limit reached")]
    EmergencyContactLimitReached,
    #[msg("Emergency contact already exists")]
    EmergencyContactAlreadyExists,
    #[msg("Audit trail verification failed")]
    AuditTrailVerificationFailed,
    #[msg("Not a Multisig member")]
    NotAMultisigMember,
    #[msg("Insufficient permissions for the operation")]
    InsufficientPermissions,
    #[msg("Invalid execution delay")]
    InvalidExecutionDelay,
    #[msg("Invalid instruction data")]
    InvalidInstructionData,
    #[msg("Timelock operation not ready")]
    TimelockNotReady,
    #[msg("Timelock confirmation limit reached")]
    TimelockConfirmationLimitReached,
    #[msg("Timelock operation expired")]
    TimelockOperationExpired,
}
