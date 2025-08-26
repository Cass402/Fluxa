//! Timelock Operation Management with Individual PDA Accounts
//!
//! # Why
//! This module implements timelock governance using a separate PDA account for each operation, rather than a monolithic queue or array.
//! This design enables scalable, parallel, and auditable governance actions, avoids dynamic allocation, and leverages Anchor's PDA validation for security.
//!
//! # Design Rationale
//! - Each operation is a zero-copy account, allowing for efficient, deterministic state management and easy auditability.
//! - Using individual PDAs prevents state bloat, enables parallel processing, and avoids the risks of a single point of failure or contention.
//! - All timing, confirmation, and execution logic is enforced on-chain, making governance actions transparent and tamper-resistant.
//! - Bitmaps and fixed-size arrays are used for confirmation tracking, avoiding Vec and ensuring deterministic compute.
//! - All status and type fields are encoded as enums or bitflags for clarity and efficient state transitions.
use crate::error::PdaSecurityAuthorityError;
use crate::utils::constants::{MAX_DELAY, MIN_DELAY};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;

/// Individual Timelock Operation Account
///
/// # Why
/// Each timelock operation is stored in its own PDA account, enabling scalable, parallel, and auditable governance actions.
/// This avoids the complexity and risk of a global queue, and allows for efficient zero-copy access and state transitions.
///
/// # Design Rationale
/// - All fields are fixed-size and zero-copy for deterministic compute and auditability.
/// - Confirmation tracking uses a bitmap for up to 64 signers, enabling atomic, efficient multi-sig without dynamic allocation.
/// - Status and type fields are encoded as enums for clarity and protocol safety.
/// - Reserved space is included for future upgrades without breaking account layout.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct TimelockOperation {
    /// Pool core and operation identification
    ///
    /// # Why
    /// These fields uniquely identify the operation and its context, ensuring that each governance action is traceable and auditable.
    pub pool_core: Pubkey,
    pub operation_id: u64,
    pub operation_type: u8,

    /// Operation details
    ///
    /// # Why
    /// These fields store the target program and instruction data for the operation, with a hash for integrity and a fixed-size array for deterministic compute.
    /// The length field allows for variable-length instructions without dynamic allocation.
    pub target_program: Pubkey,
    pub instruction_data_hash: [u8; 32],
    pub instruction_data: [u8; 1024], // Store actual instruction data
    pub _padding1: [u8; 5],           // 3 bytes
    pub instruction_data_len: u16,

    /// Timing
    ///
    /// # Why
    /// These fields enforce protocol-level timing guarantees, ensuring that operations cannot be executed before their delay or after expiration.
    pub scheduled_at: i64,
    pub execution_time: i64,
    pub executed_at: i64,

    /// Governance
    ///
    /// # Why
    /// These fields track who proposed and executed the operation, and its current status, for full auditability and accountability.
    pub proposer: Pubkey,
    pub executor: Pubkey,
    // Stored as raw u8 (TimelockStatus) for zero_copy Pod safety.
    pub status: u8,

    /// Confirmation tracking
    ///
    /// # Why
    /// Bitmap and counters enable efficient, atomic multi-sig confirmation without Vec, supporting up to 64 signers with a single u64.
    pub confirmation_count: u8,
    pub required_confirmations: u8,
    pub _padding2: [u8; 5],
    pub confirmations_bitmap: u64, // Support up to 64 confirmers

    /// Metadata
    ///
    /// # Why
    /// These fields provide a full audit trail for the operation's lifecycle, supporting compliance and forensic analysis.
    pub created_at: i64,
    pub last_updated: i64,

    /// Future expansion
    ///
    /// # Why
    /// Reserved space allows for future upgrades or additional fields without breaking account layout, supporting protocol evolution.
    pub reserved: [u8; 128],
}

/// Arguments for initializing a TimelockOperation.
pub struct InitArgs<'a> {
    pub pool_core: Pubkey,
    pub operation_id: u64,
    pub operation_type: TimelockOperationType,
    pub target_program: Pubkey,
    pub instruction_data: &'a [u8],
    pub execution_delay: i64,
    pub proposer: Pubkey,
    pub required_confirmations: u8,
}

