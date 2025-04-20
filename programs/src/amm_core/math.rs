/// Fluxa AMM Core Math Library
///
/// This module implements the mathematical operations required for Fluxa's concentrated
/// liquidity AMM functionality. It provides functions for converting between tick indices
/// and prices, calculating token amounts from liquidity values, processing swap operations,
/// and managing fee accumulation.
///
/// The implementation uses fixed-point arithmetic throughout, primarily in Q64.64 format
/// where values are scaled by 2^64 to maintain precision during calculations.
use crate::constants::*;
use crate::errors::ErrorCode;
use anchor_lang::prelude::*;

/// Q64.64 fixed-point representation scaling factor
///
/// This constant represents 2^64, used for fixed-point calculations throughout the AMM.
/// Values are typically represented as an integer scaled by this factor to maintain
/// precision during mathematical operations.
pub const Q64: u128 = 1u128 << 64;

// Constants for tick-to-sqrt-price calculations
const _LOG_BASE: u128 = 100; // For precision in log calculation
const _BPS_PER_TICK: u128 = 1; // 0.01% per tick (1 basis point)

/// Computes the amount of token A required for a given amount of liquidity at a specified price range.
///
/// Token A represents the "base" token in the pair. This function calculates how much of token A
/// is needed to provide the specified liquidity within the given price bounds, considering the
/// current price of the pool.
///
/// # Mathematical Formula
/// When current price is in range:
/// `amount_a = liquidity * (1/sqrt_price_lower - 1/sqrt_price_current)`
///
/// When current price is below range:
/// `amount_a = liquidity * (1/sqrt_price_lower - 1/sqrt_price_upper)`
///
/// When current price is above range:
/// `amount_a = 0`
///
/// # Parameters
/// * `liquidity` - The amount of liquidity to provide, in L-units (Q64.64 fixed-point)
/// * `sqrt_price_lower` - The lower sqrt price bound of the position (Q64.64 fixed-point)
/// * `sqrt_price_upper` - The upper sqrt price bound of the position (Q64.64 fixed-point)
/// * `sqrt_price_current` - The current sqrt price of the pool (Q64.64 fixed-point)
///
/// # Returns
/// * `Result<u64>` - The calculated amount of token A needed, or an error
///
/// # Errors
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn get_token_a_from_liquidity(
    liquidity: u128,
    sqrt_price_lower: u128,
    sqrt_price_upper: u128,
    sqrt_price_current: u128,
) -> Result<u64> {
    // Price range logic: different calculations based on where current price sits relative to position range
    let sqrt_price_to_use = if sqrt_price_current > sqrt_price_upper {
        // Price is above range: position is 100% token B, 0% token A
        return Ok(0);
    } else if sqrt_price_current < sqrt_price_lower {
        // Price is below range: position is 100% token A, 0% token B
        // For token A calculation when below range, we use the upper price bound
        sqrt_price_upper
    } else {
        // Price is in range: position is a mix of token A and token B
        sqrt_price_current
    };

    // Calculate amount_a = liquidity * (1/sqrt_price_lower - 1/sqrt_price_to_use)
    // Using fixed-point arithmetic for precision

    // Compute (1/sqrt_price_lower) * Q64 - invert the lower bound
    let inv_lower = Q64
        .checked_mul(Q64)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(sqrt_price_lower)
        .ok_or(ErrorCode::MathOverflow)?;

    // Compute (1/sqrt_price_to_use) * Q64 - invert the comparison price
    let inv_current = Q64
        .checked_mul(Q64)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(sqrt_price_to_use)
        .ok_or(ErrorCode::MathOverflow)?;

    // Safety check: inv_lower should always be >= inv_current due to price ordering
    if inv_lower < inv_current {
        return Err(ErrorCode::MathOverflow.into());
    }

    // Calculate liquidity * (inv_lower - inv_current)
    let delta_inv = inv_lower
        .checked_sub(inv_current)
        .ok_or(ErrorCode::MathOverflow)?;

    let amount_a_u128 = liquidity
        .checked_mul(delta_inv)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(Q64) // Scale back from Q128.128 to Q64.64
        .ok_or(ErrorCode::MathOverflow)?;

    // Convert to u64, ensuring we don't overflow
    if amount_a_u128 > u64::MAX as u128 {
        return Err(ErrorCode::MathOverflow.into());
    }

    Ok(amount_a_u128 as u64)
}

