use crate::error::PositionError;
use crate::math::core_arithmetic::Q64x64;
use crate::state::position::position_account::CompressedPosition;
use crate::state::position::position_utils;
use crate::utils::constants::ACCOUNT_SIZE_LIMIT;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;

/// Batch account for managing multiple compressed positions with a focus on on-chain efficiency, deterministic layout, and future extensibility.
///
/// # Design rationale
/// - **No Vec prefix**: Avoids Rust Vec overhead and length prefix, enabling direct, predictable memory layout for zero-copy access and account resizing.
/// - **Zero-copy**: Marked `unsafe` to signal the need for careful handling, especially in CPI and multi-threaded contexts. All fields are 8-byte aligned for safe zero-copy.
/// - **Merkle root and compression**: Supports scalable proof-of-inclusion and batch state integrity, with fields for incremental Merkle updates and compression tuning.
/// - **Reserved fields**: Allocated for future upgrades, allowing for non-breaking migrations and protocol evolution.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct PositionBatch {
    /// Pool this batch belongs to. Used for deterministic PDA derivation and access control.
    pub pool: Pubkey, // 32 bytes
    /// Owner of all positions in this batch. Enables efficient per-user queries and batch operations.
    pub owner: Pubkey, // 32 bytes
    /// Unique batch identifier for this (pool, owner) pair. Allows multiple batches per user/pool.
    pub batch_id: u32, // 4 bytes
    /// Number of active positions in this batch. Used for bounds checking and Merkle tree sizing.
    pub position_count: u32, // 4 bytes
    /// Last slot at which this batch was updated. Enables time-based triggers and off-chain analytics.
    pub last_update_slot: u64, // 8 bytes

    /// Total liquidity in this batch. Maintained for fast aggregate queries and protocol accounting.
    pub total_liquidity: Q64x64, // 16 bytes
    /// Cached Merkle root for all positions. Used for proof-of-inclusion and batch integrity.
    pub merkle_root: [u8; 32], // 32 bytes
    /// Compression level for Merkle tree and batch operations. Tuned dynamically for CU/space trade-offs.
    pub compression_level: u16, // 2 bytes
    /// Maximum number of positions allowed in this batch. Enforced for account size and CU safety.
    pub max_positions: u16, // 2 bytes
    /// Bitfield for batch status flags (e.g., active, paused). Enables atomic multi-state transitions.
    pub status_flags: u32, // 4 bytes

    /// Merkle tree state for incremental updates and future optimizations.
    pub merkle_tree_height: u16, // 2 bytes
    /// Padding for 8-byte alignment. Required for safe zero-copy.
    pub _merkle_padding1: [u8; 2], // 2 bytes
    /// Next available leaf index for Merkle tree. Enables efficient append and proof generation.
    pub next_leaf_index: u32, // 4 bytes
    /// Last slot at which the Merkle root was updated. Used for deferred update logic.
    pub last_merkle_update_slot: u64, // 8 bytes
    /// Number of deferred Merkle updates. Enables batching for CU efficiency.
    pub pending_merkle_updates: u32, // 4 bytes
    /// Padding for 8-byte alignment. Required for safe zero-copy.
    pub _merkle_padding2: [u8; 4], // 4 bytes
    /// Reserved for future Merkle tree state (e.g., incremental node storage).
    pub merkle_reserved: [u64; 5], // 40 bytes

    /// Reserved for future protocol upgrades. Ensures backward compatibility and seamless migrations.
    pub reserved: [u64; 5], // 40 bytes
}

impl PositionBatch {
    /// Calculates the total space required for a batch account, including all positions, without a Vec prefix.
    ///
    /// # Why
    /// - Ensures deterministic account sizing for Anchor's `realloc` and Solana's rent model.
    /// - Avoids dynamic Vec overhead, making account resizing and zero-copy safe.
    pub fn calculate_space(max_positions: usize) -> usize {
        8 + // Anchor discriminator (always present)
        std::mem::size_of::<PositionBatch>() +
        max_positions * std::mem::size_of::<CompressedPosition>()
    }

