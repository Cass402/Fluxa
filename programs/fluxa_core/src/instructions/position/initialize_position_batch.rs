use crate::state::position::position_batch::PositionBatch;
use anchor_lang::prelude::*;

/// Context for initializing a new batch account, with explicit zero-copy and PDA constraints for safety and efficiency.
///
/// # Design rationale
/// - Uses Anchor's PDA and zero-copy patterns to ensure deterministic address derivation and efficient account creation.
/// - All seeds and bumps are explicit for replay protection and auditability.
/// - The pool account is left as CHECK to allow for flexible validation logic in the handler.
#[derive(Accounts)]
#[instruction(batch_id: u32, max_positions: u16)]
pub struct InitializeBatch<'info> {
    /// The new batch account, initialized with zero-copy for performance. PDA seeds ensure uniqueness and replay protection.
    #[account(
        init,
        payer = payer,
        space = PositionBatch::calculate_space(max_positions as usize),
        seeds = [b"batch_v2", pool.key().as_ref(), owner.key().as_ref(), &batch_id.to_le_bytes()],
        bump,
    )]
    pub batch: AccountLoader<'info, PositionBatch>,

    /// CHECK: Pool account. Marked as CHECK to allow for custom validation logic in the handler, as pool structure may evolve.
    pub pool: AccountInfo<'info>,

    /// The owner of the batch. Must sign to prevent unauthorized creation.
    pub owner: Signer<'info>,

    /// Pays for account creation and reallocation. Marked as mutable to allow for rent deduction.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// System program for account creation and rent management.
    pub system_program: Program<'info, System>,
}

/// Handler for initializing a new batch account with optimal parameters.
///
/// # Design rationale
/// - Loads the batch account with zero-copy for safety and performance.
/// - Uses the current slot for deterministic lifecycle tracking.
/// - All initialization logic is explicit and auditable, reducing the risk of uninitialized fields.
pub fn initialize_batch(
    ctx: Context<InitializeBatch>,
    batch_id: u32,
    max_positions: u16,
) -> Result<()> {
    let batch = &mut ctx.accounts.batch.load_init()?;
    let clock = Clock::get()?;

    batch.initialize(
        ctx.accounts.pool.key(),
        ctx.accounts.owner.key(),
        batch_id,
        clock.slot,
        max_positions,
    )?;

    Ok(())
}
