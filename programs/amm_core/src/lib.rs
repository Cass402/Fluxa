#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use errors::ErrorCode;
use position::PositionData;
use state::pool::Pool;
use tick::TickData;

// Your program's on-chain ID.
// Replace with your actual program ID after deployment.
declare_id!("BrbPGefYKdXgfmZTnasv3dkcE7TfQ82ueBwqmQX1Y8Ly");

// Modules for constants, errors, core math, and state definitions
pub mod constants;
pub mod errors;
pub mod math;
pub mod position; // Defines PositionData
pub mod state; // Defines Pool state (state::pool::Pool)
pub mod tick; // Defines TickData
pub mod tick_bitmap;

// Only include entrypoint if not building with no-entrypoint feature
pub mod instructions;

#[cfg(test)]
pub mod unit_test;

#[program]
pub mod amm_core {

    use super::*;

    /// Initializes a new liquidity pool for a pair of tokens.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context containing all necessary accounts.
    /// * `initial_sqrt_price_q64` - The initial sqrt(price) for the pool, in Q64.64 format.
    /// * `fee_rate` - The fee rate for swaps in this pool, in basis points (e.g., 30 for 0.3%).
    /// * `tick_spacing` - The spacing between usable ticks in this pool.
    pub fn initialize_pool_handler(
        ctx: Context<InitializePool>,
        initial_sqrt_price_q64: u128,
        fee_rate: u16,
        tick_spacing: u16,
    ) -> Result<()> {
        instructions::initialize_pool::handler(ctx, initial_sqrt_price_q64, fee_rate, tick_spacing)
    }

    /// Creates a new concentrated liquidity position or adds liquidity to an existing one.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context containing all necessary accounts.
    /// * `tick_lower_index` - The lower tick boundary of the position.
    /// * `tick_upper_index` - The upper tick boundary of the position.
    /// * `liquidity_amount_desired` - The amount of liquidity to add to this position.
    pub fn mint_position_handler(
        ctx: Context<MintPosition>,
        tick_lower_index: i32,
        tick_upper_index: i32,
        liquidity_amount_desired: u128,
    ) -> Result<()> {
        instructions::mint_position::handler(
            ctx,
            tick_lower_index,
            tick_upper_index,
            liquidity_amount_desired,
        )
    }

    /// Swaps an exact amount of an input token for an output token.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context containing all necessary accounts.
    /// * `amount_in` - The exact amount of the input token to swap.
    /// * `amount_out_minimum` - The minimum amount of the output token the swapper is willing to receive.
    /// * `sqrt_price_limit_q64` - A price limit for the swap. If the price moves beyond this limit,
    ///                            the swap will not consume the entire input amount.
    pub fn swap_exact_input_handler<'info>(
        ctx: Context<'_, '_, '_, 'info, SwapExactInput<'info>>,
        amount_in: u64,
        amount_out_minimum: u64,
        sqrt_price_limit_q64: u128,
    ) -> Result<()> {
        instructions::swap_exact_input::handler(
            ctx,
            amount_in,
            amount_out_minimum,
            sqrt_price_limit_q64,
        )
    }

    /// Updates an existing concentrated liquidity position's tick boundaries.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context containing all necessary accounts.
    /// * `new_tick_lower_index` - The new lower tick boundary for the position.
    /// * `new_tick_upper_index` - The new upper tick boundary for the position.
    pub fn update_position_handler(
        ctx: Context<UpdatePosition>,
        new_tick_lower_index: i32,
        new_tick_upper_index: i32,
    ) -> Result<()> {
        instructions::update_position::handler(ctx, new_tick_lower_index, new_tick_upper_index)
    }

    // Potentially add decrease_liquidity_handler and collect_fees_handler for MVP+
}

#[derive(Accounts)]
#[instruction(tick_lower_index: i32, tick_upper_index: i32)]
pub struct MintPosition<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,

    #[account(
        init,
        payer = payer,
        space = PositionData::LEN,
        seeds = [
            b"position".as_ref(),
            pool.key().as_ref(),
            owner.key().as_ref(),
            tick_lower_index.to_le_bytes().as_ref(),
            tick_upper_index.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub position: Account<'info, PositionData>,

    #[account(
        init_if_needed,
        payer = payer,
        space = TickData::LEN,
        seeds = [
            b"tick".as_ref(),
            pool.key().as_ref(),
            tick_lower_index.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub tick_lower: AccountLoader<'info, TickData>,

    #[account(
        init_if_needed,
        payer = payer,
        space = TickData::LEN,
        seeds = [
            b"tick".as_ref(),
            pool.key().as_ref(),
            tick_upper_index.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub tick_upper: AccountLoader<'info, TickData>,

    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>, // Needed for init and init_if_needed
}

#[derive(Accounts)]
#[instruction(amount_in: u64, amount_out_minimum: u64, sqrt_price_limit_q64: u128)]
pub struct SwapExactInput<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,

    #[account(
        mut,
        constraint = token0_vault.key() == pool.token0_vault @ ErrorCode::InvalidTokenVault,
        constraint = token0_vault.mint == pool.token0_mint @ ErrorCode::InvalidVaultMint
    )]
    pub token0_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = token1_vault.key() == pool.token1_vault @ ErrorCode::InvalidTokenVault,
        constraint = token1_vault.mint == pool.token1_mint @ ErrorCode::InvalidVaultMint
    )]
    pub token1_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_in_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_out_account: Account<'info, TokenAccount>,

    pub user_authority: Signer<'info>,

    pub token_program: Program<'info, Token>,

    // For an MVP, pass a fixed number of tick accounts.
    // The client is responsible for providing the correct tick accounts
    // that are expected to be crossed during this swap.
    // These should be PDAs for initialized ticks.
    // The Pool::swap method will need to be able to load() these.
    // More advanced: use remaining_accounts to pass a dynamic list.
    // Note: The actual number of tick accounts needed depends on the swap's price impact.
    // For a hackathon/MVP, 3-5 might be a reasonable fixed number to start with.
    // Ensure these are ordered correctly if Pool::swap expects a certain order.
    pub tick_account_0: Option<AccountLoader<'info, TickData>>,
    pub tick_account_1: Option<AccountLoader<'info, TickData>>,
    pub tick_account_2: Option<AccountLoader<'info, TickData>>,
    // Add more if needed, e.g., tick_account_3, tick_account_4
}

