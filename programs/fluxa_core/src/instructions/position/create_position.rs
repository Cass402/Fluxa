//! Position creation instruction for the Fluxa AMM.
//!
//! This module implements position creation with:
//! - Nonce-based PDA derivation for multiple positions on same tick range
//! - Dual mode support: individual positions or batched positions with Merkle verification
//! - Configurable precision optimization for liquidity calculations
//! - Pool-level configurable batch threshold with protocol default fallback
//! - Comprehensive security checks including pause state, deadline, and slippage validation
//! - Q64x64 fixed-point math throughout for deterministic cross-platform behavior

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::error::{MathError, PositionError};
use crate::math::core_arithmetic::{tick_to_sqrt_x64, Q64x64, Q64x64Signed};
use crate::math::liquidity_math::{calculate_amounts_for_liquidity_piecewise, calculate_liquidity};
use crate::state::pool::pool_config::PoolConfig;
use crate::state::pool::pool_core::PoolCore;
use crate::state::pool::pool_security::PoolSecurity;
use crate::state::position::position_account::{CompressedPosition, InitArgs, Position};
use crate::state::position::position_batch::PositionBatch;
use crate::state::tick::tick_data::TickData;
use crate::utils::constants::{
    MAX_BATCH_POSITIONS, MAX_DEADLINE_EXTENSION, MAX_POSITION_LIQUIDITY, MAX_SLIPPAGE_BPS,
    MAX_TICK, MIN_TICK,
};
use crate::utils::event::PositionCreatedEvent;

// ============================================================================
// INSTRUCTION PARAMETERS
// ============================================================================

/// Parameters for position creation with comprehensive validation options.
///
/// # Design Rationale
/// - All amounts use u64 to match SPL token accounting
/// - Deadline prevents stale transactions from executing at unfavorable prices
/// - Batch preference allows users to optimize for compute/rent trade-offs
/// - Optional precision optimization trades CU for cleaner token amounts
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PositionCreationParams {
    /// Lower tick boundary of the position (must be < tick_upper and aligned to tick_spacing).
    pub tick_lower: i32,
    /// Upper tick boundary of the position (must be > tick_lower and aligned to tick_spacing).
    pub tick_upper: i32,
    /// Desired amount of token0 to deposit (used for liquidity calculation).
    pub amount_0_desired: u64,
    /// Desired amount of token1 to deposit (used for liquidity calculation).
    pub amount_1_desired: u64,
    /// Minimum acceptable amount of token0 after slippage.
    pub amount_0_min: u64,
    /// Minimum acceptable amount of token1 after slippage.
    pub amount_1_min: u64,
    /// Unix timestamp deadline; transaction fails if current time > deadline.
    pub deadline: i64,
    /// Unique nonce for this position (enables multiple positions on same tick range).
    pub position_nonce: u16,
    /// User preference for individual vs batched position storage.
    pub batch_preference: BatchPreference,
    /// Whether to apply precision optimization to liquidity calculation.
    /// Trades ~5K additional CU for potentially cleaner token amounts.
    pub optimize_precision: bool,
}

/// User preference for position storage mode.
///
/// # Design Rationale
/// - Individual: Higher rent cost but simpler management and no Merkle overhead
/// - Batch: Lower rent per position but requires batch account and Merkle updates
/// - AutoOptimize: Let protocol decide based on liquidity threshold and batch availability
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub enum BatchPreference {
    /// Always create as individual position account.
    PreferIndividual,
    /// Prefer batched storage if batch account is provided.
    PreferBatch,
    /// Let protocol decide based on liquidity threshold and efficiency.
    AutoOptimize,
}

/// Result of position creation for return to caller.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PositionCreationResult {
    /// Key of the position account (for individual) or batch account (for batched).
    pub position_key: Pubkey,
    /// Actual liquidity minted.
    pub liquidity: u128,
    /// Actual token0 deposited.
    pub amount_0_deposited: u64,
    /// Actual token1 deposited.
    pub amount_1_deposited: u64,
    /// Whether position was batched or individual.
    pub is_batched: bool,
    /// Batch ID if batched, None otherwise.
    pub batch_id: Option<u32>,
}

