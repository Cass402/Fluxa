use anchor_lang::prelude::*;

/// Math errors for all core arithmetic and protocol math operations.
///
/// # Why this enum?
/// - Each error is mapped to a specific failure mode, enabling precise debugging and safe error handling.
/// - Error codes are kept granular to allow for targeted mitigation and protocol upgrades.
/// - All messages are concise for on-chain log efficiency, but descriptive enough for auditors and integrators.
#[error_code]
pub enum MathError {
    /// Math operation exceeded the maximum representable value.
    /// Why: Prevents silent overflows that could corrupt protocol state or allow exploits.
    #[msg("overflow")]
    Overflow,
    /// Math operation produced a value too small to represent.
    /// Why: Prevents silent underflows that could break invariants or allow exploits.
    #[msg("underflow")]
    Underflow,
    /// Division by zero attempted.
    /// Why: Division by zero is undefined and would panic; must be caught for protocol safety.
    #[msg("division by zero")]
    DivideByZero,
    /// Input value was outside the allowed range.
    /// Why: Ensures all math operations are performed on valid, protocol-safe values.
    #[msg("input out of bounds")]
    OutOfRange,
    /// Square root calculation did not converge.
    /// Why: Prevents infinite loops or incorrect results in iterative math routines.
    #[msg("sqrt did not converge")]
    SqrtNoConverge,
    /// Provided price range is invalid for the operation.
    /// Why: Ensures all price math is performed within protocol-allowed bounds.
    #[msg("Invalid Price Range")]
    InvalidPriceRange,
    /// Input did not meet protocol requirements.
    /// Why: Catches generic invalid input to prevent undefined behavior.
    #[msg("Invalid input")]
    InvalidInput,
    /// Token amount exceeds protocol or type limits.
    /// Why: Prevents overflow, DoS, or draining attacks via excessive token values.
    #[msg("Excessive Token Amount")]
    ExcessiveTokenAmount,
    /// Provided liquidity is not valid for the operation.
    /// Why: Ensures all liquidity math is performed on valid, protocol-safe values.
    #[msg("Invalid Liquidity")]
    InvalidLiquidity,
    /// Provided sqrt price is not valid for the operation.
    /// Why: Ensures all price math is performed on valid, protocol-safe values.
    #[msg("Invalid Sqrt Price")]
    InvalidSqrtPrice,
    /// Provided price is not valid for the operation.
    /// Why: Ensures all price math is performed on valid, protocol-safe values.
    #[msg("Invalid Price")]
    InvalidPrice,
}

/// Errors for PDA security authority, multisig, and timelock operations.
///
/// # Why this enum?
/// - Each error is mapped to a specific failure mode in authority management, enabling robust governance and upgrade safety.
/// - Error codes are granular to support advanced multisig, timelock, and emergency response logic.
#[error_code]
pub enum PdaSecurityAuthorityError {
    #[msg("Invalid PDA bump seed")]
    InvalidBumpSeed,
    /// Authority change already in progress; prevents overlapping or replayed changes.
    #[msg("Authority change already in progress")]
    AuthorityChangeInProgress,
    /// Unauthorized attempt to change authority; ensures only valid signers can initiate changes.
    #[msg("Unauthorized authority change")]
    Unauthorized,
    /// No pending authority change; prevents accidental or replayed confirmations.
    #[msg("No pending authority change")]
    NoAuthorityChangeRequested,
    /// Invalid signature threshold for multisig; ensures protocol cannot be bricked by misconfiguration.
    #[msg("Invalid signature threshold for multisig")]
    InvalidSignatureThreshold,
    /// Not enough multisig signatures; prevents unauthorized or rushed changes.
    #[msg("Insufficient multisig signatures")]
    InsufficientSignatures,
    /// Emergency contact list is full; prevents DoS or accidental overwrites.
    #[msg("Emergency contact limit reached")]
    EmergencyContactLimitReached,
    /// Emergency contact already exists; prevents duplicate entries.
    #[msg("Emergency contact already exists")]
    EmergencyContactAlreadyExists,
    /// Audit trail verification failed; ensures all authority changes are tracked and verifiable.
    #[msg("Audit trail verification failed")]
    AuditTrailVerificationFailed,
    /// Not a multisig member; prevents unauthorized access to critical operations.
    #[msg("Not a Multisig member")]
    NotAMultisigMember,
    /// Insufficient permissions for the operation; generic catch-all for access control.
    #[msg("Insufficient permissions for the operation")]
    InsufficientPermissions,
    /// Invalid execution delay for timelock; prevents accidental or malicious misconfiguration.
    #[msg("Invalid execution delay")]
    InvalidExecutionDelay,
    /// Invalid instruction data; ensures only valid instructions are processed.
    #[msg("Invalid instruction data")]
    InvalidInstructionData,
    /// Timelock operation not ready; prevents premature execution.
    #[msg("Timelock operation not ready")]
    TimelockNotReady,
    /// Timelock confirmation limit reached; prevents spam or DoS.
    #[msg("Timelock confirmation limit reached")]
    TimelockConfirmationLimitReached,
    /// Timelock operation expired; ensures stale operations cannot be executed.
    #[msg("Timelock operation expired")]
    TimelockOperationExpired,
}

