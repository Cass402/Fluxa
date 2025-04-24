use crate::constants::{MAX_TICK, MIN_SQRT_PRICE};

/// Swap Instruction Module
///
/// This module implements the core swap functionality for the Fluxa AMM.
/// Swaps allow users to exchange one token for another using the liquidity available
/// in a pool, with the exchange rate determined by the current price and the
/// constant product formula within each active tick range.
///
/// The swap execution updates the pool's price and transfers tokens between
/// the user's accounts and the pool's vaults, collecting fees in the process.
use crate::errors::ErrorCode;
use crate::math::{self, sqrt_price_to_tick};
use crate::pool_state::PoolState;
use crate::Swap;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Transfer};

/// Handler function for executing a token swap
///
/// This function executes a swap between token A and token B, calculating the
/// output amount based on the current pool price and liquidity. It verifies that
/// the resulting output meets the minimum amount required by the user to protect
/// against slippage, and updates the pool state accordingly.
///
/// # Arguments
/// * `ctx` - The context containing all accounts involved in the operation
/// * `amount_in` - The amount of input tokens to swap
/// * `min_amount_out` - The minimum acceptable amount of output tokens
/// * `is_token_a` - Whether the input token is token A (true) or token B (false)
///
/// # Returns
/// * `Result<()>` - Result indicating success or failure
///
/// # Errors
/// * `ErrorCode::SlippageExceeded` - If the output amount is less than min_amount_out
/// * `ErrorCode::InsufficientLiquidity` - If there's not enough liquidity to execute the swap
/// * `ErrorCode::ZeroOutputAmount` - If the calculation results in zero output
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn handler(
    ctx: Context<Swap>,
    amount_in: u64,
    min_amount_out: u64,
    is_token_a: bool,
) -> Result<()> {
    // Validate input parameters
    require!(amount_in > 0, ErrorCode::InsufficientInputAmount);
    require!(min_amount_out > 0, ErrorCode::InsufficientInputAmount);

    // Store the pool key before borrowing pool mutably
    let pool_key = ctx.accounts.pool.key();

    let pool = &mut ctx.accounts.pool;

    // Store protocol fee percentage before creating pool state
    let protocol_fee_bps = pool.protocol_fee;

    // Initialize pool state manager
    let mut pool_state = PoolState::new(pool);

    // Get the pre-swap virtual reserves for price impact calculation
    let pre_swap_virtual_reserves = pool_state.get_virtual_reserves()?;

    // Perform the swap calculation with precise math
    let (amount_out, fee_amount) = execute_swap(&mut pool_state, amount_in, is_token_a)?;

    // Get the post-swap virtual reserves to calculate price impact
    let post_swap_virtual_reserves = pool_state.get_virtual_reserves()?;

    // Verify slippage constraint
    require!(amount_out >= min_amount_out, ErrorCode::SlippageExceeded);
    require!(amount_out > 0, ErrorCode::ZeroOutputAmount);

    // Calculate protocol fee if applicable
    let protocol_fee = if let Some(protocol_fee_account) = &ctx.accounts.protocol_fee_account {
        // Protocol fee is specified in basis points (e.g., 2000 = 20%)
        let protocol_fee_amount = fee_amount * protocol_fee_bps as u64 / 10000;

        // If there's a protocol fee and it's greater than zero, we'll transfer it later
        if protocol_fee_amount > 0 {
            Some((protocol_fee_account, protocol_fee_amount))
        } else {
            None
        }
    } else {
        None
    };

    // Update global fee growth accumulators
    let fee_amount_a = if is_token_a { fee_amount } else { 0 };
    let fee_amount_b = if is_token_a { 0 } else { fee_amount };
    pool_state.update_fees(fee_amount_a, fee_amount_b)?;

    // Emit event with swap details including virtual reserves for tracking
    emit!(SwapEvent {
        pool: pool_key,
        input_token: if is_token_a { 0 } else { 1 },
        input_amount: amount_in,
        output_amount: amount_out,
        fee_amount,
        pre_virtual_reserve_a: pre_swap_virtual_reserves.0,
        pre_virtual_reserve_b: pre_swap_virtual_reserves.1,
        post_virtual_reserve_a: post_swap_virtual_reserves.0,
        post_virtual_reserve_b: post_swap_virtual_reserves.1,
        sqrt_price: pool_state.pool.sqrt_price,
        liquidity: pool_state.pool.liquidity,
    });

    // Transfer tokens from user to pool (input token)
    let (source_token_account, destination_token_vault) = if is_token_a {
        (&ctx.accounts.token_a_account, &ctx.accounts.token_a_vault)
    } else {
        (&ctx.accounts.token_b_account, &ctx.accounts.token_b_vault)
    };

    // Execute transfer of input tokens from user to pool
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: source_token_account.to_account_info(),
                to: destination_token_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount_in,
    )?;

    // Transfer tokens from pool to user (output token)
    let (output_vault, destination_token_account) = if is_token_a {
        (&ctx.accounts.token_b_vault, &ctx.accounts.token_b_account)
    } else {
        (&ctx.accounts.token_a_vault, &ctx.accounts.token_a_account)
    };

    // Execute transfer of output tokens from pool to user
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: output_vault.to_account_info(),
                to: destination_token_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount_out,
    )?;

    // Handle protocol fee transfer if applicable
    if let Some((protocol_fee_account, protocol_fee_amount)) = protocol_fee {
        let protocol_fee_destination = if is_token_a {
            &ctx.accounts.token_a_vault
        } else {
            &ctx.accounts.token_b_vault
        };

        // Transfer protocol fee to the designated account
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: protocol_fee_destination.to_account_info(),
                    to: protocol_fee_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            protocol_fee_amount,
        )?;
    }

    Ok(())
}

