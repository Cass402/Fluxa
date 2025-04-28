use crate::errors::ErrorCode;
use crate::pool_state::PoolState;
/// Modify Position Instruction Module
///
/// This module implements instructions for modifying existing liquidity positions
/// in the Fluxa AMM. It supports both increasing and decreasing the liquidity amount
/// of a position without changing its price range.
///
/// Modifications to positions require recalculating token amounts and updating the
/// pool's global liquidity state, ensuring proper accounting of fees and efficient
/// use of provided capital.
use crate::ModifyPosition;
use anchor_lang::prelude::*;
use anchor_spl::token;
use anchor_spl::token::Transfer;

/// Handler function for increasing position liquidity
///
/// This function adds liquidity to an existing position, calculating the required
/// token amounts based on the current pool price and the position's tick range.
/// It transfers the additional tokens from the owner to the pool and updates
/// position and pool state accordingly.
///
/// # Arguments
/// * `ctx` - The context containing all accounts involved in the operation
/// * `liquidity_delta` - The amount of liquidity to add to the position
///
/// # Returns
/// * `Result<()>` - Result indicating success or failure
pub fn increase_handler(ctx: Context<ModifyPosition>, liquidity_delta: u128) -> Result<()> {
    let position = &mut ctx.accounts.position;
    let pool = &mut ctx.accounts.pool;
    let token_program = &ctx.accounts.token_program;

    // Extract necessary values before mutable borrow
    let fee_growth_global_a = pool.fee_growth_global_a;
    let fee_growth_global_b = pool.fee_growth_global_b;

    // First ensure all fees are collected and accounted for
    // Update fees before modifying the position
    let mut pool_state = PoolState::new(pool);

    // Update position's fee accounting (aligns with fee growth global)
    // Calculate fee growth delta
    let fee_growth_delta_a = fee_growth_global_a.wrapping_sub(position.fee_growth_inside_a);
    let fee_growth_delta_b = fee_growth_global_b.wrapping_sub(position.fee_growth_inside_b);

    // Calculate new tokens owed
    if position.liquidity > 0 {
        let delta_a = (position
            .liquidity
            .checked_mul(fee_growth_delta_a)
            .map(|n| n / (1u128 << 64))
            .unwrap_or(0)) as u64;

        let delta_b = (position
            .liquidity
            .checked_mul(fee_growth_delta_b)
            .map(|n| n / (1u128 << 64))
            .unwrap_or(0)) as u64;

        // Add to tokens owed
        position.tokens_owed_a = position.tokens_owed_a.saturating_add(delta_a);
        position.tokens_owed_b = position.tokens_owed_b.saturating_add(delta_b);
    }

    // Update fee growth tracking
    position.fee_growth_inside_a = fee_growth_global_a;
    position.fee_growth_inside_b = fee_growth_global_b;

    // Use the public modify_position method to handle liquidity changes and calculate token amounts
    let (amount_a, amount_b) = pool_state.modify_position(
        position,
        liquidity_delta as i128,
        true, // is_increase = true
    )?;

    msg!(
        "Increasing position by {} liquidity. Token amounts: A={}, B={}",
        liquidity_delta,
        amount_a,
        amount_b
    );

    // Execute token transfers
    if amount_a > 0 {
        token::transfer(
            CpiContext::new(
                token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_a_account.to_account_info(),
                    to: ctx.accounts.token_a_vault.to_account_info(),
                    authority: ctx.accounts.owner.to_account_info(),
                },
            ),
            amount_a,
        )?;
    }

    if amount_b > 0 {
        token::transfer(
            CpiContext::new(
                token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_b_account.to_account_info(),
                    to: ctx.accounts.token_b_vault.to_account_info(),
                    authority: ctx.accounts.owner.to_account_info(),
                },
            ),
            amount_b,
        )?;
    }

    // Note: We don't need to update position state or pool state here
    // because the modify_position method already did that for us

    Ok(())
}

