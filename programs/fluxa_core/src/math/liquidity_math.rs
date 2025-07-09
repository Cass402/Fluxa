use crate::error::MathError;
use crate::math::core_arithmetic::{
    liquidity_from_amount_0, liquidity_from_amount_1, tick_to_sqrt_x64, Q64x64,
};
use crate::utils::constants::MAX_TOKEN_AMOUNT;
use anchor_lang::prelude::*;

/// Calculates the amount of token0 required for a given liquidity between two square root price boundaries.
///
/// # Arguments
///
/// * `sqrt_price_lower` - The lower boundary of the price range as a Q64x64 fixed-point number.
/// * `sqrt_price_upper` - The upper boundary of the price range as a Q64x64 fixed-point number.
/// * `liquidity` - The amount of liquidity as a Q64x64 fixed-point number.
///
/// # Returns
///
/// Returns `Ok(u64)` with the amount of token0 required, or an error if the input is invalid or an arithmetic operation fails.
///
/// # Errors
///
/// Returns an error if:
/// - The lower price is not less than the upper price.
/// - The lower price is zero.
/// - Any arithmetic operation overflows or underflows.
///
/// # Formula
///
/// The calculation is based on the formula:
/// amount0 = liquidity * (sqrt_price_upper - sqrt_price_lower) / (sqrt_price_lower * sqrt_price_upper)
///
/// The result is returned as a u64 value.
#[inline(always)]
fn calculate_amount_0_delta(
    sqrt_price_lower: Q64x64,
    sqrt_price_upper: Q64x64,
    liquidity: Q64x64,
) -> Result<u64> {
    // Ensure that the lower price is less than the upper price and not zero
    if sqrt_price_lower.raw() >= sqrt_price_upper.raw() || sqrt_price_lower.raw() == 0 {
        return Err(MathError::InvalidPriceRange.into());
    }

    // Calculate the price difference between the upper and lower square root prices
    let price_diff = sqrt_price_upper.checked_sub(sqrt_price_lower)?;

    // Calculate the numerator as liquidity multiplied by the price difference
    let numerator = liquidity.checked_mul(price_diff)?;

    // Calculate the denominator as the product of the lower and upper square root prices
    // This is used to normalize the liquidity based on the price range
    let denominator = sqrt_price_lower.checked_mul(sqrt_price_upper)?;

    // Calculate the result by dividing the numerator by the denominator
    // This gives the amount of token0 required for the specified liquidity in the price range
    let result = numerator.checked_div(denominator)?;

    // Extract the amount of token0 from the result
    // The result is in Q64x64 format, so we shift right by 64
    let amount0 = (result.raw() >> 64) as u64;

    Ok(amount0)
}

/// Calculates the amount of token1 required for a given liquidity between two square root price boundaries.
///
/// # Arguments
///
/// * `sqrt_price_lower` - The lower boundary of the price range as a Q64x64 fixed-point number.
/// * `sqrt_price_upper` - The upper boundary of the price range as a Q64x64 fixed-point number.
/// * `liquidity` - The amount of liquidity as a Q64x64 fixed-point number.
///
/// # Returns
///
/// Returns `Ok(u64)` with the amount of token1 required, or an error if the input is invalid or an arithmetic operation fails.
///
/// # Errors
///
/// Returns an error if:
/// - The lower price is not less than the upper price.
/// - Any arithmetic operation overflows or underflows.
///
/// # Formula
///
/// The calculation is based on the formula:
/// amount1 = liquidity * (sqrt_price_upper - sqrt_price_lower)
///
/// The result is returned as a u64 value.
#[inline(always)]
fn calculate_amount_1_delta(
    sqrt_price_lower: Q64x64,
    sqrt_price_upper: Q64x64,
    liquidity: Q64x64,
) -> Result<u64> {
    // Ensure that the lower price is less than the upper price
    if sqrt_price_lower.raw() >= sqrt_price_upper.raw() {
        return Err(MathError::InvalidPriceRange.into());
    }

    // Calculate the price difference between the upper and lower square root prices
    let price_diff = sqrt_price_upper.checked_sub(sqrt_price_lower)?;

    // Calculate the result by multiplying the liquidity by the price difference
    let result = liquidity.checked_mul(price_diff)?;

    // Extract the amount of token1 from the result
    // The result is in Q64x64 format, so we shift right by 64
    // This gives the amount of token1 required for the specified liquidity in the price range
    let amount1 = (result.raw() >> 64) as u64;

    Ok(amount1)
}

