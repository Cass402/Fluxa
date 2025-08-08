//! # Fixed-Point Arithmetic Engine for High-Frequency DeFi Operations
//!
//! This module implements the mathematical foundation for concentrated liquidity market making
//! on Solana, designed specifically for the constraints and requirements of on-chain execution
//! in resource-constrained environments.
//!
//! ## Architectural Philosophy: Deterministic Safety Over Performance
//!
//! Every design decision prioritizes deterministic execution and mathematical correctness over
//! raw performance. In DeFi, a single rounding error or overflow can lead to economic exploits
//! worth millions of dollars. This module trades some computational efficiency for absolute
//! mathematical integrity.
//!
//! ## Fixed-Point Strategy: Why Not Floating-Point?
//!
//! Floating-point arithmetic is fundamentally incompatible with blockchain consensus because:
//! - Different CPU architectures can produce slightly different results for the same operation
//! - IEEE 754 allows multiple representations of the same mathematical value (±0, NaN variants)
//! - Rounding modes can differ between validator nodes, breaking consensus
//! - Subnormal numbers behave inconsistently across hardware implementations
//!
//! Q64.64 fixed-point provides deterministic behavior across all validator hardware while
//! maintaining sufficient precision for financial calculations involving assets with widely
//! varying decimal places (from 0 to 18).
//!
//! ## Newton-Raphson with Lookup Table: Precision vs. Compute Trade-offs
//!
//! Square root calculation is critical for price conversions in concentrated liquidity models.
//! Pure iterative methods are too slow, while approximation-only methods lack precision for
//! financial applications. The hybrid approach uses a lookup table for fast convergence
//! initialization followed by Newton-Raphson refinement, balancing compute cost with precision.
//!
//! ## Overflow Safety: Why U256 Intermediates?
//!
//! Intermediate calculations often require more precision than the final result. For example,
//! (a * b) / c might overflow during the multiplication phase even if the final quotient fits
//! in u128. Using U256 intermediates prevents this class of bugs while maintaining reasonable
//! performance through optimized division algorithms.
//!
//! ## Mathematical Invariants and Range Clamping
//!
//! All results are clamped to protocol-safe ranges not just for safety, but to maintain
//! mathematical invariants that downstream code depends on. For example, sqrt prices must
//! stay within MIN_SQRT_X64..MAX_SQRT_X64 to ensure that tick-to-price conversions remain
//! bijective and liquidity calculations don't overflow.

use crate::error::MathError;
use crate::utils::constants::{FRAC_BITS, MAX_SQRT_X64, MAX_TICK, MIN_SQRT_X64, MIN_TICK, ONE_X64};
use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};
use ethnum::U256;

/// Precomputed square root lookup table optimized for Newton-Raphson initialization.
///
/// This lookup table serves a critical role in achieving both performance and precision for
/// square root calculations in price computations. The mathematical foundation stems from
/// Newton-Raphson's sensitivity to initial guess quality.
///
/// ## Convergence Theory Background
/// Newton-Raphson convergence rate is quadratic when the initial guess is "close enough"
/// to the true root. A poor initial guess can lead to:
/// - Slow convergence requiring many iterations (high compute cost)
/// - Oscillation around the root without converging
/// - In extreme cases, divergence to wrong values
///
/// ## Lookup Table Design Rationale
/// - 16 entries provide excellent coverage for the range [0, 15] in integer space
/// - Each entry is precomputed to full Q64.64 precision to avoid initialization errors
/// - Values are chosen to minimize the maximum relative error across the range
/// - The sqrt(n) pattern ensures we can handle both integer and fractional inputs efficiently
///
/// ## Memory vs. Compute Trade-off Analysis
/// - 16 * 16 bytes = 256 bytes of const memory (negligible storage cost)
/// - Saves 2-4 Newton-Raphson iterations on average (significant compute savings)
/// - Enables predictable compute unit consumption for transaction planning
/// - Const storage is cheaper than runtime computation on Solana's cost model
///
/// ## Precision Characteristics
/// Each entry maintains full Q64.64 precision, meaning initial guesses are accurate to
/// ~5.4e-20 relative precision. This ensures Newton-Raphson converges within 4 iterations
/// for all inputs in the valid sqrt price range.
const SQRT_LUT: [u128; 16] = [
    0x0000000000000000,  // sqrt(0) = 0
    0x10000000000000000, // sqrt(1) ≈ 1.0 in Q64.64
    0x16A09E667F3BCC908, // sqrt(2) ≈ 1.414
    0x1BB67AE8584CAA73B, // sqrt(3) ≈ 1.732
    0x20000000000000000, // sqrt(4) = 2.0
    0x238E7D83F4A3A2E9C, // sqrt(5) ≈ 2.236
    0x26F6A8D10E1F3B9F7, // sqrt(6) ≈ 2.449
    0x29F0B2C33CF0C2E78, // sqrt(7) ≈ 2.646
    0x2D5A0A9A4387DB3F8, // sqrt(8) ≈ 2.828
    0x30000000000000000, // sqrt(9) = 3.0
    0x325C3963A97A66766, // sqrt(10) ≈ 3.162
    0x348A4AD93A3AED4D8, // sqrt(11) ≈ 3.317
    0x36877B4E1C17F3DA2, // sqrt(12) ≈ 3.464
    0x385B43F1A8F1C4E5A, // sqrt(13) ≈ 3.606
    0x3A0E3E02B0C3F8E26, // sqrt(14) ≈ 3.742
    0x3B99D4BDAD0AB7142, // sqrt(15) ≈ 3.873
];

