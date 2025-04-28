/// Fluxa AMM Core Math Library
///
/// This module implements the mathematical operations required for Fluxa's concentrated
/// liquidity AMM functionality. It provides functions for converting between tick indices
/// and prices, calculating token amounts from liquidity values, processing swap operations,
/// and managing fee accumulation.
///
/// The implementation uses fixed-point arithmetic throughout, primarily in Q64.64 format
/// where values are scaled by 2^64 to maintain precision during calculations.
/// Additionally, it supports Q64.96 format for high-precision sqrt price operations
/// as specified in the technical design document.
use crate::constants::*;
use crate::errors::ErrorCode;
use anchor_lang::prelude::*;

/// Q64.64 fixed-point representation scaling factor
///
/// This constant represents 2^64, used for fixed-point calculations throughout the AMM.
/// Values are typically represented as an integer scaled by this factor to maintain
/// precision during mathematical operations.
pub const Q64: u128 = 1u128 << 64;

/// Q64.96 fixed-point representation scaling factor
///
/// This constant represents 2^96, used for specific precision-sensitive calculations.
/// This provides higher precision for sqrt price operations as specified in the technical design.
pub const Q96: u128 = 1u128 << 96;

/// Maximum value for u128
pub const U128MAX: u128 = u128::MAX;

// Constants for tick-to-sqrt-price calculations
const _LOG_BASE: u128 = 100; // For precision in log calculation
const _BPS_PER_TICK: u128 = 1; // 0.01% per tick (1 basis point)

// Additional fixed-point arithmetic operations for Q64.96 format

/// Convert from Q64.64 sqrt price to Q64.96 sqrt price
pub fn convert_sqrt_price_to_q96(sqrt_price_q64: u128) -> Result<u128> {
    // Multiply by 2^32 to shift from Q64.64 to Q64.96
    sqrt_price_q64
        .checked_shl(32)
        .ok_or(ErrorCode::MathOverflow.into())
}

/// Convert from Q64.96 sqrt price to Q64.64 sqrt price
pub fn convert_sqrt_price_from_q96(sqrt_price_q96: u128) -> Result<u128> {
    // Divide by 2^32 to shift from Q64.96 to Q64.64
    sqrt_price_q96
        .checked_shr(32)
        .ok_or(ErrorCode::MathOverflow.into())
}

/// Add two Q64.96 values 
pub fn add_q96(a: u128, b: u128) -> Result<u128> {
    a.checked_add(b).ok_or(ErrorCode::MathOverflow.into())
}

/// Subtract two Q64.96 values
pub fn sub_q96(a: u128, b: u128) -> Result<u128> {
    a.checked_sub(b).ok_or(ErrorCode::MathOverflow.into())
}

/// Multiply two Q64.96 values (returning Q64.96)
pub fn mul_q96(a: u128, b: u128) -> Result<u128> {
    // For full u128 multiplication, we need to handle overflow carefully
    // This is a simplified implementation - in production, consider using a u256 library
    
    // Split high and low bits for multiplication
    let a_hi = a >> 64;
    let a_lo = a & 0xFFFFFFFFFFFFFFFF;
    let b_hi = b >> 64;
    let b_lo = b & 0xFFFFFFFFFFFFFFFF;
    
    // Check that the high bits won't cause overflow when multiplied
    if a_hi > 0 && b_hi > 0 {
        return Err(ErrorCode::MathOverflow.into());
    }
    
    // Perform multiplication parts
    let lo_lo = a_lo * b_lo;
    let hi_lo = a_hi * b_lo;
    let lo_hi = a_lo * b_hi;
    
    // Combine results and shift to maintain Q64.96 format
    let mut result = lo_lo >> 96;
    result = result.checked_add((hi_lo << (64 - 96))).ok_or(ErrorCode::MathOverflow)?;
    result = result.checked_add((lo_hi << (64 - 96))).ok_or(ErrorCode::MathOverflow)?;
    
    Ok(result)
}

/// Divide a Q64.96 value by another Q64.96 value (returning Q64.96)
pub fn div_q96(a: u128, b: u128) -> Result<u128> {
    if b == 0 {
        return Err(ErrorCode::MathOverflow.into());
    }
    
    // To maintain precision, scale up the numerator before division
    let scaled_a = a.checked_shl(96).ok_or(ErrorCode::MathOverflow)?;
    
    // Perform the division
    scaled_a.checked_div(b).ok_or(ErrorCode::MathOverflow.into())
}

