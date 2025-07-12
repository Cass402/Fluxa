//! Timelock Operation Management with Individual PDA Accounts
//!
//! This module provides timelock functionality using separate PDA accounts
//! for each operation, enabling efficient and scalable governance.
//! Uses Anchor's built-in PDA validation for security.
use crate::error::PdaSecurityAuthorityError;
use crate::utils::constants::{MAX_DELAY, MIN_DELAY};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;

/// Individual Timelock Operation Account
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct TimelockOperation {
    /// Account discriminator
    pub discriminator: [u8; 8],

    /// Pool core and operation identification
    /// 'pool_core' - The public key of the pool core associated with this operation.
    /// 'operation_id' - Unique identifier for the operation.
    /// 'operation_type' - Type of the operation (e.g., upgrade, parameter change).
    pub pool_core: Pubkey,
    pub operation_id: u64,
    pub operation_type: u8,

    /// Operation details
    /// 'target_program' - The program to be invoked by this operation.
    /// 'instruction_data_hash' - Hash of the instruction data for integrity checks.
    /// 'instruction_data' - Actual instruction data to be executed.
    /// 'instruction_data_len' - Length of the instruction data.
    pub target_program: Pubkey,
    pub instruction_data_hash: [u8; 32],
    pub instruction_data: [u8; 1024], // Store actual instruction data
    pub instruction_data_len: u16,

    /// Timing
    /// 'scheduled_at' - Timestamp when the operation is scheduled.
    /// 'execution_time' - Timestamp when the operation can be executed.
    /// 'executed_at' - Timestamp when the operation was executed, if applicable.
    pub scheduled_at: i64,
    pub execution_time: i64,
    pub executed_at: i64,

    /// Governance
    /// 'proposer' - The public key of the proposer initiating the operation.
    /// 'executor' - The public key of the executor who will execute the operation.
    /// 'status' - Current status of the operation (Pending, Approved, Executed,
    pub proposer: Pubkey,
    pub executor: Pubkey,
    pub status: TimelockStatus,

    /// Confirmation tracking
    /// 'confirmation_count' - Number of confirmations received for this operation.
    /// 'required_confirmations' - Number of confirmations required to approve the operation.
    /// 'confirmations_bitmap' - Bitmap to track which confirmers have confirmed (up to 64).
    pub confirmation_count: u8,
    pub required_confirmations: u8,
    pub confirmations_bitmap: u64, // Support up to 64 confirmers

    /// Metadata
    /// 'created_at' - Timestamp when the operation was created.
    /// 'last_updated' - Timestamp when the operation was last updated.
    pub created_at: i64,
    pub last_updated: i64,

    /// Future expansion
    pub reserved: [u8; 128],
}

/// Implementation of TimelockOperation methods
impl TimelockOperation {
    /// Initialize a new timelock operation
    /// This method sets up the operation with the provided parameters.
    /// It validates the execution delay and instruction data size, initializes the
    /// operation's metadata, and prepares it for confirmation and execution.
    /// # Arguments
    /// * `pool_core` - The public key of the pool core associated with this operation.
    /// * `operation_id` - Unique identifier for the operation.
    /// * `operation_type` - Type of the operation (e.g., upgrade, parameter change).
    /// * `target_program` - The program to be invoked by this operation.
    /// * `instruction_data` - Actual instruction data to be executed.
    /// * `execution_delay` - Delay in seconds before the operation can be executed.
    /// * `proposer` - The public key of the proposer initiating the operation.
    /// * `required_confirmations` - Number of confirmations required to approve the operation.
    /// # Returns
    /// A `Result` indicating success or failure of the operation.
    pub fn initialize(
        &mut self,
        pool_core: Pubkey,
        operation_id: u64,
        operation_type: TimelockOperationType,
        target_program: Pubkey,
        instruction_data: &[u8],
        execution_delay: i64,
        proposer: Pubkey,
        required_confirmations: u8,
    ) -> Result<()> {
        // Validate timing
        if execution_delay < MIN_DELAY || execution_delay > MAX_DELAY {
            return Err(PdaSecurityAuthorityError::InvalidExecutionDelay.into());
        }

        // Validate instruction data size
        if instruction_data.len() > 1024 {
            return Err(PdaSecurityAuthorityError::InvalidInstructionData.into());
        }

        // Initialize the discriminator
        self.discriminator = Self::discriminator();

        // Pool core and operation identification
        self.pool_core = pool_core;
        self.operation_id = operation_id;
        self.operation_type = operation_type as u8;

        // Operation details
        self.target_program = target_program;
        self.proposer = proposer;
        self.required_confirmations = required_confirmations;
        self.status = TimelockStatus::Pending;
        self.executed_at = 0;
        self.executor = Pubkey::default();

        // Store instruction data
        self.instruction_data[..instruction_data.len()].copy_from_slice(instruction_data);
        self.instruction_data_len = instruction_data.len() as u16;
        self.instruction_data_hash = hashv(&[instruction_data]).to_bytes();

        let clock = Clock::get()?;
        self.scheduled_at = clock.unix_timestamp;
        self.execution_time = clock.unix_timestamp + execution_delay;
        self.created_at = clock.unix_timestamp;
        self.last_updated = clock.unix_timestamp;

        // Initialize confirmation tracking
        self.confirmation_count = 0;
        self.confirmations_bitmap = 0;

        Ok(())
    }