/// Handler function for decreasing position liquidity
///
/// This function removes liquidity from an existing position, calculating the token
/// amounts to return based on the current pool price and the position's tick range.
/// It transfers tokens from the pool to the owner and updates position and pool state.
///
/// # Arguments
/// * `ctx` - The context containing all accounts involved in the operation
/// * `liquidity_delta` - The amount of liquidity to remove from the position
///
/// # Returns
/// * `Result<()>` - Result indicating success or failure
///
/// # Errors
/// * `ErrorCode::InsufficientLiquidity` - If trying to remove more liquidity than exists in the position
pub fn decrease_handler(ctx: Context<ModifyPosition>, liquidity_delta: u128) -> Result<()> {
    let position = &mut ctx.accounts.position;
    let pool = &mut ctx.accounts.pool;
    let token_program = &ctx.accounts.token_program;

    // Validate amount
    require!(
        liquidity_delta <= position.liquidity,
        ErrorCode::InsufficientLiquidity
    );

    // Extract necessary values before mutable borrow
    let fee_growth_global_a = pool.fee_growth_global_a;
    let fee_growth_global_b = pool.fee_growth_global_b;

    // First ensure all fees are collected and accounted for
    // Update fees before modifying the position
    let mut pool_state = PoolState::new(pool);

    // Update position's fee accounting (aligns with fee growth global)
    // Calculate fee growth delta
    let fee_growth_delta_a = fee_growth_global_a.wrapping_sub(position.fee_growth_inside_a);
    let fee_growth_delta_b = fee_growth_global_b.wrapping_sub(position.fee_growth_inside_b);

    // Calculate new tokens owed
    if position.liquidity > 0 {
        let delta_a = (position
            .liquidity
            .checked_mul(fee_growth_delta_a)
            .map(|n| n / (1u128 << 64))
            .unwrap_or(0)) as u64;

        let delta_b = (position
            .liquidity
            .checked_mul(fee_growth_delta_b)
            .map(|n| n / (1u128 << 64))
            .unwrap_or(0)) as u64;

        // Add to tokens owed
        position.tokens_owed_a = position.tokens_owed_a.saturating_add(delta_a);
        position.tokens_owed_b = position.tokens_owed_b.saturating_add(delta_b);
    }

    // Update fee growth tracking
    position.fee_growth_inside_a = fee_growth_global_a;
    position.fee_growth_inside_b = fee_growth_global_b;

    // Use the public modify_position method to handle liquidity changes and calculate token amounts
    let (amount_a, amount_b) = pool_state.modify_position(
        position,
        -(liquidity_delta as i128),
        false, // is_increase = false
    )?;

    msg!(
        "Decreasing position by {} liquidity. Token amounts to return: A={}, B={}",
        liquidity_delta,
        amount_a,
        amount_b
    );

    // Execute token transfers from pool to user
    if amount_a > 0 {
        // Find the authority PDA for the pool
        let (_authority_pda, authority_bump) =
            Pubkey::find_program_address(&[b"pool_authority", pool.key().as_ref()], &crate::ID);

        // Create seeds array for signer derivation with proper lifetime
        let pool_key = pool.key();
        let seeds = [
            b"pool_authority".as_ref(),
            pool_key.as_ref(),
            &[authority_bump],
        ];

        // Use the vault's owner address directly as the authority
        token::transfer(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_a_vault.to_account_info(),
                    to: ctx.accounts.token_a_account.to_account_info(),
                    authority: pool.to_account_info(),
                },
                &[&seeds[..]],
            ),
            amount_a,
        )?;
    }

    if amount_b > 0 {
        // Find the authority PDA for the pool
        let (_authority_pda, authority_bump) =
            Pubkey::find_program_address(&[b"pool_authority", pool.key().as_ref()], &crate::ID);

        // Create seeds array for signer derivation with proper lifetime
        let pool_key = pool.key();
        let seeds = [
            b"pool_authority".as_ref(),
            pool_key.as_ref(),
            &[authority_bump],
        ];

        // Use the vault's owner address directly as the authority
        token::transfer(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_b_vault.to_account_info(),
                    to: ctx.accounts.token_b_account.to_account_info(),
                    authority: pool.to_account_info(),
                },
                &[&seeds[..]],
            ),
            amount_b,
        )?;
    }

    // If position is now empty, suggest using close_position instruction
    if position.liquidity == 0 {
        msg!("Position has zero liquidity, consider using close_position instruction to reclaim rent");
    }

    Ok(())
}
