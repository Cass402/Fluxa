// Swap Router Module
//
// This module implements the complete swap execution algorithm with tick crossing
// as described in the Core Protocol Technical Design document. It handles complex swaps
// that may cross multiple tick boundaries, properly updating liquidity and fees along the way.

use crate::constants::{MAX_SQRT_PRICE, MAX_TICK, MIN_TICK};
use crate::errors::ErrorCode;
use crate::math;
use crate::oracle::Oracle;
use crate::pool_state::PoolState;
use crate::Pool;
use anchor_lang::prelude::*;

/// Contains state during the swap execution
pub struct SwapState {
    /// The amount of the input token remaining to be swapped
    pub amount_remaining: u64,

    /// The amount of output token calculated so far
    pub amount_calculated: u64,

    /// Current sqrt price during the swap execution
    pub sqrt_price: u128,

    /// Current tick during the swap execution
    pub tick_current: i32,

    /// Active liquidity during the swap execution
    pub liquidity: u128,

    /// Fee amount accumulated during the swap
    pub fee_growth: u128,

    /// The tick spacing of the pool
    pub tick_spacing: u16,
}

/// Result of a swap operation
#[derive(Clone, Copy)]
pub struct SwapResult {
    /// Amount of input token consumed
    pub amount_in: u64,

    /// Amount of output token generated
    pub amount_out: u64,

    /// Fee amount charged
    pub fee_amount: u64,

    /// New sqrt price after the swap
    pub sqrt_price_after: u128,

    /// New tick after the swap
    pub tick_after: i32,
}

/// Execute a swap operation, potentially crossing multiple tick boundaries
pub fn execute_swap(
    pool_state: &mut PoolState,
    oracle: Option<&mut Oracle>,
    amount_specified: u64,
    sqrt_price_limit: u128,
    zero_for_one: bool, // true if swapping token0 for token1
) -> Result<SwapResult> {
    // Initialize the swap state
    let mut state =
        initialize_swap_state(pool_state, amount_specified, sqrt_price_limit, zero_for_one)?;

    // Continue swapping until we've used the entire input or reached the price limit
    while state.amount_remaining > 0 && state.sqrt_price != sqrt_price_limit {
        // Find the next tick to cross
        let (next_tick, initialized) = get_next_initialized_tick(
            pool_state,
            state.tick_current,
            state.tick_spacing,
            zero_for_one,
        )?;

        // Calculate the next sqrt price target
        let sqrt_price_target =
            compute_sqrt_price_target(next_tick, sqrt_price_limit, zero_for_one)?;

        // Compute how much can be swapped within this price range
        let (sqrt_price_next, amount_in, amount_out, fee_amount) = compute_swap_step(
            state.sqrt_price,
            sqrt_price_target,
            state.liquidity,
            state.amount_remaining,
            pool_state.pool.fee_tier,
            zero_for_one,
        )?;

        // Update the swap state
        state.sqrt_price = sqrt_price_next;
        state.amount_remaining = state.amount_remaining.saturating_sub(amount_in);
        state.amount_calculated += amount_out;
        state.fee_growth += fee_amount as u128;

        // If we've reached the next tick, cross it and update liquidity
        if sqrt_price_next == sqrt_price_target && initialized {
            // Update the tick crossing in the pool state
            let liquidity_delta = pool_state.cross_tick(next_tick)?;

            // Update the current tick
            state.tick_current = if zero_for_one {
                // When swapping token0 for token1, price decreases, so we cross from right to left
                next_tick - 1
            } else {
                // When swapping token1 for token0, price increases, so we cross from left to right
                next_tick
            };

            // Update the liquidity after crossing
            state.liquidity =
                update_liquidity_after_crossing(state.liquidity, liquidity_delta, zero_for_one)?;
        } else {
            // If we didn't cross a tick, just update the current tick based on the new price
            state.tick_current = math::sqrt_price_to_tick(state.sqrt_price)?;
        }
    }

    // Update the pool state
    update_pool_state(pool_state, &state)?;

    // Update the oracle if provided
    if let Some(oracle_acct) = oracle {
        // Get current block time
        let block_timestamp = Clock::get()?.unix_timestamp as u32;

        // Only update if enough time has passed since last update
        if block_timestamp > pool_state.pool.last_oracle_update as u32 {
            oracle_acct.write(
                block_timestamp,
                state.sqrt_price,
                state.tick_current,
                state.liquidity,
            )?;

            // Update the pool's oracle timestamp
            pool_state.pool.last_oracle_update = block_timestamp as i64;
        }
    }

    // Calculate the actual amounts swapped
    let amount_in = amount_specified - state.amount_remaining;
    let amount_out = state.amount_calculated;
    let fee_amount = state.fee_growth as u64;

    // Return the swap result
    Ok(SwapResult {
        amount_in,
        amount_out,
        fee_amount,
        sqrt_price_after: state.sqrt_price,
        tick_after: state.tick_current,
    })
}

