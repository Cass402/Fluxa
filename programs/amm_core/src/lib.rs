// Importing necessary modules and crates
#![allow(unexpected_cfgs)]
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use errors::ErrorCode;
use instructions::multi_hop_swap::SwapRoute;
use token_pair::TokenPair;
use token_pair::TokenPairError;
use utils::price_range::{PriceRange, PriceRangePreset};
declare_id!("HyxBoSbjENm23GRot8Q3dLd22Y7XFTaNTP51ZH4s7ZQr");

/// Fluxa AMM Core: A Hybrid Adaptive Automated Market Maker with Concentrated Liquidity
///
/// This module provides the core functionality for Fluxa's AMM implementation on Solana,
/// featuring concentrated liquidity positions, dynamic IL mitigation, and high capital efficiency.
/// The AMM core module forms the foundation of the Fluxa protocol, managing liquidity pools,
/// positions, swaps, and fee accrual.
#[program]
pub mod amm_core {
    use super::*;

    /// Creates a new token pair entry in the AMM.
    ///
    /// This instruction registers a new trading pair between two SPL tokens.
    /// It's the foundational step required before creating any liquidity pools
    /// for these tokens. The token pair is created as a PDA derived from both token mints.
    ///
    /// # Parameters
    /// * `ctx` - The context object containing all accounts needed for this operation
    ///
    /// # Accounts Required
    /// * `authority` - The signer paying for account creation
    /// * `token_pair` - PDA for the token pair (derived from both token mints)
    /// * `token_a_mint` - The mint address of the first token in the pair
    /// * `token_b_mint` - The mint address of the second token in the pair
    ///
    /// # Accounts Required
    /// * `authority` - The signer paying for account creation
    /// * `token_pair` - PDA for the token pair (derived from both token mints)
    /// * `token_a_mint` - The mint address of the first token in the pair
    /// * `token_b_mint` - The mint address of the second token in the pair
    ///
    /// # Returns
    /// * `Result<()>` - Result indicating success or containing an error code
    pub fn create_token_pair(ctx: Context<CreateTokenPair>) -> Result<()> {
        instructions::create_token_pair::handler(ctx)
    }

    /// Initializes a new liquidity pool for a token pair.
    ///
    /// Creates a new concentrated liquidity pool with specified initial price and fee tier.
    /// Each token pair can have multiple pools with different fee tiers to optimize for
    /// various volatility levels and trading strategies.
    ///
    /// # Parameters
    /// * `ctx` - The context object containing all accounts needed for this operation
    /// * `initial_sqrt_price` - The initial square root price in Q64.64 fixed-point format
    /// * `fee_tier` - The fee tier in basis points (e.g. 3000 = 0.///
    /// # Accounts Required
    /// * `payer` - The signer paying for account creation
    /// * `pool` - The pool account to be initialized
    /// * `token_pair` - The existing token pair account
    /// * `token_a_mint` - The mint address of the first token
    /// * `token_b_mint` - The mint address of the second token
    /// * `token_a_vault` - PDA for the token A vault (derived from pool and token_a_mint)
    /// * `token_b_vault` - PDA for the token B vault (derived from pool and token_b_mint)
    ///
    /// # Accounts Required
    /// * `payer` - The signer paying for account creation
    /// * `pool` - The pool account to be initialized
    /// * `token_pair` - The existing token pair account
    /// * `token_a_mint` - The mint address of the first token
    /// * `token_b_mint` - The mint address of the second token
    /// * `token_a_vault` - PDA for the token A vault (derived from pool and token_a_mint)
    /// * `token_b_vault` - PDA for the token B vault (derived from pool and token_b_mint)
    ///
    /// # Returns
    /// * `Result<()>` - Result indicating success or containing an err///
    /// # Errors
    /// * `InvalidTickSpacing` - If the fee tier is invalid
    ///
    /// # Errors
    /// * `InvalidTickSpacing` - If the fee tier is invalid
    pub fn initialize_pool(
        ctx: Context<InitializePool>,
        initial_sqrt_price: u128,
        fee_tier: u16,
    ) -> Result<()> {
        instructions::initialize_pool::handler(ctx, initial_sqrt_price, fee_tier)
    }

    /// Initializes a new liquidity pool with price-based parameters.
    ///
    /// Creates a new concentrated liquidity pool using a human-readable price value
    /// instead of requiring a sqrt_price in Q64.64 format. This provides a more
    /// intuitive interface for pool initialization.
    ///
    /// # Parameters
    /// * `ctx` - The context object containing all accounts needed for this operation
    /// * `initial_price` - The initial price as a floating point value (token_b/token_a)
    /// * `fee_tier` - The fee tier in basis points (e.g., 3000 = 0.3%)
    ///
    /// # Returns
    /// * `Result<()>` - Result indicating success or containing an error code
    ///
    /// # Errors
    /// * `InvalidTickSpacing` - If the fee tier is invalid
    /// * `InvalidInitialPrice` - If the initial price is outside acceptable range
    pub fn initialize_pool_with_price(
        ctx: Context<InitializePool>,
        initial_price: f64,
        fee_tier: u16,
    ) -> Result<()> {
        // Validate initial price
        if initial_price <= 0.0 {
            return Err(error!(ErrorCode::InvalidInitialPrice));
        }

        // Convert price to sqrt_price in Q64.64 format
        // sqrt_price = sqrt(price) * 2^64
        let sqrt_price = (initial_price.sqrt() * (1u128 << 64) as f64) as u128;

        // Call standard initialization with converted price
        instructions::initialize_pool::handler(ctx, sqrt_price, fee_tier)
    }