/// Executes the core swap calculation and updates pool state
///
/// This internal function handles the detailed swap computation logic including:
/// - Computing output amount based on input and current liquidity
/// - Handling tick crossings if the swap crosses price boundaries
/// - Updating pool state (price and tick)
/// - Calculating fee amount
///
/// # Arguments
/// * `pool_state` - Mutable reference to the pool state manager
/// * `amount_in` - The exact amount of input token being swapped
/// * `is_token_a` - Whether token A is being swapped for token B (true) or vice versa
///
/// # Returns
/// * `Result<(u64, u64)>` - Tuple containing (output_amount, fee_amount), or an error
///
/// # Errors
/// * Various error codes for mathematical or liquidity-related issues
fn execute_swap(
    pool_state: &mut PoolState,
    amount_in: u64,
    is_token_a: bool,
) -> Result<(u64, u64)> {
    // Get current pool state
    let sqrt_price = pool_state.pool.sqrt_price;
    let liquidity = pool_state.pool.liquidity;
    let fee_tier = pool_state.pool.fee_tier;

    // Cannot swap if there's zero liquidity
    require!(liquidity > 0, ErrorCode::InsufficientLiquidity);

    // Calculate fee amount (fee_tier is in basis points, e.g. 3000 = 0.3%)
    let fee_amount = amount_in * fee_tier as u64 / 10000;

    // Amount after fee deduction
    let amount_after_fee = amount_in
        .checked_sub(fee_amount)
        .ok_or(ErrorCode::MathOverflow)?;

    // Track how much has been consumed and how much output has been generated
    let mut amount_remaining = amount_after_fee;
    let mut amount_out: u64 = 0;

    // Current state for iteration
    let mut current_sqrt_price = sqrt_price;
    let mut current_liquidity = liquidity;
    let mut current_tick = pool_state.pool.current_tick;

    // Swap until the entire input amount is consumed or we run out of liquidity
    while amount_remaining > 0 && current_liquidity > 0 {
        // Calculate next tick boundary
        let next_tick = if is_token_a {
            // When swapping A for B (selling A), price decreases
            // Find the next lower initialized tick
            let mut lower_tick = current_tick;

            // Search for the next initialized tick below current
            for (tick_idx, tick) in &pool_state.ticks {
                if *tick_idx < current_tick
                    && tick.initialized
                    && (*tick_idx > lower_tick || lower_tick == current_tick)
                {
                    lower_tick = *tick_idx;
                }
            }

            lower_tick
        } else {
            // When swapping B for A (selling B), price increases
            // Find the next higher initialized tick
            let mut upper_tick = current_tick;

            // Search for the next initialized tick above current
            for (tick_idx, tick) in &pool_state.ticks {
                if *tick_idx > current_tick
                    && tick.initialized
                    && (*tick_idx < upper_tick || upper_tick == current_tick)
                {
                    upper_tick = *tick_idx;
                }
            }

            upper_tick
        };

        // Calculate target sqrt price at the next tick boundary
        let target_sqrt_price = if next_tick != current_tick {
            math::tick_to_sqrt_price(next_tick)?
        } else {
            // If no next initialized tick is found, use extreme price bounds
            if is_token_a {
                // When selling A, price goes down toward zero (but not zero)
                MIN_SQRT_PRICE
            } else {
                // When selling B, price goes up toward infinity (but within safe bounds)
                math::tick_to_sqrt_price(MAX_TICK)?
            }
        };

        // Execute a single swap step to the next price limit
        let (new_sqrt_price, amount_consumed) = math::calculate_swap_step(
            current_sqrt_price,
            current_liquidity,
            amount_remaining,
            is_token_a,
        )?;

        // Calculate output amount for this step
        let step_output = if is_token_a {
            // For A->B swap: Calculate how much B we get
            math::get_amount_b_delta_for_price_range(
                current_liquidity,
                new_sqrt_price,
                current_sqrt_price,
                false, // Round down for output (conservative)
            )? as u64
        } else {
            // For B->A swap: Calculate how much A we get
            math::get_amount_a_delta_for_price_range(
                current_liquidity,
                new_sqrt_price,
                current_sqrt_price,
                false, // Round down for output (conservative)
            )? as u64
        };

        // Accumulate the output amount
        amount_out = amount_out
            .checked_add(step_output)
            .ok_or(ErrorCode::MathOverflow)?;

        // Deduct the consumed amount from the remaining
        amount_remaining = amount_remaining
            .checked_sub(amount_consumed)
            .ok_or(ErrorCode::MathOverflow)?;

        // Update current state for the next iteration
        current_sqrt_price = new_sqrt_price;

        // If we hit a tick boundary, we need to update liquidity and cross the tick
        if (is_token_a && new_sqrt_price <= target_sqrt_price)
            || (!is_token_a && new_sqrt_price >= target_sqrt_price)
        {
            // Find the exact tick for this sqrt_price
            let new_tick = sqrt_price_to_tick(new_sqrt_price)?;

            // Update the pool price and handle tick crossings
            pool_state.update_price(new_sqrt_price, new_tick)?;

            // Update current state for next iteration
            current_tick = new_tick;
            current_liquidity = pool_state.pool.liquidity;
        }
    }

    // Final update of the pool price
    let final_tick = sqrt_price_to_tick(current_sqrt_price)?;
    pool_state.update_price(current_sqrt_price, final_tick)?;

    // Verify invariant: virtual reserves match the constant product formula
    debug_assert!(
        pool_state.verify_constant_product(),
        "Constant product invariant violated after swap"
    );

    Ok((amount_out, fee_amount))
}

/// Event emitted when a swap is executed
#[event]
pub struct SwapEvent {
    /// The pool where the swap was executed
    pub pool: Pubkey,

    /// Which token was input (0 = A, 1 = B)
    pub input_token: u8,

    /// Amount of input tokens
    pub input_amount: u64,

    /// Amount of output tokens
    pub output_amount: u64,

    /// Amount of tokens taken as fee
    pub fee_amount: u64,

    /// Virtual reserve of token A before swap
    pub pre_virtual_reserve_a: u64,

    /// Virtual reserve of token B before swap
    pub pre_virtual_reserve_b: u64,

    /// Virtual reserve of token A after swap
    pub post_virtual_reserve_a: u64,

    /// Virtual reserve of token B after swap
    pub post_virtual_reserve_b: u64,

    /// Final sqrt price after swap
    pub sqrt_price: u128,

    /// Final liquidity after swap
    pub liquidity: u128,
}