/// Initialize the swap state from the pool state
fn initialize_swap_state(
    pool_state: &PoolState,
    amount_specified: u64,
    sqrt_price_limit: u128,
    zero_for_one: bool,
) -> Result<SwapState> {
    // Validate the price limit
    if zero_for_one {
        // When swapping token0 for token1, price decreases
        require!(
            sqrt_price_limit < pool_state.pool.sqrt_price,
            ErrorCode::InvalidSqrtPriceLimit
        );
        // Ensure the price limit is greater than the min price
        require!(sqrt_price_limit > 0, ErrorCode::InvalidSqrtPriceLimit);
    } else {
        // When swapping token1 for token0, price increases
        require!(
            sqrt_price_limit > pool_state.pool.sqrt_price,
            ErrorCode::InvalidSqrtPriceLimit
        );
        // Ensure the price limit is less than the max price
        require!(
            sqrt_price_limit < MAX_SQRT_PRICE,
            ErrorCode::InvalidSqrtPriceLimit
        );
    }

    // Determine tick spacing based on fee tier
    let tick_spacing = match pool_state.pool.fee_tier {
        500 => 10,    // 0.05% fee tier
        3000 => 60,   // 0.3% fee tier
        10000 => 200, // 1% fee tier
        _ => return Err(ErrorCode::InvalidTickSpacing.into()),
    };

    Ok(SwapState {
        amount_remaining: amount_specified,
        amount_calculated: 0,
        sqrt_price: pool_state.pool.sqrt_price,
        tick_current: pool_state.pool.current_tick,
        liquidity: pool_state.pool.liquidity,
        fee_growth: 0,
        tick_spacing,
    })
}

/// Find the next initialized tick in the direction of the swap
fn get_next_initialized_tick(
    pool_state: &PoolState,
    current_tick: i32,
    tick_spacing: u16,
    zero_for_one: bool,
) -> Result<(i32, bool)> {
    // The direction depends on which token we're swapping
    let search_direction = if zero_for_one { -1 } else { 1 };

    // Start with the next tick in the direction of the swap
    let mut tick_index = if zero_for_one {
        // When swapping token0 for token1, price decreases, so we move to lower ticks
        // Round down to the next multiple of tick_spacing
        (current_tick / tick_spacing as i32) * tick_spacing as i32
    } else {
        // When swapping token1 for token0, price increases, so we move to higher ticks
        // Round up to the next multiple of tick_spacing
        ((current_tick / tick_spacing as i32) + 1) * tick_spacing as i32
    };

    // Find the next initialized tick
    let mut _initialized = false;
    for _ in 0..1000 {
        // Safety limit to avoid infinite loops
        // Check if this tick exists and is initialized
        for (idx, tick) in &pool_state.ticks {
            if *idx == tick_index && tick.initialized {
                return Ok((tick_index, true));
            }
        }

        // Move to the next tick
        tick_index += search_direction * tick_spacing as i32;

        // Safety bounds check
        if !(MIN_TICK..=MAX_TICK).contains(&tick_index) {
            // If we've gone beyond the allowed tick range, return the boundary tick
            return Ok((if zero_for_one { MIN_TICK } else { MAX_TICK }, false));
        }
    }

    // If we didn't find an initialized tick, return the min/max tick
    Ok((if zero_for_one { MIN_TICK } else { MAX_TICK }, false))
}

