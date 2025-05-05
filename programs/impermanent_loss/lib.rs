#![allow(unexpected_cfgs)]
#![allow(unused_imports)]
#![allow(unused_variables)]
use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};

declare_id!("EHnY4rkdv8cLZrQQL14yVrxBSBpQv9zweK1GfxGC3pZ"); // Placeholder ID, replace with actual one

#[program]
pub mod impermanent_loss {
    use super::*;

    /// Initializes the IL mitigation module for a specific AMM pool.
    /// This sets up the volatility tracking and parameters for that pool.
    pub fn initialize_il_mitigation(
        ctx: Context<InitializeILMitigation>,
        pool_id: Pubkey,
        volatility_window: u64,
        adjustment_threshold: u64,
        max_adjustment_factor: u64,
        rebalance_cooldown: u64,
        reserve_imbalance_threshold: u64,
    ) -> Result<()> {
        instructions::initialize_il_mitigation::handler(
            ctx,
            pool_id,
            volatility_window,
            adjustment_threshold,
            max_adjustment_factor,
            rebalance_cooldown,
            reserve_imbalance_threshold,
        )
    }

    /// Updates price data for volatility calculation.
    /// This should be called periodically to feed price data to the system.
    pub fn update_price_data(
        ctx: Context<UpdatePriceData>,
        price: u64,
        timestamp: i64,
    ) -> Result<()> {
        instructions::update_price_data::handler(ctx, price, timestamp)
    }

    /// Calculates current volatility based on the price history.
    pub fn calculate_volatility(ctx: Context<CalculateVolatility>) -> Result<()> {
        instructions::calculate_volatility::handler(ctx)
    }

    /// Determines if a position should be rebalanced based on current conditions.
    pub fn check_rebalance_condition(
        ctx: Context<CheckRebalanceCondition>,
        position_id: Pubkey,
    ) -> Result<()> {
        instructions::check_rebalance_condition::handler(ctx, position_id)
    }

    /// Executes a position rebalance by adjusting the liquidity boundaries.
    pub fn execute_rebalance(
        ctx: Context<ExecuteRebalance>,
        position_id: Pubkey,
        new_lower_tick: i32,
        new_upper_tick: i32,
    ) -> Result<()> {
        instructions::execute_rebalance::handler(ctx, position_id, new_lower_tick, new_upper_tick)
    }
}

/// Accounts required to initialize IL mitigation for a pool
#[derive(Accounts)]
pub struct InitializeILMitigation<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(init, payer = authority, space = 8 + ILMitigationParams::LEN)]
    pub il_params: Account<'info, ILMitigationParams>,

    #[account(init, payer = authority, space = 8 + VolatilityState::LEN)]
    pub volatility_state: Account<'info, VolatilityState>,

    #[account(init, payer = authority, space = 8 + PriceHistory::LEN)]
    pub price_history: Account<'info, PriceHistory>,

    pub system_program: Program<'info, System>,
}

/// Accounts required to update price data
#[derive(Accounts)]
pub struct UpdatePriceData<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub price_history: Account<'info, PriceHistory>,

    #[account(mut)]
    pub volatility_state: Account<'info, VolatilityState>,
}

/// Accounts required to calculate volatility
#[derive(Accounts)]
pub struct CalculateVolatility<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub volatility_state: Account<'info, VolatilityState>,

    pub price_history: Account<'info, PriceHistory>,

    pub il_params: Account<'info, ILMitigationParams>,
}

/// Accounts required to check rebalance condition
#[derive(Accounts)]
pub struct CheckRebalanceCondition<'info> {
    /// Authority that can check rebalance conditions
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Rebalance state account
    #[account(mut)]
    pub rebalance_state: Account<'info, RebalanceState>,

    /// Volatility state account
    pub volatility_state: Account<'info, VolatilityState>,

    /// IL mitigation parameters
    pub il_params: Account<'info, ILMitigationParams>,

    /// Pool account from the AMM Core
    #[account(mut)]
    pub pool: Account<'info, amm_core::Pool>,
}

/// Accounts required to execute a rebalance
#[derive(Accounts)]
pub struct ExecuteRebalance<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub rebalance_state: Account<'info, RebalanceState>,

    // These accounts would be used to interact with the AMM core program
    // to modify the position's boundaries. The actual structure would depend
    // on how the AMM Core program is designed.
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