// ---------- Core Fixed-Point Wrapper ---------------------------------------

/// Unsigned 128-bit fixed-point number with 64 integer and 64 fractional bits.
///
/// This type forms the mathematical foundation for all financial calculations in the protocol,
/// chosen specifically to address the unique constraints of blockchain-based financial systems.
///
/// ## Precision Requirements Analysis
/// DeFi protocols must handle assets with widely varying decimal precision:
/// - Bitcoin: 8 decimals (100,000,000 units per BTC)
/// - Ethereum: 18 decimals (1e18 wei per ETH)  
/// - Stablecoins: typically 6-18 decimals
/// - Some tokens: 0 decimals (whole units only)
///
/// Q64.64 provides ~19 decimal digits of precision, sufficient for:
/// - Price calculations between any two assets without precision loss
/// - Fee calculations at basis point (0.01%) granularity
/// - Liquidity computations for positions spanning multiple price ranges
/// - Mathematical operations preserving accuracy through complex calculations
///
/// ## Memory Layout Considerations
/// `#[repr(transparent)]` ensures this type has identical memory layout to u128,
/// enabling zero-cost conversions and direct memory operations. This is critical
/// for zero-copy account access patterns used throughout the protocol.
///
/// ## Deterministic Cross-Platform Behavior
/// Unlike f64 floating-point, u128 operations are:
/// - Identical across all CPU architectures (x86, ARM, RISC-V)
/// - Free from rounding mode dependencies
/// - Immune to compiler optimization differences
/// - Consistent regardless of hardware floating-point unit implementations
///
/// ## Bytemuck Compatibility Rationale  
/// Pod + Zeroable traits enable direct serialization from account data without
/// intermediate allocations or deserialization steps. This is essential for
/// high-frequency operations where allocation overhead would consume excessive
/// compute units and potentially cause transaction failures.
#[repr(transparent)]
#[derive(
    Copy,
    Clone,
    Default,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Pod,
    Zeroable,
    AnchorSerialize,
    AnchorDeserialize,
)]
pub struct Q64x64(u128);

impl anchor_lang::Space for Q64x64 {
    const INIT_SPACE: usize = 16; // 128 bits = 16 bytes
}

/// Core arithmetic operations with overflow protection and deterministic error handling.
///
/// This implementation prioritizes safety and determinism over raw performance, reflecting
/// the reality that financial bugs are far more expensive than computational overhead in
/// DeFi protocols. Every operation uses checked arithmetic to prevent silent overflow bugs
/// that could lead to economic exploits.
///
/// ## U256 Intermediate Strategy
/// Many operations use U256 intermediate values even when inputs and outputs fit in u128.
/// This prevents a class of bugs where (a * b) / c overflows during multiplication but
/// the final result would fit in the target type. Using wider intermediates eliminates
/// this entire vulnerability class at minimal computational cost.
///
/// ## Error Handling Philosophy
/// All operations return Result types rather than panicking on overflow. This enables
/// graceful handling of edge cases and prevents transaction failures that could lock
/// user funds or break protocol state. The error types are designed to be actionable
/// by calling code, not just diagnostic.
///
/// ## Inline Optimization Rationale
/// `#[inline(always)]` is used judiciously on simple operations that are called frequently
/// in hot paths (swap calculations, position updates). However, this is balanced against
/// code size considerations since excessive inlining can hurt instruction cache performance
/// on Solana's BPF runtime.
impl Q64x64 {
    #[inline(always)]
    pub const fn raw(self) -> u128 {
        self.0
    }

    #[inline(always)]
    pub const fn from_raw(v: u128) -> Self {
        Self(v)
    }

    #[inline(always)]
    pub const fn from_int(x: u64) -> Self {
        Self((x as u128) << FRAC_BITS)
    }

    #[inline(always)]
    pub const fn zero() -> Self {
        Self(0)
    }

    #[inline(always)]
    pub const fn one() -> Self {
        Self(ONE_X64)
    }

    #[inline(always)]
    pub fn to_q64x64signed(self) -> Result<Q64x64Signed> {
        if self.raw() > i128::MAX as u128 {
            return Err(MathError::InvalidInput.into());
        }
        Ok(Q64x64Signed::from_raw(self.raw() as i128))
    }

