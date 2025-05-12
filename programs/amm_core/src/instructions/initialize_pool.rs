use anchor_lang::prelude::*;

use crate::errors::ErrorCode;
use crate::state::pool::*;
use crate::InitializePool;

pub fn handler(
    ctx: Context<InitializePool>,
    initial_sqrt_price_q64: u128,
    fee_rate: u16,
    tick_spacing: u16,
) -> Result<()> {
    // Ensure canonical mint order for PDA derivation consistency.
    // This check reinforces the client-side responsibility.
    if ctx.accounts.mint_a.key() >= ctx.accounts.mint_b.key() {
        return err!(ErrorCode::MintsNotInCanonicalOrder);
    }

    msg!(
        "Initializing new pool for mints: {} and {}",
        ctx.accounts.mint_a.key(),
        ctx.accounts.mint_b.key()
    );

    // Anchor provides the bump directly if the PDA account is named in `ctx.bumps`.
    // The `pool` account is named `pool` in the `InitializePool` struct.
    let bump = ctx.bumps.pool;

    let params = InitializePoolParams {
        bump,
        factory: ctx.accounts.factory.key(),
        token0_mint: ctx.accounts.mint_a.key(), // mint_a is canonically smaller
        token1_mint: ctx.accounts.mint_b.key(), // mint_b is canonically larger
        token0_vault: ctx.accounts.pool_vault_a.key(),
        token1_vault: ctx.accounts.pool_vault_b.key(),
        initial_sqrt_price_q64,
        fee_rate,
        tick_spacing,
    };

    ctx.accounts.pool.initialize(params)?;

    msg!("Pool initialized successfully: {}", ctx.accounts.pool.key());
    msg!(
        "  Vault A: {}, Vault B: {}",
        ctx.accounts.pool_vault_a.key(),
        ctx.accounts.pool_vault_b.key()
    );
    msg!("  Initial SqrtPriceQ64: {}", initial_sqrt_price_q64);
    msg!("  Fee Rate (bps): {}", fee_rate);
    msg!("  Tick Spacing: {}", tick_spacing);

    Ok(())
}
