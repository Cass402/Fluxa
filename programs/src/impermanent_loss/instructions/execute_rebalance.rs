use crate::*;
use amm_core::Pool;
use amm_core::Position;
use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

pub fn handler(
    ctx: Context<ExecuteRebalance>,
    position_id: Pubkey,
    new_lower_tick: i32,
    new_upper_tick: i32,
) -> Result<()> {
    // Store these values early to avoid borrowing conflicts later
    let original_lower_tick;
    let original_upper_tick;
    let bump = ctx.bumps.rebalance_state;

    // Scope the mutable borrow of rebalance_state for validation
    {
        let rebalance_state = &ctx.accounts.rebalance_state;

        // Validate position ID
        require!(
            position_id == rebalance_state.position_id,
            ErrorCode::InvalidPriceData
        );

        // Validate new tick bounds against the optimal boundaries calculated previously
        // This is to ensure that the rebalance follows the protocol's recommendation
        require!(
            new_lower_tick <= rebalance_state.optimal_lower_tick,
            ErrorCode::NoRebalanceNeeded
        );
        require!(
            new_upper_tick >= rebalance_state.optimal_upper_tick,
            ErrorCode::NoRebalanceNeeded
        );

        // Store values needed later
        original_lower_tick = rebalance_state.original_lower_tick;
        original_upper_tick = rebalance_state.original_upper_tick;
    }

    // Now we can pass ctx to other functions without borrowing conflicts
    // 1. Collect any accrued fees from the current position
    collect_fees_from_position(&ctx, bump, position_id)?;

    // 2. Calculate current position value and determine amounts to withdraw
    let (liquidity_withdrawn, amount_a, amount_b) = withdraw_position_liquidity(
        &ctx,
        bump,
        position_id,
        original_lower_tick,
        original_upper_tick,
    )?;

    // 3. Create a new position with the adjusted tick range
    create_new_position(
        &ctx,
        bump,
        position_id,
        new_lower_tick,
        new_upper_tick,
        amount_a,
        amount_b,
    )?;

    // 4. Calculate IL savings from this rebalance
    let il_saved = calculate_il_savings(
        &ctx.accounts.pool,
        original_lower_tick,
        original_upper_tick,
        new_lower_tick,
        new_upper_tick,
    )?;

    // Now update the rebalance state at the end
    let current_timestamp = Clock::get()?.unix_timestamp;
    let rebalance_state = &mut ctx.accounts.rebalance_state;
    rebalance_state.last_rebalance = current_timestamp;

    // Add to the cumulative IL saved
    rebalance_state.estimated_il_saved = rebalance_state
        .estimated_il_saved
        .checked_add(il_saved)
        .unwrap_or(rebalance_state.estimated_il_saved);

    // Update the tracked boundaries to the new ones
    rebalance_state.optimal_lower_tick = new_lower_tick;
    rebalance_state.optimal_upper_tick = new_upper_tick;

    Ok(())
}

/// Collect fees from a position
fn collect_fees_from_position(
    ctx: &Context<ExecuteRebalance>,
    bump: u8,
    position_id: Pubkey,
) -> Result<()> {
    let accounts = &ctx.accounts;
    let amm_program = accounts.amm_program.to_account_info();
    let authority = accounts.authority.to_account_info();
    let position = accounts.position.to_account_info();
    let pool = accounts.pool.to_account_info();
    let token_a_account = accounts.token_a_account.to_account_info();
    let token_b_account = accounts.token_b_account.to_account_info();
    let token_program = accounts.token_program.to_account_info();

    // Build the collect_fees CPI instruction manually since the AMM core doesn't expose it directly
    let seeds = &[b"rebalance_authority".as_ref(), &[bump]];
    let signer = &[&seeds[..]];

    // We'll create an empty data instruction - normally you'd load the right instruction data
    let instruction_data = vec![];

    // Call collect_fees via generic CPI
    let cpi_accounts = vec![
        AccountMeta::new(authority.key(), true),
        AccountMeta::new(position.key(), false),
        AccountMeta::new(pool.key(), false),
        AccountMeta::new(token_a_account.key(), false),
        AccountMeta::new(token_b_account.key(), false),
        AccountMeta::new(token_program.key(), false),
    ];

    // Create and invoke the instruction
    let instruction = Instruction {
        program_id: amm_program.key(),
        accounts: cpi_accounts,
        data: instruction_data,
    };

    invoke_signed(
        &instruction,
        &[
            authority,
            position,
            pool,
            token_a_account,
            token_b_account,
            token_program,
        ],
        &[seeds],
    )?;

    Ok(())
}

