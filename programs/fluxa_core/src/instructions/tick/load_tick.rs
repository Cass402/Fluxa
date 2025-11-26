use crate::error::TickError;
use crate::state::pool::pool_core::PoolCore;
use crate::state::tick::tick_data::TickData;
use anchor_lang::prelude::*;

/// Anchor context for loading an existing tick account with strict validation.
///
/// # Why
/// - Ensures the tick account is correctly derived and initialized, preventing accidental or malicious access to uninitialized or misaddressed ticks.
/// - Pool reference is required for address derivation and protocol context.
#[derive(Accounts)]
pub struct LoadTick<'info> {
    /// Tick account to load; address is derived and must be initialized for safety.
    #[account(
        seeds = [
            b"tick",
            pool.key().as_ref(),
            &tick.load()?.tick_index.to_le_bytes(),
            &tick.load()?.initialization_nonce.to_le_bytes()
        ],
        bump,
        constraint = tick.load()?.initialized == 1 @ TickError::TickNotInitialized,
    )]
    pub tick: AccountLoader<'info, TickData>,

    /// Reference to the parent pool for context and address derivation.
    pub pool: AccountLoader<'info, PoolCore>,
}

/// Instruction handler for loading a tick with strict initialization validation.
///
/// # Why
/// - Ensures that only initialized ticks are loaded, preventing accidental or malicious access to uninitialized state.
/// - Can be extended for additional runtime checks (e.g., emergency pause) if needed.
pub fn load_tick(ctx: Context<LoadTick>) -> Result<()> {
    let tick = ctx.accounts.tick.load()?;

    // Protocol safety: enforce that tick is initialized before any operation
    require!(tick.initialized == 1, TickError::TickNotInitialized);

    Ok(())
}
