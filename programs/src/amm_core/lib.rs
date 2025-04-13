use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use std::ops::Deref;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS"); // Replace with your program ID when deployed

#[program]
pub mod amm_core {
    use super::*;

    /// Initializes a new token pair liquidity pool.
    /// This creates the necessary accounts to manage a concentrated liquidity AMM.
    pub fn initialize_pool(
        ctx: Context<InitializePool>,
        initial_sqrt_price: u128,
        fee_tier: u16,
    ) -> Result<()> {
        instructions::initialize_pool::handler(ctx, initial_sqrt_price, fee_tier)
    }

    /// Creates a new concentrated liquidity position for an LP.
    /// Allows specifying a custom price range for liquidity provision.
    pub fn create_position(
        ctx: Context<CreatePosition>,
        lower_tick: i32,
        upper_tick: i32,
        liquidity_amount: u128,
    ) -> Result<()> {
        instructions::create_position::handler(ctx, lower_tick, upper_tick, liquidity_amount)
    }

    /// Executes a swap between the two tokens in the pool.
    /// Calculates price impact and fees based on concentrated liquidity.
    pub fn swap(
        ctx: Context<Swap>,
        amount_in: u64,
        min_amount_out: u64,
        is_token_a: bool,
    ) -> Result<()> {
        instructions::swap::handler(ctx, amount_in, min_amount_out, is_token_a)
    }
    
    /// Collects fees accrued to a specific position.
    pub fn collect_fees(ctx: Context<CollectFees>) -> Result<()> {
        instructions::collect_fees::handler(ctx)
    }

    /// Increases liquidity in an existing position.
    pub fn increase_liquidity(
        ctx: Context<ModifyPosition>,
        liquidity_delta: u128,
    ) -> Result<()> {
        instructions::modify_position::increase_handler(ctx, liquidity_delta)
    }

    /// Decreases liquidity from an existing position.
    pub fn decrease_liquidity(
        ctx: Context<ModifyPosition>,
        liquidity_delta: u128,
    ) -> Result<()> {
        instructions::modify_position::decrease_handler(ctx, liquidity_delta)
    }
}

/// Accounts required to initialize a new liquidity pool
#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    
    #[account(init, payer = payer, space = 8 + Pool::LEN)]
    pub pool: Account<'info, Pool>,
    
    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,
    
    #[account(
        init,
        payer = payer,
        token::mint = token_a_mint,
        token::authority = pool,
    )]
    pub token_a_vault: Account<'info, TokenAccount>,
    
    #[account(
        init,
        payer = payer,
        token::mint = token_b_mint,
        token::authority = pool,
    )]
    pub token_b_vault: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

/// Accounts required to create a new position
#[derive(Accounts)]
pub struct CreatePosition<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    
    #[account(
        init,
        payer = owner,
        space = 8 + Position::LEN
    )]
    pub position: Account<'info, Position>,
    
    #[account(mut, constraint = token_a_account.mint == pool.token_a_mint)]
    pub token_a_account: Account<'info, TokenAccount>,
    
    #[account(mut, constraint = token_b_account.mint == pool.token_b_mint)]
    pub token_b_account: Account<'info, TokenAccount>,
    
    #[account(mut, constraint = token_a_vault.key() == pool.token_a_vault)]
    pub token_a_vault: Account<'info, TokenAccount>,
    
    #[account(mut, constraint = token_b_vault.key() == pool.token_b_vault)]
    pub token_b_vault: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

/// Accounts required to execute a swap
#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    
    #[account(mut)]
    pub token_source: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub token_destination: Account<'info, TokenAccount>,
    
    #[account(mut, constraint = token_a_vault.key() == pool.token_a_vault)]
    pub token_a_vault: Account<'info, TokenAccount>,
    
    #[account(mut, constraint = token_b_vault.key() == pool.token_b_vault)]
    pub token_b_vault: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
}

/// Accounts required to collect fees from a position
#[derive(Accounts)]
pub struct CollectFees<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    
    #[account(mut, has_one = owner)]
    pub position: Account<'info, Position>,
    
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    
    #[account(mut)]
    pub token_a_account: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub token_b_account: Account<'info, TokenAccount>,
    
    #[account(mut, constraint = token_a_vault.key() == pool.token_a_vault)]
    pub token_a_vault: Account<'info, TokenAccount>,
    
    #[account(mut, constraint = token_b_vault.key() == pool.token_b_vault)]
    pub token_b_vault: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
}

/// Accounts required to modify a position's liquidity
#[derive(Accounts)]
pub struct ModifyPosition<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    
    #[account(mut, has_one = owner)]
    pub position: Account<'info, Position>,
    
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    
    #[account(mut)]
    pub token_a_account: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub token_b_account: Account<'info, TokenAccount>,
    
    #[account(mut, constraint = token_a_vault.key() == pool.token_a_vault)]
    pub token_a_vault: Account<'info, TokenAccount>,
    
    #[account(mut, constraint = token_b_vault.key() == pool.token_b_vault)]
    pub token_b_vault: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
}

/// Data structure for the AMM pool
#[account]
pub struct Pool {
    /// The pool authority address
    pub authority: Pubkey,
    /// Token mint address for token A
    pub token_a_mint: Pubkey,
    /// Token mint address for token B
    pub token_b_mint: Pubkey,
    /// Vault account for token A
    pub token_a_vault: Pubkey,
    /// Vault account for token B
    pub token_b_vault: Pubkey,
    /// Current sqrt price as Q64.64
    pub sqrt_price: u128,
    /// Current tick index
    pub current_tick: i32,
    /// Fee tier in basis points (e.g. 3000 = 0.3%)
    pub fee_tier: u16,
    /// Total fee growth for token A as Q64.64
    pub fee_growth_global_a: u128,
    /// Total fee growth for token B as Q64.64
    pub fee_growth_global_b: u128,
    /// Protocol fee percentage in basis points
    pub protocol_fee: u16,
    /// Total liquidity currently active in the pool
    pub liquidity: u128,
    /// Total number of positions created in this pool
    pub position_count: u64,
}

impl Pool {
    pub const LEN: usize = 32 + // authority
        32 + // token_a_mint
        32 + // token_b_mint
        32 + // token_a_vault
        32 + // token_b_vault
        16 + // sqrt_price
        4 +  // current_tick
        2 +  // fee_tier
        16 + // fee_growth_global_a
        16 + // fee_growth_global_b
        2 +  // protocol_fee
        16 + // liquidity
        8;   // position_count
}

/// Data structure for a liquidity position
#[account]
pub struct Position {
    /// The owner of this position
    pub owner: Pubkey,
    /// The pool this position belongs to
    pub pool: Pubkey,
    /// The lower tick bound of the position
    pub lower_tick: i32,
    /// The upper tick bound of the position
    pub upper_tick: i32,
    /// Amount of liquidity in this position
    pub liquidity: u128,
    /// Fee growth for token A inside position's range as of last update
    pub fee_growth_inside_a: u128,
    /// Fee growth for token B inside position's range as of last update
    pub fee_growth_inside_b: u128,
    /// Uncollected token A fees
    pub tokens_owed_a: u64,
    /// Uncollected token B fees
    pub tokens_owed_b: u64,
}

impl Position {
    pub const LEN: usize = 32 + // owner
        32 + // pool
        4 +  // lower_tick
        4 +  // upper_tick
        16 + // liquidity
        16 + // fee_growth_inside_a
        16 + // fee_growth_inside_b
        8 +  // tokens_owed_a
        8;   // tokens_owed_b
}

// Import instruction implementations
pub mod instructions;
pub mod math;
pub mod errors;
pub mod constants;