// ============================================================================
// ACCOUNT CONTEXT
// ============================================================================

/// Account context for creating a new liquidity position.
///
/// # Design Rationale
/// - Uses AccountLoader for zero-copy access to large structs (PoolCore, PoolSecurity, etc.)
/// - Position and tick accounts use init/init_if_needed for atomic creation
/// - Batch account is optional to support both individual and batched modes
/// - Vault derivation uses cached bumps from PoolCore for efficiency
#[derive(Accounts)]
#[instruction(params: PositionCreationParams)]
pub struct CreatePosition<'info> {
    // ========== Pool State Accounts ==========
    /// Pool core state containing price, liquidity, and fee parameters.
    #[account(mut)]
    pub pool_core: AccountLoader<'info, PoolCore>,

    /// Pool security state for pause checks and position counting.
    #[account(
        mut,
        constraint = pool_security.load()?.pool_core == pool_core.key() @ PositionError::InvalidTickRange,
    )]
    pub pool_security: AccountLoader<'info, PoolSecurity>,

    /// Pool configuration for batch threshold override.
    #[account(
        constraint = pool_config.load()?.pool_core == pool_core.key() @ PositionError::InvalidTickRange,
    )]
    pub pool_config: AccountLoader<'info, PoolConfig>,

    // ========== Position Account (for individual positions) ==========
    /// Position account to be created (nonce-based PDA).
    /// Seeds: ["position", pool_core, owner, position_nonce]
    #[account(
        init,
        seeds = [
            b"position",
            pool_core.key().as_ref(),
            owner.key().as_ref(),
            &params.position_nonce.to_le_bytes(),
        ],
        bump,
        payer = owner,
        space = 8 + std::mem::size_of::<Position>(),
    )]
    pub position: AccountLoader<'info, Position>,

    // ========== Optional Batch Account (for batched positions) ==========
    /// Optional batch account for batched position storage.
    /// If provided and batch preference allows, position will be added to batch.
    #[account(mut)]
    pub position_batch: Option<AccountLoader<'info, PositionBatch>>,

    // ========== Tick Accounts ==========
    /// Lower tick account (created if not exists).
    #[account(
        init_if_needed,
        seeds = [b"tick", pool_core.key().as_ref(), &params.tick_lower.to_le_bytes()],
        bump,
        payer = owner,
        space = 8 + std::mem::size_of::<TickData>(),
    )]
    pub tick_lower: AccountLoader<'info, TickData>,

    /// Upper tick account (created if not exists).
    #[account(
        init_if_needed,
        seeds = [b"tick", pool_core.key().as_ref(), &params.tick_upper.to_le_bytes()],
        bump,
        payer = owner,
        space = 8 + std::mem::size_of::<TickData>(),
    )]
    pub tick_upper: AccountLoader<'info, TickData>,

    // ========== Token Vaults ==========
    /// Pool's token0 vault.
    #[account(mut)]
    pub vault_0: Account<'info, TokenAccount>,

    /// Pool's token1 vault.
    #[account(mut)]
    pub vault_1: Account<'info, TokenAccount>,

    // ========== User Token Accounts ==========
    /// User's token0 account for deposit.
    #[account(
        mut,
        constraint = user_token_0.owner == owner.key() @ PositionError::InsufficientBalance,
    )]
    pub user_token_0: Account<'info, TokenAccount>,

    /// User's token1 account for deposit.
    #[account(
        mut,
        constraint = user_token_1.owner == owner.key() @ PositionError::InsufficientBalance,
    )]
    pub user_token_1: Account<'info, TokenAccount>,

    // ========== Signer and Programs ==========
    /// Position owner and transaction payer.
    #[account(mut)]
    pub owner: Signer<'info>,

    /// SPL Token program.
    pub token_program: Program<'info, Token>,

    /// System program for account creation.
    pub system_program: Program<'info, System>,
}