    /// Creates a new concentrated liquidity position in a pool.
    ///
    /// This allows a liquidity provider to deposit tokens within a specified price range,
    /// providing capital efficiency by focusing liquidity where it's most needed.
    /// Positions earn trading fees proportional to their share of liquidity within the
    /// active price range.
    ///
    /// # Parameters
    /// * `ctx` - The context object containing all accounts needed for this operation
    /// * `lower_tick` - The lower tick boundary of the position
    /// * `upper_tick` - The upper tick boundary of the position
    /// * `liquidity_amount` - The amount of liquidity///
    /// # Mathematical Representation
    /// The position boundaries are defined by:
    /// * Lower price = 1.0001^lower_tick
    /// * Upper price = 1.0001^upper_tick
    ///
    /// # Accounts Required
    /// * `owner` - The signer who will own the position
    /// * `pool` - The pool account where liquidity will be provided
    /// * `position` - The new position account to be created
    /// * `token_a_account` - The owner's token A account
    /// * `token_b_account` - The owner's token B account
    /// * `token_a_vault` - The pool's token A vault
    /// * `token_b_vault` - The pool's token B vault
    ///
    /// # Mathematical Representation
    /// The position boundaries are defined by:
    /// * Lower price = 1.0001^lower_tick
    /// * Upper price = 1.0001^upper_tick
    ///
    /// # Accounts Required
    /// * `owner` - The signer who will own the position
    /// * `pool` - The pool account where liquidity will be provided
    /// * `position` - The new position account to be created
    /// * `token_a_account` - The owner's token A account
    /// * `token_b_account` - The owner's token B account
    /// * `token_a_vault` - The pool's token A vault
    /// * `token_b_vault` - The pool's token B vault
    ///
    /// # Returns
    /// * `Result<()>` - Result indicating success or containin///
    /// # Errors
    /// * `InvalidTickRange` - If the provided tick range is invalid
    ///
    /// # Errors
    /// * `InvalidTickRange` - If the provided tick range is invalid
    pub fn create_position(
        ctx: Context<CreatePosition>,
        lower_tick: i32,
        upper_tick: i32,
        liquidity_amount: u128,
    ) -> Result<()> {
        instructions::create_position::handler(ctx, lower_tick, upper_tick, liquidity_amount)
    }

    /// Creates a new concentrated liquidity position using price range specification.
    ///
    /// This enhanced version allows liquidity providers to create positions using
    /// standard price range presets or explicit price values, providing a more
    /// intuitive interface than working directly with tick indices.
    ///
    /// # Parameters
    /// * `ctx` - The context object containing all accounts needed for this operation
    /// * `preset` - Optional preset for standard ranges (Narrow, Medium, Wide)
    /// * `lower_price` - Optional explicit lower price bound (ignored if preset is used)
    /// * `upper_price` - Optional explicit upper price bound (ignored if preset is used)
    /// * `liquidity_amount` - The amount of liquidity to provide
    ///
    /// # Accounts Required
    /// * `owner` - The signer who will own the position
    /// * `pool` - The pool account where liquidity will be provided
    /// * `position` - The new position account to be created
    /// * `token_a_account` - The owner's token A account
    /// * `token_b_account` - The owner's token B account
    /// * `token_a_vault` - The pool's token A vault
    /// * `token_b_vault` - The pool's token B vault
    ///
    /// # Returns
    /// * `Result<()>` - Result indicating success or containing an error code
    ///
    /// # Errors
    /// * `InvalidPriceRange` - If the provided price range is invalid
    /// * `RangeTooNarrow` - If the range is narrower than the pool's tick spacing allows
    pub fn create_position_with_range(
        ctx: Context<CreatePosition>,
        preset: Option<u8>,
        lower_price: Option<f64>,
        upper_price: Option<f64>,
        liquidity_amount: u128,
    ) -> Result<()> {
        // Get the current pool price as a f64 for price range calculations
        let pool = &ctx.accounts.pool;
        let current_tick = pool.current_tick;
        let current_price = utils::price_range::PriceRange::tick_to_price(current_tick);

        // Store the range preset value for later use
        let range_preset = preset.unwrap_or(0);

        // Determine ticks based on preset or explicit price values
        let (lower_tick, upper_tick) = if let Some(preset_value) = preset {
            // Convert u8 to PriceRangePreset enum
            let preset_enum = match preset_value {
                0 => return Err(error!(ErrorCode::InvalidPreset)), // Custom should use explicit prices
                1 => PriceRangePreset::Narrow,
                2 => PriceRangePreset::Medium,
                3 => PriceRangePreset::Wide,
                _ => return Err(error!(ErrorCode::InvalidPreset)),
            };

            // Create price range from preset
            let price_range = PriceRange::new_from_preset(preset_enum, current_price)?;
            (price_range.lower_tick, price_range.upper_tick)
        } else if let (Some(lower), Some(upper)) = (lower_price, upper_price) {
            // Create price range from explicit prices
            let price_range = PriceRange::new_from_prices(lower, upper)?;
            (price_range.lower_tick, price_range.upper_tick)
        } else {
            // Neither preset nor explicit price range was provided
            return Err(error!(ErrorCode::InvalidPriceRange));
        };

        // Check if range width is compatible with pool's tick spacing
        let tick_spacing = match pool.fee_tier {
            500 => 10,    // 0.05% fee tier, 0.1% tick spacing
            3000 => 60,   // 0.3% fee tier, 0.6% tick spacing
            10000 => 200, // 1% fee tier, 2% tick spacing
            _ => return Err(error!(ErrorCode::InvalidTickSpacing)),
        };

        // Verify ticks are properly spaced
        if lower_tick % tick_spacing != 0 || upper_tick % tick_spacing != 0 {
            // Round to nearest valid tick if needed
            let adjusted_lower_tick = (lower_tick / tick_spacing) * tick_spacing;
            let adjusted_upper_tick = (upper_tick / tick_spacing) * tick_spacing;

            // Ensure adjusted range maintains at least one tick spacing
            if adjusted_upper_tick - adjusted_lower_tick < tick_spacing {
                return Err(error!(ErrorCode::RangeTooNarrow));
            }

            // Set the preset value directly on the position account before calling the handler
            ctx.accounts.position.range_preset = range_preset;

            // Use adjusted ticks
            return instructions::create_position::handler(
                ctx,
                adjusted_lower_tick,
                adjusted_upper_tick,
                liquidity_amount,
            );
        }

        // Set the preset value directly on the position account before calling the handler
        ctx.accounts.position.range_preset = range_preset;

        // Call the standard handler with calculated tick indices
        instructions::create_position::handler(ctx, lower_tick, upper_tick, liquidity_amount)
    }

