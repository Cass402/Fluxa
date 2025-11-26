use crate::math::core_arithmetic::Q64x64;
use crate::state::pool::pool_core::PoolCore;
use crate::state::tick::tick_data::TickData;
use crate::state::tick::tick_rate_limit::{cross_tick_with_enhanced_security, TickRateLimit};
use anchor_lang::prelude::*;

/// Anchor account constraints for tick crossing, using seeds for strong validation.
///
/// # Why these constraints?
/// - All accounts are marked `mut` to allow in-place updates, minimizing Anchor account loads.
/// - `rate_limit` uses seeds validation, so Anchor enforces the correct pool association at the constraint level, not at runtime.
/// - This design eliminates unnecessary BPF loads and reduces the risk of account spoofing.
#[derive(Accounts)]
pub struct TickCross<'info> {
    #[account(mut)]
    pub pool: AccountLoader<'info, PoolCore>,

    #[account(mut)]
    pub tick_data: AccountLoader<'info, TickData>,

    // Seeds validation ensures this account is always associated with the correct pool, enforced by Anchor at the constraint level.
    #[account(
        mut,
        seeds = [b"rate_limit", pool.key().as_ref()],
        bump
    )]
    pub rate_limit: AccountLoader<'info, TickRateLimit>,

    pub authority: Signer<'info>,
}

/// Anchor instruction handler for tick crossing, with minimal overhead.
///
/// # Why this pattern?
/// - Loads all accounts as mutable references up front, minimizing Anchor account loads and maximizing cache efficiency.
/// - All logic is delegated to a single batched function, ensuring atomicity and reducing the risk of partial state updates.
pub fn tick_cross_instruction(
    ctx: Context<TickCross>,
    zero_for_one: u8,
    volume_0: Q64x64,
    volume_1: Q64x64,
) -> Result<()> {
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    let pool = &mut ctx.accounts.pool.load_mut()?;
    let mut tick_data = ctx.accounts.tick_data.load_mut()?;
    let mut rate_limit = ctx.accounts.rate_limit.load_mut()?;

    cross_tick_with_enhanced_security(
        pool,
        &mut tick_data,
        &mut rate_limit,
        zero_for_one,
        current_slot,
        volume_0,
        volume_1,
    )?;

    Ok(())
}
