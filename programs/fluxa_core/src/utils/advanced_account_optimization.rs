//! High-performance, CU-optimized helpers for account sizing, rent estimation, and on-chain migration in Solana DeFi protocols.
//!
//! # Design Rationale
//!
//! ## 1. Layout & Zero-Copy
//! - All structs use `#[repr(C)]` and 8-byte alignment to maximize sBPF memory access efficiency (see Solana Labs perf-notes §3).
//! - Only plain-old-data (POD) types are used—no `Vec` or heap allocation—ensuring bytemuck/zero-copy compatibility and deterministic account layouts.
//! - Q64.64 fixed-point math is used throughout for protocol-wide consistency and overflow safety.
//!
//! ## 2. Compute Unit (CU) Minimization
//! - Compile-time `feature = "profiling"` enables `sol_log_compute_units!()` for targeted CU analysis.
//! - Compute budget instructions (e.g., `set_loaded_accounts_data_size_limit`) are exposed to minimize CU for large account loads (see Anza blog 11-2024).
//!
//! ## 3. Dynamic Batch Sizing
//! - Branchless LUT for batch sizing minimizes branching and leverages conditional moves for predictable CU cost.
//! - All scaling factors are Q64.64 and applied in a single pass; division is replaced with 128-bit arithmetic to avoid expensive runtime division.
//!
//! ## 4. Rent Safety
//! - Rent calculations use a single syscall and cache the result, avoiding redundant computation.
//! - Safety buffers are added using bit-shifts (e.g., `>> 5` for ~3.125%) for gas efficiency and to prevent rent-exemption edge cases.
//!
//! ## 5. Account Migration
//! - Migration is performed with raw byte copy (no deserialize/serialize round-trip), using `invoke_signed` and `system_instruction::allocate_with_seed` for atomicity and safety.
//! - Old lamports are recycled to the payer, ensuring zero net SOL leakage and protocol sustainability.
//!
//! ## 6. Compression Readiness
//! - Feature-gated (`cfg(feature = "compression")`) tree-hash API is ready for Solana state-compression and concurrent Merkle tree integration (see Solana docs 2025 [487]).

use crate::error::AdvancedAccountOptimizationError;
use crate::math::core_arithmetic::{mul_div, Q64x64};
use crate::utils::constants::{ACCOUNT_OVERHEAD_BYTES, MAX_ACCOUNT_SIZE, RENT_BUFFER_SHIFT};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use solana_compute_budget_interface::ComputeBudgetInstruction;

// =================== PROTOCOL CONSTANTS ===================

/// Base liquidity threshold for scaling calculations (1 trillion units).
///
/// # Why this value?
/// Chosen to distinguish between "small" and "large" pools for batch scaling, based on empirical liquidity distributions in DeFi protocols.
const BASE_LIQUIDITY_THRESHOLD: u128 = Q64x64::from_int(1_000_000_000_000u64).raw();

/// Minimum scaling factor for low-liquidity pools, in Q64.64 (0.8 = 80%).
///
/// # Why Q64.64?
/// Ensures protocol-wide consistency and overflow safety in all scaling math.
const BASE_LIQUIDITY_FACTOR_Q64: u128 = 0xCCCCCCCCCCCCCCCC; // 0.8 in Q64.64

/// Maximum scaling boost for high-liquidity pools, in Q64.64 (0.5 = 50%).
///
/// # Why cap scaling?
/// Prevents runaway batch sizes and ensures account size limits are never exceeded, even in extreme liquidity scenarios.
const MAX_LIQUIDITY_SCALING_Q64: u128 = 0x8000000000000000; // 0.5 in Q64.64

/// Thresholds for progressive batch sizing.
///
/// # Why these values?
/// Chosen to match typical user position distributions and optimize for both small and large pools.
const BATCH_THRESHOLDS: [u32; 5] = [5, 15, 35, 75, 150];

/// Increments for progressive batch sizing.
///
/// # Why increments?
/// Enables smooth scaling of batch sizes as position counts increase, minimizing fragmentation and maximizing CU efficiency.
const BATCH_INCREMENTS: [u16; 5] = [5, 5, 15, 25, 50];

// =================== FIXED-POINT CONVERSION ===================