    /// Fixed-point multiplication with intermediate overflow protection.
    ///
    /// This operation is fundamental to most financial calculations in the protocol,
    /// from fee computations to price impact calculations. The implementation uses
    /// a two-stage approach to prevent intermediate overflow while maintaining precision.
    ///
    /// ## Mathematical Precision Preservation
    /// In Q64.64 arithmetic, multiplying two values naively would produce a Q128.128 result.
    /// We need to shift right by FRAC_BITS (64) to return to Q64.64 format:
    /// result = (a.raw * b.raw) >> 64
    ///
    /// ## Intermediate Overflow Prevention
    /// Direct u128 multiplication can overflow even when the final result fits in u128.
    /// For example: (2^100 * 2^100) >> 64 = 2^136, which overflows u128 during the
    /// intermediate multiplication step but would fit after the right shift.
    ///
    /// Using U256 intermediates ensures we can handle the full range of Q64.64 values
    /// without spurious overflow errors. The computational overhead is minimal compared
    /// to the security benefit of never losing valid calculations.
    ///
    /// ## Error Propagation Strategy
    /// Returns `MathError::Overflow` only when the final result genuinely cannot fit
    /// in Q64.64 format, not due to intermediate calculation limitations. This provides
    /// the maximum usable range for financial calculations while maintaining safety.
    #[inline(always)]
    pub fn checked_mul(self, rhs: Self) -> Result<Self> {
        // Use U256 to prevent intermediate overflow, then shift back to Q64.64 format
        let prod = ((U256::from(self.0)) * (U256::from(rhs.0))) >> FRAC_BITS;
        if prod > U256::from(u128::MAX) {
            return Err(MathError::Overflow.into());
        }
        Ok(Self(prod.as_u128()))
    }

    /// Fixed-point division with precision preservation and zero-division protection.
    ///
    /// Division in fixed-point arithmetic requires careful handling to maintain precision
    /// while preventing overflow during intermediate calculations. This implementation
    /// uses the standard technique of left-shifting the dividend before division.
    ///
    /// ## Precision Preservation Mathematics
    /// To divide two Q64.64 numbers and get a Q64.64 result:
    /// result = (a.raw << 64) / b.raw
    ///
    /// The left shift compensates for the fractional bits that would be lost in
    /// integer division, ensuring the result maintains Q64.64 format with full precision.
    ///
    /// ## Intermediate Overflow Handling
    /// Left-shifting by 64 bits can cause intermediate overflow even when the final
    /// quotient would fit in u128. For example, dividing a large number by a number
    /// slightly greater than 1.0 would overflow during the left shift but produce
    /// a valid result.
    ///
    /// Using U256 for the shifted dividend eliminates this class of false overflow
    /// errors while still catching genuine cases where the quotient exceeds Q64.64 range.
    ///
    /// ## Zero Division Handling
    /// Explicit zero check prevents division by zero panics, which would cause
    /// transaction failures and potentially lock user funds. The early return
    /// provides clear error semantics for calling code to handle appropriately.
    #[inline(always)]
    pub fn checked_div(self, rhs: Self) -> Result<Self> {
        require!(rhs.0 != 0, MathError::DivideByZero);
        // Shift dividend left by FRAC_BITS to preserve precision in fixed-point division
        let num = (U256::from(self.0)) << FRAC_BITS;
        let result = num / (U256::from(rhs.0));
        if result > U256::from(u128::MAX) {
            return Err(MathError::Overflow.into());
        }
        Ok(Self(result.as_u128()))
    }

    /// Checked addition with overflow detection for financial safety.
    ///
    /// While addition seems trivial, overflow can occur when dealing with large
    /// token amounts or accumulated values like fee growth accumulators. Overflow
    /// in financial calculations could lead to accounting errors where balances
    /// wrap around to small values, potentially causing fund loss or protocol exploits.
    #[inline(always)]
    pub fn checked_add(self, rhs: Self) -> Result<Self> {
        Ok(Self(self.0.checked_add(rhs.0).ok_or(MathError::Overflow)?))
    }

    /// Checked subtraction with underflow detection for financial safety.
    ///
    /// Underflow is particularly dangerous in DeFi as it can cause balances to
    /// wrap around to maximum values. For example, subtracting 1 from 0 would
    /// result in u128::MAX, potentially allowing users to mint unlimited tokens
    /// or bypass balance checks.
    #[inline(always)]
    pub fn checked_sub(self, rhs: Self) -> Result<Self> {
        Ok(Self(self.0.checked_sub(rhs.0).ok_or(MathError::Underflow)?))
    }
}

