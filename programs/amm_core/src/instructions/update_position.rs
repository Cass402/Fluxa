use crate::constants::{MAX_TICK, MIN_TICK};
use crate::errors::ErrorCode;
use crate::UpdatePosition;
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<UpdatePosition>,
    new_tick_lower_index: i32,
    new_tick_upper_index: i32,
) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    let position = &mut ctx.accounts.position;

    // Validate new tick indices
    if new_tick_lower_index >= new_tick_upper_index {
        return err!(ErrorCode::InvalidTickRange);
    }
    if new_tick_lower_index < MIN_TICK || new_tick_upper_index > MAX_TICK {
        return err!(ErrorCode::InvalidTickRange);
    }
    let tick_spacing = pool.tick_spacing as i32;
    if new_tick_lower_index % tick_spacing != 0 || new_tick_upper_index % tick_spacing != 0 {
        return err!(ErrorCode::InvalidTickSpacing);
    }

    let old_tick_lower_idx = position.tick_lower_index;
    let old_tick_upper_idx = position.tick_upper_index;
    let liquidity_to_move = position.liquidity; // This is u128

    if liquidity_to_move == 0 {
        // If no liquidity, just update the position's ticks
        position.tick_lower_index = new_tick_lower_index;
        position.tick_upper_index = new_tick_upper_index;
        msg!(
            "Position {} ticks updated with zero liquidity.",
            position.key()
        );
        return Ok(());
    }

    // 1. Remove liquidity from the old range
    // The liquidity_delta is negative as we are removing liquidity.
    pool.modify_liquidity(
        old_tick_lower_idx,
        old_tick_upper_idx,
        -(liquidity_to_move as i128), // Cast u128 to i128 and negate
        &ctx.accounts.old_tick_lower,
        &ctx.accounts.old_tick_upper,
    )?;
    msg!(
        "Liquidity removed from old range [{}, {}]",
        old_tick_lower_idx,
        old_tick_upper_idx
    );

    // 2. Update the position's tick boundaries
    position.tick_lower_index = new_tick_lower_index;
    position.tick_upper_index = new_tick_upper_index;

    // 3. Initialize new TickData if they were newly created by init_if_needed
    let mut new_tick_lower_data = ctx.accounts.new_tick_lower.load_mut()?;
    if new_tick_lower_data.pool == Pubkey::default() {
        // Check if it's uninitialized
        new_tick_lower_data.initialize(pool.key(), new_tick_lower_index);
        msg!(
            "NewTickLower account {} initialized for index {}",
            ctx.accounts.new_tick_lower.to_account_info().key(),
            new_tick_lower_index
        );
    }
    drop(new_tick_lower_data); // Release borrow

    let mut new_tick_upper_data = ctx.accounts.new_tick_upper.load_mut()?;
    if new_tick_upper_data.pool == Pubkey::default() {
        // Check if it's uninitialized
        new_tick_upper_data.initialize(pool.key(), new_tick_upper_index);
        msg!(
            "NewTickUpper account {} initialized for index {}",
            ctx.accounts.new_tick_upper.to_account_info().key(),
            new_tick_upper_index
        );
    }
    drop(new_tick_upper_data); // Release borrow

    // 4. Add liquidity to the new range
    // The liquidity_delta is positive.
    pool.modify_liquidity(
        new_tick_lower_index,
        new_tick_upper_index,
        liquidity_to_move as i128, // Cast u128 to i128
        &ctx.accounts.new_tick_lower,
        &ctx.accounts.new_tick_upper,
    )?;
    msg!(
        "Liquidity added to new range [{}, {}]",
        new_tick_lower_index,
        new_tick_upper_index
    );
    msg!(
        "Position {} rebalanced. New pool liquidity: {}",
        position.key(),
        pool.liquidity
    );

    // MVP Simplification: Token transfers are complex.
    // A full rebalance would calculate token amounts based on current price and new range,
    // withdraw from vaults, and potentially require user to deposit/withdraw difference.
    // For hackathon, "ghost-moving" liquidity by just updating ticks is a common simplification.

    Ok(())
}