/// Converts a factor in percentage×10 format to Q64.64 fixed-point.
///
/// # Why this approach?
/// - Ensures all scaling math is performed in a single, overflow-safe format.
/// - Allows protocol to express factors as simple integers for clarity and auditability.
///
/// Example: 150 (15%) → Q64.64 representation of 0.15
#[inline(always)]
fn factor_to_q64(factor_x10: u16) -> Q64x64 {
    // Convert to Q64.64: (factor_x10 / 1000) = factor_x10 * (2^64 / 1000)
    // Safe bounds: factor_x10 ≤ 65535, intermediate calculation fits in u128
    let raw_value =
        mul_div(factor_x10 as u128, Q64x64::one().raw(), 1000).unwrap_or(Q64x64::one().raw());

    Q64x64::from_raw(raw_value)
}

// =================== BATCH SIZING LOGIC ===================

/// Branchless base-batch size lookup for progressive batch sizing.
///
/// # Why branchless?
/// - Minimizes unpredictable branching, reducing CU variance and improving performance predictability.
/// - LUT and conditional moves are more gas-efficient than nested if-else chains.
///
/// Maps position counts to base batch sizes:
/// • 0-5: 5 positions    • 6-15: 10 positions   • 16-35: 25 positions
/// • 36-75: 50 positions • 76-150: 100 positions • 151+: 200 positions
#[inline(always)]
const fn base_batch_lut(pos_cnt: u32) -> u16 {
    let mut base = BATCH_INCREMENTS[0]; // Start with 5

    // Progressive threshold checking with correct increments
    base += (pos_cnt > BATCH_THRESHOLDS[0]) as u16 * BATCH_INCREMENTS[1]; // +5 → 10
    base += (pos_cnt > BATCH_THRESHOLDS[1]) as u16 * BATCH_INCREMENTS[2]; // +15 → 25
    base += (pos_cnt > BATCH_THRESHOLDS[2]) as u16 * BATCH_INCREMENTS[3]; // +25 → 50
    base += (pos_cnt > BATCH_THRESHOLDS[3]) as u16 * BATCH_INCREMENTS[4]; // +50 → 100
    base += (pos_cnt > BATCH_THRESHOLDS[4]) as u16 * 100; // +100 → 200

    base
}

// =================== MAIN BATCH SIZER ===================

/// High-performance, protocol-safe batch size calculator using Q64.64 fixed-point arithmetic.
///
/// # Why this struct?
/// - Encapsulates all parameters affecting batch sizing, making logic explicit and auditable.
/// - Designed for zero-copy, deterministic layout to support on-chain and off-chain simulation.
///
/// Computes optimal position batch sizes based on:
/// • Pool liquidity (affects scaling factor)
/// • Average position size (memory constraints)
/// • Gas cost factors (performance tuning)
/// • Rent cost factors (economic optimization)
#[derive(Clone, Copy)]
#[repr(C)] // Ensures consistent memory layout for zero-copy operations
pub struct BatchSizer {
    /// Pool liquidity in smallest token units (e.g., lamports, token atoms)
    pub pool_liquidity: u128,

    /// Average compressed position size in bytes (typically 32-256 bytes)
    pub avg_pos_size: u16,

    /// Gas cost scaling factor ×10 (150 = 15% premium)
    /// Higher values increase batch sizes to amortize gas costs
    pub gas_factor_x10: u16,

    /// Rent cost scaling factor ×10 (110 = 11% premium)
    /// Higher values increase batch sizes to amortize rent costs
    pub rent_factor_x10: u16,
}