/// Convert a floating point price to Q64.96 sqrt price format
pub fn price_to_sqrt_price_q96(price: f64) -> Result<u128> {
    let sqrt_price = price.sqrt();
    // Convert to Q64.96 format
    let sqrt_price_q96 = (sqrt_price * (Q96 as f64)) as u128;
    Ok(sqrt_price_q96)
}

/// Convert a Q64.96 sqrt price to floating point price
pub fn sqrt_price_q96_to_price(sqrt_price_q96: u128) -> f64 {
    let sqrt_price_float = (sqrt_price_q96 as f64) / (Q96 as f64);
    sqrt_price_float * sqrt_price_float
}

/// Square a Q64.96 value, maintaining precision
pub fn square_q96(value: u128) -> Result<u128> {
    mul_q96(value, value)
}

/// Calculate square root of a Q64.96 value
/// Uses the Babylonian method for approximation
pub fn sqrt_q96(value: u128) -> Result<u128> {
    if value == 0 {
        return Ok(0);
    }
    
    // Initial guess - use a power of 2 close to the square root
    let msb = 127 - value.leading_zeros();
    let guess = 1u128 << ((msb / 2) + 48); // +48 for Q64.96 format
    
    // Perform iterations of the Babylonian method
    let mut result = guess;
    for _ in 0..10 {  // 10 iterations is typically enough for convergence
        // r = (r + x/r) / 2
        let value_div_result = div_q96(value, result)?;
        result = add_q96(result, value_div_result)?;
        result = result / 2;
    }
    
    Ok(result)
}

/// Calculate reciprocal of a Q64.96 value (1/x)
pub fn reciprocal_q96(value: u128) -> Result<u128> {
    if value == 0 {
        return Err(ErrorCode::MathOverflow.into());
    }
    
    // Calculate 1/x by dividing Q96 by the value
    div_q96(Q96, value)
}

/// Compute the tick index for a given sqrt price in Q64.96 format
///
/// # Parameters
/// * `sqrt_price_q96` - The sqrt price in Q64.96 format
///
/// # Returns
/// * `Result<i32>` - The corresponding tick index
pub fn get_tick_at_sqrt_price_q96(sqrt_price_q96: u128) -> Result<i32> {
    // Convert to a floating point for the logarithm calculation
    // In a production environment, consider using a fixed-point logarithm implementation
    let sqrt_price = (sqrt_price_q96 as f64) / (Q96 as f64);
    
    // Calculate log base 1.0001
    let log_base_1_0001 = sqrt_price.ln() / 0.0001.ln();
    
    // Convert to tick index
    let tick = (log_base_1_0001 / 2.0).round() as i32;
    
    Ok(tick)
}

/// Calculate the square root price at a given tick index in Q64.96 format
///
/// # Parameters
/// * `tick` - The tick index
///
/// # Returns
/// * `Result<u128>` - The sqrt price in Q64.96 format
pub fn get_sqrt_price_at_tick_q96(tick: i32) -> Result<u128> {
    // Calculate price = 1.0001^tick
    let price_power = (0.0001 * tick as f64).exp();
    
    // Convert to sqrt price
    let sqrt_price = price_power.sqrt();
    
    // Convert to Q64.96
    let sqrt_price_q96 = (sqrt_price * (Q96 as f64)) as u128;
    
    Ok(sqrt_price_q96)
}

/// Increases numerical precision when working with small price differences
/// 
/// # Parameters
/// * `price_a` - First price in Q64.96 format
/// * `price_b` - Second price in Q64.96 format
/// 
/// # Returns
/// * `Result<u128>` - Absolute difference in Q64.96 format, with enhanced precision
pub fn enhanced_price_difference_q96(price_a: u128, price_b: u128) -> Result<u128> {
    // Determine which price is larger
    if price_a >= price_b {
        sub_q96(price_a, price_b)
    } else {
        sub_q96(price_b, price_a)
    }
}

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

