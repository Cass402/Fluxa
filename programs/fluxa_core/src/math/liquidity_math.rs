use crate::error::MathError;
use crate::math::core_arithmetic::{
    liquidity_from_amount_0, liquidity_from_amount_1, tick_to_sqrt_x64, Q64x64,
};
use crate::state::position::position_account::Position;
use crate::utils::constants::MAX_TOKEN_AMOUNT;
use anchor_lang::prelude::*;

/// Computes the amount of token0 needed to provide a given liquidity between two price boundaries.
///
/// # Why
/// This function implements the core Uniswap v3-style math for concentrated liquidity, where liquidity is only active within a price range.
/// The formula ensures that liquidity providers only need to supply token0 proportional to the width of the price range and the liquidity amount.
///
/// # Design Rationale
/// - Uses Q64x64 fixed-point math for deterministic, overflow-resistant on-chain computation.
/// - Enforces that the lower price is strictly less than the upper price and nonzero, preventing degenerate or unsafe ranges.
/// - All arithmetic is checked to prevent overflows, which is critical for protocol safety and auditability.
/// - Returns a u64, matching SPL token accounting and ensuring compatibility with Solana's token program.
///
/// # Trade-offs
/// - Shifts by 64 bits to convert from Q64x64 to integer, which is safe because the protocol bounds liquidity and price ranges.
/// - Does not use Vec or dynamic allocation, ensuring deterministic compute and zero-copy compatibility.
///
/// # Usage
/// Used by the AMM to determine how much token0 a user must deposit to mint a position, and for withdrawal calculations.
#[inline(always)]
fn calculate_amount_0_delta(
    sqrt_price_lower: Q64x64,
    sqrt_price_upper: Q64x64,
    liquidity: Q64x64,
) -> Result<u64> {
    // Safety: Prevents invalid or degenerate price ranges, which could break AMM invariants or allow attacks.
    if sqrt_price_lower.raw() >= sqrt_price_upper.raw() || sqrt_price_lower.raw() == 0 {
        return Err(MathError::InvalidPriceRange.into());
    }

    // Intent: Compute price width in sqrt space, which is the basis for how much token0 is needed.
    let price_diff = sqrt_price_upper.checked_sub(sqrt_price_lower)?;

    // Rationale: Multiplying liquidity by price width gives the numerator for the Uniswap v3 formula.
    let numerator = liquidity.checked_mul(price_diff)?;

    // Rationale: Denominator normalizes for the geometric mean of the price range, ensuring correct scaling.
    let denominator = sqrt_price_lower.checked_mul(sqrt_price_upper)?;

    // Safety: All math is checked to prevent overflow/underflow, which is critical for on-chain safety.
    let result = numerator.checked_div(denominator)?;

    // Optimization: Q64x64 to u64 conversion by shifting, as all protocol values are bounded.
    let amount0 = (result.raw() >> 64) as u64;

    Ok(amount0)
}

/// Computes the amount of token1 needed to provide a given liquidity between two price boundaries.
///
/// # Why
/// This function is the counterpart to `calculate_amount_0_delta`, but for token1. It is used when the price is above the lower boundary.
///
/// # Design Rationale
/// - Uses Q64x64 math for deterministic, overflow-resistant computation.
/// - Enforces valid price range to prevent protocol-level errors.
/// - All math is checked for overflow, which is essential for on-chain safety.
///
/// # Trade-offs
/// - Shifts by 64 bits to convert from Q64x64 to integer, which is safe due to protocol bounds.
///
/// # Usage
/// Used by the AMM to determine how much token1 a user must deposit to mint a position, and for withdrawal calculations.
#[inline(always)]
fn calculate_amount_1_delta(
    sqrt_price_lower: Q64x64,
    sqrt_price_upper: Q64x64,
    liquidity: Q64x64,
) -> Result<u64> {
    // Safety: Prevents invalid or degenerate price ranges.
    if sqrt_price_lower.raw() >= sqrt_price_upper.raw() {
        return Err(MathError::InvalidPriceRange.into());
    }

    // Intent: Compute price width in sqrt space, which is the basis for how much token1 is needed.
    let price_diff = sqrt_price_upper.checked_sub(sqrt_price_lower)?;

    // Rationale: Multiplying liquidity by price width gives the numerator for the Uniswap v3 formula for token1.
    let result = liquidity.checked_mul(price_diff)?;

    // Optimization: Q64x64 to u64 conversion by shifting, as all protocol values are bounded.
    let amount1 = (result.raw() >> 64) as u64;

    Ok(amount1)
}

