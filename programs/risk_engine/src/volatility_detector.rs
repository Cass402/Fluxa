//! Calculates volatility using a rolling window standard deviation.
//! This implementation uses fixed-point arithmetic with u128/i128 for on-chain compatibility.
//!
//! Assumptions:
//! 1. Input `price_history` contains u128 values representing scaled prices
//!    (e.g., actual_price * 10^N, where N is the number of decimal places of precision).
//!    The specific price scaling factor does not affect the relative returns calculation.
//! 2. Log returns (ln(P₂/P₁)) are approximated by simple percentage returns ((P₂ - P₁)/P₁).
//!    This approximation (ln(1+x) ≈ x) is more accurate for smaller relative price changes.
//! 3. The output standard deviation is also a scaled integer. Using `RETURN_SCALING_FACTOR`,
//!    a returned value of `X` represents an actual standard deviation of `X / RETURN_SCALING_FACTOR`.
//!    For example, if `RETURN_SCALING_FACTOR` is 10^9, a result of 50,000,000 means 0.05 or 5%.
use anchor_lang::prelude::*;
/// Scaling factor for representing returns and standard deviation.
/// For example, 10^9 means 9 decimal places of precision for the percentage return.
pub(crate) const RETURN_SCALING_FACTOR: u128 = 1_000_000_000; // 10^9
const RETURN_SCALING_FACTOR_I128: i128 = 1_000_000_000; // 10^9 as i128

/// Calculates the integer square root of a u128 number using the Babylonian method.
/// Returns floor(sqrt(n)).
pub(crate) fn isqrt_u128(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x = n; // Initial guess
                   // Iteratively improve the guess.
                   // The loop condition `y < x` ensures termination.
                   // `n / x` performs integer division.
    let mut y = (x + n / x) / 2; // First iteration outside loop to handle x=1, n=0 edge case if not for n==0 check
    if y >= x {
        // if x is already sqrt or n=0,1
        if x * x > n && x > 0 {
            // handle case where initial x is too high, e.g. n=2, x=2, y=1. x becomes 1.
            return x - 1;
        }
        return x;
    }

    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

pub fn calculate_rolling_std_dev_volatility(
    price_history: &[u128],
    window_size: usize,
) -> Result<u128> {
    if price_history.len() < window_size || window_size == 0 {
        return Ok(0); // Not enough data or invalid window size
    }

    let relevant_prices = &price_history[price_history.len() - window_size..];

    // Standard deviation requires at least 2 data points to calculate returns,
    // and at least 2 returns for sample variance.
    // If relevant_prices has < 2 points, no returns can be calculated.
    if relevant_prices.len() < 2 {
        return Ok(0);
    }

    let mut returns_scaled: Vec<i128> = Vec::new();
    for i in 1..relevant_prices.len() {
        let p1 = relevant_prices[i - 1];
        let p2 = relevant_prices[i];

        if p1 == 0 {
            // Cannot calculate return if previous price is zero. Skip this data point.
            // Depending on requirements, could also return an error or a specific value.
            continue;
        }

        // Calculate simple percentage return: (p2 - p1) / p1
        // All prices are u128, diff can be negative.
        let diff: i128 = (p2 as i128) - (p1 as i128);

        // Scale the return: (diff * SCALING_FACTOR) / p1
        // (diff / p1) is the unscaled return. Multiplying by SCALING_FACTOR gives scaled return.
        // Order of operations: multiply first to maintain precision before division.
        let return_scaled: i128 = (diff * RETURN_SCALING_FACTOR_I128) / (p1 as i128);
        returns_scaled.push(return_scaled);
    }

    // Sample standard deviation requires at least 2 returns.
    if returns_scaled.len() < 2 {
        return Ok(0);
    }

    let num_returns = returns_scaled.len() as i128;
    let sum_returns: i128 = returns_scaled.iter().sum();
    let mean_return_scaled: i128 = sum_returns / num_returns; // Preserves scale

    // Sum of squared deviations from the mean.
    // (return - mean_return)^2. This will have scale RETURN_SCALING_FACTOR^2.
    let sum_squared_deviations: i128 = returns_scaled
        .iter()
        .map(|r_scaled| {
            let deviation = r_scaled - mean_return_scaled;
            deviation.pow(2) // or deviation * deviation
        })
        .sum();

    // Sample variance: sum_squared_deviations / (n - 1)
    // This variance_scaled_twice has a scale of RETURN_SCALING_FACTOR^2.
    // sum_squared_deviations is non-negative.
    let variance_scaled_twice: u128 = (sum_squared_deviations / (num_returns - 1)) as u128;

    // Standard deviation is sqrt(variance).
    // isqrt_u128(value_scaled_by_S^2) returns sqrt(value) * S.
    // So, the result std_dev_scaled has a scale of RETURN_SCALING_FACTOR.
    let std_dev_scaled = isqrt_u128(variance_scaled_twice);

    Ok(std_dev_scaled)
}
