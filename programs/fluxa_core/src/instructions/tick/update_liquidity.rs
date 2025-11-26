use crate::math::core_arithmetic::Q64x64Signed;
use crate::state::tick::tick_data::TickData;
use anchor_lang::prelude::*;

/// Anchor context for updating tick liquidity with protocol safety.
///
/// # Why
/// - Ensures only mutable, properly derived tick accounts can be updated.
/// - Used for all liquidity delta operations, enforcing protocol invariants.
#[derive(Accounts)]
pub struct UpdateLiquidity<'info> {
    /// Mutable tick account to update; must be properly derived and validated.
    #[account(mut)]
    pub tick: AccountLoader<'info, TickData>,
}

/// Instruction handler for updating tick liquidity with protocol safety and minimal sysvar access.
///
/// # Why
/// - Ensures all liquidity changes are validated for initialization and emergency pause, preventing protocol invariant violations.
/// - Fetches clock once for deterministic and auditable state updates.
pub fn update_liquidity_instruction(
    ctx: Context<UpdateLiquidity>,
    delta: Q64x64Signed,
) -> Result<()> {
    // Minimize sysvar hits: fetch clock once for deterministic state
    let clock = Clock::get()?;
    let tick = &mut ctx.accounts.tick.load_mut()?;
    tick.update_liquidity_delta(delta, clock.slot, clock.unix_timestamp)?;

    Ok(())
}