    /// Initializes a new batch with all fields set for safe, deterministic operation.
    ///
    /// # Why
    /// - Ensures all fields are explicitly set, preventing uninitialized state.
    /// - Zeroes out reserved and padding fields for deterministic hashes and future migrations.
    /// - Sets status to active by default, as batches are expected to be live upon creation.
    ///
    /// # Safety
    /// - Must only be called on freshly allocated, zeroed memory (enforced by Anchor's init).
    /// - Not safe to call concurrently due to zero-copy mutability; external synchronization is required in multi-threaded contexts.
    pub fn initialize(
        &mut self,
        pool: Pubkey,
        owner: Pubkey,
        batch_id: u32,
        current_slot: u64,
        max_positions: u16,
    ) -> Result<()> {
        self.pool = pool;
        self.owner = owner;
        self.batch_id = batch_id;
        self.position_count = 0;
        self.last_update_slot = current_slot;
        self.total_liquidity = Q64x64::zero();
        self.merkle_root = [0u8; 32];
        self.compression_level = position_utils::calculate_compression_level(0);
        self.max_positions = max_positions;
        self.status_flags = 0x01; // Set active flag by convention

        // Merkle tree state is always zeroed for deterministic root and future upgrades.
        self.merkle_tree_height = 0;
        self.next_leaf_index = 0;
        self._merkle_padding1 = [0u8; 2];
        self.last_merkle_update_slot = current_slot;
        self.pending_merkle_updates = 0;
        self.merkle_reserved = [0u64; 5];

        self.reserved = [0u64; 5];

        Ok(())
    }