/// Computes the amount of token B required for a given amount of liquidity at a specified price range.
///
/// Token B represents the "quote" token in the pair. This function calculates how much of token B
/// is needed to provide the specified liquidity within the given price bounds, considering the
/// current price of the pool.
///
/// # Mathematical Formula
/// When current price is in range:
/// `amount_b = liquidity * (sqrt_price_current - sqrt_price_lower)`
///
/// When current price is below range:
/// `amount_b = 0`
///
/// When current price is above range:
/// `amount_b = liquidity * (sqrt_price_upper - sqrt_price_lower)`
///
/// # Parameters
/// * `liquidity` - The amount of liquidity to provide, in L-units (Q64.64 fixed-point)
/// * `sqrt_price_lower` - The lower sqrt price bound of the position (Q64.64 fixed-point)
/// * `sqrt_price_upper` - The upper sqrt price bound of the position (Q64.64 fixed-point)
/// * `sqrt_price_current` - The current sqrt price of the pool (Q64.64 fixed-point)
///
/// # Returns
/// * `Result<u64>` - The calculated amount of token B needed, or an error
///
/// # Errors
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn get_token_b_from_liquidity(
    liquidity: u128,
    sqrt_price_lower: u128,
    sqrt_price_upper: u128,
    sqrt_price_current: u128,
) -> Result<u64> {
    // Price range logic: different calculations based on where current price sits relative to position range
    let sqrt_price_to_use = if sqrt_price_current < sqrt_price_lower {
        // Price is below range: position is 100% token A, 0% token B
        return Ok(0);
    } else if sqrt_price_current > sqrt_price_upper {
        // Price is above range: position is 100% token B, 0% token A
        // For token B calculation when above range, we use the upper price bound
        sqrt_price_upper
    } else {
        // Price is in range: position is a mix of token A and token B
        sqrt_price_current
    };

    // Calculate amount_b = liquidity * (sqrt_price_to_use - sqrt_price_lower)
    // Safety check: sqrt_price_to_use should always be >= sqrt_price_lower due to price ordering
    if sqrt_price_to_use < sqrt_price_lower {
        return Err(ErrorCode::MathOverflow.into());
    }

    let delta_sqrt_price = sqrt_price_to_use
        .checked_sub(sqrt_price_lower)
        .ok_or(ErrorCode::MathOverflow)?;

    let amount_b_u128 = liquidity
        .checked_mul(delta_sqrt_price)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(Q64) // Scale back from Q128.128 to Q64.64
        .ok_or(ErrorCode::MathOverflow)?;

    // Convert to u64, ensuring we don't overflow
    if amount_b_u128 > u64::MAX as u128 {
        return Err(ErrorCode::MathOverflow.into());
    }

    Ok(amount_b_u128 as u64)
}