/// Calculates the required amounts of token0 and token1 for a given liquidity position,
/// based on the current, lower, and upper square root price boundaries.
///
/// This function determines the amounts of token0 and token1 needed to provide the specified
/// liquidity, depending on the current price's position relative to the price range:
/// - If the current price is below the lower boundary, only token0 is required.
/// - If the current price is above the upper boundary, only token1 is required.
/// - If the current price is within the range, both token0 and token1 are required.
///
/// # Arguments
///
/// * `sqrt_price_current` - The current square root price as a Q64x64 fixed-point number.
/// * `sqrt_price_lower` - The lower boundary of the price range as a Q64x64 fixed-point number.
/// * `sqrt_price_upper` - The upper boundary of the price range as a Q64x64 fixed-point number.
/// * `liquidity` - The amount of liquidity as a Q64x64 fixed-point number.
///
/// # Returns
///
/// Returns `Ok((u64, u64))` with the amounts of token0 and token1 required, respectively.
/// Returns an error if the input is invalid or if any arithmetic operation fails.
///
/// # Errors
///
/// Returns an error if:
/// - Any of the price boundaries or liquidity is zero.
/// - The lower price is not less than the upper price.
/// - Any arithmetic operation overflows or underflows.
/// - The resulting token amounts exceed the maximum allowed value.
///
/// # Examples
///
/// ```
/// let (amount_0, amount_1) = PriceMath::calculate_amounts_for_liquidity_piecewise(
///     sqrt_price_current,
///     sqrt_price_lower,
///     sqrt_price_upper,
///     liquidity,
/// )?;
/// ```
#[inline(always)]
pub fn calculate_amounts_for_liquidity_piecewise(
    sqrt_price_current: Q64x64,
    sqrt_price_lower: Q64x64,
    sqrt_price_upper: Q64x64,
    liquidity: Q64x64,
) -> Result<(u64, u64)> {
    // Validate the input prices and liquidity
    if sqrt_price_lower.raw() == 0
        || sqrt_price_upper.raw() == 0
        || sqrt_price_lower.raw() >= sqrt_price_upper.raw()
        || liquidity.raw() == 0
    {
        return Err(MathError::InvalidInput.into());
    }

    // Optimized branching - most common case first (active range)
    let (amount_0, amount_1) = if sqrt_price_current.raw() > sqrt_price_lower.raw()
        && sqrt_price_current.raw() < sqrt_price_upper.raw()
    {
        // Calculate the amounts of token0 and token1 required for the active range
        let amount_0 = calculate_amount_0_delta(sqrt_price_current, sqrt_price_upper, liquidity)?;

        let amount_1 = calculate_amount_1_delta(sqrt_price_lower, sqrt_price_current, liquidity)?;

        (amount_0, amount_1)
    } else if sqrt_price_current.raw() <= sqrt_price_lower.raw() {
        // If the current price is below the lower boundary, only token0 is required
        let amount_0 = calculate_amount_0_delta(sqrt_price_lower, sqrt_price_upper, liquidity)?;

        (amount_0, 0)
    } else {
        // If the current price is above the upper boundary, only token1 is required
        let amount_1 = calculate_amount_1_delta(sqrt_price_lower, sqrt_price_upper, liquidity)?;

        (0, amount_1)
    };

    // Ensure that the calculated amounts do not exceed the maximum allowed value
    // This is to prevent overflow and ensure the amounts are within a reasonable range
    if amount_0 > MAX_TOKEN_AMOUNT || amount_1 > MAX_TOKEN_AMOUNT {
        return Err(MathError::ExcessiveTokenAmount.into());
    }

    Ok((amount_0, amount_1))
}

/// Calculates the liquidity for a given price range and amounts of token0 and token1.
/// This function determines the liquidity that can be provided based on the current square root price,
/// the lower and upper square root prices, and the amounts of token0 and token1 available.
///
/// # Arguments
///
/// * `sqrt_price_current` - The current square root price as a Q64x64 fixed-point number.
/// * `sqrt_price_lower` - The lower boundary of the price range as a Q64x64 fixed-point number.
/// * `sqrt_price_upper` - The upper boundary of the price range as a Q64x64 fixed-point number.
/// * `amount_0` - The amount of token0 available for liquidity as a `u64`.
/// * `amount_1` - The amount of token1 available for liquidity as a `u64`.
///
/// # Returns
/// Returns `Ok(u128)` with the calculated liquidity, or an error if the input is invalid or
/// if any arithmetic operation fails.
///
/// # Errors
/// Returns an error if:
/// - The lower price is not less than the upper price.
/// - The current price is outside the specified range.
/// - The amounts of token0 and token1 are both zero.
/// - Any arithmetic operation overflows or underflows.
/// - The calculated liquidity is zero.
pub fn calculate_liquidity(
    sqrt_price_current: Q64x64,
    sqrt_price_lower: Q64x64,
    sqrt_price_upper: Q64x64,
    amount_0: u64,
    amount_1: u64,
) -> Result<u128> {
    // Validate the input prices and amounts
    if sqrt_price_lower.raw() == 0 || sqrt_price_upper.raw() == 0 {
        return Err(MathError::InvalidPriceRange.into());
    }

    // Ensure that the lower price is less than the upper price
    if sqrt_price_current.raw() <= sqrt_price_lower.raw()
        || sqrt_price_current.raw() >= sqrt_price_upper.raw()
    {
        return Err(MathError::InvalidPriceRange.into());
    }

    // Ensure that at least one of the amounts is non-zero
    if amount_0 == 0 && amount_1 == 0 {
        return Err(MathError::InvalidInput.into());
    }

    // Calculate the liquidity based on the amounts of token0 and token1
    // This uses the core arithmetic functions to compute the liquidity from the provided amounts
    // The liquidity is calculated piecewise based on the current price and the price boundaries
    // If amount_0 is greater than zero, we calculate liquidity from token0; otherwise, we set it to the maximum possible value.
    // Similarly, if amount_1 is greater than zero, we calculate liquidity from token1; otherwise, we set it to the maximum possible value
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

    // The final liquidity is the minimum of the two calculated values
    // This ensures that the liquidity is constrained by the lesser of the two amounts
    // If both amounts are zero, the final liquidity will also be zero.
    let final_liquidity = core::cmp::min(liquidity_0, liquidity_1);

    // Ensure that the final liquidity is not zero
    // This is to prevent invalid liquidity positions and ensure that the position can be used in the AMM
    // If the final liquidity is zero, we return an error indicating invalid liquidity
    if final_liquidity == 0 {
        return Err(MathError::InvalidLiquidity.into());
    }

    Ok(final_liquidity)
}