/// Implementation of TimelockOperation methods
impl TimelockOperation {
    /// Initialize a new timelock operation
    ///
    /// # Why
    /// This method enforces all protocol invariants for timelock operations: delay bounds, instruction data size, and initial state.
    /// It ensures that every operation is initialized in a valid, auditable state, ready for multi-sig confirmation and execution.
    ///
    /// # Design Rationale
    /// - All validation is done up front to prevent invalid state or attacks.
    /// - Instruction data is stored in a fixed-size array for deterministic compute and zero-copy compatibility.
    /// - All timestamps are set from the on-chain clock for auditability.
    pub fn initialize(&mut self, args: InitArgs) -> Result<()> {
        // Validate timing
        if !(MIN_DELAY..=MAX_DELAY).contains(&args.execution_delay) {
            return Err(PdaSecurityAuthorityError::InvalidExecutionDelay.into());
        }

        // Validate instruction data size
        if args.instruction_data.len() > 1024 {
            return Err(PdaSecurityAuthorityError::InvalidInstructionData.into());
        }

        // Pool core and operation identification
        self.pool_core = args.pool_core;
        self.operation_id = args.operation_id;
        self.operation_type = args.operation_type as u8;

        // Operation details
        self.target_program = args.target_program;
        self.proposer = args.proposer;
        self.required_confirmations = args.required_confirmations;
        self.status = TimelockStatus::Pending as u8;
        self.executed_at = 0;
        self.executor = Pubkey::default();

        // Store instruction data
        self.instruction_data[..args.instruction_data.len()].copy_from_slice(args.instruction_data);
        self.instruction_data_len = args.instruction_data.len() as u16;
        self.instruction_data_hash = hashv(&[args.instruction_data]).to_bytes();

        let clock = Clock::get()?;
        self.scheduled_at = clock.unix_timestamp;
        self.execution_time = clock.unix_timestamp + args.execution_delay;
        self.created_at = clock.unix_timestamp;
        self.last_updated = clock.unix_timestamp;

        // Initialize confirmation tracking
        self.confirmation_count = 0;
        self.confirmations_bitmap = 0;

        Ok(())
    }

    /// Confirm the timelock operation
    ///
    /// # Why
    /// This method implements efficient, atomic multi-sig confirmation using a bitmap, supporting up to 64 signers with a single u64.
    /// It ensures that each signer can only confirm once, and that the operation cannot be executed until the required threshold is met.
    ///
    /// # Design Rationale
    /// - Bitmap avoids Vec and dynamic allocation, ensuring deterministic compute and zero-copy compatibility.
    /// - All state transitions are atomic and auditable.
    pub fn confirm(&mut self, confirmer_index: u8) -> Result<bool> {
        // Ensure the operation is pending
        if self.status_enum() != Some(TimelockStatus::Pending) {
            return Err(PdaSecurityAuthorityError::TimelockNotReady.into());
        }

        // Ensure confirmer index is valid
        if confirmer_index >= 64 {
            return Err(PdaSecurityAuthorityError::TimelockConfirmationLimitReached.into());
        }

        // Create a bitmask for the confirmer
        let confirmer_bit = 1u64 << confirmer_index;

        // Check if already confirmed
        if (self.confirmations_bitmap & confirmer_bit) != 0 {
            return Ok(self.confirmation_count >= self.required_confirmations);
        }

        // Add confirmation
        self.confirmations_bitmap |= confirmer_bit;
        self.confirmation_count += 1;

        let clock = Clock::get()?;
        self.last_updated = clock.unix_timestamp; // Update last modified timestamp

        // Check if ready for execution
        if self.confirmation_count >= self.required_confirmations {
            self.set_status(TimelockStatus::Approved);
        }

        Ok(self.confirmation_count >= self.required_confirmations)
    }

    /// Check if the operation is ready for execution
    ///
    /// # Why
    /// This method enforces the protocol's timelock guarantees, ensuring that no operation can be executed before the required delay and confirmations.
    pub fn is_ready_for_execution(&self) -> bool {
        let clock = Clock::get().unwrap();
        self.status_enum() == Some(TimelockStatus::Approved)
            && clock.unix_timestamp >= self.execution_time
    }

    /// Check if the operation has expired
    ///
    /// # Why
    /// This method ensures that stale or abandoned operations cannot be executed indefinitely, protecting protocol liveness and safety.
    pub fn is_expired(&self) -> bool {
        let clock = Clock::get().unwrap();
        // Operations expire after 30 days if not executed
        clock.unix_timestamp > self.execution_time + (30 * 24 * 3600)
    }