/// Errors for factory-level operations, sharding, and protocol configuration.
///
/// # Why this enum?
/// - Each error is mapped to a specific failure mode in factory management, enabling robust protocol safety and upgradeability.
/// - Error codes are granular to support advanced sharding, fee, and authority logic.
#[error_code]
pub enum FactoryError {
    /// Fee tier is not supported or out of range.
    /// Why: Prevents protocol bricking or user confusion from unsupported fee tiers.
    #[msg("Invalid fee tier specified")]
    InvalidFeeTier,
    /// Shard index is out of range or not valid.
    /// Why: Prevents accidental or malicious creation of invalid shards.
    #[msg("Invalid shard index")]
    InvalidShardIndex,
    /// Factory is paused (maintenance or emergency).
    /// Why: Prevents protocol operations when paused for upgrades or emergencies.
    #[msg("Factory is paused")]
    FactoryPaused,
    /// Caller does not have required permissions.
    /// Why: Prevents unauthorized access to critical protocol operations.
    #[msg("Insufficient Permissions")]
    InsufficientPermissions,
    /// Provided authority is not valid for this operation.
    /// Why: Ensures only the correct authority can perform sensitive actions.
    #[msg("Invalid authority provided")]
    InvalidAuthority,
    /// Shard has reached its maximum pool capacity.
    /// Why: Prevents overflows and ensures safe, predictable sharding.
    #[msg("Shard is at maximum capacity")]
    ShardAtCapacity,
}

/// Errors for pool-level operations, volatility tracking, and risk controls.
///
/// # Why this enum?
/// - Each error is mapped to a specific failure mode in pool management, enabling robust risk controls and protocol safety.
#[error_code]
pub enum PoolError {
    /// Volatility update called too frequently.
    /// Why: Prevents DoS and ensures protocol can rate-limit expensive operations.
    #[msg("Volatility update too frequent")]
    VolatilityUpdateTooFrequent,
}

/// Errors for tick-level operations, rate limiting, and anomaly detection.
///
/// # Why this enum?
/// - Each error is mapped to a specific failure mode in tick management, enabling robust rate limiting, MEV protection, and protocol safety.
/// - Error codes are granular to support advanced anomaly detection and anti-manipulation logic.
#[error_code]
pub enum TickError {
    /// Tick is already initialized.
    /// Why: Prevents accidental or malicious double-initialization of tick state.
    #[msg("Tick already initialized")]
    TickAlreadyInitialized,
    /// Tick index is not aligned to tick spacing.
    /// Why: Ensures all ticks are created at valid, protocol-aligned indices.
    #[msg("Invalid tick alignment")]
    InvalidTickAlignment,
    /// Arithmetic overflow in tick math.
    /// Why: Prevents silent overflows that could corrupt tick state or allow exploits.
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    /// Tick is not initialized.
    /// Why: Prevents access to uninitialized tick state, which could cause undefined behavior.
    #[msg("Tick not initialized")]
    TickNotInitialized,
    /// Tick index is out of range or not valid.
    /// Why: Ensures all tick math is performed on valid, protocol-safe indices.
    #[msg("Invalid tick index")]
    InvalidTickIndex,
    /// Suspicious activity detected at tick.
    /// Why: Enables protocol to halt or investigate potential MEV or manipulation.
    #[msg("Suspicious activity detected")]
    SuspiciousActivityDetected,
    /// Delta value is too large for safe conversion.
    /// Why: Prevents overflows or underflows in tick math.
    #[msg("Delta too large for safe conversion")]
    DeltaTooLarge,
    /// Tick is in emergency pause state.
    /// Why: Prevents protocol operations on ticks that are paused for safety.
    #[msg("Tick in emergency pause")]
    TickInEmergencyPause,
    /// Tick spacing configuration is invalid.
    /// Why: Ensures all tick spacing is protocol-safe and prevents bricking.
    #[msg("Invalid tick spacing configuration")]
    InvalidTickSpacing,
    /// Tick spacing does not match fee tier.
    /// Why: Prevents inconsistent or unsupported tick/fee configurations.
    #[msg("Tick spacing fee tier mismatch")]
    TickSpacingFeeTierMismatch,
    /// Tick spacing exceeds bitmap capacity.
    /// Why: Prevents out-of-bounds access and memory corruption in tick bitmaps.
    #[msg("Tick spacing exceeds bitmap capacity")]
    TickSpacingExceedsCapacity,
    /// Fee tier is not supported for this tick.
    /// Why: Prevents protocol bricking or user confusion from unsupported fee tiers.
    #[msg("Unsupported fee tier")]
    UnsupportedFeeTier,
    /// Tick crossing rate exceeds protocol limits.
    /// Why: Enables rate limiting and DoS protection at the tick level.
    #[msg("Excessive tick crossing rate detected")]
    ExcessiveTickCrossing,
    /// Rapid tick manipulation detected.
    /// Why: Enables protocol to halt or investigate high-frequency manipulation attempts.
    #[msg("Rapid tick manipulation detected")]
    RapidTickManipulation,
    /// Suspicious tick activity detected.
    /// Why: Enables protocol to halt or investigate potential MEV or manipulation at the tick level.
    #[msg("Suspicious tick activity detected")]
    SuspiciousTickActivity,
    /// Liquidity overflow in tick calculation.
    /// Why: Prevents silent overflows that could corrupt tick or pool state.
    #[msg("Liquidity overflow in calculation")]
    LiquidityOverflow,
    /// Price manipulation threshold exceeded.
    /// Why: Enables protocol to halt or investigate price manipulation attempts.
    #[msg("Price manipulation threshold exceeded")]
    PriceManipulationDetected,
    /// Volume spike anomaly detected.
    /// Why: Enables protocol to halt or investigate sudden, suspicious volume changes.
    #[msg("Volume spike anomaly detected")]
    VolumeSpikeDetected,
    /// Liquidity underflow in tick calculation.
    /// Why: Prevents silent underflows that could corrupt tick or pool state.
    #[msg("Liquidity Underflow in calculation")]
    LiquidityUnderflow,
    #[msg("Storage capacity exceeded")]
    StorageCapacityExceeded,
    #[msg("Excessive compression loss")]
    ExcessiveCompressionLoss,
    #[msg("Tick not found")]
    TickNotFound,
    #[msg("Tick index out of valid range")]
    TickIndexOutOfRange,
    #[msg("Page not found")]
    PageNotFound,
    #[msg("Invalid page index")]
    InvalidPageIndex,
    #[msg("Slot overflow detected")]
    SlotOverflow,
    #[msg("Invalid slot value")]
    InvalidSlot,
    #[msg("Compression overflow")]
    CompressionOverflow,
    #[msg("Zero-copy layout mismatch")]
    LayoutMismatch,
    #[msg("Alignment error")]
    AlignmentError,
}

