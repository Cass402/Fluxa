use crate::error::PositionError;
use crate::math::core_arithmetic::Q64x64;
use crate::state::position::position_batch::PositionBatch;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use bytemuck::{Pod, Zeroable};

#[account(zero_copy(unsafe))]
#[repr(C)]
/// Represents a single liquidity position in the protocol, with a layout and field selection optimized for on-chain efficiency, safety, and future extensibility.
///
/// # Design rationale
/// - **Zero-copy**: Enables direct deserialization from account data, minimizing compute and memory overhead. Marked `unsafe` to signal the need for careful handling, especially in CPI contexts.
/// - **Explicit alignment and padding**: All fields are aligned to 8-byte boundaries to ensure compatibility with Solana's account storage and to avoid undefined behavior in zero-copy reads/writes.
/// - **Field grouping**: Related fields are grouped for cache locality and to facilitate future extensions without breaking layout.
/// - **Hash-based integrity**: A cached hash is stored to allow fast, on-chain integrity checks, reducing the risk of undetected state corruption.
/// - **Reserved space**: Allocated for future upgrades, allowing for non-breaking migrations.
pub struct Position {
    /// The owner of this position. Chosen as the primary identifier for access control and lookup.
    pub owner: Pubkey, // 32 bytes
    /// Lower bound of the tick range. Chosen as i32 for compatibility with tick math and to save space.
    pub tick_lower: i32, // 4 bytes
    /// Upper bound of the tick range.
    pub tick_upper: i32, // 4 bytes

    /// Bitfield for status flags. Enables efficient multi-state tracking (active, closed, paused, etc.) and atomic updates.
    pub status_flags: u32, // 4 bytes
    /// Nonce to prevent replay and ensure uniqueness of PDAs. u16 is sufficient for practical use and saves space over u64.
    pub position_nonce: u16, // 2 bytes

    pub _padding1: [u8; 2],

    /// Amount of liquidity provided. Uses Q64x64 for high-precision math, matching protocol arithmetic.
    pub liquidity: Q64x64, // 16 bytes

    /// Tracks fee growth inside the position's tick range at last update for both tokens. This enables precise fee accounting and minimizes on-chain computation.
    pub fee_growth_inside_0_last: Q64x64, // 16 bytes
    pub fee_growth_inside_1_last: Q64x64, // 16 bytes
    /// Amount of uncollected fees owed to the position owner. Split by token for composability and protocol flexibility.
    pub tokens_owed_0: Q64x64, // 16 bytes
    pub tokens_owed_1: Q64x64,            // 16 bytes
    /// Cumulative fees collected over the lifetime of the position. Useful for analytics and auditing.
    pub total_fees_collected_0: Q64x64, // 16 bytes
    pub total_fees_collected_1: Q64x64,   // 16 bytes

    /// Slot and timestamp metadata for lifecycle tracking and off-chain analytics. Both are stored to support both deterministic and wall-clock queries.
    pub creation_slot: u64, // 8 bytes
    pub last_update_slot: u64,   // 8 bytes
    pub creation_timestamp: i64, // 8 bytes

    /// Cached hash of core position fields. Used for fast, on-chain integrity verification and to detect unauthorized mutations.
    pub position_hash: [u8; 32], // 32 bytes

    /// Reserved for future protocol upgrades. Ensures backward compatibility and allows for seamless migrations.
    pub reserved: [u64; 3], // 24 bytes
}

/// Bundles all required arguments for initializing a Position, ensuring explicit and auditable construction.
///
/// # Why
/// - Prevents accidental omission of required fields during initialization.
/// - Groups all context needed for a safe, deterministic position creation.
pub struct InitArgs {
    pub owner: Pubkey,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub liquidity: Q64x64,
    pub position_nonce: u16,
    pub current_slot: u64,
    pub current_timestamp: i64,
}

