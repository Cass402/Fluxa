use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};

declare_id!("OB111111111111111111111111111111111111111"); // Placeholder ID, replace with actual one

#[program]
pub mod order_book {
    use super::*;

    /// Initialize a new order book for a specific liquidity pool
    pub fn initialize_order_book(
        ctx: Context<InitializeOrderBook>,
        pool_id: Pubkey,
        tick_size: u64,
    ) -> Result<()> {
        // This will be implemented in the post-hackathon phase
        // For now, the function signature serves as a placeholder
        Ok(())
    }

    /// Place a limit order on the book
    pub fn place_limit_order(
        ctx: Context<PlaceLimitOrder>,
        price: u64,
        amount: u64,
        is_bid: bool,
        expiry: i64,
    ) -> Result<()> {
        // This will be implemented in the post-hackathon phase
        // For now, the function signature serves as a placeholder
        Ok(())
    }

    /// Cancel an existing limit order
    pub fn cancel_order(ctx: Context<CancelOrder>, order_id: u64) -> Result<()> {
        // This will be implemented in the post-hackathon phase
        // For now, the function signature serves as a placeholder
        Ok(())
    }

    /// Execute matching orders when conditions are met
    pub fn execute_match(ctx: Context<ExecuteMatch>, bid_id: u64, ask_id: u64) -> Result<()> {
        // This will be implemented in the post-hackathon phase
        // For now, the function signature serves as a placeholder
        Ok(())
    }
}

/// Accounts for initializing a new order book
#[derive(Accounts)]
pub struct InitializeOrderBook<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(init, payer = authority, space = 8 + OrderBook::LEN)]
    pub order_book: Account<'info, OrderBook>,
    
    pub system_program: Program<'info, System>,
}

/// Accounts for placing a limit order
#[derive(Accounts)]
pub struct PlaceLimitOrder<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    
    #[account(mut)]
    pub order_book: Account<'info, OrderBook>,
    
    #[account(init, payer = user, space = 8 + Order::LEN)]
    pub order: Account<'info, Order>,
    
    #[account(mut)]
    pub token_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

/// Accounts for cancelling an order
#[derive(Accounts)]
pub struct CancelOrder<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    
    #[account(mut, has_one = user)]
    pub order: Account<'info, Order>,
    
    #[account(mut)]
    pub order_book: Account<'info, OrderBook>,
    
    #[account(mut)]
    pub token_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

/// Accounts for executing matching orders
#[derive(Accounts)]
pub struct ExecuteMatch<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(mut)]
    pub order_book: Account<'info, OrderBook>,
    
    #[account(mut)]
    pub bid_order: Account<'info, Order>,
    
    #[account(mut)]
    pub ask_order: Account<'info, Order>,
    
    #[account(mut)]
    pub bid_user_token_account: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub ask_user_token_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
}

/// Order book state
#[account]
pub struct OrderBook {
    /// The liquidity pool this order book is associated with
    pub pool_id: Pubkey,
    
    /// Size of the minimum price increment (tick size)
    pub tick_size: u64,
    
    /// Total number of open orders
    pub order_count: u64,
    
    /// Total open bid volume
    pub bid_volume: u64,
    
    /// Total open ask volume
    pub ask_volume: u64,
}

impl OrderBook {
    pub const LEN: usize = 32 + // pool_id
        8 +  // tick_size
        8 +  // order_count
        8 +  // bid_volume
        8;   // ask_volume
}

/// Individual order state
#[account]
pub struct Order {
    /// User who placed the order
    pub user: Pubkey,
    
    /// Order book this order belongs to
    pub order_book: Pubkey,
    
    /// Unique order ID
    pub id: u64,
    
    /// Price of the order
    pub price: u64,
    
    /// Original amount of the order
    pub original_amount: u64,
    
    /// Remaining amount of the order
    pub remaining_amount: u64,
    
    /// Whether this is a bid (buy) or ask (sell) order
    pub is_bid: bool,
    
    /// Timestamp when order was placed
    pub created_at: i64,
    
    /// Timestamp when order expires (0 for no expiry)
    pub expires_at: i64,
}

impl Order {
    pub const LEN: usize = 32 + // user
        32 + // order_book
        8 +  // id
        8 +  // price
        8 +  // original_amount
        8 +  // remaining_amount
        1 +  // is_bid
        8 +  // created_at
        8;   // expires_at
}

/// Errors
#[error_code]
pub enum ErrorCode {
    #[msg("Order price must be a multiple of tick size")]
    PriceNotAlignedWithTick,
    #[msg("Order amount too small")]
    OrderAmountTooSmall,
    #[msg("Order already filled or cancelled")]
    OrderNotActive,
    #[msg("Order has expired")]
    OrderExpired,
    #[msg("Insufficient funds to place order")]
    InsufficientFunds,
}