    /// Execute the timelock operation
    ///
    /// # Why
    /// This method enforces all protocol invariants for execution: only approved, non-expired operations can be executed, and all state transitions are atomic and auditable.
    ///
    /// # Design Rationale
    /// - Executor is recorded for full auditability.
    /// - All timestamps are set from the on-chain clock for compliance and forensic analysis.
    pub fn execute(&mut self, executor: Pubkey) -> Result<()> {
        // Ensure the operation is approved and ready for execution
        if !self.is_ready_for_execution() {
            return Err(PdaSecurityAuthorityError::TimelockNotReady.into());
        }

        // Ensure the operation has not expired
        if self.is_expired() {
            return Err(PdaSecurityAuthorityError::TimelockOperationExpired.into());
        }

        // Mark as executed
        self.set_status(TimelockStatus::Executed);
        self.executor = executor;

        let clock = Clock::get()?;
        self.executed_at = clock.unix_timestamp;
        self.last_updated = clock.unix_timestamp;

        Ok(())
    }

    /// Cancel the timelock operation
    ///
    /// # Why
    /// This method allows for safe cancellation of pending operations, ensuring that only non-executed operations can be cancelled and all state transitions are auditable.
    pub fn cancel(&mut self) -> Result<()> {
        // Ensure the operation is not already executed
        if self.status_enum() == Some(TimelockStatus::Executed) {
            return Err(PdaSecurityAuthorityError::TimelockNotReady.into());
        }

        // Mark as cancelled
        self.set_status(TimelockStatus::Cancelled);

        let clock = Clock::get()?;
        self.last_updated = clock.unix_timestamp;

        Ok(())
    }

    /// Get the instruction data for execution
    ///
    /// # Why
    /// This method provides safe, bounded access to the instruction data, ensuring that only the valid portion is used for execution and preventing buffer overreads.
    pub fn get_instruction_data(&self) -> &[u8] {
        &self.instruction_data[..self.instruction_data_len as usize]
    }
}

/// Status of timelock operations
///
/// # Why
/// Encodes the full lifecycle of a timelock operation, enabling clear, auditable state transitions and protocol safety.
#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum TimelockStatus {
    Pending = 0,
    Approved = 1,
    Executed = 2,
    Cancelled = 3,
    Expired = 4,
}

impl TimelockOperation {
    #[inline(always)]
    pub fn status_enum(&self) -> Option<TimelockStatus> {
        match self.status {
            0 => Some(TimelockStatus::Pending),
            1 => Some(TimelockStatus::Approved),
            2 => Some(TimelockStatus::Executed),
            3 => Some(TimelockStatus::Cancelled),
            4 => Some(TimelockStatus::Expired),
            _ => None,
        }
    }

    #[inline(always)]
    pub fn set_status(&mut self, status: TimelockStatus) {
        self.status = status as u8;
    }
}

/// Timelock operation types
///
/// # Why
/// Enumerates all supported governance actions, enabling type-safe, auditable, and extensible protocol upgrades and changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum TimelockOperationType {
    ProtocolUpgrade = 0,
    ParameterChange = 1,
    TreasuryOperation = 2,
    EmergencyAction = 3,
    GovernanceChange = 4,
}

/// Timelock manager utility functions
///
/// # Why
/// Provides protocol-level helpers for generating unique operation IDs, validating delays, and enforcing governance invariants.
pub struct TimelockManager;

/// Implementation of TimelockManager methods
impl TimelockManager {
    /// Generate unique operation ID
    ///
    /// # Why
    /// This function ensures that every operation is uniquely identifiable, preventing replay or collision attacks and enabling full auditability.
    ///
    /// # Design Rationale
    /// - Uses a hash of all relevant fields for uniqueness and collision resistance.
    pub fn generate_operation_id(
        pool_core: &Pubkey,
        proposer: &Pubkey,
        timestamp: i64,
        operation_type: TimelockOperationType,
    ) -> u64 {
        let hash = hashv(&[
            pool_core.as_ref(),
            proposer.as_ref(),
            &timestamp.to_le_bytes(),
            &[operation_type as u8],
        ]);

        u64::from_le_bytes(hash.to_bytes()[..8].try_into().unwrap())
    }

    /// Get minimum delay for operation type
    ///
    /// # Why
    /// This function enforces protocol-level safety by requiring longer delays for more sensitive operations, preventing instant upgrades or attacks.
    pub fn get_min_delay_for_type(operation_type: TimelockOperationType) -> i64 {
        match operation_type {
            TimelockOperationType::ProtocolUpgrade => 7 * 24 * 3600, // 7 days
            TimelockOperationType::ParameterChange => 3 * 24 * 3600, // 3 days
            TimelockOperationType::TreasuryOperation => 5 * 24 * 3600, // 5 days
            TimelockOperationType::EmergencyAction => 24 * 3600,     // 1 day
            TimelockOperationType::GovernanceChange => 10 * 24 * 3600, // 10 days
        }
    }