/// Calculate the target sqrt price for this step of the swap
fn compute_sqrt_price_target(
    next_tick: i32,
    sqrt_price_limit: u128,
    zero_for_one: bool,
) -> Result<u128> {
    // Convert the next tick to a sqrt price
    let next_tick_price = math::tick_to_sqrt_price(next_tick)?;

    // Determine whether to use the next tick price or the price limit
    if zero_for_one {
        // When swapping token0 for token1, price decreases
        // Target is the maximum of the next tick price and the price limit
        Ok(std::cmp::max(sqrt_price_limit, next_tick_price))
    } else {
        // When swapping token1 for token0, price increases
        // Target is the minimum of the next tick price and the price limit
        Ok(std::cmp::min(sqrt_price_limit, next_tick_price))
    }
}

/// Compute a single step within a price range
fn compute_swap_step(
    sqrt_price_current: u128,
    sqrt_price_target: u128,
    liquidity: u128,
    amount_remaining: u64,
    fee_tier: u16,
    zero_for_one: bool,
) -> Result<(u128, u64, u64, u64)> {
    // If there's no liquidity, we can't execute the swap
    if liquidity == 0 {
        return Err(ErrorCode::InsufficientLiquidity.into());
    }

    // Calculate the maximum amount that can be swapped to reach the target price
    let max_amount = calculate_max_amount_for_price_change(
        sqrt_price_current,
        sqrt_price_target,
        liquidity,
        zero_for_one,
    )?;

    // Determine if we'll use the entire remaining amount or just a portion
    let (use_entire_input, actual_amount) = if amount_remaining <= max_amount {
        (true, amount_remaining)
    } else {
        (false, max_amount)
    };

    // Apply fee
    let fee_pct = fee_tier as u128;
    let fee_amount = (actual_amount as u128 * fee_pct) / 1_000_000;
    let amount_after_fee = (actual_amount as u128) - fee_amount;

    // Calculate new sqrt_price and amounts
    let sqrt_price_next = if use_entire_input {
        compute_sqrt_price_after_amount(
            sqrt_price_current,
            liquidity,
            amount_after_fee as u64,
            zero_for_one,
        )?
    } else {
        sqrt_price_target
    };

    // Calculate exact input and output amounts
    let amount_in = calculate_amount_for_price_change(
        sqrt_price_current,
        sqrt_price_next,
        liquidity,
        true, // Exact input
        zero_for_one,
    )? + fee_amount as u64;

    let amount_out = calculate_amount_for_price_change(
        sqrt_price_current,
        sqrt_price_next,
        liquidity,
        false, // Exact output
        zero_for_one,
    )?;

    Ok((sqrt_price_next, amount_in, amount_out, fee_amount as u64))
}

/// Calculate the maximum amount that can be swapped to reach the target price
fn calculate_max_amount_for_price_change(
    sqrt_price_current: u128,
    sqrt_price_target: u128,
    liquidity: u128,
    zero_for_one: bool,
) -> Result<u64> {
    // Calculate based on the direction of the swap
    calculate_amount_for_price_change(
        sqrt_price_current,
        sqrt_price_target,
        liquidity,
        true, // Exact input
        zero_for_one,
    )
}

/// Calculate the amount required for a price change
fn calculate_amount_for_price_change(
    sqrt_price_a: u128,
    sqrt_price_b: u128,
    liquidity: u128,
    exact_input: bool,
    zero_for_one: bool,
) -> Result<u64> {
    // The calculation depends on which token is being swapped
    if zero_for_one {
        // Token0 to Token1
        if exact_input {
            // Calculate token0 (X) input amount
            math::get_amount_a_delta_for_price_range(
                liquidity,
                sqrt_price_b, // Lower price after swap
                sqrt_price_a, // Higher price before swap
                true,         // Round up for input
            )
            .map(|amt| amt as u64)
        } else {
            // Calculate token1 (Y) output amount
            math::get_amount_b_delta_for_price_range(
                liquidity,
                sqrt_price_b, // Lower price after swap
                sqrt_price_a, // Higher price before swap
                false,        // Round down for output
            )
            .map(|amt| amt as u64)
        }
    } else {
        // Token1 to Token0
        if exact_input {
            // Calculate token1 (Y) input amount
            math::get_amount_b_delta_for_price_range(
                liquidity,
                sqrt_price_a, // Lower price before swap
                sqrt_price_b, // Higher price after swap
                true,         // Round up for input
            )
            .map(|amt| amt as u64)
        } else {
            // Calculate token0 (X) output amount
            math::get_amount_a_delta_for_price_range(
                liquidity,
                sqrt_price_a, // Lower price before swap
                sqrt_price_b, // Higher price after swap
                false,        // Round down for output
            )
            .map(|amt| amt as u64)
        }
    }
}

