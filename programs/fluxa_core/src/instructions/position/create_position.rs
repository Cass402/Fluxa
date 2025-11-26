use crate::math::core_arithmetic::Q64x64;
use crate::state::position::position_account::{CompressedPosition, InitArgs, Position};
use crate::state::position::position_batch::PositionBatch;
use anchor_lang::prelude::*;

/// Context for creating a new position, with batch integration for efficient state management.
///
/// # Design rationale
/// - Uses Anchor's PDA and zero-copy patterns for safety and efficiency.
/// - Batch account is reallocated in-place to append the new position, minimizing account churn and rent costs.
/// - All seeds and bumps are explicit to ensure deterministic address derivation and replay protection.
/// - The pool account is left as a CHECK to allow for flexible validation logic in the handler.
#[derive(Accounts)]
#[instruction(position_nonce: u16)]
pub struct CreatePosition<'info> {
    /// The new position account, initialized with zero-copy for performance. PDA seeds ensure uniqueness and replay protection.
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<Position>(),
        seeds = [b"position_v2", pool.key().as_ref(), owner.key().as_ref(), &position_nonce.to_le_bytes()],
        bump,
    )]
    pub position: AccountLoader<'info, Position>,

    /// The batch account, which aggregates multiple positions for the same owner/pool. Reallocated in-place to append the new position, saving rent and compute.
    #[account(
        mut,
        seeds = [b"batch_v2", pool.key().as_ref(), owner.key().as_ref(), &batch.load()?.batch_id.to_le_bytes()],
        bump,
        realloc = PositionBatch::calculate_space((batch.load()?.position_count + 1) as usize),
        realloc::payer = payer,
        realloc::zero = false, // Preserves existing data for safety and auditability.
    )]
    pub batch: AccountLoader<'info, PositionBatch>,

    /// CHECK: Pool account. Marked as CHECK to allow for custom validation logic in the handler, as pool structure may evolve.
    pub pool: AccountInfo<'info>,

    /// The owner of the position. Must sign to prevent unauthorized creation.
    pub owner: Signer<'info>,

    /// Pays for account creation and reallocation. Marked as mutable to allow for rent deduction.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// System program for account creation and rent management.
    pub system_program: Program<'info, System>,
}

/// Creates a new position and adds it to the batch, initializing all fields and computing the initial hash.
/// # Design rationale
/// - Initializes the position with both slot and timestamp for robust lifecycle tracking.
/// - Adds the position to the batch with dynamic account data, allowing for efficient state management.
/// - Uses zero-copy patterns to minimize compute and memory overhead.
/// - Ensures that the position is always created with a valid owner and nonce, preventing replay attacks.
pub fn create_position(
    ctx: Context<CreatePosition>,
    tick_lower: i32,
    tick_upper: i32,
    liquidity: u128,
    position_nonce: u16,
) -> Result<()> {
    // Load mutable references to position and batch accounts
    // This ensures that we can modify the position and batch data in-place without unnecessary copies.
    let position = &mut ctx.accounts.position.load_init()?;
    let batch = &mut ctx.accounts.batch.load_mut()?;
    let clock = Clock::get()?;

    // Initialize the position with all required fields
    position.initialize(InitArgs {
        owner: ctx.accounts.owner.key(),
        tick_lower,
        tick_upper,
        liquidity: Q64x64::from_raw(liquidity),
        position_nonce,
        current_slot: clock.slot,
        current_timestamp: clock.unix_timestamp,
    })?;

    // Add the position to the batch, which reallocates the batch account in-place to append the new position.
    // This minimizes account churn and rent costs, while also ensuring that the batch state is always derived from canonical on-chain state.
    let compressed_position = CompressedPosition::from_position(position);
    let account_info = ctx.accounts.batch.to_account_info();
    let mut account_data = account_info.data.borrow_mut();

    batch.add_position(compressed_position, &mut account_data)?;

    Ok(())
}