    /// Confirm the timelock operation
    /// This method allows a confirmer to confirm the operation, updating the confirmation count
    /// and bitmap. It checks if the operation is ready for execution based on the required confirmations
    /// and updates the status accordingly.
    /// # Arguments
    /// * `confirmer_index` - The index of the confirmer (0-63)
    /// # Returns
    /// A `Result` indicating whether the confirmation was successful and if the operation is ready for execution.
    /// If the operation is already confirmed by this confirmer, it returns the current confirmation status.
    /// # Errors
    /// * `TimelockNotReady` - If the operation is not in a pending state.
    /// * `TimelockConfirmationLimitReached` - If the confirmer index is out of bounds (0-63).
    pub fn confirm(&mut self, confirmer_index: u8) -> Result<bool> {
        // Ensure the operation is pending
        if self.status != TimelockStatus::Pending {
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
            self.status = TimelockStatus::Approved;
        }

        Ok(self.confirmation_count >= self.required_confirmations)
    }

    /// Check if the operation is ready for execution
    /// This method checks if the operation has been approved and if the current time
    /// is past the execution time. It returns true if the operation can be executed.
    /// # Returns
    /// A boolean indicating whether the operation is ready for execution.
    pub fn is_ready_for_execution(&self) -> bool {
        let clock = Clock::get().unwrap();
        self.status == TimelockStatus::Approved && clock.unix_timestamp >= self.execution_time
    }

    /// Check if the operation has expired
    /// This method checks if the operation has not been executed within 30 days of its execution
    /// time. If it has not been executed and the current time is past the expiration time,
    /// it returns true indicating the operation has expired.
    /// # Returns
    /// A boolean indicating whether the operation has expired.
    pub fn is_expired(&self) -> bool {
        let clock = Clock::get().unwrap();
        // Operations expire after 30 days if not executed
        clock.unix_timestamp > self.execution_time + (30 * 24 * 3600)
    }

    /// Execute the timelock operation
    /// This method marks the operation as executed, updates the executor's public key,
    /// and sets the executed timestamp. It checks if the operation is ready for execution
    /// and if it has not expired. If the operation is not ready or has expired,
    /// it returns an error.
    /// # Arguments
    /// * `executor` - The public key of the executor who will execute the operation.
    /// # Returns
    /// A `Result` indicating success or failure of the execution.
    ///
    /// # Errors
    /// * `TimelockNotReady` - If the operation is not approved or ready for execution.
    /// * `TimelockOperationExpired` - If the operation has expired and
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
        self.status = TimelockStatus::Executed;
        self.executor = executor;

        let clock = Clock::get()?;
        self.executed_at = clock.unix_timestamp;
        self.last_updated = clock.unix_timestamp;

