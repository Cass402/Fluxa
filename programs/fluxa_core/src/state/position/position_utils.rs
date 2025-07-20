use crate::error::PositionError;
use anchor_lang::prelude::*;

/// Determines the optimal compression level for a batch based on the number of positions, tuning Merkle update frequency and CU cost.
///
/// # Why
/// - Compression level is used to balance proof freshness, CU cost, and batch update frequency.
/// - Granular levels allow the protocol to scale efficiently from small to very large batches.
pub fn calculate_compression_level(position_count: u32) -> u16 {
    match position_count {
        0..=25 => 1,     // Minimal compression: fast updates, low CU for small batches
        26..=75 => 2,    // Light compression: slightly deferred updates
        76..=150 => 3,   // Moderate compression: balances update cost and proof freshness
        151..=300 => 4,  // Medium-high compression: for growing batches
        301..=600 => 5,  // High compression: for large batches, more deferred updates
        601..=1200 => 6, // Very high compression: for very large batches, infrequent updates
        _ => 7,          // Maximum compression: only update when absolutely necessary
    }
}

/// Estimates the compute unit (CU) cost for a batch operation, factoring in batch size, operation type, and Merkle update policy.
///
/// # Why
/// - Used to enforce protocol CU limits and optimize transaction packing.
/// - Merkle cost is dynamically adjusted based on compression level, reflecting deferred or immediate updates.
pub fn estimate_batch_cu_cost(position_count: u32, operation_type: BatchOperationType) -> u32 {
    let base_cost = match operation_type {
        BatchOperationType::Create => 5000, // Higher cost for account creation and initialization
        BatchOperationType::Update => 3000, // Moderate cost for updates
        BatchOperationType::Remove => 2000, // Lower cost for removals
    };

    let per_position_cost = match operation_type {
        BatchOperationType::Create => 200, // Each new position increases CU due to hashing, sorting
        BatchOperationType::Update => 150, // Updates are cheaper but still scale with batch size
        BatchOperationType::Remove => 100, // Removals are cheapest, mostly shifting and hash update
    };

    // Merkle cost reflects the trade-off between proof freshness and CU savings
    let compression_level = calculate_compression_level(position_count);
    let merkle_cost = match compression_level {
        1..=2 => 200,  // Minimal cost: updates are deferred
        3..=4 => 800,  // Regular updates: moderate cost
        5..=6 => 1200, // Frequent updates: higher cost
        _ => 1500,     // Maximum update frequency: highest cost
    };

    base_cost + (position_count * per_position_cost) + merkle_cost
}

/// Validates position parameters for logical correctness and protocol safety.
///
/// # Why
/// - Prevents creation of invalid or degenerate positions that could break invariants or cause loss of funds.
/// - Ensures tick range is non-empty and liquidity is nonzero.
pub fn validate_position_parameters(
    tick_lower: i32,
    tick_upper: i32,
    liquidity: u128,
) -> Result<()> {
    if tick_lower >= tick_upper {
        // Defensive: tick range must be non-empty for valid price intervals
        return Err(PositionError::InvalidPositionHash.into());
    }

    if liquidity == 0 {
        // Defensive: zero-liquidity positions are not allowed
        return Err(PositionError::InvalidPositionHash.into());
    }

    Ok(())
}

/// Determines whether a Merkle root update should be forced based on batch state and protocol thresholds.
///
/// # Why
/// - Balances proof freshness with CU efficiency by deferring updates when safe.
/// - Protocol can tune thresholds for different compression levels to optimize for scale or latency.
pub fn should_force_merkle_update(
    pending_updates: u32,
    slots_since_last_update: u64,
    compression_level: u16,
) -> bool {
    let max_pending = match compression_level {
        1..=2 => 50, // Low compression: allow many deferred updates for CU savings
        3..=4 => 20, // Moderate: balance between update frequency and cost
        5..=6 => 10, // High: require more frequent updates
        _ => 5,      // Maximum: force updates quickly for proof freshness
    };

    let max_slots = match compression_level {
        1..=2 => 200, // Low compression: tolerate long delays
        3..=4 => 100, // Moderate: moderate delay
        5..=6 => 50,  // High: short delay
        _ => 25,      // Maximum: very short delay
    };

    pending_updates >= max_pending || slots_since_last_update >= max_slots
}

/// Enumerates the types of batch operations, used for CU estimation and protocol logic.
///
/// # Why
/// - Enables type-safe, explicit handling of batch operation costs and logic.
#[derive(Clone, Copy, Debug)]
pub enum BatchOperationType {
    /// Creating a new batch or position (highest CU cost)
    Create,
    /// Updating an existing position (moderate CU cost)
    Update,
    /// Removing a position (lowest CU cost)
    Remove,
}