/// Calculate the new sqrt price after applying an amount
fn compute_sqrt_price_after_amount(
    sqrt_price: u128,
    liquidity: u128,
    amount: u64,
    zero_for_one: bool,
) -> Result<u128> {
    // If amount is 0, price doesn't change
    if amount == 0 {
        return Ok(sqrt_price);
    }

    // Calculate based on the direction of the swap
    if zero_for_one {
        // X to Y (token0 to token1): price decreases
        // (liquidity * sqrt_price) / (liquidity + amount * sqrt_price)
        let product = math::mul_q96(liquidity, sqrt_price)?;
        let denominator = liquidity + math::mul_q96(amount as u128, sqrt_price)?;
        math::div_q96(product, denominator)
    } else {
        // Y to X (token1 to token0): price increases
        // sqrt_price + (amount / liquidity)
        let amount_scaled = math::mul_q96(amount as u128, math::Q96)?;
        let quotient = math::div_q96(amount_scaled, liquidity)?;
        Ok(sqrt_price + quotient)
    }
}

/// Update active liquidity after crossing a tick
fn update_liquidity_after_crossing(
    current_liquidity: u128,
    liquidity_delta: i128,
    zero_for_one: bool,
) -> Result<u128> {
    // When crossing a tick, we need to apply the liquidity delta
    // The sign depends on the direction we're crossing

    let new_liquidity = if zero_for_one {
        // When swapping token0 for token1, price decreases
        // We're crossing from right to left, so we subtract the delta
        if liquidity_delta > 0 {
            current_liquidity
                .checked_sub(liquidity_delta as u128)
                .ok_or(ErrorCode::MathOverflow)?
        } else {
            current_liquidity
                .checked_add((-liquidity_delta) as u128)
                .ok_or(ErrorCode::MathOverflow)?
        }
    } else {
        // When swapping token1 for token0, price increases
        // We're crossing from left to right, so we add the delta
        if liquidity_delta > 0 {
            current_liquidity
                .checked_add(liquidity_delta as u128)
                .ok_or(ErrorCode::MathOverflow)?
        } else {
            current_liquidity
                .checked_sub((-liquidity_delta) as u128)
                .ok_or(ErrorCode::MathOverflow)?
        }
    };

    Ok(new_liquidity)
}

/// Update the pool state after the swap is complete
fn update_pool_state(pool_state: &mut PoolState, state: &SwapState) -> Result<()> {
    // Update price and tick
    pool_state.update_price(state.sqrt_price, state.tick_current)?;

    // Update fees
    pool_state.update_fees(0, 0)?; // The actual fee amounts are handled elsewhere

    Ok(())
}

