use crate::errors::ErrorCode;
use crate::pool_state::PoolState;
use crate::CollectFees;
/// Collect Fees Instruction Module
///
/// This module implements the instruction for collecting accumulated trading fees
/// from a liquidity position in the Fluxa AMM. As trades occur through a pool,
/// fees accrue to the liquidity providers proportional to their contribution
/// within the active price range.
///
/// The fee collection process calculates the fees earned since the last collection,
/// updates the position's accounting, and transfers the fee tokens to the owner.
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Transfer};

/// Helper function to find the program address and bump for a pool
///
/// This derives the PDA for the pool based on standard Fluxa PDA derivation parameters
fn find_pool_address(
    program_id: &Pubkey,
    token_a_mint: &Pubkey,
    token_b_mint: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"pool", token_a_mint.as_ref(), token_b_mint.as_ref()],
        program_id,
    )
}

/// Handler function for collecting accumulated fees from a position
///
/// This function calculates the fees earned by a position since the last collection,
/// transfers those fees from the pool's vaults to the owner's accounts, and updates
/// the position's accounting to reflect the collection.
///
/// Fees are earned when trades occur through price ranges where the position has
/// provided liquidity, with the amount earned proportional to the position's share
/// of the total liquidity in that range.
///
/// # Arguments
/// * `ctx` - The context containing all accounts involved in the operation
///
/// # Returns
/// * `Result<()>` - Result indicating success or failure
///
/// # Errors
/// * `ErrorCode::NoFeesToCollect` - If there are no fees to collect
/// * `ErrorCode::TransferFailed` - If the fee transfer fails
pub fn handler(ctx: Context<CollectFees>) -> Result<()> {
    // Extract necessary values from the pool before mutable borrow
    let token_a_mint = ctx.accounts.pool.token_a_mint;
    let token_b_mint = ctx.accounts.pool.token_b_mint;
    let pool_key = ctx.accounts.pool.key();

    // Now handle the mutable borrows
    let position = &mut ctx.accounts.position;
    let pool = &mut ctx.accounts.pool;

    // Update position fee tracking to ensure we account for the latest fees
    let mut pool_state = PoolState::new(pool);
    pool_state.update_position_fees(position)?;

    // Get fees to collect
    let tokens_owed_a = position.tokens_owed_a;
    let tokens_owed_b = position.tokens_owed_b;

    // Check if there are any fees to collect
    if tokens_owed_a == 0 && tokens_owed_b == 0 {
        return Err(ErrorCode::NoFeesToCollect.into());
    }

    msg!(
        "Collecting fees: {} token A, {} token B",
        tokens_owed_a,
        tokens_owed_b
    );

    // Reset amounts owed in position tracking
    position.tokens_owed_a = 0;
    position.tokens_owed_b = 0;

    // Get the current program ID
    let program_id = ctx.program_id;

    // Derive PDA and bump for pool signer using the extracted values
    let (_, bump) = find_pool_address(program_id, &token_a_mint, &token_b_mint);

    // Transfer token A fees if any
    if tokens_owed_a > 0 {
        // Create correct PDA signer with derived bump
        let pool_signer_seeds = &[
            b"pool".as_ref(),
            token_a_mint.as_ref(),
            token_b_mint.as_ref(),
            &[bump],
        ];
        let signer_seeds = &[&pool_signer_seeds[..]];

        // Transfer tokens from vault to user account
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_a_vault.to_account_info(),
                    to: ctx.accounts.token_a_account.to_account_info(),
                    authority: ctx.accounts.pool.to_account_info(),
                },
                signer_seeds,
            ),
            tokens_owed_a,
        )
        .map_err(|_| ErrorCode::TransferFailed)?;
    }

    // Transfer token B fees if any
    if tokens_owed_b > 0 {
        // Use the same derived PDA signer with correct bump
        let pool_signer_seeds = &[
            b"pool".as_ref(),
            token_a_mint.as_ref(),
            token_b_mint.as_ref(),
            &[bump],
        ];
        let signer_seeds = &[&pool_signer_seeds[..]];

        // Transfer tokens from vault to user account
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_b_vault.to_account_info(),
                    to: ctx.accounts.token_b_account.to_account_info(),
                    authority: ctx.accounts.pool.to_account_info(),
                },
                signer_seeds,
            ),
            tokens_owed_b,
        )
        .map_err(|_| ErrorCode::TransferFailed)?;
    }

    emit!(FeeCollectionEvent {
        position: position.key(),
        owner: ctx.accounts.owner.key(),
        pool: pool_key,
        token_a_amount: tokens_owed_a,
        token_b_amount: tokens_owed_b,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Event emitted when fees are collected from a position
#[event]
pub struct FeeCollectionEvent {
    /// The position from which fees were collected
    pub position: Pubkey,

    /// The owner who collected the fees
    pub owner: Pubkey,

    /// The pool where the position exists
    pub pool: Pubkey,

    /// The amount of token A fees collected
    pub token_a_amount: u64,

    /// The amount of token B fees collected
    pub token_b_amount: u64,

    /// The timestamp when the collection occurred
    pub timestamp: i64,
}
