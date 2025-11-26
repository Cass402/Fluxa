use crate::math::core_arithmetic::Q64x64;
use crate::state::position::position_batch::{PositionBatch, PositionUpdate};
use anchor_lang::prelude::*;

/// Context for batch position updates, with explicit zero-copy and PDA constraints for safety and efficiency.
///
/// # Design rationale
/// - Uses Anchor's PDA and zero-copy patterns to ensure deterministic address derivation and efficient batch mutation.
/// - All seeds and bumps are explicit for replay protection and auditability.
/// - The pool account is left as CHECK to allow for flexible validation logic in the handler.
#[derive(Accounts)]
pub struct BatchUpdatePositions<'info> {
    /// The batch account to update, loaded with zero-copy for performance.
    #[account(
        mut,
        seeds = [b"batch_v2", pool.key().as_ref(), owner.key().as_ref(), &batch.load()?.batch_id.to_le_bytes()],
        bump,
    )]
    pub batch: AccountLoader<'info, PositionBatch>,

    /// CHECK: Pool account. Marked as CHECK to allow for custom validation logic in the handler, as pool structure may evolve.
    pub pool: AccountInfo<'info>,

    /// The owner of the batch. Must sign to prevent unauthorized updates.
    pub owner: Signer<'info>,
}

/// Handler for batch position updates, with Merkle root verification and deferred update logic.
///
/// # Design rationale
/// - Processes multiple position updates atomically, reducing CU cost and improving UX.
/// - Uses zero-copy for efficient batch mutation and Merkle root recalculation.
/// - Defers Merkle root updates when possible, but forces update if too many slots have passed for proof freshness.
pub fn batch_update_positions(
    ctx: Context<BatchUpdatePositions>,
    updates: Vec<PositionUpdate>,
) -> Result<()> {
    let batch = &mut ctx.accounts.batch.load_mut()?;
    let clock = Clock::get()?;
    let account_info = ctx.accounts.batch.to_account_info();
    let mut account_data = account_info.data.borrow_mut();

    // Process all updates in a single loop for efficiency and atomicity
    for update in updates {
        match update {
            PositionUpdate::UpdateLiquidity {
                position_nonce,
                new_liquidity,
            } => {
                batch.update_position_liquidity(
                    position_nonce,
                    Q64x64::from_raw(new_liquidity),
                    &mut account_data,
                )?;
            }
            PositionUpdate::ClosePosition { position_nonce } => {
                batch.remove_position(position_nonce, &mut account_data)?;
            } // Future extensions (fee sync, pause, reopen) would be handled here, requiring cross-account logic.
        }
    }

    // Force Merkle root update if we have pending updates and it's been too long (prevents stale proofs)
    let slots_since_last_update = clock.slot.saturating_sub(batch.last_merkle_update_slot);
    if batch.pending_merkle_updates > 0 && slots_since_last_update > 100 {
        batch.batch_update_merkle_root(&account_data)?;
    }

    batch.last_update_slot = clock.slot;

    Ok(())
}