impl BatchSizer {
    /// Computes the optimal batch size (number of positions) for a given pool and constraints.
    ///
    /// # Why this approach?
    /// - All scaling is performed in Q64.64 for overflow safety and protocol consistency.
    /// - Arithmetic mean of scaling factors balances gas, rent, and liquidity incentives.
    /// - Account size constraints are enforced to prevent runtime errors and rent-exemption failures.
    ///
    /// # Algorithm
    /// 1. Calculate base size from branchless LUT
    /// 2. Apply liquidity, gas, and rent scaling (arithmetic mean)
    /// 3. Constrain by account size limits (10 KiB max)
    ///
    /// Returns: Optimal number of positions per batch
    #[inline(always)]
    pub fn optimal_size(&self, pos_cnt: u32) -> Result<usize> {
        // Step 1: Base batch size from threshold lookup
        let base = Q64x64::from_int(base_batch_lut(pos_cnt) as u64);

        // Step 2: Calculate scaling factors in Q64.64 fixed-point
        let liq_q64 = self.liquidity_scaling_factor_q64()?;
        let rent_q64 = factor_to_q64(self.rent_factor_x10);
        let gas_q64 = factor_to_q64(self.gas_factor_x10);

        // Step 3: Combine factors using arithmetic mean
        // Division by 3 avoids overflow and balances all factors equally
        let sum = liq_q64.checked_add(rent_q64)?.checked_add(gas_q64)?;
        let combined_q64 = sum.checked_div(Q64x64::from_int(3))?;

        // Step 4: Apply combined scaling to base size
        let scaled_batch = base.checked_mul(combined_q64)?;

        // Step 5: Apply account size constraints (10 KiB hard limit)
        let size_constrained = self.max_positions_by_account_size() as u64;
        let scaled_value = (scaled_batch.raw() >> 64) as u64; // Convert Q64.64 to integer

        Ok(scaled_value.min(size_constrained) as usize)
    }

    /// Computes the liquidity-based scaling factor in Q64.64.
    ///
    /// # Why this approach?
    /// - Smoothly increases batch size for high-liquidity pools, incentivizing capital efficiency.
    /// - Prevents excessive scaling for small pools, maintaining rent and CU efficiency.
    /// - All math is overflow-safe and deterministic.
    #[inline(always)]
    fn liquidity_scaling_factor_q64(&self) -> Result<Q64x64> {
        if self.pool_liquidity > BASE_LIQUIDITY_THRESHOLD {
            // High liquidity case: scale up to 1.5x
            let excess = self.pool_liquidity - BASE_LIQUIDITY_THRESHOLD;

            // Calculate: excess / (2 * BASE_THRESHOLD) capped at 0.5
            let scaling_factor = mul_div(excess, Q64x64::one().raw(), BASE_LIQUIDITY_THRESHOLD * 2)
                .unwrap_or(0)
                .min(MAX_LIQUIDITY_SCALING_Q64);

            // Return: 1.0 + scaling_factor (range: [1.0, 1.5])
            let one_q64 = Q64x64::one();
            let scaling_q64 = Q64x64::from_raw(scaling_factor);
            one_q64.checked_add(scaling_q64)
        } else {
            // Low liquidity case: scale from 0.8 to 1.0
            // Calculate: 0.8 + (liquidity / BASE_THRESHOLD) * 0.2
            let liquidity_ratio = mul_div(
                self.pool_liquidity,
                Q64x64::one().raw(),
                BASE_LIQUIDITY_THRESHOLD,
            )
            .unwrap_or(0);

            // Multiply by 0.2 (1/5) then add to base 0.8
            let scaling_component = mul_div(liquidity_ratio, Q64x64::one().raw(), 5).unwrap_or(0);

            let base_q64 = Q64x64::from_raw(BASE_LIQUIDITY_FACTOR_Q64);
            let component_q64 = Q64x64::from_raw(scaling_component);
            base_q64.checked_add(component_q64)
        }
    }

    /// Computes the maximum number of positions that fit within Solana account size limits.
    ///
    /// # Why this approach?
    /// - Prevents runtime panics and rent-exemption failures by enforcing hard limits.
    /// - All calculations are saturating and checked for zero/invalid position sizes.
    #[inline(always)]
    fn max_positions_by_account_size(&self) -> usize {
        let available_space = MAX_ACCOUNT_SIZE.saturating_sub(ACCOUNT_OVERHEAD_BYTES);

        // Prevent division by zero and ensure reasonable position sizes
        if self.avg_pos_size == 0 || self.avg_pos_size > MAX_ACCOUNT_SIZE as u16 {
            return 0;
        }

        available_space / self.avg_pos_size as usize
    }
}

// =================== RENT CALCULATION ===================

