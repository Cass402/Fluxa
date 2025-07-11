//! Timelock Operation Management with Individual PDA Accounts
//!
//! This module provides timelock functionality using separate PDA accounts
//! for each operation, enabling efficient and scalable governance.
//! Uses Anchor's built-in PDA validation for security.
use crate::error::PdaSecurityAuthorityError;
use crate::utils::constants::{MAX_DELAY, MIN_DELAY};
use anchor_lang::prelude::*; // Updated to use your error module

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
    pub executed_at: Option<i64>,

    /// Governance
    /// 'proposer' - The public key of the proposer initiating the operation.
    /// 'executor' - The public key of the executor who will execute the operation.
    /// 'status' - Current status of the operation (Pending, Approved, Executed,
    pub proposer: Pubkey,
    pub executor: Option<Pubkey>,
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

        self.discriminator = Self::discriminator();
        self.pool_core = pool_core;
        self.operation_id = operation_id;
        self.operation_type = operation_type as u8;
        self.target_program = target_program;
        self.proposer = proposer;
        self.required_confirmations = required_confirmations;
        self.status = TimelockStatus::Pending;

        // Store instruction data
        self.instruction_data[..instruction_data.len()].copy_from_slice(instruction_data);
        self.instruction_data_len = instruction_data.len() as u16;
        self.instruction_data_hash =
            anchor_lang::solana_program::hash::hash(instruction_data).to_bytes();

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

    pub fn confirm(&mut self, confirmer_index: u8) -> Result<bool> {
        if self.status != TimelockStatus::Pending {
            return Err(PdaSecurityAuthorityError::TimelockNotReady.into());
        }

        if confirmer_index >= 64 {
            return Err(PdaSecurityAuthorityError::TimelockConfirmationLimitReached.into());
        }

        let confirmer_bit = 1u64 << confirmer_index;

        // Check if already confirmed
        if (self.confirmations_bitmap & confirmer_bit) != 0 {
            return Ok(self.confirmation_count >= self.required_confirmations);
        }

        // Add confirmation
        self.confirmations_bitmap |= confirmer_bit;
        self.confirmation_count += 1;

        let clock = Clock::get()?;
        self.last_updated = clock.unix_timestamp;

        // Check if ready for execution
        if self.confirmation_count >= self.required_confirmations {
            self.status = TimelockStatus::Approved;
        }

        Ok(self.confirmation_count >= self.required_confirmations)
    }

    pub fn is_ready_for_execution(&self) -> bool {
        let clock = Clock::get().unwrap();
        self.status == TimelockStatus::Approved && clock.unix_timestamp >= self.execution_time
    }

    pub fn is_expired(&self) -> bool {
        let clock = Clock::get().unwrap();
        // Operations expire after 30 days if not executed
        clock.unix_timestamp > self.execution_time + (30 * 24 * 3600)
    }

    pub fn execute(&mut self, executor: Pubkey) -> Result<()> {
        if !self.is_ready_for_execution() {
            return Err(PdaSecurityAuthorityError::TimelockNotReady.into());
        }

        if self.is_expired() {
            return Err(PdaSecurityAuthorityError::TimelockOperationExpired.into());
        }

        self.status = TimelockStatus::Executed;
        self.executor = Some(executor);

        let clock = Clock::get()?;
        self.executed_at = Some(clock.unix_timestamp);
        self.last_updated = clock.unix_timestamp;

        Ok(())
    }

    pub fn cancel(&mut self) -> Result<()> {
        if self.status == TimelockStatus::Executed {
            return Err(PdaSecurityAuthorityError::TimelockNotReady.into());
        }

        self.status = TimelockStatus::Cancelled;

        let clock = Clock::get()?;
        self.last_updated = clock.unix_timestamp;

        Ok(())
    }

    pub fn get_instruction_data(&self) -> &[u8] {
        &self.instruction_data[..self.instruction_data_len as usize]
    }

    fn discriminator() -> [u8; 8] {
        [0x15, 0x25, 0x35, 0x45, 0x55, 0x65, 0x75, 0x85]
    }
}

/// Status of timelock operations
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
pub struct TimelockManager;