    /// Returns a slice of all positions in this batch, using direct memory access for zero-copy efficiency.
    ///
    /// # Why
    /// - Avoids Vec prefix and heap allocation, enabling deterministic, low-CU access to all positions.
    /// - Used for Merkle root calculation, batch queries, and proof generation.
    pub fn get_positions<'a>(&self, account_data: &'a [u8]) -> Result<&'a [CompressedPosition]> {
        let base_size = std::mem::size_of::<PositionBatch>();
        let positions_start = 8 + base_size; // 8 for Anchor discriminator

        if account_data.len() < positions_start {
            return Ok(&[]);
        }

        let positions_data = &account_data[positions_start..];
        let position_count = self.position_count as usize;

        if positions_data.len() < position_count * std::mem::size_of::<CompressedPosition>() {
            return Err(PositionError::BatchSizeExceeded.into());
        }

        let positions = bytemuck::cast_slice::<u8, CompressedPosition>(positions_data);

        Ok(&positions[..position_count])
    }

    /// Returns a mutable slice of all positions in this batch, using direct memory access for zero-copy efficiency.
    ///
    /// # Why
    /// - Enables in-place mutation of positions without heap allocation or Vec prefix overhead.
    /// - Used for batch updates, insertions, and removals.
    pub fn get_positions_mut<'a>(
        &mut self,
        account_data: &'a mut [u8],
    ) -> Result<&'a mut [CompressedPosition]> {
        let base_size = std::mem::size_of::<PositionBatch>();
        let positions_start = 8 + base_size; // 8 for Anchor discriminator

        if account_data.len() < positions_start {
            return Ok(&mut []);
        }

        let positions_data = &mut account_data[positions_start..];
        let position_count = self.position_count as usize;

        if positions_data.len() < position_count * std::mem::size_of::<CompressedPosition>() {
            return Err(PositionError::BatchSizeExceeded.into());
        }

        let positions = bytemuck::cast_slice_mut::<u8, CompressedPosition>(positions_data);

        Ok(&mut positions[..position_count])
    }

    /// Adds a new position to the batch, maintaining sorted order and preventing duplicates.
    ///
    /// # Why
    /// - Maintains sorted order by position_nonce for O(log n) binary search and efficient proof generation.
    /// - Prevents duplicate nonces to ensure position uniqueness and Merkle tree correctness.
    /// - Updates compression level and Merkle root as needed for protocol invariants and proof freshness.
    pub fn add_position(
        &mut self,
        position: CompressedPosition,
        account_data: &mut [u8],
    ) -> Result<()> {
        if self.position_count >= self.max_positions as u32 {
            return Err(PositionError::BatchSizeExceeded.into());
        }

        let positions = self.get_positions_mut(account_data)?;

        // Check for duplicate nonce using binary search (positions are sorted)
        if positions
            .binary_search_by_key(&position.position_nonce, |p| p.position_nonce)
            .is_ok()
        {
            return Err(PositionError::DuplicatePositionNonce.into());
        }

        // Find insertion point to maintain sorted order
        let insert_index = positions
            .binary_search_by_key(&position.position_nonce, |p| p.position_nonce)
            .unwrap_or_else(|x| x);

        // Extend the slice to accommodate new position
        let all_positions = self.get_positions_mut(account_data)?;
        if all_positions.len() > self.position_count as usize {
            // Shift positions to make room for insertion
            all_positions.copy_within(insert_index..self.position_count as usize, insert_index + 1);
            all_positions[insert_index] = position;
        }

        self.position_count += 1;
        self.total_liquidity = self.total_liquidity.checked_add(position.liquidity)?;

        // Update compression level based on new count
        self.compression_level = position_utils::calculate_compression_level(self.position_count);

        // Handle Merkle root update based on compression level
        if self.should_update_merkle_immediately() {
            self.update_merkle_root(account_data)?;
        } else {
            self.pending_merkle_updates += 1;
        }

        Ok(())
    }

    /// Determines if the Merkle root should be updated immediately based on batch size and protocol thresholds.
    ///
    /// # Why
    /// - Balances compute cost (CU) with proof freshness. Small batches defer updates, large batches update more frequently.
    fn should_update_merkle_immediately(&self) -> bool {
        // Updated thresholds for better granularity
        match self.compression_level {
            1..=2 => false,                             // Defer for small batches
            3..=4 => true,                              // Update for medium batches
            5..=6 => self.pending_merkle_updates >= 10, // Batch updates for large batches
            _ => self.pending_merkle_updates >= 5,      // Frequent updates for very large batches
        }
    }

    /// Finds a position by nonce using O(log n) binary search on the sorted batch.
    ///
    /// # Why
    /// - Enables fast, deterministic lookups for proof generation, updates, and removals.
    pub fn find_position<'a>(
        &self,
        position_nonce: u16,
        account_data: &'a [u8],
    ) -> Result<Option<&'a CompressedPosition>> {
        let positions = self.get_positions(account_data)?;

        match positions.binary_search_by_key(&position_nonce, |p| p.position_nonce) {
            Ok(index) => Ok(Some(&positions[index])),
            Err(_) => Ok(None),
        }
    }

    /// Removes a position by nonce, filling the gap to maintain sorted order.
    ///
    /// # Why
    /// - Maintains batch invariants and Merkle tree correctness.
    /// - Efficiently updates total liquidity and compression level.
    pub fn remove_position(&mut self, position_nonce: u16, account_data: &mut [u8]) -> Result<()> {
        let positions = self.get_positions_mut(account_data)?;

        let found_index = positions
            .binary_search_by_key(&position_nonce, |p| p.position_nonce)
            .map_err(|_| PositionError::PositionNotFound)?;

        // Update total liquidity
        self.total_liquidity = self
            .total_liquidity
            .checked_sub(positions[found_index].liquidity)?;

        // Shift positions to fill gap (maintains sorted order)
        let position_count = self.position_count as usize;
        positions.copy_within((found_index + 1)..position_count, found_index);

        self.position_count -= 1;

        // Update compression level
        self.compression_level = position_utils::calculate_compression_level(self.position_count);

        // Handle Merkle root update
        if self.should_update_merkle_immediately() {
            self.update_merkle_root(account_data)?;
        } else {
            self.pending_merkle_updates += 1;
        }

        Ok(())
    }

    /// Updates the liquidity of a position, using binary search for O(log n) access.
    ///
    /// # Why
    /// - Ensures batch totals and Merkle root remain correct after liquidity changes.
    /// - Maintains sorted order and enables efficient proof updates.
    pub fn update_position_liquidity(
        &mut self,
        position_nonce: u16,
        new_liquidity: Q64x64,
        account_data: &mut [u8],
    ) -> Result<()> {
        let positions = self.get_positions_mut(account_data)?;

        let found_index = positions
            .binary_search_by_key(&position_nonce, |p| p.position_nonce)
            .map_err(|_| PositionError::PositionNotFound)?;

        let old_liquidity = positions[found_index].liquidity;
        positions[found_index].liquidity = new_liquidity;

        // Update batch totals
        self.total_liquidity = self
            .total_liquidity
            .checked_sub(old_liquidity)?
            .checked_add(new_liquidity)?;

        // Handle Merkle root update
        if self.should_update_merkle_immediately() {
            self.update_merkle_root(account_data)?;
        } else {
            self.pending_merkle_updates += 1;
        }

        Ok(())
    }

    /// Updates the Merkle root for the batch. Currently a full recalculation; future versions may use true incremental updates.
    ///
    /// # Why
    /// - Ensures batch integrity and enables proof-of-inclusion for all positions.
    /// - Full recalculation is used for simplicity and safety; incremental updates would require storing intermediate nodes.
    ///
    /// # Note
    /// - This is a placeholder for future optimization. True incremental updates would further reduce CU cost.
    fn update_merkle_root(&mut self, account_data: &[u8]) -> Result<()> {
        let positions = self.get_positions(account_data)?;

        if positions.is_empty() {
            self.merkle_root = [0u8; 32];
            self.merkle_tree_height = 0;
            self.next_leaf_index = 0;
        } else {
            // For now, this is still a full recalculation
            // TODO: Implement true incremental updates with stored tree state
            self.merkle_root = Self::calculate_merkle_root_from_positions(positions)?;
            self.merkle_tree_height = (positions.len() as f64).log2().ceil() as u16;
            self.next_leaf_index = positions.len() as u32;
        }

        self.last_merkle_update_slot = self.last_update_slot;
        self.pending_merkle_updates = 0;

        Ok(())
    }

    /// Calculates the Merkle root from the current positions array.
    ///
    /// # Why
    /// - Used for batch integrity and proof-of-inclusion. Hashes are computed in pairs up the tree.
    fn calculate_merkle_root_from_positions(positions: &[CompressedPosition]) -> Result<[u8; 32]> {
        if positions.is_empty() {
            return Ok([0u8; 32]);
        }

        if positions.len() == 1 {
            return Ok(positions[0].calculate_hash());
        }

        let mut leaves: Vec<[u8; 32]> = positions.iter().map(|p| p.calculate_hash()).collect();

        while leaves.len() > 1 {
            let mut next_level = Vec::new();

            for chunk in leaves.chunks(2) {
                let hash = if chunk.len() == 2 {
                    keccak::hashv(&[&chunk[0], &chunk[1]]).to_bytes()
                } else {
                    keccak::hashv(&[&chunk[0], &chunk[0]]).to_bytes()
                };
                next_level.push(hash);
            }

            leaves = next_level;
        }

        Ok(leaves[0])
    }

    /// Forces a Merkle root update if there are deferred updates pending.
    ///
    /// # Why
    /// - Used to amortize CU cost by batching Merkle updates, but ensures proof freshness when needed.
    pub fn batch_update_merkle_root(&mut self, account_data: &[u8]) -> Result<()> {
        if self.pending_merkle_updates > 0 {
            self.update_merkle_root(account_data)?;
        }
        Ok(())
    }

    /// Verifies a Merkle proof for position inclusion in the batch.
    ///
    /// # Why
    /// - Enables off-chain and on-chain clients to verify that a position is included in the batch without full account data.
    pub fn verify_position_proof(
        &self,
        position: &CompressedPosition,
        proof: &[[u8; 32]],
        account_data: &[u8],
    ) -> Result<()> {
        let positions = self.get_positions(account_data)?;
        let position_index = positions
            .binary_search_by_key(&position.position_nonce, |p| p.position_nonce)
            .map_err(|_| PositionError::PositionNotFound)?;

        let leaf_hash = position.calculate_hash();
        let mut current_hash = leaf_hash;
        let mut current_index = position_index;

        // Verify proof path
        for sibling_hash in proof {
            current_hash = if current_index % 2 == 0 {
                keccak::hashv(&[&current_hash, sibling_hash]).to_bytes()
            } else {
                keccak::hashv(&[sibling_hash, &current_hash]).to_bytes()
            };
            current_index /= 2;
        }

        if current_hash != self.merkle_root {
            return Err(PositionError::MerkleProofFailed.into());
        }

        Ok(())
    }

    /// Returns the utilization percentage of the batch (positions used vs. max allowed).
    ///
    /// # Why
    /// - Used for monitoring, analytics, and protocol-level scaling decisions.
    #[inline(always)]
    pub fn utilization_percentage(&self) -> u8 {
        if self.max_positions == 0 {
            return 0;
        }
        ((self.position_count * 100) / self.max_positions as u32) as u8
    }
}