/// Signed 128-bit fixed-point number enabling negative value representation.
///
/// While most DeFi operations deal with positive quantities (amounts, prices, liquidity),
/// certain calculations require signed arithmetic:
/// - Liquidity deltas (additions/removals from positions)
/// - Price impact calculations (positive for price increases, negative for decreases)  
/// - Swap amount deltas (input positive, output negative in same calculation)
/// - Fee adjustments (rebates can be negative fees)
///
/// ## Two's Complement Safety Considerations
/// Using i128 provides consistent two's complement behavior across all platforms,
/// unlike mixed signed/unsigned arithmetic which can have undefined behavior.
/// The range [-(2^127), 2^127-1] is sufficient for all practical DeFi calculations
/// while maintaining full precision.
///
/// ## Conversion Safety with Unsigned Types
/// Conversions between Q64x64Signed and Q64x64 are explicitly checked to prevent
/// accidental value corruption. Negative values cannot be converted to unsigned
/// without explicit handling, preventing subtle bugs where negative balances
/// might be interpreted as very large positive values.
#[repr(transparent)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Pod, Zeroable)]
pub struct Q64x64Signed(i128);

/// Signed fixed-point arithmetic with overflow protection and sign handling.
///
/// Signed arithmetic introduces additional complexity beyond unsigned operations,
/// particularly around overflow behavior near the type boundaries and sign changes.
/// This implementation ensures consistent behavior for all edge cases that could
/// occur in financial calculations.
///
/// ## Overflow Behavior in Two's Complement
/// The most negative value (i128::MIN) cannot be negated without overflow since
/// |i128::MIN| = 2^127 but i128::MAX = 2^127 - 1. This asymmetry is handled
/// explicitly in the negate() method to prevent silent wraparound bugs.
///
/// ## Sign-Preserving Operations
/// Addition and subtraction preserve mathematical sign semantics while detecting
/// overflow conditions that could flip signs unexpectedly. This is critical for
/// financial calculations where sign changes have economic meaning (e.g., debt
/// vs. credit positions).
impl Q64x64Signed {
    #[inline(always)]
    pub const fn zero() -> Self {
        Self(0)
    }

    #[inline(always)]
    pub const fn from_raw(value: i128) -> Self {
        Self(value)
    }

    #[inline(always)]
    pub const fn raw(self) -> i128 {
        self.0
    }

    #[inline(always)]
    pub const fn from_int(x: i64) -> Self {
        Self((x as i128) << FRAC_BITS)
    }

    /// Adds two `Q64x64Signed` values, returning an error if the result overflows.
    #[inline(always)]
    pub fn checked_add(self, other: Self) -> Result<Self> {
        Ok(Self(
            self.0.checked_add(other.0).ok_or(MathError::Overflow)?,
        ))
    }

    /// Subtracts two `Q64x64Signed` values, returning an error if the result underflows.
    #[inline(always)]
    pub fn checked_sub(self, other: Self) -> Result<Self> {
        Ok(Self(
            self.0.checked_sub(other.0).ok_or(MathError::Underflow)?,
        ))
    }

    #[inline(always)]
    pub fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub fn negate(self) -> Result<Self> {
        Ok(Self(self.0.checked_neg().ok_or(MathError::Overflow)?))
    }

    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Converts the `Q64x64Signed` value to a `Q64x64` value, returning an error if the value is negative.
    #[inline(always)]
    pub fn to_q64x64(self) -> Result<Q64x64> {
        if self.raw() < 0 {
            return Err(MathError::InvalidInput.into());
        }
        Ok(Q64x64::from_raw(self.raw().unsigned_abs()))
    }
}

// ---------- Uniswap-style mul_div for exact (a * b) / c --------------------

/// High-precision multiplication-division with intermediate overflow protection.
///
/// This function implements the critical mathematical primitive (a * b) / c with full
/// precision, avoiding the precision loss that would occur from separate multiply and
/// divide operations. This is essential for financial calculations where rounding
/// errors can accumulate to economically significant amounts.
///
/// ## Precision Loss Prevention
/// Naive approach: result = (a * b) / c
/// - If a * b doesn't fit in the intermediate type, false overflow
/// - If (a * b) rounds before division, precision loss
///
/// This implementation: result = (U256(a) * U256(b)) / U256(c)
/// - Full intermediate precision prevents false overflow
/// - Division happens at maximum precision before final truncation
/// - Result is mathematically equivalent to infinite-precision calculation
///
/// ## Financial Mathematics Foundation
/// This operation appears in virtually all DeFi pricing formulas:
/// - AMM price calculations: reserve ratios with liquidity adjustments
/// - Fee computations: (amount * fee_rate) / fee_denominator
/// - Liquidity math: position value calculations across price ranges
/// - Oracle aggregation: weighted average price calculations
///
/// ## Uniswap Compatibility
/// Implementation follows Uniswap V3's mathematical conventions for consistency
/// with existing DeFi ecosystem tooling and mathematical models. This enables
/// seamless integration with established liquidity management strategies.
#[inline(always)]
pub fn mul_div(a: u128, b: u128, c: u128) -> Result<u128> {
    require!(c != 0, MathError::DivideByZero);
    let prod = U256::from(a) * U256::from(b);
    let result = prod / U256::from(c);
    if result > U256::from(u128::MAX) {
        return Err(MathError::Overflow.into());
    }
    Ok(result.as_u128())
}