        Ok(())
    }

    /// Cancel the timelock operation
    /// This method allows the operation to be cancelled if it has not been executed.
    /// It updates the status to Cancelled and sets the last updated timestamp.
    /// # Returns
    /// A `Result` indicating success or failure of the cancellation.
    /// If the operation has already been executed, it returns an error.
    /// # Errors
    /// * `TimelockNotReady` - If the operation has already been executed.
    pub fn cancel(&mut self) -> Result<()> {
        // Ensure the operation is not already executed
        if self.status == TimelockStatus::Executed {
            return Err(PdaSecurityAuthorityError::TimelockNotReady.into());
        }

        // Mark as cancelled
        self.status = TimelockStatus::Cancelled;

        let clock = Clock::get()?;
        self.last_updated = clock.unix_timestamp;

        Ok(())
    }

    /// Get the instruction data for execution
    /// This method returns a slice of the instruction data stored in the operation.
    /// It ensures that the data length is within the defined limits.
    /// # Returns
    /// A slice of the instruction data.
    pub fn get_instruction_data(&self) -> &[u8] {
        &self.instruction_data[..self.instruction_data_len as usize]
    }

    /// Get the discriminator for this account type
    fn discriminator() -> [u8; 8] {
        [0x15, 0x25, 0x35, 0x45, 0x55, 0x65, 0x75, 0x85]
    }
}

/// Status of timelock operations
/// This enum represents the various states a timelock operation can be in,
/// such as Pending, Approved, Executed, Cancelled, or Expired.
#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum TimelockStatus {
    Pending = 0,
    Approved = 1,
    Executed = 2,
    Cancelled = 3,
    Expired = 4,
}

/// Timelock operation types
/// This enum defines the different types of operations that can be performed
/// within the timelock system, such as protocol upgrades, parameter changes,
/// treasury operations, emergency actions, and governance changes.
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
/// This struct provides utility functions for managing timelock operations,
/// such as generating unique operation IDs, validating operation types and delays,
/// and creating new timelock operation data.
pub struct TimelockManager;

/// Implementation of TimelockManager methods
impl TimelockManager {
    /// Generate unique operation ID
    /// This function generates a unique operation ID based on the pool core,
    /// proposer, timestamp, and operation type. It uses a hash function to create
    /// a unique identifier that can be used to track the operation.
    /// # Arguments
    /// * `pool_core` - The public key of the pool core associated with the operation.
    /// * `proposer` - The public key of the proposer initiating the operation.
    /// * `timestamp` - The Unix timestamp when the operation is created.
    /// * `operation_type` - The type of the operation being performed.
    /// # Returns
    /// A unique operation ID as a `u64`.
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
    /// This function returns the minimum delay required for a specific
    /// type of timelock operation. It ensures that operations have appropriate
    /// delays based on their type to prevent immediate execution.
    /// # Arguments
    /// * `operation_type` - The type of the operation for which to get the minimum delay.
    /// # Returns
    /// The minimum delay in seconds for the specified operation type.
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
    /// This function checks if the provided operation type and delay
    /// are valid. It ensures that the delay meets the minimum requirements
    /// for the specified operation type and does not exceed the maximum allowed delay.
    /// # Arguments
    /// * `operation_type` - The type of the operation to validate.
    /// * `delay` - The delay in seconds for the operation.
    /// # Returns
    /// A `Result` indicating success or failure of the validation.
    /// # Errors
    /// * `InvalidExecutionDelay` - If the delay is less than the minimum required or
    ///   exceeds the maximum allowed delay.
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
/// This struct encapsulates the necessary information
/// to create a new timelock operation, including the operation type,
/// target program, instruction data, and execution delay.
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
    /// This method initializes a new instance of `TimelockOperationData` with the provided parameters.
    /// It validates the instruction data size and the execution delay to ensure they meet the requirements
    /// for a valid timelock operation.
    /// # Arguments
    /// * `operation_type` - The type of the operation.
    /// * `target_program` - The target program for the operation.
    /// * `instruction_data` - The instruction data for the operation.
    /// * `execution_delay` - The execution delay for the operation.
    /// # Returns
    /// A `Result` containing the initialized `TimelockOperationData` instance or an error if validation fails.
    /// # Errors
    /// * `InvalidInstructionData` - If the instruction data exceeds the maximum allowed size.
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
    /// This method generates a hash of the operation data, including the operation type,
    /// target program, instruction data, and execution delay.
    /// This hash can be used for integrity checks or to uniquely identify the operation.
    /// # Returns
    /// A 32-byte array representing the hash of the operation data.
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
    let timelock_operation = &mut ctx.accounts.timelock_operation.load_init()?;

    timelock_operation.initialize(
        ctx.accounts.pool_core.key(),
        operation_id,
        operation_data.operation_type,
        operation_data.target_program,
        &operation_data.instruction_data,
        operation_data.execution_delay,
        ctx.accounts.proposer.key(),
        required_confirmations,
    )?;

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