#[derive(Accounts)]
#[instruction(initial_sqrt_price_q64: u128, fee_rate: u16, tick_spacing: u16)]
pub struct InitializePool<'info> {
    #[account(
        init,
        payer = payer,
        // Seeds for the Pool PDA.
        // IMPORTANT: mint_a and mint_b keys MUST be provided in canonical order (e.g., mint_a.key < mint_b.key).
        // The client is responsible for ensuring this order before calling the instruction.
        seeds = [
            b"pool".as_ref(),
            mint_a.key().as_ref(), // Smaller address
            mint_b.key().as_ref()  // Larger address
        ],
        bump,
        space = Pool::LEN
    )]
    pub pool: Account<'info, Pool>,

    /// CHECK: mint_a and mint_b are validated by being used in PDA seeds & token::mint constraint.
    /// Client must ensure mint_a.key() < mint_b.key() for canonical pool PDA.
    pub mint_a: Account<'info, Mint>,
    pub mint_b: Account<'info, Mint>,

    /// The factory that created this pool.
    /// For MVP, this can be any account, e.g., payer or system_program.
    /// CHECK: This account is unchecked
    /// CHECK: For MVP, factory is not strictly validated beyond being a provided account.
    pub factory: UncheckedAccount<'info>,

    #[account(
        init,
        payer = payer,
        token::mint = mint_a,
        token::authority = pool, // The `pool` account (PDA) is the authority
    )]
    pub pool_vault_a: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = payer,
        token::mint = mint_b,
        token::authority = pool, // The `pool` account (PDA) is the authority
    )]
    pub pool_vault_b: Account<'info, TokenAccount>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>, // Anchor uses Rent sysvar for `init` to ensure rent exemption.
}

#[derive(Accounts)]
#[instruction(new_tick_lower_index: i32, new_tick_upper_index: i32)]
pub struct UpdatePosition<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,

    #[account(
        mut,
        // Constraint: Ensure the signer is the owner of the position
        // Or, for risk engine integration, the signer might be the risk engine's PDA
        // For MVP, owner signing is simpler.
        has_one = owner 
    )]
    pub position: Account<'info, PositionData>,

    // Old Ticks (may or may not be needed depending on how you handle liquidity removal)
    // For MVP, we might assume they are loaded if needed by modify_liquidity
    // Or, for a simpler MVP, the modify_liquidity for removal might not need them if it just updates net liquidity.
    // However, to correctly update liquidity_gross, they are needed.

    #[account(
        mut, // TickData needs to be mutable
        seeds = [b"tick".as_ref(), pool.key().as_ref(), position.tick_lower_index.to_le_bytes().as_ref()],
        bump
    )]
    pub old_tick_lower: AccountLoader<'info, TickData>,

    #[account(
        mut, // TickData needs to be mutable
        seeds = [b"tick".as_ref(), pool.key().as_ref(), position.tick_upper_index.to_le_bytes().as_ref()],
        bump
    )]
    pub old_tick_upper: AccountLoader<'info, TickData>,

    // New Ticks
    #[account(
        init_if_needed,
        payer = payer,
        space = TickData::LEN,
        seeds = [b"tick".as_ref(), pool.key().as_ref(), new_tick_lower_index.to_le_bytes().as_ref()],
        bump
    )]
    pub new_tick_lower: AccountLoader<'info, TickData>,

    #[account(
        init_if_needed,
        payer = payer,
        space = TickData::LEN,
        seeds = [b"tick".as_ref(), pool.key().as_ref(), new_tick_upper_index.to_le_bytes().as_ref()],
        bump
    )]
    pub new_tick_upper: AccountLoader<'info, TickData>,

    // Signer: Could be the position owner or the risk engine PDA
    pub owner: Signer<'info>, // For MVP, position owner triggers

    #[account(mut)]
    pub payer: Signer<'info>, // To pay for new tick accounts if created

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}