/// Enhanced version of get_token_a_from_liquidity using Q64.96 precision
///
/// # Parameters
/// * `liquidity` - The amount of liquidity to provide, in L-units (Q64.64 fixed-point)
/// * `sqrt_price_lower_q96` - The lower sqrt price bound of the position (Q64.96 fixed-point)
/// * `sqrt_price_upper_q96` - The upper sqrt price bound of the position (Q64.96 fixed-point)
/// * `sqrt_price_current_q96` - The current sqrt price of the pool (Q64.96 fixed-point)
///
/// # Returns
/// * `Result<u64>` - The calculated amount of token A needed, or an error
pub fn get_token_a_from_liquidity_q96(
    liquidity: u128,
    sqrt_price_lower_q96: u128,
    sqrt_price_upper_q96: u128,
    sqrt_price_current_q96: u128,
) -> Result<u64> {
    // Price range logic: different calculations based on where current price sits relative to position range
    let sqrt_price_to_use_q96 = if sqrt_price_current_q96 > sqrt_price_upper_q96 {
        // Price is above range: position is 100% token B, 0% token A
        return Ok(0);
    } else if sqrt_price_current_q96 < sqrt_price_lower_q96 {
        // Price is below range: position is 100% token A, 0% token B
        // For token A calculation when below range, we use the upper price bound
        sqrt_price_upper_q96
    } else {
        // Price is in range: position is a mix of token A and token B
        sqrt_price_current_q96
    };

    // Calculate amount_a = liquidity * (1/sqrt_price_lower - 1/sqrt_price_to_use)
    // Using fixed-point arithmetic for precision with Q64.96 format

    // Compute (1/sqrt_price_lower) - invert the lower bound
    let inv_lower_q96 = reciprocal_q96(sqrt_price_lower_q96)?;
    
    // Compute (1/sqrt_price_to_use) - invert the current/upper bound
    let inv_current_q96 = reciprocal_q96(sqrt_price_to_use_q96)?;
    
    // If lower price >= current/upper price, then delta is zero or negative
    if inv_lower_q96 <= inv_current_q96 {
        return Ok(0);
    }
    
    // Calculate difference of inverses: (1/sqrt_price_lower - 1/sqrt_price_to_use)
    let delta_q96 = sub_q96(inv_lower_q96, inv_current_q96)?;
    
    // Convert liquidity from Q64.64 to Q64.96
    let liquidity_q96 = convert_sqrt_price_to_q96(liquidity)?;
    
    // Calculate final result: liquidity * delta
    let token_amount_q96 = mul_q96(liquidity_q96, delta_q96)?;
    
    // Convert back to normal units
    let token_amount = token_amount_q96.checked_shr(96).ok_or(ErrorCode::MathOverflow)?;
    
    // Check if the result fits in u64
    if token_amount > u64::MAX as u128 {
        return Err(ErrorCode::MathOverflow.into());
    }
    
    Ok(token_amount as u64)
}