impl Position {
    /// Bitflags for position status. Using a bitfield allows atomic, multi-state transitions and future extensibility without breaking layout.
    pub const FLAG_ACTIVE: u32 = 0x01;
    pub const FLAG_CLOSED: u32 = 0x02;
    pub const FLAG_EMERGENCY_PAUSE: u32 = 0x04;
    pub const FLAG_PENDING_REMOVAL: u32 = 0x08;

    /// Initializes a new position, setting all fields and computing the initial hash.
    ///
    /// # Why
    /// - Ensures all fields are explicitly set, preventing uninitialized state.
    /// - Hash is computed at creation to enable later integrity checks.
    /// - Zeroes out reserved and padding fields to avoid leaking uninitialized memory.
    /// - Sets status to active by default, as positions are expected to be live upon creation.
    ///
    /// # Safety
    /// - Must only be called on freshly allocated, zeroed memory (enforced by Anchor's init).
    /// - Not safe to call concurrently due to zero-copy mutability.
    pub fn initialize(&mut self, args: InitArgs) -> Result<()> {
        self.owner = args.owner;
        self.tick_lower = args.tick_lower;
        self.tick_upper = args.tick_upper;
        self.liquidity = args.liquidity;
        self.position_nonce = args.position_nonce;
        self._padding1 = [0u8; 2]; // Defensive: always zero padding for deterministic hashes

        // Fee fields are always zero at creation to prevent fee leakage or double counting.
        self.fee_growth_inside_0_last = Q64x64::zero();
        self.fee_growth_inside_1_last = Q64x64::zero();
        self.tokens_owed_0 = Q64x64::zero();
        self.tokens_owed_1 = Q64x64::zero();
        self.total_fees_collected_0 = Q64x64::zero();
        self.total_fees_collected_1 = Q64x64::zero();

        // Both slot and timestamp are set for robust lifecycle tracking (on-chain and off-chain).
        self.creation_slot = args.current_slot;
        self.last_update_slot = args.current_slot;
        self.creation_timestamp = args.current_timestamp;
        self.status_flags = Self::FLAG_ACTIVE;

        // Hash is cached to allow O(1) integrity checks later.
        self.position_hash = self.calculate_optimized_hash();

        // Reserved space is always zeroed to avoid accidental leakage and to future-proof migrations.
        self.reserved = [0u64; 3];

        Ok(())
    }

    /// Computes a hash of the core position fields, avoiding heap allocation.
    ///
    /// # Why
    /// - Avoids Vec allocation for performance and determinism on-chain.
    /// - Only hashes fields that define the unique identity of a position.
    /// - Used for both integrity checks and as a potential anti-replay mechanism.
    #[inline(always)]
    pub fn calculate_optimized_hash(&self) -> [u8; 32] {
        keccak::hashv(&[
            &self.owner.to_bytes(),
            &self.tick_lower.to_le_bytes(),
            &self.tick_upper.to_le_bytes(),
            &self.liquidity.raw().to_le_bytes(),
            &self.position_nonce.to_le_bytes(),
        ])
        .to_bytes()
    }

    /// Verifies that the cached hash matches the current state.
    ///
    /// # Why
    /// - Detects unauthorized or accidental mutations to critical fields.
    /// - Used as a lightweight, on-chain integrity check for auditors and protocol invariants.
    #[inline(always)]
    pub fn verify_integrity(&self) -> Result<()> {
        let calculated_hash = self.calculate_optimized_hash();
        if calculated_hash != self.position_hash {
            return Err(PositionError::PositionIntegrityFailure.into());
        }
        Ok(())
    }

    /// Validates the position nonce against an expected value.
    ///
    /// # Why
    /// - Prevents replay attacks and ensures uniqueness of PDAs.
    /// - Used in all state transitions that depend on position identity.
    #[inline(always)]
    pub fn validate_nonce(&self, expected_nonce: u16) -> Result<()> {
        if self.position_nonce != expected_nonce {
            return Err(PositionError::InvalidPositionNonce.into());
        }
        Ok(())
    }

    /// Checks if the position is currently active.
    ///
    /// # Why
    /// - Bitwise flag checks are O(1) and allow for atomic multi-state transitions.
    #[inline(always)]
    pub fn is_active(&self) -> bool {
        (self.status_flags & Self::FLAG_ACTIVE) != 0
    }

