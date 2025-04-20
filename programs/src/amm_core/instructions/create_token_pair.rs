/// Create Token Pair Instruction Module
///
/// This module implements the instruction for creating a new token pair in the Fluxa AMM.
/// Token pairs are the fundamental trading relationships in the protocol, connecting two
/// tokens that can be exchanged through various liquidity pools.
///
/// The creation process initializes a new PDA-based account that tracks all information
/// related to the token pair, including references to associated liquidity pools, trading
/// statistics, and oracle price data.
use crate::CreateTokenPair;
use anchor_lang::prelude::*;

/// Handler function for creating a new token pair
///
/// This function initializes a new token pair with default values and sets up the
/// relationship between two token mints. New token pairs start in an unverified state
/// and must be approved by governance to receive certain benefits.
///
/// # Arguments
/// * `ctx` - The context containing all accounts involved in the operation
///
/// # Returns
/// * `Result<()>` - Result indicating success or failure
pub fn handler(ctx: Context<CreateTokenPair>) -> Result<()> {
    let token_pair = &mut ctx.accounts.token_pair;
    let token_a_mint = &ctx.accounts.token_a_mint;
    let token_b_mint = &ctx.accounts.token_b_mint;

    // Initialize token pair data
    token_pair.authority = ctx.accounts.authority.key();
    token_pair.token_a_mint = token_a_mint.key();
    token_pair.token_b_mint = token_b_mint.key();
    token_pair.token_a_decimals = token_a_mint.decimals;
    token_pair.token_b_decimals = token_b_mint.decimals;
    token_pair.pools = Vec::new();
    token_pair.total_volume_token_a = 0;
    token_pair.total_volume_token_b = 0;
    token_pair.total_fees_generated = 0;
    token_pair.last_oracle_price = 0;
    token_pair.last_oracle_update = 0;

    // New token pairs start as unverified until governance approves them
    token_pair.is_verified = false;

    // Version starts at 1
    token_pair.version = 1;

    // Clear reserved space
    token_pair.reserved = [0; 64];

    msg!(
        "Created token pair: {} <> {}",
        token_a_mint.key().to_string(),
        token_b_mint.key().to_string()
    );

    Ok(())
}