impl TimelockManager {
    /// Generate unique operation ID
    pub fn generate_operation_id(
        pool_core: &Pubkey,
        proposer: &Pubkey,
        timestamp: i64,
        operation_type: TimelockOperationType,
    ) -> u64 {
        let combined = [
            pool_core.as_ref(),
            proposer.as_ref(),
            &timestamp.to_le_bytes(),
            &[operation_type as u8],
        ]
        .concat();

        let hash = anchor_lang::solana_program::hash::hash(&combined);
        u64::from_le_bytes(hash.to_bytes()[..8].try_into().unwrap())
    }

    /// Get minimum delay for operation type
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
#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct TimelockOperationData {
    pub operation_type: TimelockOperationType,
    pub target_program: Pubkey,
    pub instruction_data: Vec<u8>,
    pub execution_delay: i64,
}

impl TimelockOperationData {
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

    pub fn compute_hash(&self) -> [u8; 32] {
        let combined = [
            &[self.operation_type as u8],
            self.target_program.as_ref(),
            &self.instruction_data,
            &self.execution_delay.to_le_bytes(),
        ]
        .concat();

        anchor_lang::solana_program::hash::hash(&combined).to_bytes()
    }
}

// ============================================================================
// ANCHOR ACCOUNT VALIDATION CONTEXTS
// ============================================================================

/// Create Timelock Operation Context
#[derive(Accounts)]
#[instruction(operation_id: u64)]
pub struct CreateTimelockOperation<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<TimelockOperation>(),
        seeds = [b"timelock_operation", pool_core.key().as_ref(), &operation_id.to_le_bytes()],
        bump
    )]
    pub timelock_operation: AccountLoader<'info, TimelockOperation>,

    /// CHECK: Pool core account validation
    pub pool_core: UncheckedAccount<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub proposer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Confirm Timelock Operation Context
#[derive(Accounts)]
pub struct ConfirmTimelockOperation<'info> {
    #[account(
        mut,
        seeds = [b"timelock_operation", pool_core.key().as_ref(), &timelock_operation.load()?.operation_id.to_le_bytes()],
        bump
    )]
    pub timelock_operation: AccountLoader<'info, TimelockOperation>,

    /// CHECK: Pool core account validation
    pub pool_core: UncheckedAccount<'info>,

    pub confirmer: Signer<'info>,
}

/// Execute Timelock Operation Context
#[derive(Accounts)]
pub struct ExecuteTimelockOperation<'info> {
    #[account(
        mut,
        seeds = [b"timelock_operation", pool_core.key().as_ref(), &timelock_operation.load()?.operation_id.to_le_bytes()],
        bump
    )]
    pub timelock_operation: AccountLoader<'info, TimelockOperation>,

    /// CHECK: Pool core account validation
    pub pool_core: UncheckedAccount<'info>,

    pub executor: Signer<'info>,
}

/// Cancel Timelock Operation Context
#[derive(Accounts)]
pub struct CancelTimelockOperation<'info> {
    #[account(
        mut,
        seeds = [b"timelock_operation", pool_core.key().as_ref(), &timelock_operation.load()?.operation_id.to_le_bytes()],
        bump
    )]
    pub timelock_operation: AccountLoader<'info, TimelockOperation>,

    /// CHECK: Pool core account validation
    pub pool_core: UncheckedAccount<'info>,

    pub authority: Signer<'info>,
}

// ============================================================================
// INSTRUCTION HANDLERS
// ============================================================================

/// Create Timelock Operation
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
pub fn execute_timelock_operation(ctx: Context<ExecuteTimelockOperation>) -> Result<()> {
    let timelock_operation = &mut ctx.accounts.timelock_operation.load_mut()?;

    timelock_operation.execute(ctx.accounts.executor.key())?;

    // Here you would typically execute the actual instruction
    // stored in timelock_operation.get_instruction_data()

    Ok(())
}

/// Cancel Timelock Operation
pub fn cancel_timelock_operation(ctx: Context<CancelTimelockOperation>) -> Result<()> {
    let timelock_operation = &mut ctx.accounts.timelock_operation.load_mut()?;

    timelock_operation.cancel()?;

    Ok(())
}