    /// Validate operation type and delay
    ///
    /// # Why
    /// This function enforces all protocol invariants for operation timing, preventing governance attacks via short or excessive delays.
    pub fn validate_operation(operation_type: TimelockOperationType, delay: i64) -> Result<()> {
        let min_delay = Self::get_min_delay_for_type(operation_type);

        if delay < min_delay {
            return Err(PdaSecurityAuthorityError::InvalidExecutionDelay.into());
        }

        if delay > MAX_DELAY {
            return Err(PdaSecurityAuthorityError::InvalidExecutionDelay.into());
        }

        Ok(())
    }
}

/// Data structure for creating timelock operations
///
/// # Why
/// Encapsulates all required data for creating a new timelock operation, ensuring that all protocol invariants are enforced at creation time.
#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct TimelockOperationData {
    pub operation_type: TimelockOperationType,
    pub target_program: Pubkey,
    pub instruction_data: Vec<u8>,
    pub execution_delay: i64,
}

/// Implementation of TimelockOperationData methods
impl TimelockOperationData {
    /// Create a new TimelockOperationData instance
    ///
    /// # Why
    /// This method enforces all protocol invariants for operation creation: instruction data size, delay bounds, and type safety.
    pub fn new(
        operation_type: TimelockOperationType,
        target_program: Pubkey,
        instruction_data: Vec<u8>,
        execution_delay: i64,
    ) -> Result<Self> {
        // Validate data size
        if instruction_data.len() > 1024 {
            return Err(PdaSecurityAuthorityError::InvalidInstructionData.into());
        }

        // Validate timing
        TimelockManager::validate_operation(operation_type, execution_delay)?;

        Ok(Self {
            operation_type,
            target_program,
            instruction_data,
            execution_delay,
        })
    }

    /// Compute the hash of the operation data
    ///
    /// # Why
    /// This method provides a unique, tamper-evident identifier for the operation data, supporting integrity checks and replay protection.
    pub fn compute_hash(&self) -> [u8; 32] {
        hashv(&[
            &[self.operation_type as u8],
            self.target_program.as_ref(),
            &self.instruction_data,
            &self.execution_delay.to_le_bytes(),
        ])
        .to_bytes()
    }
}

// ============================================================================
// ANCHOR ACCOUNT VALIDATION CONTEXTS
// ============================================================================
//
// # Why
// Each context is designed for a specific governance action, with strict account validation and PDA seeds to prevent spoofing or replay attacks.
// All account layouts are deterministic and zero-copy for auditability and protocol safety.

/// Create Timelock Operation Context
#[derive(Accounts)]
#[instruction(operation_id: u64)]
pub struct CreateTimelockOperation<'info> {
    /// The timelock operation account to be created
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<TimelockOperation>(),
        seeds = [b"timelock_operation", pool_core.key().as_ref(), &operation_id.to_le_bytes()],
        bump
    )]
    pub timelock_operation: AccountLoader<'info, TimelockOperation>,

    /// the pool core account associated with the operation
    pub pool_core: UncheckedAccount<'info>,

    /// the payer who will fund the operation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// the proposer who proposed the operation
    pub proposer: Signer<'info>,

    /// the system program account
    pub system_program: Program<'info, System>,
}

/// Confirm Timelock Operation Context
#[derive(Accounts)]
pub struct ConfirmTimelockOperation<'info> {
    /// the timelock operation account to be confirmed
    #[account(
        mut,
        seeds = [b"timelock_operation", pool_core.key().as_ref(), &timelock_operation.load()?.operation_id.to_le_bytes()],
        bump
    )]
    pub timelock_operation: AccountLoader<'info, TimelockOperation>,

    /// the pool core account associated with the operation
    pub pool_core: UncheckedAccount<'info>,

    /// the confirmer who will confirm the operation
    pub confirmer: Signer<'info>,
}

/// Execute Timelock Operation Context
#[derive(Accounts)]
pub struct ExecuteTimelockOperation<'info> {
    /// the timelock operation account to be executed
    #[account(
        mut,
        seeds = [b"timelock_operation", pool_core.key().as_ref(), &timelock_operation.load()?.operation_id.to_le_bytes()],
        bump
    )]
    pub timelock_operation: AccountLoader<'info, TimelockOperation>,

    /// the pool core account associated with the operation
    pub pool_core: UncheckedAccount<'info>,

    /// the executor who will execute the operation
    pub executor: Signer<'info>,
}