    /// Executes a swap between two tokens in a liquidity pool.
    ///
    /// Allows users to trade one token for another using the pool's available liquidity.
    /// The swap follows the constant product formula within each tick range, and may
    /// cross multiple tick boundaries to fulfill the requested amount.
    ///
    /// # Parameters
    /// * `ctx` - The context object containing all accounts needed for this operation
    /// * `amount_in` - The amount of input token to swap
    /// * `min_amount_out` - The minimum amount of output token to receive (slippage protection)
    /// * `is_token_a` - Whether the input token is token A (true)///
    /// # Accounts Required
    /// * `user` - The signer executing the swap
    /// * `pool` - The liquidity pool to swap in
    /// * `token_source` - The user's source token account
    /// * `token_destination` - The user's destination token account
    /// * `token_a_vault` - The pool's token A vault
    /// * `token_b_vault` - The pool's token B vault
    ///
    /// # Accounts Required
    /// * `user` - The signer executing the swap
    /// * `pool` - The liquidity pool to swap in
    /// * `token_source` - The user's source token account
    /// * `token_destination` - The user's destination token account
    /// * `token_a_vault` - The pool's token A vault
    /// * `token_b_vault` - The pool's token B vault
    ///
    /// # Returns
    /// * `Result<()>` - Result indicating success or ///
    /// # Errors
    /// * `SlippageExceeded` - If the minimum output amount isn't met
    /// * `InsufficientLiquidity` - If the pool lacks sufficient liquidity
    /// * `ZeroOutputAmount` - If the calculation results in zero output
    ///
    /// # Errors
    /// * `SlippageExceeded` - If the minimum output amount isn't met
    /// * `InsufficientLiquidity` - If the pool lacks sufficient liquidity
    /// * `ZeroOutputAmount` - If the calculation results in zero output
    pub fn swap(
        ctx: Context<Swap>,
        amount_in: u64,
        min_amount_out: u64,
        is_token_a: bool,
    ) -> Result<()> {
        instructions::swap::handler(ctx, amount_in, min_amount_out, is_token_a)
    }

    /// Collects accumulated fees from a liquidity position.
    ///
    /// Liquidity providers earn fees when traders swap through their price range.
    /// This instruction allows position owners to withdraw those accumulated fees.
    ///
    /// # Parameters
    /// * `ctx` - The context object containing all accoun///
    /// # Accounts Required
    /// * `owner` - The position owner collecting fees
    /// * `position` - The position account containing earned fees
    /// * `pool` - The liquidity pool associated with the position
    /// * `token_a_account` - The owner's token A account to receive fees
    /// * `token_b_account` - The owner's token B account to receive fees
    /// * `token_a_vault` - The pool's token A vault
    /// * `token_b_vault` - The pool's token B vault
    ///
    /// # Accounts Required
    /// * `owner` - The position owner collecting fees
    /// * `position` - The position account containing earned fees
    /// * `pool` - The liquidity pool associated with the position
    /// * `token_a_account` - The owner's token A account to receive fees
    /// * `token_b_account` - The owner's token B account to receive fees
    /// * `token_a_vault` - The pool's token A vault
    /// * `token_b_vault` - The pool's token B vault
    ///
    /// # Returns
    /// * `Result<()>` - Result indicating success or containing an error code
    pub fn collect_fees(ctx: Context<CollectFees>) -> Result<()> {
        instructions::collect_fees::handler(ctx)
    }

