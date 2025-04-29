use crate::errors::ErrorCode;
use crate::CollectProtocolFees;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Transfer};

/// Event emitted when protocol fees are collected
#[event]
pub struct ProtocolFeeCollectionEvent {
    /// The pool from which fees were collected
    pub pool: Pubkey,

    /// The authority that collected the fees
    pub authority: Pubkey,

    /// The amount of token A fees collected
    pub token_a_amount: u64,

    /// The amount of token B fees collected
    pub token_b_amount: u64,

    /// The timestamp when the collection occurred
    pub timestamp: i64,
}

/// Collects protocol fees from a pool
///
/// This function transfers accumulated protocol fees from the pool's token vaults
/// to the protocol treasury accounts. Only the authorized fee collector can execute this.
///
/// # Arguments
/// * `ctx` - The context containing all necessary accounts
///
/// # Returns
/// * `Result<()>` - Result indicating success or containing an error code
pub fn handler(ctx: Context<CollectProtocolFees>) -> Result<()> {
    // Extract necessary data before mutable borrowing to avoid multiple borrows
    let pool_account_info = ctx.accounts.pool.to_account_info();
    let token_a_mint = ctx.accounts.pool.token_a_mint;
    let token_b_mint = ctx.accounts.pool.token_b_mint;

    let pool = &mut ctx.accounts.pool;

    // Get the protocol fees accumulated
    let protocol_fee_a = pool.protocol_fee_a;
    let protocol_fee_b = pool.protocol_fee_b;

    // Verify there are fees to collect
    if protocol_fee_a == 0 && protocol_fee_b == 0 {
        return Err(ErrorCode::NoFeesToCollect.into());
    }

    // Reset protocol fee accumulators
    pool.protocol_fee_a = 0;
    pool.protocol_fee_b = 0;

    // Transfer token A protocol fees if any
    if protocol_fee_a > 0 {
        // Derive PDA signer for vault
        let (_pool_address, bump) = Pubkey::find_program_address(
            &[b"pool", &token_a_mint.to_bytes(), &token_b_mint.to_bytes()],
            ctx.program_id,
        );

        let pool_signer_seeds = &[
            b"pool".as_ref(),
            &token_a_mint.to_bytes()[..],
            &token_b_mint.to_bytes()[..],
            &[bump],
        ];
        let signer_seeds = &[&pool_signer_seeds[..]];

        // Transfer token A fees to protocol treasury
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_a_vault.to_account_info(),
                    to: ctx.accounts.token_a_destination.to_account_info(),
                    authority: pool_account_info.clone(),
                },
                signer_seeds,
            ),
            protocol_fee_a,
        )?;
    }

    // Transfer token B protocol fees if any
    if protocol_fee_b > 0 {
        // Derive PDA signer for vault
        let (_pool_address, bump) = Pubkey::find_program_address(
            &[b"pool", &token_a_mint.to_bytes(), &token_b_mint.to_bytes()],
            ctx.program_id,
        );

        let pool_signer_seeds = &[
            b"pool".as_ref(),
            &token_a_mint.to_bytes()[..],
            &token_b_mint.to_bytes()[..],
            &[bump],
        ];
        let signer_seeds = &[&pool_signer_seeds[..]];

        // Transfer token B fees to protocol treasury
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_b_vault.to_account_info(),
                    to: ctx.accounts.token_b_destination.to_account_info(),
                    authority: pool_account_info.clone(),
                },
                signer_seeds,
            ),
            protocol_fee_b,
        )?;
    }

    // Emit event for transparency
    emit!(ProtocolFeeCollectionEvent {
        pool: pool.key(),
        authority: ctx.accounts.authority.key(),
        token_a_amount: protocol_fee_a,
        token_b_amount: protocol_fee_b,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
