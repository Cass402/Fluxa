use crate::error::TickError;
use crate::state::pool::pool_core::PoolCore;
use crate::state::tick::tick_data::TickData;
use anchor_lang::prelude::*;

/// Anchor context for initializing a new tick account with protocol-aligned validation.
///
/// # Why
/// - Enforces deterministic account addressing and tick alignment for protocol safety.
/// - Ensures only valid ticks (aligned to spacing, unique nonce) can be initialized, preventing replay and misconfiguration.
/// - Payer and system program are required for account creation and rent exemption.
#[derive(Accounts)]
#[instruction(tick_index: i32, tick_spacing: u16, nonce: u32)]
pub struct InitializeTick<'info> {
    /// Tick account to be created; address is derived from pool, tick index, and nonce for uniqueness and replay protection.
    #[account(
        init,
        payer = payer,
        space = 8 + TickData::INIT_SPACE, // Ensures correct allocation for zero-copy
        seeds = [
            b"tick",
            pool.key().as_ref(),
            &tick_index.to_le_bytes(),
            &nonce.to_le_bytes()
        ],
        bump,
        constraint = tick_index % tick_spacing as i32 == 0 @ TickError::InvalidTickAlignment
    )]
    pub tick: AccountLoader<'info, TickData>,

    /// Reference to the parent pool; must be valid and non-default for protocol integrity.
    #[account(
        constraint = pool.key() != Pubkey::default() @ TickError::InvalidTickIndex
    )]
    pub pool: AccountLoader<'info, PoolCore>,

    /// Payer for account creation and rent; must be mutable to debit lamports.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// System program for CPI account creation.
    pub system_program: Program<'info, System>,
}

/// Instruction handler for initializing a tick with protocol-aligned validation and minimal sysvar access.
///
/// # Why
/// - Ensures tick is only initialized if properly aligned and not previously initialized, preventing replay and state corruption.
/// - Fetches clock once at the instruction level for efficiency and deterministic state updates.
/// - Passes all relevant context to the TickData::initialize method for full protocol safety.
pub fn initialize_tick(
    ctx: Context<InitializeTick>,
    tick_index: i32,
    tick_spacing: u16,
    nonce: u32,
) -> Result<()> {
    // Minimize sysvar hits: fetch clock once for deterministic state
    let clock = Clock::get()?;

    let tick = &mut ctx.accounts.tick.load_init()?;
    tick.initialize(
        tick_index,
        tick_spacing,
        nonce,
        clock.slot,
        clock.unix_timestamp,
    )?;

    Ok(())
}