    /// Increases liquidity in an existing position.
    ///
    /// Allows a position owner to add more liquidity to their existing position,
    /// maintaining the same price range but increasing their share of fees.
    ///
    /// # Parameters
    /// * `ctx` - The context object containing all accounts needed for this opera///
    /// # Accounts Required
    /// * `owner` - The position owner adding liquidity
    /// * `position` - The position account to modify
    /// * `pool` - The associated liquidity pool
    /// * `token_a_account` - The owner's token A account
    /// * `token_b_account` - The owner's token B account
    /// * `token_a_vault` - The pool's token A vault
    /// * `token_b_vault` - The pool's token B vault
    /// * `liquidity_delta` - The amount of liquidity to add
    /// # Accounts Required
    /// * `owner` - The position owner adding liquidity
    /// * `position` - The position account to modify
    /// * `pool` - The associated liquidity pool
    /// * `token_a_account` - The owner's token A account
    /// * `token_b_account` - The owner's token B account
    /// * `token_a_vault` - The pool's token A vault
    /// * `token_b_vault` - The pool's token B vault
    ///
    /// # Accounts Required
    /// * `owner` - The position owner adding liquidity
    /// * `position` - The position account to modify
    /// * `pool` - The associated liquidity pool
    /// * `token_a_account` - The owner's token A account
    /// * `token_b_account` - The owner's token B account
    /// * `token_a_vault` - The pool's token A vault
    /// * `token_b_vault` - The pool's token B vault
    ///
    /// # Returns
    /// * `Result<()>` - Result indicating success or containing an error code
    pub fn increase_liquidity(ctx: Context<ModifyPosition>, liquidity_delta: u128) -> Result<()> {
        instructions::modify_position::increase_handler(ctx, liquidity_delta)
    }

    /// Decreases liquidity from an existing position.
    ///
    /// Allows a position owner to remove some liquidity from their existing position,
    /// maintaining the same price range but decreasing their share of fees.
    /// This also collects any fees earned since the last update.
    ///
    /// # Parameters
    /// * `ctx` - The context object containing all accounts needed for this operation
    /// * `liquidity_delta`///
    /// # Accounts Required
    /// * `owner` - The position owner removing liquidity
    /// * `position` - The position account to modify
    /// * `pool` - The associated liquidity pool
    /// * `token_a_account` - The owner's token A account
    /// * `token_b_account` - The owner's token B account
    /// * `token_a_vault` - The pool's token A vault
    /// * `token_b_vault` - The pool's token B vault
    /// * `liquidity_delta` - The amount of liquidity to remove
    ///
    /// # Accounts Required
    /// * `owner` - The position owner removing liquidity
    /// * `position` - The position account to modify
    /// * `pool` - The associated liquidity pool
    /// * `token_a_account` - The owner's token A account
    /// * `token_b_account` - The owner's token B account
    /// * `token_a_vault` - The pool's token A vault
    /// * `token_b_vault` - The pool's token B vault
    ///
    /// # Returns
    /// * `Result<()>` - Result indicating success or containing an error code
    /// # Errors
    /// * `PositionLiquidityTooLow` - If attempting to remove more liquidity than available
    /// # Errors
    /// * `PositionLiquidityTooLow` - If attempting to remove more liquidity than available
    ///
    /// # Errors
    /// * `PositionLiquidityTooLow` - If attempting to remove more liquidity than available
    pub fn decrease_liquidity(ctx: Context<ModifyPosition>, liquidity_delta: u128) -> Result<()> {
        instructions::modify_position::decrease_handler(ctx, liquidity_delta)
    }

