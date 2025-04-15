use crate::*;
use anchor_lang::prelude::*;
use anchor_spl::token;

pub fn handler(ctx: Context<CollectFees>) -> Result<()> {
    let position = &mut ctx.accounts.position;
    let pool = &ctx.accounts.pool;

    // Calculate uncollected fees
    // TODO: Calculate current fee growth inside the position's range
    // TODO: Update the position's fee tracking variables

    // Get fees to collect
    let tokens_owed_a = position.tokens_owed_a;
    let tokens_owed_b = position.tokens_owed_b;

    // Reset amounts owed
    position.tokens_owed_a = 0;
    position.tokens_owed_b = 0;

    // Transfer tokens from pool vaults to user accounts
    if tokens_owed_a > 0 {
        // Create PDA signer for vault
        // TODO: Transfer tokens_owed_a from token_a_vault to token_a_account
    }

    if tokens_owed_b > 0 {
        // Create PDA signer for vault
        // TODO: Transfer tokens_owed_b from token_b_vault to token_b_account
    }

    Ok(())
}