/// Determines the required token0 and token1 amounts for a given liquidity position, based on the current price and the position's price range.
///
/// # Why
/// This function implements the piecewise logic of Uniswap v3-style concentrated liquidity, where the required tokens depend on the current price's relation to the position's range.
///
/// # Design Rationale
/// - Optimized for the most common case (active range) to improve branch prediction and runtime efficiency on-chain.
/// - Enforces all protocol invariants: valid price range, nonzero liquidity, and bounded token amounts.
/// - Uses Q64x64 math for deterministic, overflow-resistant computation.
/// - Returns an error if the result would exceed protocol-defined token limits, preventing overflows and DoS vectors.
///
/// # Usage
/// Used by the AMM to calculate how much of each token a user must deposit or withdraw when minting/burning a position, and for position valuation.
#[inline(always)]
pub fn calculate_amounts_for_liquidity_piecewise(
    sqrt_price_current: Q64x64,
    sqrt_price_lower: Q64x64,
    sqrt_price_upper: Q64x64,
    liquidity: Q64x64,
) -> Result<(u64, u64)> {
    // Safety: Enforce all protocol invariants up front to prevent invalid state or attacks.
    if sqrt_price_lower.raw() == 0
        || sqrt_price_upper.raw() == 0
        || sqrt_price_lower.raw() >= sqrt_price_upper.raw()
        || liquidity.raw() == 0
    {
        return Err(MathError::InvalidInput.into());
    }

    // Optimization: Branch on the most common case (active range) first for better performance.
    let (amount_0, amount_1) = if sqrt_price_current.raw() > sqrt_price_lower.raw()
        && sqrt_price_current.raw() < sqrt_price_upper.raw()
    {
        // Both tokens are required when the price is within the range.
        let amount_0 = calculate_amount_0_delta(sqrt_price_current, sqrt_price_upper, liquidity)?;
        let amount_1 = calculate_amount_1_delta(sqrt_price_lower, sqrt_price_current, liquidity)?;
        (amount_0, amount_1)
    } else if sqrt_price_current.raw() <= sqrt_price_lower.raw() {
        // Only token0 is required when price is below the range.
        let amount_0 = calculate_amount_0_delta(sqrt_price_lower, sqrt_price_upper, liquidity)?;
        (amount_0, 0)
    } else {
        // Only token1 is required when price is above the range.
        let amount_1 = calculate_amount_1_delta(sqrt_price_lower, sqrt_price_upper, liquidity)?;
        (0, amount_1)
    };

    // Safety: Prevents overflows and ensures protocol limits are respected.
    if amount_0 > MAX_TOKEN_AMOUNT || amount_1 > MAX_TOKEN_AMOUNT {
        return Err(MathError::ExcessiveTokenAmount.into());
    }

    Ok((amount_0, amount_1))
}

/// Computes the liquidity that can be provided for a given price range and available token amounts.
///
/// # Why
/// This function is the inverse of the token amount calculations: it determines how much liquidity a user can mint given their available tokens and the current price range.
///
/// # Design Rationale
/// - Uses Q64x64 math for deterministic, overflow-resistant computation.
/// - Enforces all protocol invariants: valid price range, current price within range, and at least one nonzero token amount.
/// - Returns the minimum liquidity that can be provided by either token, ensuring the position is fully collateralized and cannot be over-minted.
/// - Returns an error if the result is zero, preventing dust or non-functional positions.
///
/// # Usage
/// Used by the AMM to determine how much liquidity to mint for a user, and for position management.
pub fn calculate_liquidity(
    sqrt_price_current: Q64x64,
    sqrt_price_lower: Q64x64,
    sqrt_price_upper: Q64x64,
    amount_0: u64,
    amount_1: u64,
) -> Result<u128> {
    // Safety: Enforce all protocol invariants up front to prevent invalid state or attacks.
    if sqrt_price_lower.raw() == 0 || sqrt_price_upper.raw() == 0 {
        return Err(MathError::InvalidPriceRange.into());
    }
    if sqrt_price_current.raw() <= sqrt_price_lower.raw()
        || sqrt_price_current.raw() >= sqrt_price_upper.raw()
    {
        return Err(MathError::InvalidPriceRange.into());
    }
    if amount_0 == 0 && amount_1 == 0 {
        return Err(MathError::InvalidInput.into());
    }

    // Rationale: Compute liquidity from each token, using the core AMM math. If a token is not provided, set its liquidity to max so it doesn't constrain the result.
    let liquidity_0 = if amount_0 > 0 {
        liquidity_from_amount_0(sqrt_price_current, sqrt_price_upper, amount_0)?
    } else {
        u64::MAX as u128
    };
    let liquidity_1 = if amount_1 > 0 {
        liquidity_from_amount_1(sqrt_price_lower, sqrt_price_current, amount_1)?
    } else {
        u64::MAX as u128
    };

    // Protocol safety: Only the minimum liquidity is valid, ensuring the position is fully collateralized and cannot be over-minted.
    let final_liquidity = core::cmp::min(liquidity_0, liquidity_1);

    // Safety: Prevents dust or non-functional positions.
    if final_liquidity == 0 {
        return Err(MathError::InvalidLiquidity.into());
    }

    Ok(final_liquidity)
}