/// Converts a tick index to a square root price in Q64.64 fixed-point format.
///
/// In Fluxa's concentrated liquidity AMM, prices are discretized into "ticks" where
/// each tick represents a 0.01% (1.0001x) change in price. This function converts
/// from the discrete tick space to the continuous square root price space.
///
/// # Mathematical Formula
/// `sqrt_price = 1.0001^(tick/2) * 2^64`
///
/// # Implementation Details
/// This function uses binary exponentiation for efficiency, with precomputed powers
/// of sqrt(1.0001) to speed up the calculation.
///
/// # Parameters
/// * `tick` - The tick index to convert
///
/// # Returns
/// * `Result<u128>` - The corresponding sqrt price in Q64.64 fixed-point format, or an error
///
/// # Errors
/// * `ErrorCode::InvalidTickRange` - If the tick is outside the allowed range
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn tick_to_sqrt_price(tick: i32) -> Result<u128> {
    // Tick validation
    require!(
        (MIN_TICK..=MAX_TICK).contains(&tick),
        ErrorCode::InvalidTickRange
    );

    // Each tick represents a 0.01% (1.0001) change in price
    // sqrt_price = 1.0001^(tick/2) * Q64

    // For efficiency, we'll use the binary exponentiation method
    // First, handle negative ticks
    let abs_tick = tick.unsigned_abs() as u128;
    let is_negative = tick < 0;

    // Precomputed values for (sqrt(1.0001))^n where n is a power of 2
    // These values are pre-calculated to avoid expensive power operations at runtime
    let sqrt_1_0001_powers = [
        1_000050000000000_u128,  // sqrt(1.0001)^1
        1_000100002500000_u128,  // sqrt(1.0001)^2
        1_000200010000000_u128,  // sqrt(1.0001)^4
        1_000400040001000_u128,  // sqrt(1.0001)^8
        1_000800160008000_u128,  // sqrt(1.0001)^16
        1_001601280064000_u128,  // sqrt(1.0001)^32
        1_003204640128000_u128,  // sqrt(1.0001)^64
        1_006415808256000_u128,  // sqrt(1.0001)^128
        1_012867840512000_u128,  // sqrt(1.0001)^256
        1_025857067264000_u128,  // sqrt(1.0001)^512
        1_052213753312000_u128,  // sqrt(1.0001)^1024
        1_106801830144000_u128,  // sqrt(1.0001)^2048
        1_225785703184000_u128,  // sqrt(1.0001)^4096
        1_503213729408000_u128,  // sqrt(1.0001)^8192
        2_259689019904000_u128,  // sqrt(1.0001)^16384
        5_105885570048000_u128,  // sqrt(1.0001)^32768
        26_090033976320000_u128, // sqrt(1.0001)^65536
    ];

    let scaled_powers: Vec<u128> = sqrt_1_0001_powers
        .iter()
        .map(|&p| p * 1_000_000_000_000_000) // Scale up for precision
        .collect();

    // Binary exponentiation
    // Start with 1.0, scaled up for precision
    let mut sqrt_price = 1_000_000_000_000_000_000_000_000_000_000_u128;

    // Apply binary exponentiation: decompose tick into powers of 2 and multiply
    for (i, &power) in scaled_powers.iter().enumerate().take(17) {
        if (abs_tick & (1 << i)) != 0 {
            sqrt_price = sqrt_price
                .checked_mul(power)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(1_000_000_000_000_000_000)
                .ok_or(ErrorCode::MathOverflow)?;
        }
    }

    // If tick is negative, we need to invert the sqrt_price (1/x)
    let final_sqrt_price = if is_negative {
        1_000_000_000_000_000_000_000_000_000_000_u128 // 1.0 scaled up
            .checked_mul(1_000_000_000_000_000_000_u128)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(sqrt_price)
            .ok_or(ErrorCode::MathOverflow)?
    } else {
        sqrt_price
    };

    // Convert to Q64.64 format
    let sqrt_price_q64 = final_sqrt_price
        .checked_mul(Q64)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(1_000_000_000_000_000_000)
        .ok_or(ErrorCode::MathOverflow)?;

    Ok(sqrt_price_q64)
}

/// Converts a square root price to the nearest tick index.
///
/// This function performs the inverse operation of `tick_to_sqrt_price`, converting
/// from a continuous square root price to the discrete tick space. It uses binary search
/// to find the closest tick that corresponds to the given square root price.
///
/// # Mathematical Formula (theoretical)
/// `tick = log(sqrt_price / Q64) * 2 / log(1.0001)`
///
/// # Implementation Details
/// Rather than using logarithms directly (which are computationally expensive), this function
/// uses a binary search to find the closest tick efficiently.
///
/// # Parameters
/// * `sqrt_price` - The square root price in Q64.64 fixed-point format
///
/// # Returns
/// * `Result<i32>` - The corresponding tick index, or an error
///
/// # Errors
/// * `ErrorCode::PriceOutOfRange` - If the sqrt_price is outside the allowed range
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn sqrt_price_to_tick(sqrt_price: u128) -> Result<i32> {
    // Validate sqrt_price is within allowed range
    require!(sqrt_price >= MIN_SQRT_PRICE, ErrorCode::PriceOutOfRange);

    // We'll use a binary search to find the closest tick
    let mut low = MIN_TICK;
    let mut high = MAX_TICK;
    let mut mid;

    while low <= high {
        mid = (low + high) / 2;

        let sqrt_price_at_mid = tick_to_sqrt_price(mid)?;

        match sqrt_price_at_mid.cmp(&sqrt_price) {
            std::cmp::Ordering::Equal => return Ok(mid),
            std::cmp::Ordering::Less => low = mid + 1,
            std::cmp::Ordering::Greater => high = mid - 1,
        }
    }

    // Return the closest tick by comparing distances
    let sqrt_price_low = tick_to_sqrt_price(high)?;
    let sqrt_price_high = tick_to_sqrt_price(low)?;

    let diff_low = if sqrt_price >= sqrt_price_low {
        sqrt_price - sqrt_price_low
    } else {
        sqrt_price_low - sqrt_price
    };

    let diff_high = if sqrt_price >= sqrt_price_high {
        sqrt_price - sqrt_price_high
    } else {
        sqrt_price_high - sqrt_price
    };

    if diff_low <= diff_high {
        Ok(high)
    } else {
        Ok(low)
    }
}