    /// Executes a multi-hop swap across multiple pools.
    ///
    /// This advanced swap function allows users to swap tokens through multiple liquidity pools
    /// in a single transaction, enabling complex trading paths and improved price execution.
    ///
    /// # Parameters
    /// * `ctx` - The context object containing all accounts needed for this operation
    /// * `amount_in` - The amount of input token to swap
    /// * `min_amount_out` - The minimum amount of final output token to receive (slippage protection)
    /// * `routes` - Vector of swap routes, each containing:
    ///    * pool_index: Index of the pool to use for this hop
    ///    * is_token_a: Whether to swap token A for token B in this pool
    ///
    /// # Accounts Required
    /// * `user` - The signer executing the swap
    /// * `pools` - Array of pool accounts to swap through
    /// * `token_accounts` - Array of user's token accounts
    /// * `token_vaults` - Array of pool token vaults
    /// * `oracle_accounts` - Optional oracle accounts for each pool
    ///
    /// # Returns
    /// * `Result<()>` - Result indicating success or containing an error code
    ///
    /// # Errors
    /// * `SlippageExceeded` - If the minimum output amount isn't met
    /// * `InvalidInput` - If route parameters are invalid
    pub fn multi_hop_swap<'a, 'b, 'c: 'info, 'info>(
        ctx: Context<'a, 'b, 'c, 'info, MultiHopSwap<'info>>,
        amount_in: u64,
        min_amount_out: u64,
        routes: Vec<SwapRoute>,
    ) -> Result<()> {
        instructions::multi_hop_swap::handler(ctx, amount_in, min_amount_out, routes)
    }

    /// Collects protocol fees from a pool.
    ///
    /// This instruction allows the authorized fee collector to withdraw accumulated
    /// protocol fees from a liquidity pool into the protocol treasury accounts.
    /// Protocol fees are a percentage of the total trading fees determined by the
    /// protocol fee rate.
    ///
    /// # Parameters
    /// * `ctx` - The context object containing all accounts needed for this operation
    ///
    /// # Accounts Required
    /// * `authority` - The authorized protocol fee collector
    /// * `pool` - The pool from which to collect fees
    /// * `token_a_vault` - The pool's token A vault
    /// * `token_b_vault` - The pool's token B vault
    /// * `token_a_destination` - The treasury's token A account
    /// * `token_b_destination` - The treasury's token B account
    ///
    /// # Returns
    /// * `Result<()>` - Result indicating success or containing an error code
    ///
    /// # Errors
    /// * `NoFeesToCollect` - If there are no protocol fees to collect
    /// * `InvalidAuthority` - If the signer is not the authorized fee collector
    /// * `TransferFailed` - If token transfer fails
    pub fn collect_protocol_fees(ctx: Context<CollectProtocolFees>) -> Result<()> {
        instructions::collect_protocol_fees::handler(ctx)
    }

    /// Closes a liquidity position with zero liquidity.
    ///
    /// This instruction allows the owner to close a position that has zero remaining
    /// liquidity, collecting any final fees and reclaiming the rent deposited in the position account.
    /// A position can only be closed after all liquidity has been withdrawn.
    ///
    /// # Parameters
    /// * `ctx` - The context object containing all accounts needed for this operation
    ///
    /// # Accounts Required
    /// * `owner` - The position owner who initiated the closure
    /// * `position` - The position account to be closed (must have zero liquidity)
    /// * `pool` - The pool where the position exists
    /// * `recipient` - The account that will receive the reclaimed rent
    /// * `token_a_account` - The owner's token A account for any final fee collection
    /// * `token_b_account` - The owner's token B account for any final fee collection
    /// * `token_a_vault` - The pool's token A vault
    /// * `token_b_vault` - The pool's token B vault
    ///
    /// # Returns
    /// * `Result<()>` - Result indicating success or containing an error code
    ///
    /// # Errors
    /// * `PositionNotEmpty` - If the position still has liquidity
    /// * `PositionFeesNotCollected` - If there are uncollected fees
    pub fn close_position(ctx: Context<ClosePosition>) -> Result<()> {
        instructions::close_position::handler(ctx)
    }
}

// ----- Account Structures ----- //

/// Accounts required to create a token pair in the AMM.
#[derive(Accounts)]
pub struct CreateTokenPair<'info> {
    /// The transaction authority and fee payer
    /// This address will become the authority for the token pair with admin rights.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The token pair account to be initialized
    /// Created as a PDA with seeds derived from both token mints
    /// to ensure deterministic discovery and prevent duplicates.
    #[account(
        init,
        payer = authority,
        space = 8 + TokenPair::LEN,
        seeds = [
            b"token_pair", 
            token_a_mint.key().as_ref(),
            token_b_mint.key().as_ref()
        ],
        bump
    )]
    pub token_pair: Account<'info, TokenPair>,

    /// The first token mint account
    /// Must be a valid SPL token mint.
    pub token_a_mint: Account<'info, Mint>,

    /// The second token mint account
    /// Must be a valid SPL token mint.
    pub token_b_mint: Account<'info, Mint>,

    /// The Solana System Program
    /// Required for creating the token_pair account.
    pub system_program: Program<'info, System>,

    /// The Solana Rent Sysvar
    /// Used to ensure the account is rent-exempt.
    pub rent: Sysvar<'info, Rent>,
}

/// Accounts required to initialize a new liquidity pool.
#[derive(Accounts)]
pub struct InitializePool<'info> {
    /// The transaction authority and fee payer
    /// This address will become the authority for the pool with admin rights.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The token pair account this pool belongs to
    /// Must be already initialized and contain the same token mints as specified.
    #[account(
        mut,
        constraint = token_pair.token_a_mint == token_a_mint.key() &&
                    token_pair.token_b_mint == token_b_mint.key()
                    @ TokenPairError::InvalidTokenMints
    )]
    pub token_pair: Account<'info, TokenPair>,

    /// The pool account to be initialized
    /// Stores all pool state and parameters.
    #[account(
        init,
        payer = payer,
        space = 8 + Pool::LEN,
    )]
    pub pool: Account<'info, Pool>,

    /// The token A mint account
    /// Must match one of the mints in the token pair.
    pub token_a_mint: Account<'info, token::Mint>,

    /// The token B mint account
    /// Must match the other mint in the token pair.
    pub token_b_mint: Account<'info, token::Mint>,

    /// The token A vault account
    /// Will hold all token A liquidity deposited into the pool.
    #[account(
        constraint = token_a_vault.mint == token_a_mint.key()
    )]
    pub token_a_vault: Account<'info, TokenAccount>,

    /// The token B vault account
    /// Will hold all token B liquidity deposited into the pool.
    #[account(
        constraint = token_b_vault.mint == token_b_mint.key()
    )]
    pub token_b_vault: Account<'info, TokenAccount>,

    /// The SPL Token program
    pub token_program: Program<'info, Token>,

    /// The Solana System Program
    /// Required for creating the pool account.
    pub system_program: Program<'info, System>,

    /// The Solana Rent Sysvar
    /// Used to ensure created accounts are rent-exempt.
    pub rent: Sysvar<'info, Rent>,
}