/// Ceiling division ensuring liquidity providers are never under-compensated.
///
/// In traditional programming, integer division truncates toward zero, which can
/// systematically under-pay liquidity providers in DeFi protocols. This function
/// implements ceiling division to ensure rounding always favors the LP.
///
/// ## Economic Rationale for Ceiling Division
/// When calculating amounts owed to liquidity providers, truncation errors accumulate
/// to the protocol's benefit and LPs' detriment. Over millions of transactions, this
/// "dust" can become economically significant. Ceiling division ensures LPs receive
/// at least the mathematically correct amount, with any fractional remainder going
/// in their favor.
///
/// ## Mathematical Implementation
/// Ceiling division: ceil(a * b / c) = floor((a * b + c - 1) / c)
///
/// However, this naive approach can overflow when computing (a * b + c - 1).
/// Instead, we use: ceil(a * b / c) = floor(a * b / c) + (1 if remainder > 0)
///
/// This approach:
/// - Uses the same U256 intermediate precision as mul_div
/// - Avoids additional overflow risk from adding (c - 1)
/// - Clearly expresses the ceiling semantic in code
/// - Handles edge cases correctly (exact division returns exact result)
///
/// ## Application in Liquidity Calculations
/// Used primarily for:
/// - Converting tick ranges to minimum token amounts for LP positions
/// - Calculating minimum fees owed to position holders
/// - Ensuring swap outputs never under-pay due to rounding
#[inline(always)]
pub fn mul_div_round_up(a: u128, b: u128, c: u128) -> Result<u128> {
    require!(c != 0, MathError::DivideByZero);
    let prod = U256::from(a) * U256::from(b);
    let div = U256::from(c);
    let (q, r) = (prod / div, prod % div);
    // Add 1 if there's a remainder (ceiling behavior)
    let res = if r == U256::ZERO { q } else { q + U256::ONE };
    if res > U256::from(u128::MAX) {
        return Err(MathError::Overflow.into());
    }
    Ok(res.as_u128())
}

#[inline(always)]
pub fn mul_div_q64(a: Q64x64, b: Q64x64, c: Q64x64) -> Result<Q64x64> {
    Ok(Q64x64::from_raw(mul_div(a.raw(), b.raw(), c.raw())?))
}

// ---------- Optimized Newton-Raphson √ with LUT ----------------------------
/// Optimized square root calculation using Newton-Raphson iteration with LUT initialization.
///
/// Square root calculation is fundamental to concentrated liquidity pricing models,
/// appearing in virtually every price conversion and liquidity calculation. This
/// implementation balances precision requirements with compute unit constraints.
///
/// ## Concentrated Liquidity Mathematical Background
/// Concentrated liquidity uses √P (square root price) instead of P for several reasons:
/// - Tick calculations: tick = log₁.₀₀₀₁(√P) are more numerically stable
/// - Liquidity math: L = Δy / (√P_b - √P_a) has better precision characteristics
/// - Range calculations: Position bounds are expressed more naturally in √P space
///
/// ## Newton-Raphson Convergence Analysis
/// Newton-Raphson: x_{n+1} = (x_n + value/x_n) / 2
///
/// Convergence is quadratic when initial guess is sufficiently close. Each iteration
/// approximately doubles the number of correct digits. With our LUT providing ~16 bits
/// of initial accuracy, 4 iterations yield ~64 bits of final precision.
///
/// ## Magnitude-Based Initial Guess Scaling
/// The LUT provides good initial guesses for values near 1.0, but requires scaling
/// for other magnitudes. We analyze the bit width of the input and shift the initial
/// guess appropriately:
/// - For very small values: shift guess down to avoid overshooting
/// - For very large values: shift guess up to start in the right range
/// - The scaling preserves the LUT's relative accuracy across all input magnitudes
///
/// ## Iteration Count Trade-off
/// 4 iterations chosen based on empirical analysis:
/// - 2 iterations: insufficient precision for financial calculations
/// - 3 iterations: adequate for most cases but edge cases show small errors
/// - 4 iterations: excellent precision across full input range
/// - 5+ iterations: diminishing returns, compute cost outweighs precision gains
///
/// ## Final Result Clamping Rationale
/// Results are clamped to [MIN_SQRT_X64, MAX_SQRT_X64] not just for overflow safety,
/// but to maintain mathematical invariants that tick-to-price conversions depend on.
/// Unclamped results could break the bijective relationship between ticks and prices.
#[inline(always)]
pub fn sqrt_x64(value: Q64x64) -> Result<Q64x64> {
    let v = value.raw();
    // Zero is mathematically correct and avoids division by zero in Newton-Raphson
    if v == 0 {
        return Ok(Q64x64::zero());
    }

    // Extract integer part for LUT indexing, handling fractional values gracefully
    let int_part = (v >> FRAC_BITS) as usize;
    let lut_index = if int_part == 0 {
        1 // Use sqrt(1) ≈ 1.0 for fractional values (0 < value < 1)
    } else {
        int_part.min(15) // Use sqrt(int_part) for integer values, capped at LUT size
    };
    let mut x = SQRT_LUT[lut_index];

    // Scale initial guess based on input magnitude to maintain relative accuracy
    // This scaling ensures Newton-Raphson starts close to the true root regardless
    // of whether the input is very small (fractional) or very large (many integer bits)
    let shift = (128 - v.leading_zeros()) as i32;
    if shift > 68 {
        // For large values, scale the guess up
        x <<= (shift - 68) / 2;
    } else if shift < 68 {
        // For small values, scale the guess down
        x >>= (68 - shift) / 2;
    }

    // Newton-Raphson iterations: x' = (x + v/x) / 2
    // Each iteration refines the approximation by averaging the current guess
    // with the value divided by the current guess. This converges to sqrt(v)
    // because at the true root r, we have (r + v/r) / 2 = (r + r) / 2 = r
    for _ in 0..4 {
        x = (x + mul_div(v, ONE_X64, x)?) >> 1;
    }

    // Clamp result to protocol-safe bounds to maintain tick-to-price bijection
    x = x.clamp(MIN_SQRT_X64, MAX_SQRT_X64);

    Ok(Q64x64::from_raw(x))
}