    /// Marks the position as closed, atomically updating the status flags.
    ///
    /// # Why
    /// - Ensures that only one state transition is needed to deactivate a position.
    /// - Bitwise operations allow for future expansion of status flags without breaking logic.
    #[inline(always)]
    pub fn set_closed(&mut self) {
        self.status_flags = (self.status_flags & !Self::FLAG_ACTIVE) | Self::FLAG_CLOSED;
    }

    /// Updates the liquidity and last update slot, recalculating the hash.
    ///
    /// # Why
    /// - Ensures that any change to liquidity is tracked and integrity-protected.
    /// - Hash is only recalculated when necessary, minimizing compute cost.
    pub fn update_liquidity(&mut self, new_liquidity: Q64x64, current_slot: u64) -> Result<()> {
        self.liquidity = new_liquidity;
        self.last_update_slot = current_slot;
        self.position_hash = self.calculate_optimized_hash();
        Ok(())
    }
}

/// A compact representation of a position, designed for efficient batch storage and transfer.
///
/// # Design rationale
/// - Only includes fields essential for batch operations, omitting fee and metadata fields to save space.
/// - Packs tick range into a single u64 for atomicity and to reduce serialization overhead.
/// - Maintains alignment and padding for safe zero-copy usage in batch arrays.
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
#[repr(C)]
pub struct CompressedPosition {
    /// Owner of the position. Used for batch lookups and access control.
    pub owner: Pubkey, // 32 bytes
    /// Packed tick range (lower and upper) for atomic updates and compactness.
    pub tick_range: u64, // 8 bytes
    /// Nonce for uniqueness and replay protection.
    pub position_nonce: u16, // 2 bytes
    /// Padding for alignment, required for safe zero-copy.
    pub _padding: [u8; 6], // 6 bytes
    /// Liquidity amount, using the same precision as the main Position struct.
    pub liquidity: Q64x64, // 16 bytes
}

impl CompressedPosition {
    /// Packs two i32 tick values into a single u64 for compactness and atomicity.
    ///
    /// # Why
    /// - Reduces storage and serialization cost in batch operations.
    /// - Ensures that tick ranges are always updated together, preventing partial state.
    pub fn pack_tick_range(tick_lower: i32, tick_upper: i32) -> u64 {
        ((tick_lower as u64) << 32) | (tick_upper as u64 & 0xFFFFFFFF)
    }

    /// Unpacks a u64 into two i32 tick values.
    ///
    /// # Why
    /// - Enables efficient decoding of batch data without heap allocation.
    pub fn unpack_tick_range(tick_range: u64) -> (i32, i32) {
        let tick_lower = (tick_range >> 32) as i32;
        let tick_upper = (tick_range & 0xFFFFFFFF) as i32;
        (tick_lower, tick_upper)
    }

    /// Constructs a compressed position from a full Position struct.
    ///
    /// # Why
    /// - Used for batch writes and off-chain analytics where only essential fields are needed.
    /// - Ensures that batch state is always derived from canonical on-chain state.
    pub fn from_position(position: &Position) -> Self {
        Self {
            owner: position.owner,
            tick_range: Self::pack_tick_range(position.tick_lower, position.tick_upper),
            liquidity: position.liquidity,
            position_nonce: position.position_nonce,
            _padding: [0u8; 6],
        }
    }

    /// Computes a hash of the compressed position fields, avoiding heap allocation.
    ///
    /// # Why
    /// - Enables fast, deterministic integrity checks in batch operations.
    /// - Avoids Vec allocation for on-chain performance.
    #[inline(always)]
    pub fn calculate_hash(&self) -> [u8; 32] {
        keccak::hashv(&[
            &self.owner.to_bytes(),
            &self.tick_range.to_le_bytes(),
            &self.liquidity.raw().to_le_bytes(),
            &self.position_nonce.to_le_bytes(),
        ])
        .to_bytes()
    }
}

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