/// Accounts required to create a new concentrated liquidity position.
#[derive(Accounts)]
pub struct CreatePosition<'info> {
    /// The owner of the position, who is also the transaction signer
    /// The tokens will be taken from this account's token accounts.
    #[account(mut)]
    pub owner: Signer<'info>,

    /// The position account to be initialized
    /// Stores all position-specific data, including liquidity amounts and fee accounting.
    #[account(
        init,
        payer = owner,
        space = 8 + Position::LEN,
    )]
    pub position: Account<'info, Position>,

    /// The pool account where the position is being created
    /// Must be an initialized pool with active trading.
    #[account(mut)]
    pub pool: Account<'info, Pool>,

    /// The user's token A account
    /// Tokens will be transferred from this account to the pool's vault.
    #[account(mut)]
    pub token_a_account: AccountInfo<'info>,

    /// The user's token B account
    /// Tokens will be transferred from this account to the pool's vault.
    #[account(mut)]
    pub token_b_account: AccountInfo<'info>,

    /// The pool's token A vault
    /// Liquidity will be deposited into this account.
    #[account(mut)]
    pub token_a_vault: AccountInfo<'info>,

    /// The pool's token B vault
    /// Liquidity will be deposited into this account.
    #[account(mut)]
    pub token_b_vault: AccountInfo<'info>,

    /// The SPL Token program
    /// Used for token transfers.
    pub token_program: AccountInfo<'info>,

    /// The Solana System Program
    /// Required for creating the position account.
    pub system_program: Program<'info, System>,

    /// The Solana Rent Sysvar
    /// Used to ensure the position account is rent-exempt.
    pub rent: Sysvar<'info, Rent>,
}

/// Accounts required to execute a token swap operation.
#[derive(Accounts)]
pub struct Swap<'info> {
    /// The user executing the swap and signing the transaction
    /// Will provide input tokens and receive output tokens.
    #[account(mut)]
    pub user: Signer<'info>,

    /// The pool account where the swap will be executed
    /// Contains current price, liquidity, and fee settings.
    #[account(mut)]
    pub pool: Account<'info, Pool>,

    /// The user's token A account
    /// Will be debited if swapping A→B or credited if swapping B→A.
    #[account(mut)]
    pub token_a_account: AccountInfo<'info>,

    /// The user's token B account
    /// Will be debited if swapping B→A or credited if swapping A→B.
    #[account(mut)]
    pub token_b_account: AccountInfo<'info>,

    /// The pool's token A vault
    /// Will receive tokens if swapping A→B or send tokens if swapping B→A.
    #[account(mut)]
    pub token_a_vault: AccountInfo<'info>,

    /// The pool's token B vault
    /// Will receive tokens if swapping B→A or send tokens if swapping A→B.
    #[account(mut)]
    pub token_b_vault: AccountInfo<'info>,

    /// The SPL Token program
    /// Used for token transfers.
    pub token_program: AccountInfo<'info>,

    /// Optional account that receives protocol fees
    /// When provided, protocol fees will be transferred here.
    #[account(mut)]
    pub protocol_fee_account: Option<AccountInfo<'info>>,
}

/// Accounts required for executing a multi-hop swap
#[derive(Accounts)]
pub struct MultiHopSwap<'info> {
    /// The user executing the swap
    #[account(mut)]
    pub user: Signer<'info>,

    /// The SPL Token program
    pub token_program: Program<'info, Token>,

    /// The system program
    pub system_program: Program<'info, System>,
    // The remaining accounts will be validated in the handler function:
    // - Pool accounts
    // - User token accounts
    // - Pool token vaults
    // - Oracle accounts (optional)
}

/// Accounts required to collect fees from a liquidity position.
#[derive(Accounts)]
pub struct CollectFees<'info> {
    /// The position owner who will receive the collected fees
    /// Must match the owner recorded in the position account.
    pub owner: Signer<'info>,

    /// The position account that has accumulated fees
    /// Will be updated to track the collection and reset tokens owed.
    #[account(
        mut,
        has_one = owner @ ErrorCode::UnauthorizedAccess,
        has_one = pool @ ErrorCode::InvalidPool,
    )]
    pub position: Account<'info, Position>,

    /// The pool account where the position exists
    /// Used to calculate current fee growth and verify vault accounts.
    pub pool: Account<'info, Pool>,

    /// The owner's token A account to receive token A fees
    /// Must belong to the position owner.
    #[account(mut)]
    pub token_a_account: AccountInfo<'info>,

    /// The owner's token B account to receive token B fees
    /// Must belong to the position owner.
    #[account(mut)]
    pub token_b_account: AccountInfo<'info>,

    /// The pool's token A vault that holds fee tokens
    /// Source for token A fee transfers.
    #[account(
        mut,
        constraint = token_a_vault.key() == pool.token_a_vault @ ErrorCode::InvalidVault
    )]
    pub token_a_vault: AccountInfo<'info>,

    /// The pool's token B vault that holds fee tokens
    /// Source for token B fee transfers.
    #[account(
        mut,
        constraint = token_b_vault.key() == pool.token_b_vault @ ErrorCode::InvalidVault
    )]
    pub token_b_vault: AccountInfo<'info>,

    /// The SPL Token program
    /// Used for token transfers.
    pub token_program: Program<'info, token::Token>,
}