/// Calculates the USD value of a liquidity position at a given current square root price.
///
/// This function determines the amounts of token0 and token1 held by the position at the specified
/// current price, then computes their respective USD values using the provided token prices. The total
/// USD value of the position is also calculated, along with an indicator of whether the position is
/// currently active (i.e., the current price is within the position's range).
///
/// # Arguments
///
/// * `position` - A reference to the `Position` struct, containing the position's tick range and liquidity.
/// * `current_sqrt_price` - The current square root price as a Q64x64 fixed-point number.
/// * `token_0_price_usd` - The price of token0 in USD, as a `u64`.
/// * `token_1_price_usd` - The price of token1 in USD, as a `u64`.
///
/// # Returns
///
/// Returns `Ok(PositionValue)` containing:
/// - `amount_0`: The amount of token0 held by the position.
/// - `amount_1`: The amount of token1 held by the position.
/// - `value_0_usd`: The USD value of token0 held.
/// - `value_1_usd`: The USD value of token1 held.
/// - `total_value_usd`: The total USD value of the position.
/// - `price_range_active`: A boolean indicating if the current price is within the position's range.
///
/// Returns an error if any arithmetic operation fails, if the tick-to-sqrt conversion fails,
/// or if the token amounts or values overflow.
///
/// # Errors
///
/// Returns an error if:
/// - The tick-to-sqrt conversion fails for the position's tick boundaries.
/// - The calculation of token amounts or their USD values overflows.
/// - Any arithmetic operation fails.
///
/// # Example
///
/// ```rust
/// let position_value = PriceMath::calculate_position_value_at_price(
///     &position,
///     current_sqrt_price,
///     token_0_price_usd,
///     token_1_price_usd,
/// )?;
/// ```
#[inline(always)]
fn calculate_position_value_at_price(
    position: &Position,
    current_sqrt_price: Q64x64,
    token_0_price_usd: u64,
    token_1_price_usd: u64,
) -> Result<PositionValue> {
    // Calculate the square root prices for the lower and upper ticks of the position
    // This converts the tick values to square root prices in Q64x64 format
    let sqrt_price_lower = tick_to_sqrt_x64(position.tick_lower)?;
    let sqrt_price_upper = tick_to_sqrt_x64(position.tick_upper)?;

    // Calculate the amounts of token0 and token1 based on the current price and the position's liquidity
    let (amount_0, amount_1) = calculate_amounts_for_liquidity_piecewise(
        current_sqrt_price,
        sqrt_price_lower,
        sqrt_price_upper,
        position.liquidity,
    )?;

    // Calculate the USD values of token0 and token1 held by the position
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

    // Calculate the total USD value of the position by summing the individual token values
    let total_value_usd = value_0_usd
        .checked_add(value_1_usd)
        .ok_or(MathError::Overflow)?;

    // Precompute price range check for better branch prediction
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

/// Represents the value of a position, including token amounts, their USD values,
/// the total value in USD, and whether the price range is currently active.
///
/// # Fields
/// - `amount_0`: The amount of token 0 in the position.
/// - `amount_1`: The amount of token 1 in the position.
/// - `value_0_usd`: The USD value of token 0 in the position.
/// - `value_1_usd`: The USD value of token 1 in the position.
/// - `total_value_usd`: The total USD value of the position (sum of `value_0_usd` and `value_1_usd`).
/// - `price_range_active`: Indicates if the position's price range is currently active.
#[derive(Clone, Debug)]
pub struct PositionValue {
    pub amount_0: u64,
    pub amount_1: u64,
    pub value_0_usd: u64,
    pub value_1_usd: u64,
    pub total_value_usd: u64,
    pub price_range_active: bool,
}