/// Errors for position management, batch operations, and Merkle proof validation.
///
/// # Why this enum?
/// - Each error is mapped to a specific failure mode in position management, enabling robust accounting and batch operations.
/// - Error codes are granular to support advanced Merkle proof, batch, and compression logic.
#[error_code]
pub enum PositionError {
    /// Position nonce is not valid.
    /// Why: Prevents replay or collision attacks in position management.
    #[msg("Invalid position nonce")]
    InvalidPositionNonce,
    /// Position integrity check failed.
    /// Why: Ensures all position state is consistent and tamper-proof.
    #[msg("Position integrity check failed")]
    PositionIntegrityFailure,
    /// Batch size exceeds protocol or type limits.
    /// Why: Prevents DoS or memory exhaustion from oversized batch operations.
    #[msg("Batch size limit exceeded")]
    BatchSizeExceeded,
    /// Position hash is not valid.
    /// Why: Ensures all position hashes are protocol-compliant and tamper-proof.
    #[msg("Invalid position hash")]
    InvalidPositionHash,
    /// Merkle proof verification failed.
    /// Why: Ensures all batch and compressed position operations are cryptographically sound.
    #[msg("Merkle proof verification failed")]
    MerkleProofFailed, // Consolidated: removed duplicate InvalidMerkleProof
    /// Position not found in batch.
    /// Why: Prevents undefined behavior or silent errors in batch operations.
    #[msg("Position not found in batch")]
    PositionNotFound,
    /// Batch hash does not match expected value.
    /// Why: Ensures all batch operations are cryptographically sound and tamper-proof.
    #[msg("Batch hash mismatch")]
    BatchHashMismatch,
    /// Compressed position count exceeds protocol or type limits.
    /// Why: Prevents DoS or memory exhaustion from oversized compressed batches.
    #[msg("Compressed position limit exceeded")]
    CompressedPositionLimitExceeded,
    /// Duplicate position nonce detected.
    /// Why: Prevents replay or collision attacks in position management.
    #[msg("Duplicate position nonce")]
    DuplicatePositionNonce,
}

#[error_code]
pub enum AdvancedAccountOptimizationError {
    #[msg("Account size exceeds 10KB limit")]
    AccountSizeExceedsLimit,
    #[msg("Invalid account size for migration")]
    InvalidAccountSize,
    #[msg("Arithmetic overflow in calculation")]
    ArithmeticOverflow,
    #[msg("Invalid PDA address for migration")]
    InvalidPDAAddress,
    #[msg("Invalid account owner")]
    InvalidAccountOwner,
}