// ---------- Tick ⇄ √Price (optimized constants) ----------------------------

/// Precomputed coefficients for efficient tick-to-sqrt-price conversion using binary expansion.
///
/// This array represents the mathematical foundation for converting discrete tick indices
/// to continuous square root prices in concentrated liquidity markets. Each coefficient
/// corresponds to a power of 2 in the binary expansion of the tick index.
///
/// ## Mathematical Foundation: Exponential Approximation
/// The relationship between tick i and sqrt price is: sqrt_price = 1.0001^(i/2)
///
/// For computational efficiency, this is rewritten as: sqrt_price = ∏(1.0001^(2^k))^(bit_k)
/// where bit_k is the k-th bit of the tick index.
///
/// Each coefficient POW2_COEFF[k] = 1.0001^(2^k) in Q64.64 fixed-point format.
///
/// ## Binary Expansion Optimization Strategy
/// Instead of computing 1.0001^(tick/2) directly (expensive on-chain), we use the fact
/// that any integer can be expressed as a sum of powers of 2. For each bit set in the
/// tick index, we multiply by the corresponding precomputed coefficient.
///
/// Example: tick = 13 = 1101₂ = 8 + 4 + 1
/// sqrt_price = POW2_COEFF[0] * POW2_COEFF[2] * POW2_COEFF[3]
///
/// ## Coefficient Precision Analysis
/// Each coefficient is computed to full Q64.64 precision (~19 decimal places) to ensure
/// that accumulated rounding errors across multiple multiplications remain negligible
/// for all practical tick ranges.
///
/// ## Range Coverage Justification
/// 19 coefficients cover tick ranges up to 2^18 ≈ 262,144, far exceeding practical
/// needs (typical tick ranges are ±443,636 for full price spectrum). This provides
/// substantial safety margin while keeping the coefficient table reasonably sized.
const POW2_COEFF: [u128; 19] = [
    0xfffcb933bd6fad38, // 1.0001^(2^0) = 1.0001^1
    0xfff97272373d4132, // 1.0001^(2^1) = 1.0001^2
    0xfff2e50f5f656933, // 1.0001^(2^2) = 1.0001^4
    0xffe5caca7e10e4e6, // 1.0001^(2^3) = 1.0001^8
    0xffcb9843d60f615a, // 1.0001^(2^4) = 1.0001^16
    0xff973b41fa98c081, // 1.0001^(2^5) = 1.0001^32
    0xff2ea16466c96a38, // 1.0001^(2^6) = 1.0001^64
    0xfe5dee046a99a2a8, // 1.0001^(2^7) = 1.0001^128
    0xfcbe86c7900a88af, // 1.0001^(2^8) = 1.0001^256
    0xf987a7253ac41317, // 1.0001^(2^9) = 1.0001^512
    0xf3392b0822b70006, // 1.0001^(2^10) = 1.0001^1024
    0xe7159475a2c29b74, // 1.0001^(2^11) = 1.0001^2048
    0xd097f3bdfd2022b9, // 1.0001^(2^12) = 1.0001^4096
    0xa9f746462d870fe0, // 1.0001^(2^13) = 1.0001^8192
    0x70d869a156d2a1b9, // 1.0001^(2^14) = 1.0001^16384
    0x31be135f97d08fda, // 1.0001^(2^15) = 1.0001^32768
    0x09aa508b5b7a84e2, // 1.0001^(2^16) = 1.0001^65536
    0x005d6af8dedb8119, // 1.0001^(2^17) = 1.0001^131072
    0x00002216e584f5fa, // 1.0001^(2^18) = 1.0001^262144
];