/// Accounts required to collect protocol fees from a pool.
#[derive(Accounts)]
pub struct CollectProtocolFees<'info> {
    /// The protocol fee authority
    /// Must be the authorized signer for protocol fee collection.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The pool account from which to collect protocol fees
    #[account(mut)]
    pub pool: Account<'info, Pool>,

    /// The token A vault of the pool
    #[account(
        mut,
        constraint = token_a_vault.key() == pool.token_a_vault @ ErrorCode::InvalidVault
    )]
    pub token_a_vault: Account<'info, TokenAccount>,

    /// The token B vault of the pool
    #[account(
        mut,
        constraint = token_b_vault.key() == pool.token_b_vault @ ErrorCode::InvalidVault
    )]
    pub token_b_vault: Account<'info, TokenAccount>,

    /// The token A destination account for protocol fees
    #[account(mut)]
    pub token_a_destination: Account<'info, TokenAccount>,

    /// The token B destination account for protocol fees
    #[account(mut)]
    pub token_b_destination: Account<'info, TokenAccount>,

    /// The SPL Token program
    pub token_program: Program<'info, Token>,
}

/// Accounts required to modify a liquidity position.
#[derive(Accounts)]
pub struct ModifyPosition<'info> {
    /// The position owner who is modifying the position
    /// Must match the owner recorded in the position account.
    #[account(mut)]
    pub owner: Signer<'info>,

    /// The position account being modified
    /// Will have its liquidity increased or decreased.
    #[account(
        mut,
        has_one = owner @ ErrorCode::UnauthorizedAccess,
        has_one = pool @ ErrorCode::InvalidPool,
    )]
    pub position: Account<'info, Position>,

    /// The pool account where the position exists
    /// Will have its global liquidity updated to reflect the change.
    #[account(mut)]
    pub pool: Account<'info, Pool>,

    /// The owner's token A account
    /// Source for token A when increasing liquidity, destination when decreasing.
    #[account(mut)]
    pub token_a_account: AccountInfo<'info>,

    /// The owner's token B account
    /// Source for token B when increasing liquidity, destination when decreasing.
    #[account(mut)]
    pub token_b_account: AccountInfo<'info>,

    /// The pool's token A vault
    /// Destination for token A when increasing liquidity, source when decreasing.
    #[account(
        mut,
        constraint = token_a_vault.key() == pool.token_a_vault @ ErrorCode::InvalidVault
    )]
    pub token_a_vault: AccountInfo<'info>,

    /// The pool's token B vault
    /// Destination for token B when increasing liquidity, source when decreasing.
    #[account(
        mut,
        constraint = token_b_vault.key() == pool.token_b_vault @ ErrorCode::InvalidVault
    )]
    pub token_b_vault: AccountInfo<'info>,

    /// The SPL Token program
    /// Used for token transfers.
    pub token_program: AccountInfo<'info>,
}

/// Accounts required to close a liquidity position.
#[derive(Accounts)]
pub struct ClosePosition<'info> {
    /// The position owner who initiated the closure
    /// Must match the owner recorded in the position account.
    #[account(mut)]
    pub owner: Signer<'info>,

    /// The position account to be closed
    /// Must have zero remaining liquidity.
    #[account(
        mut,
        close = recipient,
        has_one = owner @ ErrorCode::UnauthorizedAccess,
        has_one = pool @ ErrorCode::InvalidPool,
        constraint = position.liquidity == 0 @ ErrorCode::PositionNotEmpty,
        constraint = position.tokens_owed_a == 0 && position.tokens_owed_b == 0 @ ErrorCode::PositionFeesNotCollected,
    )]
    pub position: Account<'info, Position>,

    /// The pool account where the position exists
    /// Used to verify vault accounts.
    pub pool: Account<'info, Pool>,

    /// The account that will receive the reclaimed rent
    /// Typically the position owner or a designated recipient.
    #[account(mut)]
    pub recipient: AccountInfo<'info>,

    /// The owner's token A account for any final fee collection
    /// Must belong to the position owner.
    #[account(mut)]
    pub token_a_account: AccountInfo<'info>,

    /// The owner's token B account for any final fee collection
    /// Must belong to the position owner.
    #[account(mut)]
    pub token_b_account: AccountInfo<'info>,

    /// The pool's token A vault
    /// Source for any final token A fee transfers.
    #[account(
        mut,
        constraint = token_a_vault.key() == pool.token_a_vault @ ErrorCode::InvalidVault
    )]
    pub token_a_vault: AccountInfo<'info>,

    /// The pool's token B vault
    /// Source for any final token B fee transfers.
    #[account(
        mut,
        constraint = token_b_vault.key() == pool.token_b_vault @ ErrorCode::InvalidVault
    )]
    pub token_b_vault: AccountInfo<'info>,

    /// The SPL Token program
    /// Used for token transfers.
    pub token_program: Program<'info, token::Token>,
}

/// Data structure representing an AMM liquidity pool.
///
/// This account stores the state of a concentrated liquidity pool, including price information,
/// fees, liquidity, and references to the token vaults. Each pool enables trading between
/// a specific pair of tokens with a defined fee tier.
#[account]
pub struct Pool {
    /// The authority that controls this pool (typically the protocol governance)
    pub authority: Pubkey,