/// Enumerates all supported batch position update operations, designed for extensibility and efficient batch processing.
///
/// # Design rationale
/// - Each variant is a distinct operation that can be applied to a position in a batch, enabling atomic, multi-operation updates in a single transaction.
/// - The enum is designed for future expansion (e.g., fee sync, pause/reopen) without breaking serialization or requiring protocol migration.
/// - Using explicit fields (not tuples) for each variant improves auditability and clarity for off-chain clients.
#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub enum PositionUpdate {
    /// Update the liquidity of a position (most common operation).
    UpdateLiquidity {
        /// Unique nonce for the position to update (prevents ambiguity and enables O(log n) lookup).
        position_nonce: u16,
        /// New liquidity value (raw Q64x64 format expected by batch logic).
        new_liquidity: u128,
    },
    /// Close (remove) a position from the batch.
    ClosePosition {
        /// Unique nonce for the position to close.
        position_nonce: u16,
    },
    // /// Sync fee fields for a position (future extension: requires cross-account mutation).
    // SyncFees {
    //     position_nonce: u16,
    //     tokens_owed_0: u64,
    //     tokens_owed_1: u64,
    // },
    // /// Pause a position (future extension: requires status flag mutation in position account).
    // PausePosition {
    //     position_nonce: u16,
    // },
    // /// Reopen a paused position (future extension).
    // ReopenPosition {
    //     position_nonce: u16,
    // },
}