/// Calculates the next price and consumed amount for a single step of a swap operation.
///
/// This core function computes how a swap affects the price within a single price range,
/// without crossing tick boundaries. It's used repeatedly in the main swap algorithm to
/// process swaps that may cross multiple ticks.
///
/// # Mathematical Formulas
/// For token A (x) input:
/// `new_sqrt_price = sqrt_price * liquidity / (liquidity + amount_in * sqrt_price / Q64)`
///
/// For token B (y) input:
/// `new_sqrt_price = sqrt_price + (amount_in * Q64) / liquidity`
///
/// # Parameters
/// * `sqrt_price` - Current square root price in Q64.64 fixed-point format
/// * `liquidity` - Current liquidity in the active range
/// * `amount` - Input amount of token to swap
/// * `is_token_a` - True if swapping token A for token B, false if swapping token B for token A
///
/// # Returns
/// * `Result<(u128, u64)>` - Tuple of (new_sqrt_price, amount_consumed), or an error
///
/// # Errors
/// * `ErrorCode::InsufficientLiquidity` - If there is no liquidity to perform the swap
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn calculate_swap_step(
    sqrt_price: u128,
    liquidity: u128,
    amount: u64,
    is_token_a: bool,
) -> Result<(u128, u64)> {
    // Cannot swap if there's no liquidity
    if liquidity == 0 {
        return Err(ErrorCode::InsufficientLiquidity.into());
    }

    // Early return for zero amount
    if amount == 0 {
        return Ok((sqrt_price, 0));
    }

    let new_sqrt_price: u128;
    let amount_consumed: u64;

    if is_token_a {
        // Token A to Token B swap (x to y)

        // Scale the input amount by the current sqrt price to get it in the right units
        let amount_in_scaled = (amount as u128)
            .checked_mul(sqrt_price)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(Q64)
            .ok_or(ErrorCode::MathOverflow)?;

        // Calculate new sqrt price using the constant product formula
        let denominator = liquidity
            .checked_add(amount_in_scaled)
            .ok_or(ErrorCode::MathOverflow)?;

        new_sqrt_price = sqrt_price
            .checked_mul(liquidity)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(denominator)
            .ok_or(ErrorCode::MathOverflow)?;

        // Calculate amount consumed based on the price change
        let sqrt_price_delta = sqrt_price
            .checked_sub(new_sqrt_price)
            .ok_or(ErrorCode::MathOverflow)?;

        amount_consumed = (liquidity
            .checked_mul(sqrt_price_delta)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(sqrt_price)
            .ok_or(ErrorCode::MathOverflow)? as u64)
            .min(amount); // Cap at the requested amount
    } else {
        // Token B to Token A swap (y to x)

        // Scale the input amount to Q64.64 format
        let amount_in_scaled = (amount as u128)
            .checked_mul(Q64)
            .ok_or(ErrorCode::MathOverflow)?;

        // Calculate price change based on the input amount
        let price_delta = amount_in_scaled
            .checked_div(liquidity)
            .ok_or(ErrorCode::MathOverflow)?;

        // Calculate new sqrt price by adding the delta
        new_sqrt_price = sqrt_price
            .checked_add(price_delta)
            .ok_or(ErrorCode::MathOverflow)?;

        // Calculate amount consumed based on the price change
        let sqrt_price_delta = new_sqrt_price
            .checked_sub(sqrt_price)
            .ok_or(ErrorCode::MathOverflow)?;

        amount_consumed = (liquidity
            .checked_mul(sqrt_price_delta)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(Q64)
            .ok_or(ErrorCode::MathOverflow)? as u64)
            .min(amount); // Cap at the requested amount
    }

    // Ensure new price is within global bounds
    let final_sqrt_price = new_sqrt_price.max(MIN_SQRT_PRICE);

    Ok((final_sqrt_price, amount_consumed))
}