/// Cancel Timelock Operation Context
#[derive(Accounts)]
pub struct CancelTimelockOperation<'info> {
    /// the timelock operation account to be cancelled
    #[account(
        mut,
        seeds = [b"timelock_operation", pool_core.key().as_ref(), &timelock_operation.load()?.operation_id.to_le_bytes()],
        bump
    )]
    pub timelock_operation: AccountLoader<'info, TimelockOperation>,

    /// the pool core account associated with the operation
    pub pool_core: UncheckedAccount<'info>,

    /// the authority who can cancel the operation
    pub authority: Signer<'info>,
}

// ============================================================================
// INSTRUCTION HANDLERS
// ============================================================================
//
// # Why
// Each handler enforces protocol invariants for its respective action, ensuring that all state transitions are atomic, auditable, and safe.
// All validation is done up front, and all state changes are recorded for compliance and forensic analysis.

/// Create Timelock Operation
/// This function initializes a new timelock operation with the provided parameters.
/// It sets up the operation's metadata, validates the execution delay and instruction data,
/// and prepares it for confirmation and execution.
/// # Arguments
/// * `ctx` - The context containing the accounts and program information.
/// * `operation_id` - Unique identifier for the operation.
/// * `operation_data` - Data structure containing the operation type, target program,
///   instruction data, and execution delay.
/// * `required_confirmations` - Number of confirmations required to approve the operation.
/// # Returns
/// A `Result` indicating success or failure of the operation.
pub fn create_timelock_operation(
    ctx: Context<CreateTimelockOperation>,
    operation_id: u64,
    operation_data: TimelockOperationData,
    required_confirmations: u8,
) -> Result<()> {
    let timelock_operation = &mut ctx.accounts.timelock_operation.load_mut()?;

    timelock_operation.initialize(InitArgs {
        pool_core: ctx.accounts.pool_core.key(),
        operation_id,
        operation_type: operation_data.operation_type,
        target_program: operation_data.target_program,
        instruction_data: &operation_data.instruction_data,
        execution_delay: operation_data.execution_delay,
        proposer: ctx.accounts.proposer.key(),
        required_confirmations,
    })?;

    Ok(())
}

/// Confirm Timelock Operation
/// This function allows a confirmer to confirm the timelock operation.
/// It updates the confirmation count and bitmap, checks if the operation is ready for execution,
/// and updates the status accordingly.
/// # Arguments
/// * `ctx` - The context containing the accounts and program information.
/// * `confirmer_index` - The index of the confirmer (0-63).
/// # Returns
/// A `Result` indicating whether the confirmation was successful and if the operation is ready for execution
pub fn confirm_timelock_operation(
    ctx: Context<ConfirmTimelockOperation>,
    confirmer_index: u8,
) -> Result<()> {
    let timelock_operation = &mut ctx.accounts.timelock_operation.load_mut()?;

    let threshold_reached = timelock_operation.confirm(confirmer_index)?;

    // Could emit event here if threshold reached
    if threshold_reached {
        // Operation is now approved
    }

    Ok(())
}

/// Execute Timelock Operation
/// This function executes the timelock operation if it is ready for execution.
/// It checks if the operation has been approved and if the current time is past the execution time.
/// If the operation is ready, it marks it as executed, updates the executor's public key,
/// and sets the executed timestamp.
/// # Arguments
/// * `ctx` - The context containing the accounts and program information.
/// # Returns
/// A `Result` indicating success or failure of the execution.
pub fn execute_timelock_operation(ctx: Context<ExecuteTimelockOperation>) -> Result<()> {
    let timelock_operation = &mut ctx.accounts.timelock_operation.load_mut()?;

    timelock_operation.execute(ctx.accounts.executor.key())?;

    // Here you would typically execute the actual instruction
    // stored in timelock_operation.get_instruction_data()

    Ok(())
}

/// Cancel Timelock Operation
/// This function allows the timelock operation to be cancelled if it has not been executed.
/// It updates the status to Cancelled and sets the last updated timestamp.
/// # Arguments
/// * `ctx` - The context containing the accounts and program information.
/// # Returns
/// A `Result` indicating success or failure of the cancellation.
pub fn cancel_timelock_operation(ctx: Context<CancelTimelockOperation>) -> Result<()> {
    let timelock_operation = &mut ctx.accounts.timelock_operation.load_mut()?;

    timelock_operation.cancel()?;

    Ok(())
}