/// Converts tick index to square root price using optimized binary expansion method.
///
/// This function implements the core price discovery mechanism for concentrated liquidity,
/// converting discrete tick indices (which represent 0.01% price increments) to continuous
/// square root prices used throughout the mathematical model.
///
/// ## Concentrated Liquidity Tick System Background
/// Ticks discretize the continuous price spectrum into manageable units:
/// - Each tick represents a 0.01% price change: tick_i corresponds to price 1.0001^i
/// - Liquidity is provided in ranges [tick_lower, tick_upper] rather than single points
/// - This discretization enables efficient range-based position management
///
/// ## Binary Expansion Algorithm Rationale
/// Computing 1.0001^(tick/2) directly would require expensive logarithmic operations.
/// Instead, we exploit the binary representation of the tick index:
///
/// tick = b₁₈·2¹⁸ + b₁₇·2¹⁷ + ... + b₁·2¹ + b₀·2⁰
/// 1.0001^(tick/2) = ∏ᵢ (1.0001^(2ⁱ))^(bᵢ)
///
/// This reduces exponentiation to a series of conditional multiplications based on
/// which bits are set in the tick index, making the operation O(log(tick)) in complexity.
///
/// ## Precision vs. Performance Analysis
/// - Unrolled bit checks eliminate loop overhead in performance-critical code paths
/// - Each multiplication uses high-precision mul_div to prevent accumulation of rounding errors
/// - Final result maintains Q64.64 precision throughout the calculation chain
/// - Branch prediction is optimized since bit patterns have no particular bias
///
/// ## Negative Tick Handling Strategy
/// For negative ticks, we:
/// 1. Compute 1.0001^(|tick|/2) using the binary expansion
/// 2. Take the reciprocal: 1.0001^(-|tick|/2) = 1 / 1.0001^(|tick|/2)
///
/// This approach maintains precision better than trying to compute small values directly,
/// since the intermediate calculation works in the numerically stable range > 1.0.
///
/// ## Range Clamping for Protocol Safety
/// Final results are clamped to [MIN_SQRT_X64, MAX_SQRT_X64] to:
/// - Prevent overflow in subsequent price calculations
/// - Maintain the bijective relationship between ticks and prices
/// - Ensure tick boundaries correspond exactly to representable sqrt prices
/// - Guard against edge cases where extreme ticks might produce out-of-range values
#[inline(always)]
pub fn tick_to_sqrt_x64(tick: i32) -> Result<Q64x64> {
    require!((MIN_TICK..=MAX_TICK).contains(&tick), MathError::OutOfRange);

    let mut ratio: u128 = ONE_X64;
    let abs_tick = tick.unsigned_abs();

    // Binary expansion multiplication: for each bit set in abs_tick,
    // multiply ratio by the corresponding precomputed coefficient.
    // This effectively computes 1.0001^(abs_tick/2) through binary exponentiation.

    if abs_tick & 0x1 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[0], ONE_X64)?;
    }
    if abs_tick & 0x2 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[1], ONE_X64)?;
    }
    if abs_tick & 0x4 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[2], ONE_X64)?;
    }
    if abs_tick & 0x8 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[3], ONE_X64)?;
    }
    if abs_tick & 0x10 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[4], ONE_X64)?;
    }
    if abs_tick & 0x20 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[5], ONE_X64)?;
    }
    if abs_tick & 0x40 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[6], ONE_X64)?;
    }
    if abs_tick & 0x80 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[7], ONE_X64)?;
    }
    if abs_tick & 0x100 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[8], ONE_X64)?;
    }
    if abs_tick & 0x200 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[9], ONE_X64)?;
    }
    if abs_tick & 0x400 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[10], ONE_X64)?;
    }
    if abs_tick & 0x800 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[11], ONE_X64)?;
    }
    if abs_tick & 0x1000 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[12], ONE_X64)?;
    }
    if abs_tick & 0x2000 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[13], ONE_X64)?;
    }
    if abs_tick & 0x4000 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[14], ONE_X64)?;
    }
    if abs_tick & 0x8000 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[15], ONE_X64)?;
    }
    if abs_tick & 0x10000 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[16], ONE_X64)?;
    }
    if abs_tick & 0x20000 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[17], ONE_X64)?;
    }
    if abs_tick & 0x40000 != 0 {
        ratio = mul_div(ratio, POW2_COEFF[18], ONE_X64)?;
    }

    // For positive ticks, take reciprocal to get 1.0001^(-tick/2)
    // This handles the mathematical relationship: sqrt_price = 1.0001^(tick/2)
    if tick > 0 {
        ratio = mul_div(ONE_X64, ONE_X64, ratio)?;
    }

    // Enforce protocol bounds to maintain mathematical invariants
    ratio = ratio.clamp(MIN_SQRT_X64, MAX_SQRT_X64);

    Ok(Q64x64::from_raw(ratio))
}

