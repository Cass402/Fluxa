use crate::math::core_arithmetic::{Q64x64, Q64x64Signed};
use crate::state::tick::tick_data::TickData;
use anchor_lang::prelude::*;

/// Anchor context for batch updating tick properties atomically.
///
/// # Why
/// - Enables atomic updates to multiple tick fields (liquidity, fee growth, metadata) in a single instruction.
/// - Reduces sysvar hits and improves protocol efficiency for batch operations.
#[derive(Accounts)]
pub struct BatchUpdate<'info> {
    /// Mutable tick account to update; must be properly derived and validated.
    #[account(mut)]
    pub tick: AccountLoader<'info, TickData>,
}

/// Instruction handler for batch updating tick properties with protocol safety and minimal sysvar access.
///
/// # Why
/// - Allows atomic updates to liquidity, fee growth, and metadata, reducing the number of instructions and sysvar accesses.
/// - Ensures all updates are validated for initialization, emergency pause, and suspicious activity, maintaining protocol safety.
/// - Used for protocol-level batch operations and off-chain sync scenarios.
pub fn batch_update_instruction(
    ctx: Context<BatchUpdate>,
    liquidity_delta: Option<Q64x64Signed>,
    fee_growth_0: Option<Q64x64>,
    fee_growth_1: Option<Q64x64>,
) -> Result<()> {
    // Minimize sysvar hits: fetch clock once for deterministic state
    let clock = Clock::get()?;

    let tick = &mut ctx.accounts.tick.load_mut()?;
    tick.batch_update(
        liquidity_delta,
        fee_growth_0,
        fee_growth_1,
        clock.slot,
        clock.unix_timestamp,
    )?;

    Ok(())
}