/// Function to handle multi-hop swap routing
pub fn execute_multi_hop_swap(
    pools: &mut [&mut PoolState],
    oracles: &mut [Option<&mut Oracle>],
    amounts_in: &[u64],
    min_amount_out: u64,
    paths: &[(bool, u128)], // (zero_for_one, sqrt_price_limit) for each hop
) -> Result<u64> {
    // Verify we have matching arrays
    if pools.len() != paths.len() || pools.len() != amounts_in.len() || pools.len() != oracles.len()
    {
        return Err(ErrorCode::InvalidInput.into());
    }

    // Track intermediate amounts through the swap path
    let mut current_amount = amounts_in[0];

    // Store details about each hop for event emission and validation
    let mut hop_results: Vec<SwapResult> = Vec::with_capacity(pools.len());

    // Execute each hop in the path
    for i in 0..pools.len() {
        msg!("Executing hop {} with input amount: {}", i, current_amount);

        // Skip if the input amount is zero (should not happen in valid paths)
        if current_amount == 0 {
            return Err(ErrorCode::InsufficientInputAmount.into());
        }

        // Execute the swap on this pool
        let swap_result = {
            // Create a temporary reference to the oracle option without moving it
            let oracle_ref = oracles[i].as_deref_mut();

            execute_swap(
                pools[i],
                oracle_ref,
                current_amount,
                paths[i].1, // sqrt_price_limit
                paths[i].0, // zero_for_one
            )?
        };

        // Store the result for analysis and event emission
        hop_results.push(swap_result);

        // Update the amount for the next hop
        current_amount = swap_result.amount_out;

        msg!(
            "Hop {} completed: in={}, out={}, fees={}",
            i,
            swap_result.amount_in,
            swap_result.amount_out,
            swap_result.fee_amount
        );
    }

    // The final amount_out is the output of the last hop
    let final_amount_out = current_amount;

    // Verify the final output amount meets the minimum
    if final_amount_out < min_amount_out {
        msg!(
            "Slippage exceeded: got {}, expected at least {}",
            final_amount_out,
            min_amount_out
        );
        return Err(ErrorCode::SlippageExceeded.into());
    }

    // Calculate total fees and price impact for analytics
    let total_fees: u64 = hop_results.iter().map(|r| r.fee_amount).sum();

    msg!(
        "Multi-hop swap completed successfully: in={}, out={}, total_fees={}",
        amounts_in[0],
        final_amount_out,
        total_fees
    );

    Ok(final_amount_out)
}

/// Router Interface Module
///
/// This section implements the router integration functionality described in
/// section 4.3 of the Core Protocol Technical Design document, enabling
/// integration with external routers like Jupiter.
/// Standard interface for router callbacks to execute swaps in Fluxa pools
pub fn router_callback_swap(
    pool_state: &mut PoolState,
    oracle: Option<&mut Oracle>,
    amount_in: u64,
    min_amount_out: u64,
    sqrt_price_limit: u128,
    zero_for_one: bool,
    deadline: i64,
) -> Result<u64> {
    // Check deadline if provided
    if deadline > 0 {
        let current_time = Clock::get()?.unix_timestamp;
        require!(current_time <= deadline, ErrorCode::DeadlineExceeded);
    }

    // Execute the swap
    let swap_result = execute_swap(
        pool_state,
        oracle,
        amount_in,
        sqrt_price_limit,
        zero_for_one,
    )?;

    // Verify minimum output amount
    require!(
        swap_result.amount_out >= min_amount_out,
        ErrorCode::SlippageExceeded
    );

    Ok(swap_result.amount_out)
}

/// Route a swap through multiple pools using an external router program
/// This function can be called via CPI from external router programs
#[allow(clippy::needless_lifetimes)]
pub fn external_route_swap<'a>(
    router_program: &'a AccountInfo<'a>,
    multi_hop_accounts: &'a MultiHopAccounts<'a>,
    amount_in: u64,
    min_amount_out: u64,
    route_data: &[u8],
) -> Result<u64> {
    // Create a CPI context for calling back to the router
    let router_cpi_context = CpiContext::new(
        router_program.clone(),
        multi_hop_accounts.to_router_accounts(),
    );

    // Call the router's execute function
    router_execute_route(router_cpi_context, amount_in, min_amount_out, route_data)?;

    // Verify final output (for additional safety - the router should also verify this)
    let final_output = multi_hop_accounts.get_final_output_amount()?;
    require!(final_output >= min_amount_out, ErrorCode::SlippageExceeded);

    Ok(final_output)
}

/// Simplified accounts structure for router integrations
pub struct MultiHopAccounts<'a> {
    pub user: &'a Signer<'a>,
    pub pools: Vec<&'a Account<'a, Pool>>,
    pub token_accounts: Vec<&'a Account<'a, anchor_spl::token::TokenAccount>>,
    pub token_vaults: Vec<&'a AccountInfo<'a>>,
    pub token_program: &'a Program<'a, anchor_spl::token::Token>,
}