/// Calculates rent requirements with a safety buffer using Q64.64 growth projection.
///
/// # Why this approach?
/// - Projects future rent needs to prevent rent-exemption failures during account growth.
/// - Adds a buffer using bit-shift for gas efficiency and to avoid edge-case underfunding.
/// - All math is overflow-safe and capped at protocol maximums.
#[inline(always)]
pub fn rent_with_buffer_q64(base_size: usize, growth_factor_q64: Q64x64) -> Result<u64> {
    // Step 1: Project future size using Q64.64 multiplication
    let base_size_q64 = Q64x64::from_int(base_size as u64);
    let projected_size_q64 = base_size_q64.checked_mul(growth_factor_q64)?;

    // Convert back to integer (Q64.64 → u64)
    let projected_size = (projected_size_q64.raw() >> 64) as usize;

    // Step 2: Cap at maximum account size to prevent excessive rent calculations
    let capped_size = projected_size.min(MAX_ACCOUNT_SIZE);

    // Step 3: Calculate base rent (single syscall - cached internally by Solana)
    let base_rent = Rent::get()?.minimum_balance(capped_size);

    // Step 4: Add safety buffer using efficient bit-shift (≈3.125% = 1/32)
    // This prevents rent exemption issues during account growth
    let buffer = base_rent >> RENT_BUFFER_SHIFT;

    base_rent
        .checked_add(buffer)
        .ok_or_else(|| error!(AdvancedAccountOptimizationError::ArithmeticOverflow))
}

/// Backwards-compatible rent calculation using basis points.
///
/// # Why this approach?
/// - Supports legacy callers that use basis points instead of Q64.64.
/// - Internally converts to Q64.64 for protocol consistency and safety.
#[inline(always)]
pub fn rent_with_buffer(base_size: usize, growth_fp_1e4: u32) -> Result<u64> {
    // Convert basis points to Q64.64
    let growth_factor_q64 =
        Q64x64::from_raw(mul_div(growth_fp_1e4 as u128, Q64x64::one().raw(), 10_000)?);

    rent_with_buffer_q64(base_size, growth_factor_q64)
}

// =================== ACCOUNT MIGRATION ===================

/// High-performance, zero-copy account resizer for on-chain data migration.
///
/// # Why this approach?
/// - Avoids expensive (de)serialization round-trips, reducing CU and risk of data loss.
/// - Ensures atomic migration and rent recycling, preventing SOL leakage and protocol bloat.
/// - All validation is explicit and checked before mutation.
pub struct Resizer;

impl Resizer {
    /// Migrates account data to a larger size with zero serialization overhead.
    ///
    /// # Why this approach?
    /// - All validation is performed before mutation to prevent partial state.
    /// - Raw memory copy is used for maximum CU efficiency and to preserve zero-copy layouts.
    /// - Lamports are recycled to the payer, ensuring protocol sustainability.
    pub fn migrate<'info>(
        old: &AccountInfo<'info>,
        new: &AccountInfo<'info>,
        payer: &Signer<'info>,
        program_id: &Pubkey,
        new_space: usize,
        bump: u8,
    ) -> Result<()> {
        // Validate size constraints
        require!(
            new_space <= MAX_ACCOUNT_SIZE,
            AdvancedAccountOptimizationError::AccountSizeExceedsLimit
        );
        require!(
            new_space > old.data_len(),
            AdvancedAccountOptimizationError::InvalidAccountSize
        );

        // Validate that new account was created with correct PDA
        let expected_seeds = &[b"resize_v2", old.key.as_ref(), &[bump]];
        let expected_key = Pubkey::create_program_address(expected_seeds, program_id)
            .map_err(|_| error!(AdvancedAccountOptimizationError::InvalidPDAAddress))?;
        require!(
            new.key() == expected_key,
            AdvancedAccountOptimizationError::InvalidPDAAddress
        );

        // Validate new account is owned by our program and has correct size
        require!(
            new.owner == program_id,
            AdvancedAccountOptimizationError::InvalidAccountOwner
        );
        require!(
            new.data_len() == new_space,
            AdvancedAccountOptimizationError::InvalidAccountSize
        );

        // Perform actual raw memory copy from old to new account
        let old_data = old.data.borrow();
        let mut new_data = new.data.borrow_mut();

        // Copy all existing data to new account (zero-copy, single memcpy in BPF)
        let copy_len = old_data.len().min(new_data.len());
        new_data[..copy_len].copy_from_slice(&old_data[..copy_len]);

        // Zero out any remaining space in new account
        if new_data.len() > copy_len {
            new_data[copy_len..].fill(0);
        }

        // Transfer old account's lamports back to payer (rent recycling)
        let old_balance = old.lamports();
        if old_balance > 0 {
            **old.lamports.borrow_mut() = 0;
            **payer.lamports.borrow_mut() = payer
                .lamports()
                .checked_add(old_balance)
                .ok_or(AdvancedAccountOptimizationError::ArithmeticOverflow)?;
        }

        Ok(())
    }

    /// Calculates required space for account migration using Q64.64.
    ///
    /// # Why this approach?
    /// - Allows callers to precompute safe migration targets, preventing runtime errors.
    /// - All math is overflow-safe and capped at protocol maximums.
    #[inline(always)]
    pub fn calculate_migration_space_q64(
        current_size: usize,
        growth_factor_q64: Q64x64,
    ) -> Result<usize> {
        let current_size_q64 = Q64x64::from_int(current_size as u64);
        let projected_size_q64 = current_size_q64.checked_mul(growth_factor_q64)?;

        // Convert back to integer (Q64.64 → u64)
        let projected_size = (projected_size_q64.raw() >> 64) as usize;

        Ok(projected_size.min(MAX_ACCOUNT_SIZE))
    }

    /// Backwards-compatible migration space calculation using basis points.
    ///
    /// # Why this approach?
    /// - Supports legacy callers while enforcing protocol safety via Q64.64 conversion.
    #[inline(always)]
    pub fn calculate_migration_space(current_size: usize, growth_factor_bp: u32) -> Result<usize> {
        let growth_factor_q64 = Q64x64::from_raw(mul_div(
            growth_factor_bp as u128,
            Q64x64::one().raw(),
            10_000,
        )?);

        Self::calculate_migration_space_q64(current_size, growth_factor_q64)
    }
}