/// Parameters for IL mitigation configuration
#[account]
pub struct ILMitigationParams {
    /// The pool this configuration applies to
    pub pool_id: Pubkey,

    /// Window size for volatility calculation (in seconds)
    pub volatility_window: u64,

    /// Minimum volatility to trigger adjustment (in basis points)
    pub adjustment_threshold: u64,

    /// Maximum range expansion factor (in basis points)
    pub max_adjustment_factor: u64,

    /// Minimum time between rebalances (in seconds)
    pub rebalance_cooldown: u64,

    /// Threshold for reserve imbalance to trigger rebalance (in basis points)
    pub reserve_imbalance_threshold: u64,
}

impl ILMitigationParams {
    pub const LEN: usize = 32 + // pool_id
        8 +  // volatility_window
        8 +  // adjustment_threshold
        8 +  // max_adjustment_factor
         8 +  // rebalance_cooldown
        8; // reserve_imbalance_threshold
}

/// State tracking for volatility calculations
#[account]
pub struct VolatilityState {
    /// Current short-term volatility (rolling window)
    pub short_term_volatility: u64,

    /// Current medium-term volatility (rolling window)
    pub medium_term_volatility: u64,

    /// Current long-term volatility (rolling window)
    pub long_term_volatility: u64,

    /// Rate of volatility change
    pub volatility_acceleration: i64,

    /// Timestamp of last volatility calculation
    pub last_calculation: i64,
}

impl VolatilityState {
    pub const LEN: usize = 8 +  // short_term_volatility
        8 +  // medium_term_volatility
        8 +  // long_term_volatility
        8 +  // volatility_acceleration
        8; // last_calculation
}

/// Price data for volatility calculation
#[account]
pub struct PriceHistory {
    /// Circular buffer of price data points - Reduced from 288 to 96 elements to avoid stack overflow
    pub prices: [u64; 96], // Store 15-minute intervals for 24 hours (reduced from 5-minute intervals)

    /// Corresponding timestamps for each price point - Reduced from 288 to 96 elements
    pub timestamps: [i64; 96],

    /// Current index in the circular buffer
    pub current_index: u16,

    /// Number of valid data points in the buffer
    pub data_count: u16,
}

impl PriceHistory {
    pub const LEN: usize = 8 * 96 +  // prices (reduced from 288)
        8 * 96 +  // timestamps (reduced from 288)
        2 +  // current_index
        2; // data_count
}

/// State tracking for a specific position being rebalanced
#[account]
pub struct RebalanceState {
    /// Position being monitored
    pub position_id: Pubkey,

    /// Pool this position belongs to
    pub pool_id: Pubkey,

    /// Original lower tick boundary
    pub original_lower_tick: i32,

    /// Original upper tick boundary
    pub original_upper_tick: i32,

    /// Current optimal lower tick boundary
    pub optimal_lower_tick: i32,

    /// Current optimal upper tick boundary
    pub optimal_upper_tick: i32,

    /// Last time this position was rebalanced
    pub last_rebalance: i64,

    /// Estimated IL saved by previous rebalances
    pub estimated_il_saved: u64,
}

impl RebalanceState {
    pub const LEN: usize = 32 + // position_id
        32 + // pool_id
        4 +  // original_lower_tick
        4 +  // original_upper_tick
        4 +  // optimal_lower_tick
        4 +  // optimal_upper_tick
        8 +  // last_rebalance
        8; // estimated_il_saved
}

/// Errors for the IL mitigation module
#[error_code]
pub enum ErrorCode {
    #[msg("Calculation error in volatility measurement")]
    VolatilityCalculationError,
    #[msg("Too early to rebalance position")]
    RebalanceCooldownNotMet,
    #[msg("Invalid price data")]
    InvalidPriceData,
    #[msg("Current volatility below threshold")]
    VolatilityBelowThreshold,
    #[msg("Rebalance would not improve position")]
    NoRebalanceNeeded,
    #[msg("Invalid reserve amount, cannot be zero")]
    InvalidReserveAmount,
}

// Import instruction implementations
pub mod instructions;