// ============================================================================
// MAIN INSTRUCTION HANDLER
// ============================================================================

/// Creates a new liquidity position in the pool.
///
/// # Flow
/// 1. Validate deadline and security state
/// 2. Validate tick parameters (bounds, spacing, ordering)
/// 3. Calculate liquidity from desired amounts
/// 4. Apply optional precision optimization
/// 5. Calculate actual token amounts with slippage checks
/// 6. Determine position mode (individual vs batch)
/// 7. Initialize position/add to batch
/// 8. Update tick states
/// 9. Update pool liquidity if in range
/// 10. Execute token transfers with validation
/// 11. Emit creation event
pub fn create_position(
    mut ctx: Context<CreatePosition>,
    params: PositionCreationParams,
) -> Result<PositionCreationResult> {
    // ========== 1. Clock and Deadline Validation ==========
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;
    let current_slot = clock.slot;

    require!(
        current_time <= params.deadline,
        PositionError::DeadlineExceeded
    );
    require!(
        params.deadline <= current_time + MAX_DEADLINE_EXTENSION,
        PositionError::DeadlineTooFar
    );

    // ========== 2. Security State Validation ==========
    let pool_security = ctx.accounts.pool_security.load()?;
    require!(
        !pool_security.is_emergency_paused(),
        PositionError::PoolPaused
    );
    drop(pool_security);

    // ========== 3. Load Pool State ==========
    let pool_core = ctx.accounts.pool_core.load()?;
    let tick_spacing = pool_core.tick_spacing;
    let current_sqrt_price = pool_core.sqrt_price;
    let current_tick = pool_core.tick_current;
    drop(pool_core);

    // ========== 4. Tick Parameter Validation ==========
    validate_tick_parameters(params.tick_lower, params.tick_upper, tick_spacing)?;

    // ========== 5. Calculate Sqrt Prices for Tick Range ==========
    let sqrt_price_lower = tick_to_sqrt_x64(params.tick_lower)?;
    let sqrt_price_upper = tick_to_sqrt_x64(params.tick_upper)?;

    // ========== 6. Calculate Liquidity ==========
    let base_liquidity = calculate_liquidity(
        current_sqrt_price,
        sqrt_price_lower,
        sqrt_price_upper,
        params.amount_0_desired,
        params.amount_1_desired,
    )?;

    // ========== 7. Optional Precision Optimization ==========
    let liquidity = if params.optimize_precision {
        optimize_liquidity_precision(
            base_liquidity,
            current_sqrt_price,
            sqrt_price_lower,
            sqrt_price_upper,
        )?
    } else {
        base_liquidity
    };

    // ========== 8. Validate Liquidity Bounds ==========
    require!(liquidity > 0, PositionError::InsufficientLiquidity);
    require!(
        liquidity <= MAX_POSITION_LIQUIDITY,
        PositionError::ExcessiveLiquidity
    );

    let liquidity_q64 = Q64x64::from_raw((liquidity as u128) << 64);

    // ========== 9. Calculate Actual Token Amounts ==========
    let (amount_0, amount_1) = calculate_amounts_for_liquidity_piecewise(
        current_sqrt_price,
        sqrt_price_lower,
        sqrt_price_upper,
        liquidity_q64,
    )?;

    // ========== 10. Slippage Validation ==========
    require!(
        amount_0 >= params.amount_0_min,
        PositionError::SlippageExceeded
    );
    require!(
        amount_1 >= params.amount_1_min,
        PositionError::SlippageExceeded
    );

    // Calculate slippage in basis points for event
    let slippage_0_bps = calculate_slippage_bps(params.amount_0_desired, amount_0);
    let slippage_1_bps = calculate_slippage_bps(params.amount_1_desired, amount_1);

    require!(
        slippage_0_bps <= MAX_SLIPPAGE_BPS && slippage_1_bps <= MAX_SLIPPAGE_BPS,
        PositionError::SlippageExceeded
    );

    // ========== 11. Determine Position Mode ==========
    let pool_config = ctx.accounts.pool_config.load()?;
    let batch_threshold = pool_config.get_batch_liquidity_threshold();
    drop(pool_config);

    let (is_batched, batch_id) = determine_position_mode(
        &params.batch_preference,
        liquidity,
        batch_threshold,
        &ctx.accounts.position_batch,
    )?;

    // ========== 12. Initialize Position or Add to Batch ==========
    if is_batched {
        // Add to batch
        let batch_loader = ctx
            .accounts
            .position_batch
            .as_ref()
            .ok_or(PositionError::BatchAccountRequired)?;
        let mut batch = batch_loader.load_mut()?;

        require!(
            (batch.position_count as usize) < MAX_BATCH_POSITIONS,
            PositionError::BatchCapacityExceeded
        );

        // Create compressed position
        let compressed = CompressedPosition {
            owner: ctx.accounts.owner.key(),
            tick_range: CompressedPosition::pack_tick_range(params.tick_lower, params.tick_upper),
            position_nonce: params.position_nonce,
            _padding: [0u8; 6],
            liquidity: liquidity_q64,
        };

        // Get account data for batch operations
        let batch_account_info = batch_loader.to_account_info();
        let mut batch_data = batch_account_info.try_borrow_mut_data()?;

        batch.add_position(compressed, &mut batch_data)?;
        batch.last_update_slot = current_slot;

        drop(batch);
    } else {
        // Initialize individual position
        let mut position = ctx.accounts.position.load_init()?;
        position.initialize(InitArgs {
            owner: ctx.accounts.owner.key(),
            tick_lower: params.tick_lower,
            tick_upper: params.tick_upper,
            liquidity: liquidity_q64,
            position_nonce: params.position_nonce,
            current_slot,
            current_timestamp: current_time,
        })?;
        drop(position);
    }

    // ========== 13. Update Tick States ==========
    let liquidity_delta_lower = Q64x64Signed::from_raw((liquidity as i128) << 64);
    let liquidity_delta_upper = Q64x64Signed::from_raw(-((liquidity as i128) << 64));

    // Update lower tick
    {
        let mut tick_lower = ctx.accounts.tick_lower.load_mut()?;
        if tick_lower.initialized == 0 {
            tick_lower.initialize(
                params.tick_lower,
                tick_spacing,
                0, // initialization_nonce
                current_slot,
                current_time,
            )?;
        }
        tick_lower.update_liquidity_delta(liquidity_delta_lower, current_slot, current_time)?;
    }

    // Update upper tick
    {
        let mut tick_upper = ctx.accounts.tick_upper.load_mut()?;
        if tick_upper.initialized == 0 {
            tick_upper.initialize(
                params.tick_upper,
                tick_spacing,
                0, // initialization_nonce
                current_slot,
                current_time,
            )?;
        }
        tick_upper.update_liquidity_delta(liquidity_delta_upper, current_slot, current_time)?;
    }

    // ========== 14. Update Pool Liquidity if In Range ==========
    {
        let mut pool_core = ctx.accounts.pool_core.load_mut()?;
        if current_tick >= params.tick_lower && current_tick < params.tick_upper {
            pool_core.liquidity = pool_core.liquidity.checked_add(liquidity_q64)?;
        }
        pool_core.last_update_slot = current_slot;
    }

    // ========== 15. Update Position Count ==========
    {
        let mut pool_security = ctx.accounts.pool_security.load_mut()?;
        pool_security.active_positions_count = pool_security
            .active_positions_count
            .checked_add(1)
            .ok_or(MathError::Overflow)?;
    }

    // ========== 16. Execute Token Transfers ==========
    execute_token_transfers(&mut ctx, amount_0, amount_1)?;

    // ========== 17. Emit Event ==========
    let position_key = if is_batched {
        ctx.accounts.position_batch.as_ref().unwrap().key()
    } else {
        ctx.accounts.position.key()
    };

    emit!(PositionCreatedEvent {
        position: position_key,
        owner: ctx.accounts.owner.key(),
        pool_core: ctx.accounts.pool_core.key(),
        tick_lower: params.tick_lower,
        tick_upper: params.tick_upper,
        liquidity,
        amount_0,
        amount_1,
        is_batched,
        batch_id,
        position_nonce: params.position_nonce,
        slippage_0_bps,
        slippage_1_bps,
        precision_optimized: params.optimize_precision,
        timestamp: current_time,
        slot: current_slot,
    });

    Ok(PositionCreationResult {
        position_key,
        liquidity,
        amount_0_deposited: amount_0,
        amount_1_deposited: amount_1,
        is_batched,
        batch_id,
    })
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Validates tick parameters for position creation.
fn validate_tick_parameters(tick_lower: i32, tick_upper: i32, tick_spacing: u16) -> Result<()> {
    // Check tick bounds
    require!(
        tick_lower >= MIN_TICK && tick_lower <= MAX_TICK,
        PositionError::InvalidTickRange
    );
    require!(
        tick_upper >= MIN_TICK && tick_upper <= MAX_TICK,
        PositionError::InvalidTickRange
    );

    // Check ordering
    require!(tick_lower < tick_upper, PositionError::InvalidTickRange);

    // Check tick spacing alignment
    let spacing = tick_spacing as i32;
    require!(tick_lower % spacing == 0, PositionError::TickNotAligned);
    require!(tick_upper % spacing == 0, PositionError::TickNotAligned);

    Ok(())
}

/// Calculates slippage in basis points.
#[inline(always)]
fn calculate_slippage_bps(desired: u64, actual: u64) -> u16 {
    if desired == 0 || actual >= desired {
        return 0;
    }
    let diff = desired.saturating_sub(actual);
    ((diff as u128 * 10000) / desired as u128) as u16
}

/// Determines whether to use batched or individual position storage.
fn determine_position_mode(
    preference: &BatchPreference,
    liquidity: u128,
    batch_threshold: u128,
    batch_account: &Option<AccountLoader<PositionBatch>>,
) -> Result<(bool, Option<u32>)> {
    match preference {
        BatchPreference::PreferIndividual => Ok((false, None)),
        BatchPreference::PreferBatch => {
            if let Some(batch_loader) = batch_account {
                let batch = batch_loader.load()?;
                if (batch.position_count as usize) < MAX_BATCH_POSITIONS {
                    let batch_id = batch.batch_id;
                    drop(batch);
                    return Ok((true, Some(batch_id)));
                }
            }
            Ok((false, None))
        }
        BatchPreference::AutoOptimize => {
            // High-value positions get individual accounts
            if liquidity >= batch_threshold {
                return Ok((false, None));
            }

            // Check if batch is available and has capacity
            if let Some(batch_loader) = batch_account {
                let batch = batch_loader.load()?;
                if (batch.position_count as usize) < MAX_BATCH_POSITIONS {
                    let batch_id = batch.batch_id;
                    drop(batch);
                    return Ok((true, Some(batch_id)));
                }
            }

            Ok((false, None))
        }
    }
}

/// Applies precision optimization to find cleaner token amounts.
///
/// Tests small adjustments to liquidity to find values that result in
/// token amounts that are multiples of common denominators (10, 100, 1000).
fn optimize_liquidity_precision(
    base_liquidity: u128,
    sqrt_price: Q64x64,
    sqrt_price_a: Q64x64,
    sqrt_price_b: Q64x64,
) -> Result<u128> {
    let mut best_liquidity = base_liquidity;
    let mut best_score = 0u32;

    // Test small adjustments for better precision
    for adjustment in [-10i128, -5, -2, -1, 0, 1, 2, 5, 10] {
        let adjusted_liquidity = if adjustment >= 0 {
            base_liquidity.checked_add(adjustment as u128)
        } else {
            base_liquidity.checked_sub((-adjustment) as u128)
        };

        if let Some(liq) = adjusted_liquidity {
            if liq > 0 {
                let liq_q64 = Q64x64::from_raw((liq as u128) << 64);
                if let Ok((amount_0, amount_1)) = calculate_amounts_for_liquidity_piecewise(
                    sqrt_price,
                    sqrt_price_a,
                    sqrt_price_b,
                    liq_q64,
                ) {
                    let score = calculate_precision_score(amount_0, amount_1);
                    if score > best_score {
                        best_score = score;
                        best_liquidity = liq;
                    }
                }
            }
        }
    }

    // Validate optimization didn't cause significant deviation
    let deviation = base_liquidity.abs_diff(best_liquidity);
    let max_allowed_deviation = base_liquidity / 10000; // 0.01% max

    if deviation <= max_allowed_deviation {
        Ok(best_liquidity)
    } else {
        Ok(base_liquidity)
    }
}

/// Calculates precision score based on token amount cleanliness.
#[inline(always)]
fn calculate_precision_score(amount_0: u64, amount_1: u64) -> u32 {
    let mut score = 0u32;

    // Prefer amounts that are multiples of common denominators
    if amount_0 % 1000 == 0 {
        score += 10;
    }
    if amount_0 % 100 == 0 {
        score += 5;
    }
    if amount_0 % 10 == 0 {
        score += 1;
    }

    if amount_1 % 1000 == 0 {
        score += 10;
    }
    if amount_1 % 100 == 0 {
        score += 5;
    }
    if amount_1 % 10 == 0 {
        score += 1;
    }

    score
}

/// Executes token transfers from user to pool vaults with validation.
fn execute_token_transfers(
    ctx: &mut Context<CreatePosition>,
    amount_0: u64,
    amount_1: u64,
) -> Result<()> {
    // Pre-transfer balance snapshots
    let vault_0_before = ctx.accounts.vault_0.amount;
    let vault_1_before = ctx.accounts.vault_1.amount;
    let user_0_before = ctx.accounts.user_token_0.amount;
    let user_1_before = ctx.accounts.user_token_1.amount;

    // Validate user has sufficient balance
    require!(
        user_0_before >= amount_0,
        PositionError::InsufficientBalance
    );
    require!(
        user_1_before >= amount_1,
        PositionError::InsufficientBalance
    );

    // Transfer token0
    if amount_0 > 0 {
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_token_0.to_account_info(),
            to: ctx.accounts.vault_0.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, amount_0).map_err(|_| PositionError::TransferFailed)?;
    }

    // Transfer token1
    if amount_1 > 0 {
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_token_1.to_account_info(),
            to: ctx.accounts.vault_1.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, amount_1).map_err(|_| PositionError::TransferFailed)?;
    }

    // Post-transfer validation with account reloading
    ctx.accounts.vault_0.reload()?;
    ctx.accounts.vault_1.reload()?;
    ctx.accounts.user_token_0.reload()?;
    ctx.accounts.user_token_1.reload()?;

    // Validate expected balance changes
    require!(
        ctx.accounts.vault_0.amount == vault_0_before + amount_0,
        PositionError::TransferFailed
    );
    require!(
        ctx.accounts.vault_1.amount == vault_1_before + amount_1,
        PositionError::TransferFailed
    );
    require!(
        ctx.accounts.user_token_0.amount == user_0_before - amount_0,
        PositionError::TransferFailed
    );
    require!(
        ctx.accounts.user_token_1.amount == user_1_before - amount_1,
        PositionError::TransferFailed
    );

    Ok(())
}
