use anchor_lang::prelude::*;
use anchor_spl::token::{self, Transfer};

use crate::errors::ErrorCode;
use crate::tick::TickData; // Now a zero-copy account
use crate::SwapExactInput;

pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, SwapExactInput<'info>>,
    amount_in: u64,
    amount_out_minimum: u64,
    sqrt_price_limit_q64: u128,
) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    let clock = Clock::get()?;

    // 1. Determine swap direction (zero_for_one) and validate token mints
    let zero_for_one = if ctx.accounts.user_token_in_account.mint == pool.token0_mint {
        require_keys_eq!(
            ctx.accounts.user_token_out_account.mint,
            pool.token1_mint,
            ErrorCode::InvalidOutputMint
        );
        true // Swapping token0 for token1
    } else if ctx.accounts.user_token_in_account.mint == pool.token1_mint {
        require_keys_eq!(
            ctx.accounts.user_token_out_account.mint,
            pool.token0_mint,
            ErrorCode::InvalidOutputMint
        );
        false // Swapping token1 for token0
    } else {
        return err!(ErrorCode::InvalidInputMint);
    };

    // 2. Transfer `amount_in` from user to the appropriate pool vault
    let (user_source_token_account_info, pool_destination_vault_info) = if zero_for_one {
        (
            ctx.accounts.user_token_in_account.to_account_info(),
            ctx.accounts.token0_vault.to_account_info(),
        )
    } else {
        (
            ctx.accounts.user_token_in_account.to_account_info(),
            ctx.accounts.token1_vault.to_account_info(),
        )
    };

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: user_source_token_account_info,
                to: pool_destination_vault_info,
                authority: ctx.accounts.user_authority.to_account_info(),
            },
        ),
        amount_in,
    )?;

    // 3. Collect provided tick loaders
    // The Pool::swap method will need to be adapted to accept these.
    let mut tick_loaders_vec = Vec::new();
    if let Some(ta) = &ctx.accounts.tick_account_0 {
        tick_loaders_vec.push(ta);
    }
    if let Some(ta) = &ctx.accounts.tick_account_1 {
        tick_loaders_vec.push(ta);
    }
    if let Some(ta) = &ctx.accounts.tick_account_2 {
        tick_loaders_vec.push(ta);
    }
    // Convert Vec<&AccountLoader> to &[&AccountLoader] for the call
    let tick_loaders_slice: &[&AccountLoader<'info, TickData>] = &tick_loaders_vec;

    // grab the pool key from your &mut reference
    let pool_key = pool.key();

    // 4. Call the core swap logic in `pool.swap()`
    // IMPORTANT: Pool::swap signature and implementation in pool.rs MUST be updated
    // to accept `amount_specified` as i128, `tick_loaders_slice`, and `current_timestamp`.
    // It should return (amount0_swapped_abs: u128, amount1_swapped_abs: u128).
    let (amount0_swapped_abs, amount1_swapped_abs) = pool.swap(
        zero_for_one,
        amount_in as i128, // As per instruction prompt
        sqrt_price_limit_q64,
        &pool_key,            // Pass the pool's key
        tick_loaders_slice,   // Pass the tick loaders
        clock.unix_timestamp, // Pass current timestamp
    )?;

    // 5. Determine actual `amount_out` and verify against `amount_out_minimum`
    let amount_out_u128 = if zero_for_one {
        amount1_swapped_abs // Output is token1
    } else {
        amount0_swapped_abs // Output is token0
    };

    if amount_out_u128 == 0 {
        return err!(ErrorCode::ZeroOutputAmount);
    }
    require!(
        amount_out_u128 >= amount_out_minimum as u128,
        ErrorCode::SlippageExceeded
    );

    // 6. Transfer `amount_out` from the appropriate pool vault to the user
    let (pool_source_vault_info, user_destination_token_account_info) = if zero_for_one {
        (
            ctx.accounts.token1_vault.to_account_info(), // Output was token1
            ctx.accounts.user_token_out_account.to_account_info(),
        )
    } else {
        (
            ctx.accounts.token0_vault.to_account_info(), // Output was token0
            ctx.accounts.user_token_out_account.to_account_info(),
        )
    };

    let pool_seeds = &[
        b"pool".as_ref(), // Assuming "pool" is the prefix seed
        pool.token0_mint.as_ref(),
        pool.token1_mint.as_ref(),
        &[pool.bump],
    ];
    let signer_seeds = &[&pool_seeds[..]];

    let amount_out_u64 = u64::try_from(amount_out_u128)
        .map_err(|_| error!(ErrorCode::MathOverflow).with_account_name("amount_out_u128"))?;

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: pool_source_vault_info,
                to: user_destination_token_account_info,
                // use your &mut alias here, *not* ctx.accounts.pool
                authority: pool.to_account_info(),
            },
            signer_seeds,
        ),
        amount_out_u64,
    )?;

    Ok(())
}