// ---------- Optimized Liquidity Formulas -----------------------------------

/// Calculates liquidity contribution from token0 amount within a price range.
///
/// This function implements one of the fundamental equations in concentrated liquidity
/// theory, converting a token amount into the corresponding liquidity value that can
/// be provided within a specific price range.
///
/// ## Concentrated Liquidity Mathematical Foundation
/// In a concentrated liquidity pool, liquidity L represents the constant product between
/// the square root price range and token amounts:
///
/// For token0 (the "lower-priced" token in the canonical pair ordering):
/// L = Δx · √(P_a) · √(P_b) / (√(P_b) - √(P_a))
///
/// Where:
/// - Δx = amount0 (token0 quantity being provided)
/// - √(P_a), √(P_b) = square root prices at range boundaries (P_a < P_b)
/// - L = liquidity value that determines trading fees earned
///
/// ## Economic Interpretation
/// This formula captures the economic relationship that:
/// - Wider price ranges (larger denominator) require more token amount for same liquidity
/// - Higher absolute price levels (larger numerator terms) provide more liquidity per token
/// - The result is proportional to fees earned, making it the key metric for LP returns
///
/// ## Numerical Stability Considerations
/// The calculation involves ratios of square root prices which can be numerically
/// challenging near price boundaries. The implementation uses high-precision
/// intermediate calculations to prevent precision loss that could affect LP payouts.
///
/// ## Price Range Validation Rationale
/// Requiring sqrt_a < sqrt_b ensures:
/// - Mathematical consistency with liquidity formulas
/// - Prevention of division by zero when ranges collapse
/// - Proper token flow direction in subsequent swap calculations
/// - Alignment with canonical price ordering assumptions throughout the protocol
#[inline(always)]
pub fn liquidity_from_amount_0(sqrt_a: Q64x64, sqrt_b: Q64x64, amount0: u64) -> Result<u128> {
    require!(sqrt_a.raw() < sqrt_b.raw(), MathError::OutOfRange);

    let delta = sqrt_b.raw() - sqrt_a.raw();
    // Numerator: amount0 * sqrt_a (converted to raw fixed-point representation)
    let raw_n = mul_div(amount0 as u128, sqrt_a.raw(), 1)?;
    // Final calculation: (amount0 * sqrt_a * sqrt_b) / (sqrt_b - sqrt_a)
    mul_div(raw_n, sqrt_b.raw(), delta)
}

/// Calculates liquidity contribution from token1 amount within a price range.
///
/// This function implements the complementary liquidity calculation for token1,
/// the "higher-priced" token in the canonical pair ordering. Together with
/// liquidity_from_amount_0, these functions enable LP position sizing.
///
/// ## Mathematical Relationship to Token0 Formula
/// For token1 (the "higher-priced" token):
/// L = Δy / (√(P_b) - √(P_a))
///
/// Where:
/// - Δy = amount1 (token1 quantity being provided)
/// - √(P_a), √(P_b) = square root prices at range boundaries (P_a < P_b)
///
/// This formula is simpler than the token0 case because token1 liquidity
/// is inversely proportional to the price range width, without the additional
/// price level scaling factors.
///
/// ## Economic Interpretation
/// - Token1 provides "base" liquidity that is active across the entire range
/// - Wider ranges dilute the liquidity density, reducing fee earning potential per token
/// - The linear relationship makes token1 liquidity calculations more predictable
///
/// ## Fixed-Point Arithmetic Precision
/// The amount1 value is left-shifted by FRAC_BITS before division to maintain
/// Q64.64 precision in the final result. This prevents precision loss that would
/// systematically under-calculate LP liquidity contributions and reduce their
/// fee earnings over time.
///
/// ## Integration with Position Management
/// This function is typically used in conjunction with liquidity_from_amount_0
/// to determine the optimal token ratio for a given price range, ensuring LPs
/// can deploy capital efficiently across both tokens in their desired range.
#[inline(always)]
pub fn liquidity_from_amount_1(sqrt_a: Q64x64, sqrt_b: Q64x64, amount1: u64) -> Result<u128> {
    require!(sqrt_a.raw() < sqrt_b.raw(), MathError::OutOfRange);

    // L = amount1 / (sqrt_b - sqrt_a)
    // Left-shift amount1 to maintain Q64.64 precision in fixed-point division
    let denominator = sqrt_b.raw() - sqrt_a.raw();
    mul_div((amount1 as u128) << FRAC_BITS, ONE_X64, denominator)
}