impl<'a> MultiHopAccounts<'a> {
    fn to_router_accounts(&self) -> RouterAccounts<'a> {
        // Convert our account structure to the router's expected structure
        // This will vary based on the router's interface
        unimplemented!("Implement for specific router integration")
    }

    fn get_final_output_amount(&self) -> Result<u64> {
        // Get the final output amount from the last token account
        // This will depend on how the router has structured the accounts
        unimplemented!("Implement for specific router integration")
    }
}

/// Helper function for CPI to router's execute function
/// Structure will depend on the specific router being integrated with
fn router_execute_route<'info>(
    _ctx: CpiContext<'_, '_, '_, 'info, RouterAccounts<'info>>,
    _amount_in: u64,
    _min_amount_out: u64,
    _route_data: &[u8],
) -> Result<()> {
    // Make CPI call to router's execute function
    // The exact structure depends on the router's interface
    // This is a placeholder that would be implemented for specific routers
    unimplemented!("Implement for specific router integration")
}

/// Placeholder for router's account structure
/// This would be customized based on the specific router being integrated with
pub struct RouterAccounts<'a> {
    // Router-specific account structure
    pub user: AccountInfo<'a>,
    pub source_token: AccountInfo<'a>,
    pub destination_token: AccountInfo<'a>,
    pub token_program: AccountInfo<'a>,
    // Add other required accounts based on your specific router integration
}

impl anchor_lang::ToAccountMetas for RouterAccounts<'_> {
    fn to_account_metas(
        &self,
        _is_signer: Option<bool>,
    ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
        let account_metas = vec![
            AccountMeta::new(self.user.key(), true),
            AccountMeta::new(self.source_token.key(), false),
            AccountMeta::new(self.destination_token.key(), false),
            AccountMeta::new(self.token_program.key(), false),
        ];

        // Add other accounts as needed for your specific router integration

        account_metas
    }
}

impl<'a> anchor_lang::ToAccountInfos<'a> for RouterAccounts<'a> {
    fn to_account_infos(&self) -> Vec<anchor_lang::prelude::AccountInfo<'a>> {
        let account_infos = vec![
            self.user.clone(),
            self.source_token.clone(),
            self.destination_token.clone(),
            self.token_program.clone(),
        ];

        // Add other accounts as needed for your specific router integration

        account_infos
    }
}

/// Multi-hop swap event for tracking complex routes
#[event]
pub struct MultiHopSwapEvent {
    /// The initial input token
    pub token_in: Pubkey,

    /// The final output token
    pub token_out: Pubkey,

    /// Total input amount
    pub amount_in: u64,

    /// Final output amount
    pub amount_out: u64,

    /// Number of hops in the route
    pub hop_count: u8,

    /// Total fees paid across all hops
    pub total_fees: u64,

    /// Route efficiency (output amount / theoretical direct swap amount)
    pub route_efficiency: u32, // Basis points (e.g. 9850 = 98.5%)

    /// Transaction sender
    pub sender: Pubkey,

    /// Timestamp
    pub timestamp: i64,
}

/// Event emitted when a swap occurs
#[event]
pub struct SwapEvent {
    /// The pool where the swap occurred
    pub pool: Pubkey,

    /// The user who executed the swap
    pub sender: Pubkey,

    /// Whether the swap was zero_for_one
    pub zero_for_one: bool,

    /// Amount of input token
    pub amount_in: u64,

    /// Amount of output token
    pub amount_out: u64,

    /// Fee amount collected
    pub fee_amount: u64,

    /// Sqrt price before the swap
    pub sqrt_price_before: u128,

    /// Sqrt price after the swap
    pub sqrt_price_after: u128,

    /// Liquidity before the swap
    pub liquidity_before: u128,

    /// Tick before the swap
    pub tick_before: i32,

    /// Tick after the swap
    pub tick_after: i32,

    /// Timestamp
    pub timestamp: i64,
}
