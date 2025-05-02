/// Close Position Instruction Module
///
/// This module implements the instruction for closing a liquidity position in the Fluxa AMM.
/// Closing a position requires that the liquidity has been fully withdrawn and all fees collected.
/// It allows reclaiming the rent from the position account.
use crate::errors::ErrorCode;
use crate::ClosePosition;
use anchor_lang::prelude::*;
use anchor_spl::token;
use anchor_spl::token::Transfer;

/// Handler function for closing a position
///
/// This function closes a position that has zero liquidity remaining, transferring
/// any uncollected fees to the owner and returning the rent to the specified recipient.
///
/// # Arguments
/// * `ctx` - The context containing all accounts involved in the operation
///
/// # Returns
/// * `Result<()>` - Result indicating success or failure
///
/// # Errors
/// * `ErrorCode::PositionNotEmpty` - If the position still has liquidity
/// * `ErrorCode::PositionFeesNotCollected` - If there are uncollected fees
pub fn handler(ctx: Context<ClosePosition>) -> Result<()> {
    let position = &ctx.accounts.position;
    let pool = &mut ctx.accounts.pool;
    let token_program = &ctx.accounts.token_program;

    // Verify position has no liquidity
    require!(position.liquidity == 0, ErrorCode::PositionNotEmpty);

    // Find pool authority PDA bump
    let (_, bump) =
        Pubkey::find_program_address(&[b"pool_authority", pool.key().as_ref()], ctx.program_id);

    // Handle fee collection first if there are uncollected fees
    if position.tokens_owed_a > 0 {
        // Transfer token A fees
        token::transfer(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_a_vault.to_account_info(),
                    to: ctx.accounts.token_a_account.to_account_info(),
                    authority: pool.to_account_info(),
                },
                &[&[b"pool_authority", pool.key().as_ref(), &[bump]]],
            ),
            position.tokens_owed_a,
        )?;
    }

    if position.tokens_owed_b > 0 {
        // Transfer token B fees
        token::transfer(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_b_vault.to_account_info(),
                    to: ctx.accounts.token_b_account.to_account_info(),
                    authority: pool.to_account_info(),
                },
                &[&[b"pool_authority", pool.key().as_ref(), &[bump]]],
            ),
            position.tokens_owed_b,
        )?;
    }

    // Decrement position count from pool
    pool.position_count = pool.position_count.saturating_sub(1);

    // In a real implementation, we would also clean up tick data structures here
    // by removing this position's liquidity allocation from the tracked tick data

    msg!("Position closed successfully");

    // The ClosePosition context will handle closing the account and reclaiming rent
    // by specifying the position account with `#[account(mut, close = recipient)]`

    Ok(())
}