    /// The mint address for token A
    pub token_a_mint: Pubkey,

    /// The mint address for token B
    pub token_b_mint: Pubkey,

    /// The vault address for token A (holds liquidity)
    pub token_a_vault: Pubkey,

    /// The vault address for token B (holds liquidity)
    pub token_b_vault: Pubkey,

    /// Current square root price as Q64.64 fixed-point number
    /// This is sqrt(price) * 2^64, where price = token_b/token_a
    pub sqrt_price: u128,

    /// Current tick index, representing the discretized price range
    /// Each tick represents a 0.01% price increment
    pub current_tick: i32,

    /// Fee tier in basis points (e.g., 3000 = 0.3%)
    /// Higher fees typically accompany higher volatility pairs
    pub fee_tier: u16,

    /// Total fee growth for token A as Q64.64
    /// Accumulates fees per unit of virtual liquidity for token A
    pub fee_growth_global_a: u128,

    /// Total fee growth for token B as Q64.64
    /// Accumulates fees per unit of virtual liquidity for token B
    pub fee_growth_global_b: u128,

    /// Protocol fee percentage in basis points
    /// Portion of trading fees allocated to protocol treasury
    pub protocol_fee: u16,

    /// Accumulated protocol fees for token A
    /// These are fees that can be collected by the protocol authority
    pub protocol_fee_a: u64,

    /// Accumulated protocol fees for token B
    /// These are fees that can be collected by the protocol authority
    pub protocol_fee_b: u64,

    /// Total liquidity currently active in the pool
    /// Denominated in L units (geometric mean of token amounts)
    pub liquidity: u128,

    /// Total number of positions created in this pool
    pub position_count: u64,

    /// Oracle account associated with this pool
    pub oracle: Pubkey,

    /// Last block timestamp when the oracle was updated
    pub last_oracle_update: i64,

    /// Oracle data observation index
    pub observation_index: u16,

    /// Oracle observation cardinality (number of initialized observations)
    pub observation_cardinality: u16,

    /// Next oracle observation cardinality to grow to
    pub observation_cardinality_next: u16,
}

impl Pool {
    /// Total serialized size of the Pool account in bytes
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
        8 +  // protocol_fee_a
        8 +  // protocol_fee_b
        16 + // liquidity
        8 +  // position_count
        32 + // oracle
        8 +  // last_oracle_update
        2 +  // observation_index
        2 +  // observation_cardinality
        2; // observation_cardinality_next
}

/// Data structure representing a concentrated liquidity position.
///
/// This account stores the state of a liquidity provider's position in a pool,
/// defined by a specific price range and liquidity amount. It tracks fees earned
/// and manages the owner's share of the pool within the specified bounds.
#[account]
pub struct Position {
    /// The owner of this position, who can modify it and collect fees
    pub owner: Pubkey,

    /// Reference to the pool this position belongs to
    pub pool: Pubkey,

    /// The lower tick boundary of the position
    /// Defines the lower price limit: price = 1.0001^lower_tick
    pub lower_tick: i32,

    /// The upper tick boundary of the position
    /// Defines the upper price limit: price = 1.0001^upper_tick
    pub upper_tick: i32,

    /// The lower price boundary (human readable representation)
    /// This is stored for better UX when displaying position information
    pub lower_price: u64,

    /// The upper price boundary (human readable representation)
    /// This is stored for better UX when displaying position information  
    pub upper_price: u64,

    /// The preset used to create this position, if any
    /// 0 = Custom, 1 = Narrow, 2 = Medium, 3 = Wide
    pub range_preset: u8,

    /// Amount of liquidity contributed to the pool within this range
    /// Denominated in L units (geometric mean of token amounts)
    pub liquidity: u128,

    /// Fee growth for token A inside position's range as of last update
    /// Used to calculate uncollected fees when position is modified
    pub fee_growth_inside_a: u128,

    /// Fee growth for token B inside position's range as of last update
    /// Used to calculate uncollected fees when position is modified
    pub fee_growth_inside_b: u128,

    /// Uncollected token A fees, ready for withdrawal
    pub tokens_owed_a: u64,

    /// Uncollected token B fees, ready for withdrawal
    pub tokens_owed_b: u64,
}

impl Position {
    /// Total serialized size of the Position account in bytes
    pub const LEN: usize = 32 + // owner
        32 + // pool
        4 +  // lower_tick
        4 +  // upper_tick
        8 +  // lower_price
        8 +  // upper_price
        1 +  // range_preset
        16 + // liquidity
        16 + // fee_growth_inside_a
        16 + // fee_growth_inside_b
        8 +  // tokens_owed_a
        8; // tokens_owed_b
}

// Import module declarations
pub mod constants;
pub mod errors;
pub mod instructions;
pub mod math;
pub mod oracle;
pub mod oracle_utils;
pub mod pool_state;
pub mod position_manager;
pub mod swap_router;
pub mod tick_bitmap;
pub mod token_pair;
pub mod utils;

#[cfg(kani)]
pub mod formal_verification;

#[cfg(test)]
pub mod property_based_test;

#[cfg(test)]
pub mod unit_test;
