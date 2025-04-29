// Multi-Hop Swap Instruction Handler
//
// This module implements the multi-hop swap functionality described in section 4.3 of
// the Core Protocol Technical Design. It allows users to execute complex swaps across
// multiple pools in a single transaction, enabling more efficient trading paths.

use crate::errors::ErrorCode;
use crate::oracle::Oracle;
use crate::pool_state::PoolState;
use crate::swap_router::{execute_multi_hop_swap, MultiHopSwapEvent, SwapEvent};
use crate::MultiHopSwap;
use crate::Pool;
use anchor_lang::prelude::*;
use anchor_spl::token;

/// Represents a single hop in a multi-hop swap route
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct SwapRoute {
    /// Index of the pool in the provided pools array
    pub pool_index: u8,

    /// Whether to swap token A for token B (true) or token B for token A (false)
    pub zero_for_one: bool,

    /// Square root price limit for this swap hop
    pub sqrt_price_limit: u128,
}

/// Handler function for the multi_hop_swap instruction
pub fn handler<'a, 'b, 'c: 'info, 'info>(
    ctx: Context<'a, 'b, 'c, 'info, MultiHopSwap<'info>>,
    amount_in: u64,
    min_amount_out: u64,
    routes: Vec<SwapRoute>,
) -> Result<()> {
    // Validate input parameters
    require!(!routes.is_empty(), ErrorCode::InvalidInput);
    require!(amount_in > 0, ErrorCode::InsufficientInputAmount);
    require!(min_amount_out > 0, ErrorCode::InsufficientInputAmount);

    // Get remaining accounts
    let remaining_accounts = ctx.remaining_accounts;
    let num_routes = routes.len();

    // Calculate required account counts
    let num_pools = num_routes;
    let num_token_accounts = num_routes + 1; // Input, intermediates, and output
    let num_token_vaults = num_routes * 2; // Each pool has 2 vaults (A and B)

    // Validate that we have enough remaining accounts
    let min_remaining_accounts = num_pools + num_token_accounts + num_token_vaults;
    require!(
        remaining_accounts.len() >= min_remaining_accounts,
        ErrorCode::InvalidInput
    );

    // Split remaining accounts into their respective types
    let mut account_index = 0;

    // Extract and process all accounts
    let mut extracted_data = extract_accounts(
        remaining_accounts,
        num_pools,
        num_token_accounts,
        num_token_vaults,
        &mut account_index,
    )?;

    // Record the timestamp for event emission
    let timestamp = Clock::get()?.unix_timestamp;

    // Set up the amount_in vector (only first hop has input amount)
    let mut amounts_in = vec![0u64; num_routes];
    amounts_in[0] = amount_in;

    // Map routes to format expected by the swap router
    let mut paths = Vec::with_capacity(num_routes);
    for route in &routes {
        paths.push((route.zero_for_one, route.sqrt_price_limit));
    }

    // Create the mutable references to PoolState and Oracle that execute_multi_hop_swap expects
    let mut pool_state_refs: Vec<&mut PoolState> = Vec::with_capacity(num_routes);
    let mut oracle_refs: Vec<Option<&mut Oracle>> = Vec::with_capacity(num_routes);

    // SAFETY: We're carefully managing the raw pointers to avoid multiple mutable references
    // to the same data. Each pointer will be dereferenced exactly once.
    unsafe {
        // Create references from the raw pointers
        for ptr in &extracted_data.pool_state_refs {
            pool_state_refs.push(&mut **ptr);
        }

        // Create references for oracles if they exist
        for maybe_ptr in &extracted_data.oracle_refs {
            if let Some(ptr) = maybe_ptr {
                oracle_refs.push(Some(&mut **ptr));
            } else {
                oracle_refs.push(None);
            }
        }
    }

    // Execute the multi-hop swap - this will update all pool states atomically
    let final_amount_out = execute_multi_hop_swap(
        &mut pool_state_refs,
        &mut oracle_refs,
        &amounts_in,
        min_amount_out,
        &paths,
    )?;

    // === Token Transfer Execution ===

    // First transfer: from user's input account to the first pool's vault
    let first_route = &routes[0];
    let first_pool_idx = first_route.pool_index as usize;

    // Determine which vault to send to based on zero_for_one flag
    let first_vault_idx = if first_route.zero_for_one {
        // If swapping token0 for token1, use token0 vault
        first_pool_idx * 2
    } else {
        // If swapping token1 for token0, use token1 vault
        first_pool_idx * 2 + 1
    };

    // Transfer tokens from user to first pool's vault
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: extracted_data.token_accounts[0].clone(),
                to: extracted_data.token_vaults[first_vault_idx].clone(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount_in,
    )?;

    // === Handle Virtual Transfers ===
    // For intermediate hops, actual tokens are virtually transferred between pools
    // This is done by updating the pool internal accounting
    if num_routes > 1 {
        msg!(
            "Processing {} intermediate virtual transfers",
            num_routes - 1
        );
        // Virtual transfers are handled in the swap router execution
        // The output of each hop becomes the input for the next
    }

    // === Final Transfer: Last Pool to User ===

    // Last transfer: from last pool's vault to user's output account
    let last_route = &routes[num_routes - 1];
    let last_pool_idx = last_route.pool_index as usize;

    // Determine which vault to receive from based on zero_for_one flag
    let last_vault_idx = if last_route.zero_for_one {
        // If swapping token0 for token1, output comes from token1 vault
        last_pool_idx * 2 + 1
    } else {
        // If swapping token0 for token0, output comes from token0 vault
        last_pool_idx * 2
    };

    // Get the pool key for authority derivation
    let pool_key = extracted_data.pools[last_pool_idx].key();

    // Find PDA for the pool authority
    let (_, authority_bump) =
        Pubkey::find_program_address(&[b"pool_authority", pool_key.as_ref()], &crate::ID);

    // Create seeds array for signer derivation
    let seeds = [
        b"pool_authority".as_ref(),
        pool_key.as_ref(),
        &[authority_bump],
    ];

    // Transfer output tokens from last pool's vault to user
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: extracted_data.token_vaults[last_vault_idx].clone(),
                to: extracted_data.token_accounts[num_routes].clone(),
                authority: extracted_data.pools[last_pool_idx].to_account_info(),
            },
            &[&seeds[..]],
        ),
        final_amount_out,
    )?;

    // === State Updates ===

    // Update the oracle accounts if provided
    for (i, oracle_opt) in extracted_data.oracle_accounts.iter_mut().enumerate() {
        if let Some(oracle) = oracle_opt {
            let pool = &extracted_data.pools[i];
            oracle.write(
                Clock::get()?.unix_timestamp as u32,
                pool.sqrt_price,
                pool.current_tick,
                pool.liquidity,
            )?;
        }
    }

    // === Event Emission ===
    emit_swap_events(
        &ctx,
        &routes,
        &extracted_data,
        amount_in,
        final_amount_out,
        timestamp,
    );

    Ok(())
}