/// Enhanced version of get_token_b_from_liquidity using Q64.96 precision
///
/// # Parameters
/// * `liquidity` - The amount of liquidity to provide, in L-units (Q64.64 fixed-point)
/// * `sqrt_price_lower_q96` - The lower sqrt price bound of the position (Q64.96 fixed-point)
/// * `sqrt_price_upper_q96` - The upper sqrt price bound of the position (Q64.96 fixed-point)
/// * `sqrt_price_current_q96` - The current sqrt price of the pool (Q64.96 fixed-point)
///
/// # Returns
/// * `Result<u64>` - The calculated amount of token B needed, or an error
pub fn get_token_b_from_liquidity_q96(
    liquidity: u128,
    sqrt_price_lower_q96: u128,
    sqrt_price_upper_q96: u128,
    sqrt_price_current_q96: u128,
) -> Result<u64> {
    // Price range logic: different calculations based on where current price sits relative to position range
    let sqrt_price_to_use_q96 = if sqrt_price_current_q96 < sqrt_price_lower_q96 {
        // Price is below range: position is 100% token A, 0% token B
        return Ok(0);
    } else if sqrt_price_current_q96 > sqrt_price_upper_q96 {
        // Price is above range: position is 100% token B, 0% token A
        // For token B calculation when above range, we use the lower price bound
        sqrt_price_lower_q96
    } else {
        // Price is in range: position is a mix of token A and token B
        sqrt_price_current_q96
    };

    // Calculate amount_b = liquidity * (sqrt_price_to_use - sqrt_price_lower)
    // Using fixed-point arithmetic for precision with Q64.96 format

    // If the current/lower price <= lower price, then delta is zero or negative
    if sqrt_price_to_use_q96 <= sqrt_price_lower_q96 {
        return Ok(0);
    }
    
    // Calculate difference: (sqrt_price_to_use - sqrt_price_lower)
    let delta_q96 = sub_q96(sqrt_price_to_use_q96, sqrt_price_lower_q96)?;
    
    // Convert liquidity from Q64.64 to Q64.96
    let liquidity_q96 = convert_sqrt_price_to_q96(liquidity)?;
    
    // Calculate final result: liquidity * delta
    let token_amount_q96 = mul_q96(liquidity_q96, delta_q96)?;
    
    // Convert back to normal units
    let token_amount = token_amount_q96.checked_shr(96).ok_or(ErrorCode::MathOverflow)?;
    
    // Check if the result fits in u64
    if token_amount > u64::MAX as u128 {
        return Err(ErrorCode::MathOverflow.into());
    }
    
    Ok(token_amount as u64)
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

/// Rounds a tick to the nearest tick that is usable based on the given tick spacing.
///
/// In concentrated liquidity AMMs, ticks are often restricted to multiples of a certain spacing
/// value (e.g., every 10th tick only) to reduce the computational overhead and storage
/// requirements of tracking each tick. This function rounds a given tick to the nearest
/// valid tick according to the specified spacing.
///
/// # Mathematical Formula
/// `nearest_tick = round(tick / spacing) * spacing`
///
/// Where the rounding direction is determined by which is closer: the next lower usable tick
/// or the next higher usable tick.
///
/// # Parameters
/// * `tick` - The tick to round to the nearest usable tick
/// * `tick_spacing` - The spacing between usable ticks (e.g., 1, 10, 60)
///
/// # Returns
/// * `i32` - The nearest valid tick that is a multiple of tick_spacing
///
/// # Examples
/// - With tick=123 and tick_spacing=10, returns 120
/// - With tick=155 and tick_spacing=10, returns 160
/// - With tick=-43 and tick_spacing=5, returns -45
/// - With tick=0 and tick_spacing=any, returns 0
pub fn nearest_usable_tick(tick: i32, tick_spacing: i32) -> i32 {
    // Handle special case of tick_spacing = 1 (all ticks are usable)
    if tick_spacing == 1 {
        tick
    } else {
        // Calculate the remainder when dividing by tick_spacing
        let remainder = tick.rem_euclid(tick_spacing);

        // If tick is already a multiple of tick_spacing, it's already usable
        if remainder == 0 {
            tick
        } else {
            // Calculate distance to lower and upper usable ticks
            let distance_to_lower = remainder;
            let distance_to_upper = tick_spacing - remainder;

            // Round to the nearest usable tick
            if distance_to_lower <= distance_to_upper {
                // Closer to lower tick
                tick - distance_to_lower
            } else {
                // Closer to upper tick
                tick + distance_to_upper
            }
        }
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

/// Calculates the amount of token A required for a specific liquidity amount in a price range.
///
/// This function determines how much of token A is needed to provide the specified liquidity
/// between the given square root price bounds. It's used during position creation and
/// liquidity management.
///
/// # Parameters
/// * `liquidity` - The amount of liquidity to be provided
/// * `sqrt_price_lower` - The lower square root price bound
/// * `sqrt_price_upper` - The upper square root price bound
/// * `round_up` - Whether to round up (for deposits) or down (for withdrawals)
///
/// # Returns
/// * `Result<u128>` - The calculated token A amount, or an error
///
/// # Errors
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn get_amount_a_delta_for_price_range(
    liquidity: u128,
    sqrt_price_lower: u128,
    sqrt_price_upper: u128,
    round_up: bool,
) -> Result<u128> {
    // Safety check: ensure price bounds are valid
    if sqrt_price_lower > sqrt_price_upper {
        return Err(ErrorCode::InvalidTickRange.into());
    }

    // Calculate amount_a = liquidity * (1/sqrt_price_lower - 1/sqrt_price_upper)
    // Using fixed-point arithmetic for precision

    // Compute (1/sqrt_price_lower) * Q64 - invert the lower bound
    let inv_lower = Q64
        .checked_mul(Q64)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(sqrt_price_lower)
        .ok_or(ErrorCode::MathOverflow)?;

    // Compute (1/sqrt_price_upper) * Q64 - invert the upper bound
    let inv_upper = Q64
        .checked_mul(Q64)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(sqrt_price_upper)
        .ok_or(ErrorCode::MathOverflow)?;

    // Calculate the difference of the inverses
    let delta_inv = inv_lower
        .checked_sub(inv_upper)
        .ok_or(ErrorCode::MathOverflow)?;

    // Calculate liquidity * (inv_lower - inv_upper)
    let amount = liquidity
        .checked_mul(delta_inv)
        .ok_or(ErrorCode::MathOverflow)?;

    // Apply rounding based on the round_up parameter
    let result = if round_up {
        // Rounding up: Add (Q64 - 1) to the numerator before division
        // This ensures any fractional part becomes 1 more in the result
        amount
            .checked_add(Q64 - 1)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(Q64)
            .ok_or(ErrorCode::MathOverflow)?
    } else {
        // Rounding down: Simple division
        amount.checked_div(Q64).ok_or(ErrorCode::MathOverflow)?
    };

    Ok(result)
}

/// Calculates the amount of token B required for a specific liquidity amount in a price range.
///
/// This function determines how much of token B is needed to provide the specified liquidity
/// between the given square root price bounds. It's used during position creation and
/// liquidity management.
///
/// # Parameters
/// * `liquidity` - The amount of liquidity to be provided
/// * `sqrt_price_lower` - The lower square root price bound
/// * `sqrt_price_upper` - The upper square root price bound
/// * `round_up` - Whether to round up (for deposits) or down (for withdrawals)
///
/// # Returns
/// * `Result<u128>` - The calculated token B amount, or an error
///
/// # Errors
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
pub fn get_amount_b_delta_for_price_range(
    liquidity: u128,
    sqrt_price_lower: u128,
    sqrt_price_upper: u128,
    round_up: bool,
) -> Result<u128> {
    // Safety check: ensure price bounds are valid
    if sqrt_price_lower > sqrt_price_upper {
        return Err(ErrorCode::InvalidTickRange.into());
    }

    // Calculate amount_b = liquidity * (sqrt_price_upper - sqrt_price_lower)
    let delta_sqrt_price = sqrt_price_upper
        .checked_sub(sqrt_price_lower)
        .ok_or(ErrorCode::MathOverflow)?;

    // Calculate liquidity * (sqrt_price_upper - sqrt_price_lower)
    let amount = liquidity
        .checked_mul(delta_sqrt_price)
        .ok_or(ErrorCode::MathOverflow)?;

    // Apply rounding based on the round_up parameter
    let result = if round_up {
        // Rounding up: Add (Q64 - 1) to the numerator before division
        // This ensures any fractional part becomes 1 more in the result
        amount
            .checked_add(Q64 - 1)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(Q64)
            .ok_or(ErrorCode::MathOverflow)?
    } else {
        // Rounding down: Simple division
        amount.checked_div(Q64).ok_or(ErrorCode::MathOverflow)?
    };

    Ok(result)
}

/// Calculates the virtual reserves of token A and B at the current price.
///
/// In concentrated liquidity AMMs, virtual reserves represent the effective amounts
/// of tokens that determine the price at the current point. These are calculated
/// based on the current active liquidity in the pool and the current price.
///
/// # Mathematical Formula
/// For token A: `virtual_reserve_a = liquidity / sqrt_price`
/// For token B: `virtual_reserve_b = liquidity * sqrt_price`
///
/// # Parameters
/// * `liquidity` - The current active liquidity at the price point
/// * `sqrt_price` - The current sqrt price in Q64.64 fixed-point format
///
/// # Returns
/// * `Result<(u64, u64)>` - A tuple of (virtual_reserve_a, virtual_reserve_b), or an error
///
/// # Errors
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
/// * `ErrorCode::InsufficientLiquidity` - If the liquidity is zero
pub fn calculate_virtual_reserves(liquidity: u128, sqrt_price: u128) -> Result<(u64, u64)> {
    // Calculate individual reserves
    let virtual_a = calculate_virtual_reserve_a(liquidity, sqrt_price)?;
    let virtual_b = calculate_virtual_reserve_b(liquidity, sqrt_price)?;

    Ok((virtual_a, virtual_b))
}

/// Calculate virtual reserve for token A
///
/// # Parameters
/// * `liquidity` - Current liquidity
/// * `sqrt_price` - Current sqrt price in Q64.64 format
///
/// # Returns
/// * `Result<u64>` - Virtual reserve for token A
pub fn calculate_virtual_reserve_a(liquidity: u128, sqrt_price: u128) -> Result<u64> {
    if liquidity == 0 || sqrt_price == 0 {
        return Ok(0);
    }

    // For token A: virtual reserve = L / sqrt(P)
    // Multiply by Q96 to maintain precision, then divide by sqrt_price
    let numerator = liquidity.checked_mul(Q96).ok_or(ErrorCode::MathOverflow)?;

    // Divide by sqrt_price
    let result = numerator
        .checked_div(sqrt_price)
        .ok_or(ErrorCode::MathOverflow)?;

    // Ensure the result fits in u64
    if result > u64::MAX as u128 {
        return Err(ErrorCode::MathOverflow.into());
    }

    Ok(result as u64)
}

/// Calculate virtual reserve for token B
///
/// # Parameters
/// * `liquidity` - Current liquidity
/// * `sqrt_price` - Current sqrt price in Q64.64 format
///
/// # Returns
/// * `Result<u64>` - Virtual reserve for token B
pub fn calculate_virtual_reserve_b(liquidity: u128, sqrt_price: u128) -> Result<u64> {
    if liquidity == 0 || sqrt_price == 0 {
        return Ok(0);
    }

    // For token B: virtual reserve = L * sqrt(P) / Q96
    // Multiply liquidity by sqrt_price
    let product = liquidity
        .checked_mul(sqrt_price)
        .ok_or(ErrorCode::MathOverflow)?;

    // Divide by Q96 to get the actual value
    let result = product.checked_div(Q96).ok_or(ErrorCode::MathOverflow)?;

    // Ensure the result fits in u64
    if result > u64::MAX as u128 {
        return Err(ErrorCode::MathOverflow.into());
    }

    Ok(result as u64)
}

/// Calculate virtual reserves in a specific price range
///
/// # Parameters
/// * `liquidity` - Liquidity in the range
/// * `current_sqrt_price` - Current sqrt price
/// * `lower_sqrt_price` - Lower bound sqrt price
/// * `upper_sqrt_price` - Upper bound sqrt price
///
/// # Returns
/// * `Result<(u64, u64)>` - Virtual reserves in the range as (token_a, token_b)
pub fn calculate_virtual_reserves_in_range(
    liquidity: u128,
    current_sqrt_price: u128,
    lower_sqrt_price: u128,
    upper_sqrt_price: u128,
) -> Result<(u64, u64)> {
    if liquidity == 0 {
        return Ok((0, 0));
    }

    if current_sqrt_price <= lower_sqrt_price {
        // All liquidity is in token A
        let amount_a = get_amount_a_delta_for_price_range(
            liquidity,
            lower_sqrt_price,
            upper_sqrt_price,
            false,
        )? as u64;
        Ok((amount_a, 0))
    } else if current_sqrt_price >= upper_sqrt_price {
        // All liquidity is in token B
        let amount_b = get_amount_b_delta_for_price_range(
            liquidity,
            lower_sqrt_price,
            upper_sqrt_price,
            false,
        )? as u64;
        Ok((0, amount_b))
    } else {
        // Liquidity is split between token A and token B
        let amount_a = get_amount_a_delta_for_price_range(
            liquidity,
            current_sqrt_price,
            upper_sqrt_price,
            false,
        )? as u64;

        let amount_b = get_amount_b_delta_for_price_range(
            liquidity,
            lower_sqrt_price,
            current_sqrt_price,
            false,
        )? as u64;

        Ok((amount_a, amount_b))
    }
}

/// Calculates the effective liquidity from token amounts at the current price.
///
/// This function performs the reverse calculation of `calculate_virtual_reserves`,
/// determining how much liquidity corresponds to given token amounts at the current price.
///
/// # Mathematical Formula
/// From token A: `liquidity = virtual_reserve_a * sqrt_price`
/// From token B: `liquidity = virtual_reserve_b / sqrt_price`
///
/// # Parameters
/// * `reserve_a` - The virtual reserve of token A
/// * `reserve_b` - The virtual reserve of token B
/// * `sqrt_price` - The current sqrt price in Q64.64 fixed-point format
/// * `from_token_a` - If true, calculate from token A; if false, calculate from token B
///
/// # Returns
/// * `Result<u128>` - The calculated liquidity value, or an error
///
/// # Errors
/// * `ErrorCode::MathOverflow` - If any calculation results in an overflow
/// * `ErrorCode::ZeroReserveAmount` - If the token amount is zero
pub fn calculate_liquidity_from_reserves(
    reserve_a: u64,
    reserve_b: u64,
    sqrt_price: u128,
    from_token_a: bool,
) -> Result<u128> {
    if from_token_a {
        if reserve_a == 0 {
            return Err(ErrorCode::ZeroReserveAmount.into());
        }

        // L = virtual_reserve_a * sqrt(P)
        (reserve_a as u128)
            .checked_mul(sqrt_price)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(Q64)
            .ok_or(ErrorCode::MathOverflow.into())
    } else {
        if reserve_b == 0 {
            return Err(ErrorCode::ZeroReserveAmount.into());
        }

        // L = virtual_reserve_b / sqrt(P)
        (reserve_b as u128)
            .checked_mul(Q64)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(sqrt_price)
            .ok_or(ErrorCode::MathOverflow.into())
    }
}

/// Verifies the virtual reserves match the constant product formula.
///
/// In a concentrated liquidity AMM, the product of virtual reserves should equal the
/// square of the liquidity at the current price point. This function verifies this
/// invariant holds, within a small tolerance for rounding errors.
///
/// # Mathematical Formula
/// `virtual_reserve_a * virtual_reserve_b ≈ liquidity^2`
///
/// # Parameters
/// * `virtual_reserve_a` - Virtual reserve of token A
/// * `virtual_reserve_b` - Virtual reserve of token B  
/// * `expected_liquidity` - Current liquidity at the price point
///
/// # Returns
/// * `bool` - True if the invariant holds, false otherwise
pub fn verify_virtual_reserves_invariant(
    virtual_reserve_a: u64,
    virtual_reserve_b: u64,
    expected_liquidity: u128,
) -> bool {
    // Early return for edge cases
    if virtual_reserve_a == 0 || virtual_reserve_b == 0 {
        return expected_liquidity == 0;
    }

    // Calculate the product of virtual reserves
    let reserve_product = (virtual_reserve_a as u128).checked_mul(virtual_reserve_b as u128);
    if reserve_product.is_none() {
        return false;
    }

    // Calculate liquidity squared
    let liquidity_squared = expected_liquidity.checked_mul(expected_liquidity);
    if liquidity_squared.is_none() {
        return false;
    }

    // Compare with a small tolerance for rounding errors
    let reserve_product = reserve_product.unwrap();
    let liquidity_squared = liquidity_squared.unwrap();

    // Allow for small rounding errors - 0.1% tolerance (1 part in 1000)
    let tolerance = liquidity_squared / 1000;

    // Check if the difference between the products is within tolerance
    let difference = if reserve_product > liquidity_squared {
        reserve_product - liquidity_squared
    } else {
        liquidity_squared - reserve_product
    };

    difference <= tolerance
}

/// Helper function for dividing a value by sqrt price
#[allow(dead_code)]
fn div_by_sqrt_price_x64(value: u128, sqrt_price_x64: u128) -> Result<u64> {
    if sqrt_price_x64 == 0 {
        return Err(ErrorCode::MathOverflow.into());
    }

    // value * 2^64 / sqrt_price_x64
    let result = (value << 64)
        .checked_div(sqrt_price_x64)
        .ok_or(ErrorCode::MathOverflow)?;

    // Convert to u64, ensuring it doesn't overflow
    Ok(result.try_into().map_err(|_| ErrorCode::MathOverflow)?)
}

/// Helper function for multiplying a value by sqrt price
#[allow(dead_code)]
fn mul_by_sqrt_price_x64(value: u128, sqrt_price_x64: u128) -> Result<u64> {
    // value * sqrt_price_x64 / 2^64
    let result = value
        .checked_mul(sqrt_price_x64)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_shr(64)
        .ok_or(ErrorCode::MathOverflow)?;

    // Convert to u64, ensuring it doesn't overflow
    Ok(result.try_into().map_err(|_| ErrorCode::MathOverflow)?)
}

/// Calculate square root of a u128 value
///
/// # Parameters
/// * `value` - The value to calculate the square root of
///
/// # Returns
/// * `u128` - The square root of the input value
#[allow(dead_code)]
fn sqrt_u128(value: u128) -> u128 {
    if value == 0 {
        return 0;
    }

    // Initial estimate
    let mut x = value;
    let mut y = (x + 1) >> 1; // (x + 1) / 2

    // Newton's method for square root approximation
    while y < x {
        x = y;
        y = (x + value / x) >> 1;
    }

    x
}