/// Calculates the fee growth inside a position's price range.
///
/// This function determines how much of the global fee growth applies to a specific
/// position based on its price range and the current price. It's critical for
/// tracking earned fees for concentrated liquidity positions.
///
/// # Mathematical Concept
/// For a position with tick range [lower, upper], we calculate:
/// `fee_growth_inside = fee_growth_global - fee_growth_below - fee_growth_above`
///
/// Where fee_growth_below/above represent the accumulated fee growth outside
/// the position's range.
///
/// # Parameters
/// * `tick_lower` - Lower tick bound of the position
/// * `tick_upper` - Upper tick bound of the position
/// * `tick_current` - Current tick of the pool
/// * `fee_growth_global` - Global fee growth across all ticks
/// * `fee_growth_below` - Fee growth below the lower tick
/// * `fee_growth_above` - Fee growth above the upper tick
///
/// # Returns
/// * `Result<u128>` - The fee growth inside the position's range, or an error
///
/// # Errors
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn calculate_fee_growth_inside(
    tick_lower: i32,
    tick_upper: i32,
    tick_current: i32,
    fee_growth_global: u128,
    fee_growth_below: u128,
    fee_growth_above: u128,
) -> Result<u128> {
    // Determine which fee growth values to use based on the current tick position
    // relative to the position's range

    // For the "below" component: if current tick is >= lower bound,
    // use the tracked fee growth below; otherwise, complement it.
    let fee_growth_below_used = if tick_current >= tick_lower {
        fee_growth_below
    } else {
        fee_growth_global
            .checked_sub(fee_growth_below)
            .ok_or(ErrorCode::MathOverflow)?
    };

    // For the "above" component: if current tick is < upper bound,
    // use the tracked fee growth above; otherwise, complement it.
    let fee_growth_above_used = if tick_current < tick_upper {
        fee_growth_above
    } else {
        fee_growth_global
            .checked_sub(fee_growth_above)
            .ok_or(ErrorCode::MathOverflow)?
    };

    // Calculate fee growth inside as global minus outside components
    fee_growth_global
        .checked_sub(fee_growth_below_used)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_sub(fee_growth_above_used)
        .ok_or(ErrorCode::MathOverflow.into())
}

/// Converts a numeric price to a square root price in Q64.64 fixed-point format.
///
/// This utility function converts a regular price value to the square root price
/// format used throughout the AMM's internal calculations.
///
/// # Mathematical Formula
/// `sqrt_price = sqrt(price) * 2^64`
///
/// # Implementation Details
/// Uses Newton's method for computing the square root efficiently.
///
/// # Parameters
/// * `price` - The price value to convert
///
/// # Returns
/// * `Result<u128>` - The square root price in Q64.64 format, or an error
///
/// # Errors
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn price_to_sqrt_price(price: u64) -> Result<u128> {
    // Convert price to a u128 and scale it to Q64.64 format
    let price_u128 = (price as u128)
        .checked_mul(Q64)
        .ok_or(ErrorCode::MathOverflow)?;

    // We'll use Newton's method to compute the square root
    // x_{n+1} = (x_n + a/x_n) / 2

    // Initial guess: a/2 (works well for square roots)
    let mut x = price_u128.checked_add(1).ok_or(ErrorCode::MathOverflow)? / 2;

    // Newton's method iterations
    // Usually converges to sufficient precision in few iterations
    for _ in 0..10 {
        let next_x = (x + price_u128 / x) / 2;
        if next_x >= x {
            // If we're not improving, break early
            break;
        }
        x = next_x;
    }

    // x now contains sqrt(price) * 2^64
    Ok(x)
}