/// Calculates the optimal batch size for a given CU (compute unit) limit and account size constraints.
///
/// # Design rationale
/// - Ensures that batch operations do not exceed Solana's CU or account size limits, preventing transaction failures.
/// - Considers both compute and storage constraints, using the most restrictive as the upper bound.
/// - Scales batch size based on actual position count for efficient packing and protocol flexibility.
pub fn calculate_optimal_batch_size_v2(
    position_count: u32,
    target_cu_limit: u32,
    _avg_position_size: usize,
) -> Result<usize> {
    // Estimate CU cost for batch operations (empirically tuned for protocol safety)
    let base_cu_cost = 5000; // Fixed cost for batch initialization and account setup
    let per_position_cu_cost = 150; // Marginal cost per position (hashing, sorting, Merkle update)
    let merkle_update_cu_cost = 1000; // Cost for Merkle tree update (can be deferred)

    // Compute-based limit: how many positions fit within the CU budget
    let available_cu = target_cu_limit.saturating_sub(base_cu_cost + merkle_update_cu_cost);
    let max_positions_by_cu = available_cu / per_position_cu_cost;

    // Storage-based limit: how many positions fit within the account size (e.g., 10KB)
    let base_size = PositionBatch::calculate_space(0);
    let available_space = ACCOUNT_SIZE_LIMIT.saturating_sub(base_size);
    let max_positions_by_space = available_space / std::mem::size_of::<CompressedPosition>();

    // Use the most restrictive limit to avoid exceeding either constraint
    let max_positions = std::cmp::min(max_positions_by_cu, max_positions_by_space as u32);

    // Scale batch size for practical usage: small batches are capped for efficiency, large batches for safety
    let optimal_size = match position_count {
        0..=50 => std::cmp::min(50, max_positions),
        51..=200 => std::cmp::min(100, max_positions),
        201..=1000 => std::cmp::min(200, max_positions),
        _ => max_positions,
    };

    Ok(optimal_size as usize)
}

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