/// Get information about the best routes for a given token pair
pub fn get_route_information(
    _input_token: Pubkey,
    _output_token: Pubkey,
    _amount_in: u64,
) -> Result<Vec<SwapRoute>> {
    // This function would use router logic to find optimal paths
    // In practice, this would often be done off-chain by the client
    // or by an external router service

    // Implementation could include:
    // 1. Graph traversal to find all possible paths
    // 2. Price impact simulation for each path
    // 3. Gas cost estimation
    // 4. Optimization for highest output amount

    // Placeholder for actual routing algorithm
    msg!("Route optimization would normally use a graph algorithm to find optimal paths");

    // Return a simple route as placeholder
    Ok(vec![])
}

/// Integration point for external routers to use Fluxa pools within their own routing system
pub fn external_router_entrypoint<'a, 'b, 'c: 'info, 'info>(
    ctx: Context<'a, 'b, 'c, 'info, MultiHopSwap<'info>>,
    amount_in: u64,
    min_amount_out: u64,
    route_data: Vec<u8>,
) -> Result<u64> {
    // Parse route data from external router
    let routes: Vec<SwapRoute> = deserialize_route_data(&route_data)?;

    // Execute the swap
    handler(ctx, amount_in, min_amount_out, routes)?;

    // Return the output amount for the router to continue
    // In a real implementation, we would track and return the actual output amount
    Ok(min_amount_out)
}

/// Helper function to parse route data provided by external routers
fn deserialize_route_data(route_data: &[u8]) -> Result<Vec<SwapRoute>> {
    // Parse binary route data from an external router
    // This is a simplified implementation - real code would handle
    // various router formats and serialization schemes

    if route_data.is_empty() {
        return Err(ErrorCode::InvalidInput.into());
    }

    // In a real implementation, this would deserialize based on the
    // specific format used by the router
    let routes: Vec<SwapRoute> = match AnchorDeserialize::deserialize(&mut &route_data[..]) {
        Ok(routes) => routes,
        Err(_) => return Err(ErrorCode::InvalidInput.into()),
    };

    Ok(routes)
}

/// Structure to hold extracted account data to avoid borrowing issues
#[derive(Default)]
struct ExtractedAccountData<'info> {
    pools: Vec<Account<'info, Pool>>,
    pool_states: Vec<PoolState<'info>>,
    pool_state_refs: Vec<*mut PoolState<'info>>, // Using raw pointers
    token_accounts: Vec<AccountInfo<'info>>,
    token_vaults: Vec<AccountInfo<'info>>,
    oracle_accounts: Vec<Option<Account<'info, Oracle>>>,
    oracle_refs: Vec<Option<*mut Oracle>>, // Using raw pointers
}