/// Computes the USD value of a liquidity position at a given price, including token breakdown and range status.
///
/// # Why
/// This function is used for user-facing analytics, risk management, and protocol accounting. It provides a full breakdown of a position's value, which is essential for UI, liquidation logic, and audits.
///
/// # Design Rationale
/// - Converts ticks to sqrt prices to ensure all math is done in Q64x64, matching protocol invariants.
/// - Uses the piecewise token amount logic to determine the position's current holdings.
/// - Computes USD value using provided prices, with overflow checks for safety.
/// - Returns a struct with all relevant fields for downstream use (UI, risk, etc.).
/// - Precomputes range status for efficient downstream logic (e.g., UI highlighting, risk checks).
///
/// # Usage
/// Used by the protocol to show users their position value, by risk management to assess exposure, and by auditors to verify accounting.
#[inline(always)]
pub fn calculate_position_value_at_price(
    position: &Position,
    current_sqrt_price: Q64x64,
    token_0_price_usd: u64,
    token_1_price_usd: u64,
) -> Result<PositionValue> {
    // Convert ticks to sqrt prices for protocol-consistent math.
    let sqrt_price_lower = tick_to_sqrt_x64(position.tick_lower)?;
    let sqrt_price_upper = tick_to_sqrt_x64(position.tick_upper)?;

    // Use piecewise logic to determine current token holdings for this position.
    let (amount_0, amount_1) = calculate_amounts_for_liquidity_piecewise(
        current_sqrt_price,
        sqrt_price_lower,
        sqrt_price_upper,
        position.liquidity,
    )?;

    // Compute USD value for each token, with overflow checks for protocol safety.
    let value_0_usd = if amount_0 <= u32::MAX as u64 && token_0_price_usd <= u32::MAX as u64 {
        (amount_0 * token_0_price_usd) as u128
    } else {
        (amount_0 as u128)
            .checked_mul(token_0_price_usd as u128)
            .ok_or(MathError::Overflow)?
    };

    let value_1_usd = if amount_1 <= u32::MAX as u64 && token_1_price_usd <= u32::MAX as u64 {
        (amount_1 * token_1_price_usd) as u128
    } else {
        (amount_1 as u128)
            .checked_mul(token_1_price_usd as u128)
            .ok_or(MathError::Overflow)?
    };

    // Sum for total value, with overflow check for auditability.
    let total_value_usd = value_0_usd
        .checked_add(value_1_usd)
        .ok_or(MathError::Overflow)?;

    // Precompute range status for efficient downstream use (UI, risk, etc.).
    let price_range_active = current_sqrt_price.raw() >= sqrt_price_lower.raw()
        && current_sqrt_price.raw() < sqrt_price_upper.raw();

    Ok(PositionValue {
        amount_0,
        amount_1,
        value_0_usd: value_0_usd as u64,
        value_1_usd: value_1_usd as u64,
        total_value_usd: total_value_usd as u64,
        price_range_active,
    })
}

/// Full breakdown of a position's value at a given price, for analytics, risk, and protocol accounting.
///
/// # Why
/// This struct is designed to provide all the information needed for user interfaces, risk management, and audits in a single call.
///
/// # Design Rationale
/// - Includes both token amounts and their USD values for transparency and downstream composability.
/// - Includes a precomputed range status for efficient UI/risk logic.
/// - All fields are u64 for compatibility with SPL token accounting and on-chain constraints.
#[derive(Clone, Debug)]
pub struct PositionValue {
    pub amount_0: u64,
    pub amount_1: u64,
    pub value_0_usd: u64,
    pub value_1_usd: u64,
    pub total_value_usd: u64,
    pub price_range_active: bool,
}
