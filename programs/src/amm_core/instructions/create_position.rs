/// Create Position Instruction Module
///
/// This module implements the instruction for creating a new liquidity position in a pool
/// within the Fluxa AMM. A position represents concentrated liquidity provided within a
/// specific price range, allowing for efficient capital utilization.
///
/// The creation process initializes a new position account and transfers the appropriate
/// token amounts to the pool vaults based on the current price and specified price range.
use crate::pool_state::PoolState;
use crate::CreatePosition;
use anchor_lang::prelude::*;
use anchor_spl::token;
use anchor_spl::token::Transfer;

/// Handler function for creating a new liquidity position
///
/// This function initializes a new position with the specified parameters, calculates
/// the required token amounts based on the current pool price, and transfers those tokens
/// to the pool's vaults. It also updates the pool's global liquidity and position count.
///
/// # Arguments
/// * `ctx` - The context containing all accounts involved in the operation
/// * `lower_tick` - The lower tick bound of the position (inclusive)
/// * `upper_tick` - The upper tick bound of the position (exclusive)
/// * `liquidity_amount` - The amount of liquidity to provide (in L-units)
///
/// # Returns
/// * `Result<()>` - Result indicating success or failure
///
/// # Errors
/// * `ErrorCode::InvalidTickRange` - If the tick range is invalid (e.g., lower >= upper)
pub fn handler(
    ctx: Context<CreatePosition>,
    lower_tick: i32,
    upper_tick: i32,
    liquidity_amount: u128,
) -> Result<()> {
    let position = &mut ctx.accounts.position;
    let pool = &mut ctx.accounts.pool;
    let token_program = &ctx.accounts.token_program;

    // Initialize position owner and pool reference
    position.owner = ctx.accounts.owner.key();
    position.pool = pool.key();

    // Create pool state manager for handling the concentrated liquidity logic
    let mut pool_state = PoolState::new(pool);

    // Use pool state manager to create the position and calculate token amounts
    let (amount_a, amount_b) =
        pool_state.create_position(position, lower_tick, upper_tick, liquidity_amount)?;

    msg!(
        "Creating position with {} liquidity in range [{}, {}]. Token amounts: A={}, B={}",
        liquidity_amount,
        lower_tick,
        upper_tick,
        amount_a,
        amount_b
    );

    // Transfer token A if needed
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

    // Transfer token B if needed
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

    // Increment position count in pool
    pool.position_count = pool.position_count.checked_add(1).unwrap_or_else(|| {
        msg!("Warning: Position count overflow");
        u64::MAX
    });

    msg!(
        "Position created successfully. Total pool liquidity: {}",
        pool.liquidity
    );

    Ok(())
}
