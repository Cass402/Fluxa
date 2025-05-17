use anchor_lang::prelude::*;

use crate::constants::{MAX_TICK, MIN_LIQUIDITY, MIN_TICK};
use crate::errors::ErrorCode;
use crate::MintPosition;

pub fn handler(
    ctx: Context<MintPosition>,
    tick_lower_index: i32,
    tick_upper_index: i32,
    liquidity_amount_desired: u128,
) -> Result<()> {
    // Validate tick indices
    if tick_lower_index >= tick_upper_index {
        return err!(ErrorCode::InvalidTickRange);
    }
    if tick_lower_index < MIN_TICK || tick_upper_index > MAX_TICK {
        return err!(ErrorCode::InvalidTickRange);
    }

    // Validate tick alignment with pool's tick_spacing
    let tick_spacing = ctx.accounts.pool.tick_spacing as i32;
    if tick_lower_index % tick_spacing != 0 || tick_upper_index % tick_spacing != 0 {
        return err!(ErrorCode::InvalidTickSpacing);
    }

    // Validate liquidity amount
    if liquidity_amount_desired == 0 {
        return err!(ErrorCode::ZeroLiquidityDelta);
    }
    if liquidity_amount_desired < MIN_LIQUIDITY {
        // Or a more specific error like LiquidityAmountTooLow
        return err!(ErrorCode::InvalidInput);
    }

    // Initialize PositionData
    ctx.accounts.position.initialize(
        ctx.accounts.owner.key(),
        ctx.accounts.pool.key(),
        tick_lower_index,
        tick_upper_index,
        liquidity_amount_desired,
    )?;
    msg!(
        "Position account {} initialized for owner {} in pool {}",
        ctx.accounts.position.key(),
        ctx.accounts.owner.key(),
        ctx.accounts.pool.key()
    );

    // Initialize TickData if they were newly created by init_if_needed
    // A common check is if a field that initialize() sets is still at its Default::default() value.
    // For zero-copy accounts, we need to load_mut() to modify.
    // The check for initialization needs to be done on the loaded data.
    let mut tick_lower_data = ctx.accounts.tick_lower.load_mut()?;
    if tick_lower_data.pool == Pubkey::default() {
        tick_lower_data.initialize(ctx.accounts.pool.key(), tick_lower_index);
        msg!(
            "TickLower account {} initialized for index {}",
            ctx.accounts.tick_lower.to_account_info().key(),
            tick_lower_index
        );
    }
    // Drop tick_lower_data to release the mutable borrow before potentially borrowing tick_upper mutably
    // if they happen to be the same account (though unlikely with different seeds).
    // Or, ensure they are distinct if that's a design constraint.
    // For this case, they are distinct due to different tick_index in seeds.

    let mut tick_upper_data = ctx.accounts.tick_upper.load_mut()?;
    if tick_upper_data.pool == Pubkey::default() {
        tick_upper_data.initialize(ctx.accounts.pool.key(), tick_upper_index);
        msg!(
            "TickUpper account {} initialized for index {}",
            ctx.accounts.tick_upper.to_account_info().key(),
            tick_upper_index
        );
    }
    // tick_lower_data and tick_upper_data go out of scope here, their changes will be written back on drop.

    // Call pool's modify_liquidity logic
    // The liquidity_delta is positive as we are adding liquidity.
    ctx.accounts.pool.modify_liquidity(
        tick_lower_index,
        tick_upper_index,
        liquidity_amount_desired as i128, // Cast u128 to i128
        &ctx.accounts.tick_lower,         // Pass the AccountLoader
        &ctx.accounts.tick_upper,         // Pass the AccountLoader
    )?;
    msg!(
        "Pool liquidity updated. New pool liquidity: {}",
        ctx.accounts.pool.liquidity
    );

    // MVP Simplification: Skip actual token transfers from user to vaults.

    Ok(())
}