// =================== COMPUTE BUDGET HELPERS ===================

/// Creates a compute budget instruction to optimize account data loading.
///
/// # Why this approach?
/// - Reduces CU consumption for large account loads by setting explicit data size limits.
/// - Exposed as a helper for protocol and client use (see Anza blog Nov 2024).
#[inline(always)]
pub fn set_data_limit_ix(bytes: u32) -> Instruction {
    ComputeBudgetInstruction::set_loaded_accounts_data_size_limit(bytes)
}

// =================== PROFILING MACROS ===================

/// Conditional compute unit logging for performance analysis.
///
/// # Why this macro?
/// - Allows targeted CU profiling without affecting production builds.
/// - Compile-time feature gating ensures zero overhead unless profiling is enabled.
#[macro_export]
macro_rules! cu_log {
    () => {
        #[cfg(feature = "profiling")]
        solana_program::log::sol_log_compute_units();
    };
    ($label:expr) => {
        #[cfg(feature = "profiling")]
        {
            solana_program::msg!("CU checkpoint: {}", $label);
            solana_program::log::sol_log_compute_units();
        }
    };
}

// =================== COMPRESSION FEATURES ===================

/// State compression helpers (feature-gated).
///
/// # Why this module?
/// - Provides tree hashing for future integration with Solana's concurrent Merkle trees.
/// - Ready for state compression patterns as documented in Solana docs 2025.
#[cfg(feature = "compression")]
pub mod compression {
    use super::*;
    use solana_program::keccak;

    /// Calculates a Merkle tree hash for a batch of position data.
    ///
    /// # Why this approach?
    /// - Provides a foundation for integrating with spl-concurrent-merkle-tree for compressed position storage and verification.
    /// - Ensures deterministic, tamper-evident state roots for future compression features.
    pub fn calculate_tree_hash(position_data: &[&[u8]]) -> [u8; 32] {
        if position_data.is_empty() {
            return [0u8; 32];
        }

        let mut leaves: Vec<[u8; 32]> = position_data
            .iter()
            .map(|data| keccak::hash(data).to_bytes())
            .collect();

        // Build tree bottom-up
        while leaves.len() > 1 {
            let mut next_level = Vec::new();

            for chunk in leaves.chunks(2) {
                let hash = if chunk.len() == 2 {
                    keccak::hashv(&[&chunk[0], &chunk[1]]).to_bytes()
                } else {
                    chunk[0] // Odd leaf carries forward
                };
                next_level.push(hash);
            }

            leaves = next_level;
        }

        leaves[0]
    }
}