/// Extract and process all accounts needed for the swap
fn extract_accounts<'a>(
    remaining_accounts: &'a [AccountInfo<'a>],
    num_pools: usize,
    num_token_accounts: usize,
    num_token_vaults: usize,
    account_index: &mut usize,
) -> Result<ExtractedAccountData<'a>> {
    let mut data = ExtractedAccountData::default();

    // Extract pool accounts
    for _ in 0..num_pools {
        let pool = Account::<Pool>::try_from(&remaining_accounts[*account_index])?;
        data.pools.push(pool);
        *account_index += 1;
    }

    // Extract token accounts
    for _ in 0..num_token_accounts {
        data.token_accounts
            .push(remaining_accounts[*account_index].clone());
        *account_index += 1;
    }

    // Extract token vaults
    for _ in 0..num_token_vaults {
        data.token_vaults
            .push(remaining_accounts[*account_index].clone());
        *account_index += 1;
    }

    // Extract optional oracle accounts if provided
    data.oracle_accounts = vec![None; num_pools];
    if *account_index < remaining_accounts.len() {
        // Attempt to extract oracle accounts (they're optional)
        for i in 0..num_pools {
            if *account_index < remaining_accounts.len() {
                if let Ok(oracle) = Account::<Oracle>::try_from(&remaining_accounts[*account_index])
                {
                    data.oracle_accounts[i] = Some(oracle);
                    *account_index += 1;
                }
            }
        }
    }

    // Initialize pool states vector
    for i in 0..num_pools {
        // Get a raw pointer to the pool
        let pool_ptr = data.pools[i].to_account_info().data.as_ptr() as *mut Pool;

        // SAFETY: We're taking ownership of the data and not creating multiple mutable references
        unsafe {
            let pool_ref = &mut *pool_ptr;
            let pool_state = PoolState::new(pool_ref);
            data.pool_states.push(pool_state);
        }
    }

    // Initialize pool state pointers
    for i in 0..num_pools {
        let ptr = &mut data.pool_states[i] as *mut PoolState;
        data.pool_state_refs.push(ptr);
    }

    // Initialize oracle references if they exist
    for i in 0..num_pools {
        if let Some(ref oracle) = data.oracle_accounts[i] {
            // Get a raw pointer to the oracle
            let oracle_ptr = oracle.to_account_info().data.as_ptr() as *mut Oracle;
            data.oracle_refs.push(Some(oracle_ptr));
        } else {
            data.oracle_refs.push(None);
        }
    }

    Ok(data)
}

/// Emit swap events with information about the swap
fn emit_swap_events<'info>(
    ctx: &Context<'_, '_, '_, 'info, MultiHopSwap<'info>>,
    routes: &[SwapRoute],
    data: &ExtractedAccountData<'info>,
    amount_in: u64,
    final_amount_out: u64,
    timestamp: i64,
) {
    let first_route = &routes[0];
    let first_pool_idx = first_route.pool_index as usize;
    let first_pool = &data.pools[first_pool_idx];

    let last_route = &routes[routes.len() - 1];
    let last_pool_idx = last_route.pool_index as usize;
    let last_pool = &data.pools[last_pool_idx];

    // Emit individual swap event for the first pool (for compatibility)
    emit!(SwapEvent {
        pool: first_pool.key(),
        sender: ctx.accounts.user.key(),
        zero_for_one: first_route.zero_for_one,
        amount_in,
        amount_out: final_amount_out,
        fee_amount: 0, // Detailed fee tracking handled per hop
        sqrt_price_before: first_pool.sqrt_price,
        sqrt_price_after: last_pool.sqrt_price,
        liquidity_before: first_pool.liquidity,
        tick_before: first_pool.current_tick,
        tick_after: last_pool.current_tick,
        timestamp,
    });

    // Emit dedicated multi-hop event with route information
    let input_token = if first_route.zero_for_one {
        first_pool.token_a_mint
    } else {
        first_pool.token_b_mint
    };

    let output_token = if last_route.zero_for_one {
        last_pool.token_b_mint
    } else {
        last_pool.token_a_mint
    };

    emit!(MultiHopSwapEvent {
        token_in: input_token,
        token_out: output_token,
        amount_in,
        amount_out: final_amount_out,
        hop_count: routes.len() as u8,
        total_fees: 0,       // Would need to calculate from individual hops
        route_efficiency: 0, // Would need price oracle to calculate theoretical direct path
        sender: ctx.accounts.user.key(),
        timestamp,
    });
}