/// Withdraw liquidity from a position
fn withdraw_position_liquidity(
    ctx: &Context<ExecuteRebalance>,
    bump: u8,
    position_id: Pubkey,
    lower_tick: i32,
    upper_tick: i32,
) -> Result<(u128, u64, u64)> {
    let accounts = &ctx.accounts;
    let amm_program = accounts.amm_program.to_account_info();
    let authority = accounts.authority.to_account_info();
    let position = accounts.position.to_account_info();
    let pool = accounts.pool.to_account_info();
    let token_a_account = accounts.token_a_account.to_account_info();
    let token_b_account = accounts.token_b_account.to_account_info();
    let token_program = accounts.token_program.to_account_info();

    // Get position's current liquidity
    let position_data = Position::try_deserialize(&mut position.data.borrow().as_ref())?;
    let liquidity = position_data.liquidity;

    // Since we don't have direct access to AMM Core CPI functions, we'll build the instruction manually
    let seeds = &[b"rebalance_authority".as_ref(), &[bump]];
    let signer = &[&seeds[..]];

    // Similar to above, build a manual CPI call for withdraw_liquidity
    // Normally you'd use the proper instruction data here
    let instruction_data = vec![];

    let cpi_accounts = vec![
        AccountMeta::new(authority.key(), true),
        AccountMeta::new(position.key(), false),
        AccountMeta::new(pool.key(), false),
        AccountMeta::new(token_a_account.key(), false),
        AccountMeta::new(token_b_account.key(), false),
        AccountMeta::new(token_program.key(), false),
    ];

    let instruction = Instruction {
        program_id: amm_program.key(),
        accounts: cpi_accounts,
        data: instruction_data,
    };

    invoke_signed(
        &instruction,
        &[
            authority,
            position,
            pool,
            token_a_account,
            token_b_account,
            token_program,
        ],
        &[seeds],
    )?;

    // Calculate token amounts
    // Note: In a real implementation, we would calculate the exact amounts based on
    // liquidity, current price, and tick range. This is simplified.
    let amount_a = position_data.tokens_owed_a;
    let amount_b = position_data.tokens_owed_b;

    Ok((liquidity, amount_a, amount_b))
}

/// Create a new position with adjusted tick range
fn create_new_position(
    ctx: &Context<ExecuteRebalance>,
    bump: u8,
    position_id: Pubkey,
    new_lower_tick: i32,
    new_upper_tick: i32,
    amount_a: u64,
    amount_b: u64,
) -> Result<()> {
    let accounts = &ctx.accounts;
    let amm_program = accounts.amm_program.to_account_info();
    let authority = accounts.authority.to_account_info();
    let position = accounts.position.to_account_info();
    let pool = accounts.pool.to_account_info();
    let token_a_account = accounts.token_a_account.to_account_info();
    let token_b_account = accounts.token_b_account.to_account_info();
    let token_program = accounts.token_program.to_account_info();

    // Similar approach as above functions
    let seeds = &[b"rebalance_authority".as_ref(), &[bump]];
    let signer = &[&seeds[..]];

    // Similar to above, build a manual CPI call for add_liquidity
    // Normally you'd use the proper instruction data here
    let instruction_data = vec![];

    let cpi_accounts = vec![
        AccountMeta::new(authority.key(), true),
        AccountMeta::new(position.key(), false),
        AccountMeta::new(pool.key(), false),
        AccountMeta::new(token_a_account.key(), false),
        AccountMeta::new(token_b_account.key(), false),
        AccountMeta::new(token_program.key(), false),
    ];

    let instruction = Instruction {
        program_id: amm_program.key(),
        accounts: cpi_accounts,
        data: instruction_data,
    };

    invoke_signed(
        &instruction,
        &[
            authority,
            position,
            pool,
            token_a_account,
            token_b_account,
            token_program,
        ],
        &[seeds],
    )?;

    Ok(())
}

/// Calculate estimated IL savings from rebalancing
fn calculate_il_savings(
    pool: &AccountInfo,
    old_lower_tick: i32,
    old_upper_tick: i32,
    new_lower_tick: i32,
    new_upper_tick: i32,
) -> Result<u64> {
    // Get current market data from pool
    let pool_data = Pool::try_deserialize(&mut pool.data.borrow().as_ref())?;
    let current_tick = pool_data.current_tick;

    // Calculate effective volatility based on tick ranges
    let old_width = (old_upper_tick - old_lower_tick) as u64;
    let new_width = (new_upper_tick - new_lower_tick) as u64;

    // Wider ranges generally reduce IL, but the relationship is non-linear
    // This is a simplified heuristic - a real implementation would use more sophisticated models

    // No savings if narrowing the range
    if new_width <= old_width {
        return Ok(0);
    }

    // Base savings proportional to width increase
    let width_increase_pct = ((new_width - old_width) * 10000) / old_width;

    // Adjust based on current tick position relative to ranges
    let old_range_centered = is_tick_centered(current_tick, old_lower_tick, old_upper_tick);
    let new_range_centered = is_tick_centered(current_tick, new_lower_tick, new_upper_tick);

    let position_improvement = if !old_range_centered && new_range_centered {
        // Significant improvement if we're now centered around the current price
        200 // 2% additional reduction
    } else if old_range_centered && new_range_centered {
        // Modest improvement if both were centered
        100 // 1% additional reduction
    } else {
        0 // No additional improvement
    };

    // IL reduction is proportional to width increase and positioning improvement
    // Convert to basis points (1/100 of a percent)
    let il_reduction_bps = width_increase_pct / 100 + position_improvement;

    // Estimate value of position (simplified)
    let estimated_position_value = 100_000_000; // Example value: 100 tokens

    // Calculate IL savings: position value * IL reduction percentage
    let il_saved = (estimated_position_value as u128 * il_reduction_bps as u128 / 10000) as u64;

    Ok(il_saved)
}

/// Check if current tick is relatively centered in the range
fn is_tick_centered(current_tick: i32, lower_tick: i32, upper_tick: i32) -> bool {
    let range_width = upper_tick - lower_tick;
    let center_tick = lower_tick + (range_width / 2);
    let distance_from_center = (current_tick - center_tick).abs();

    // Consider "centered" if within 20% of the middle
    distance_from_center < (range_width / 5)
}
