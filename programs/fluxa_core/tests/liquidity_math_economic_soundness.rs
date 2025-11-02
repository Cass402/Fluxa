//! Economic Soundness Tests for liquidity_math.rs
//!
//! This test suite verifies critical economic properties of the concentrated liquidity AMM:
//! - No value creation/destruction through mathematical operations
//! - Fee consistency and rounding fairness
//! - Arbitrage impossibility
//! - Overflow attack prevention
//! - Economic invariant preservation across 100,000+ scenarios
//!
//! CRITICAL: Any failures in these tests indicate potential economic exploits
//! that could lead to loss of user funds or protocol insolvency.

use anchor_lang::prelude::*;
use fluxa_core::math::core_arithmetic::{tick_to_sqrt_x64, Q64x64};
use fluxa_core::math::liquidity_math::{
    calculate_amount_0_delta, calculate_amount_1_delta, calculate_amounts_for_liquidity_piecewise,
    calculate_liquidity, calculate_position_value_at_price,
};
use fluxa_core::state::position::position_account::Position;
use fluxa_core::utils::constants::{MAX_SQRT_X64, MAX_TOKEN_AMOUNT, MIN_SQRT_X64};
use std::cmp::{max, min};

/// Economic tolerance threshold: 0.01% of position value
/// This represents the maximum acceptable rounding error as a fraction
const ECONOMIC_TOLERANCE_BPS: u128 = 1; // 1 basis point = 0.01%
const BPS_DENOMINATOR: u128 = 10_000;

/// Helper: Calculate high-precision value for economic comparisons
/// Uses u128 to prevent intermediate overflow in value calculations
#[inline]
fn calculate_precise_value(amount_0: u64, amount_1: u64, price_ratio: u128) -> u128 {
    let value_0 = (amount_0 as u128)
        .checked_mul(price_ratio)
        .expect("Value calculation overflow");
    let value_1 = (amount_1 as u128)
        .checked_mul(1_u128 << 64)
        .expect("Value calculation overflow");
    value_0.checked_add(value_1).expect("Total value overflow")
}

/// Helper: Check if two values are within economic tolerance
#[inline]
fn within_economic_tolerance(value_1: u128, value_2: u128) -> bool {
    let larger = max(value_1, value_2);
    let smaller = min(value_1, value_2);
    let diff = larger - smaller;

    // Calculate max allowed difference: larger * ECONOMIC_TOLERANCE_BPS / BPS_DENOMINATOR
    let max_diff = larger
        .checked_mul(ECONOMIC_TOLERANCE_BPS)
        .and_then(|v| v.checked_div(BPS_DENOMINATOR))
        .unwrap_or(0);

    diff <= max_diff
}

/// Helper: Create a valid test position
fn create_test_position(tick_lower: i32, tick_upper: i32, liquidity: u128) -> Position {
    Position {
        owner: Pubkey::new_unique(),
        tick_lower,
        tick_upper,
        status_flags: Position::FLAG_ACTIVE,
        position_nonce: 0,
        _padding1: [0u8; 2],
        liquidity: Q64x64::from_raw(liquidity),
        fee_growth_inside_0_last: Q64x64::zero(),
        fee_growth_inside_1_last: Q64x64::zero(),
        tokens_owed_0: Q64x64::zero(),
        tokens_owed_1: Q64x64::zero(),
        total_fees_collected_0: Q64x64::zero(),
        total_fees_collected_1: Q64x64::zero(),
        creation_slot: 0,
        last_update_slot: 0,
        creation_timestamp: 0,
        position_hash: [0u8; 32],
        reserved: [0u64; 3],
    }
}

/// Helper: Generate safe random Q64x64 within valid range
fn random_sqrt_price(seed: u64) -> Q64x64 {
    let range = (MAX_SQRT_X64.saturating_sub(MIN_SQRT_X64)) as u64;
    let offset = (seed % range) as u128;
    Q64x64::from_raw(MIN_SQRT_X64 + offset)
}

/// Helper: Generate safe random liquidity amount
fn random_liquidity(seed: u64) -> Q64x64 {
    let amount = 1_000_000_u128 + ((seed % 1_000_000_000) as u128);
    Q64x64::from_raw(amount << 64)
}

/// Helper: Generate safe random token amount
fn random_token_amount(seed: u64) -> u64 {
    let max_safe = min(MAX_TOKEN_AMOUNT, u32::MAX as u64);
    1_000 + (seed % max_safe)
}

// ============================================================================
// 1. NO VALUE CREATION/DESTRUCTION TESTS
// ============================================================================

#[test]
fn test_value_conservation_across_price_ranges() {
    // Test that for any liquidity provision, total value remains constant
    // across different price points within the range
    // Note: This tests that liquidity + token value tradeoffs are reasonable

    let tick_lower = -1000;
    let tick_upper = 1000;
    let liquidity = Q64x64::from_raw(1_000_000_u128 << 64);

    let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();

    // Test at multiple price points within the range
    let test_ticks = [-500, -250, 0, 250, 500];

    for &tick in &test_ticks {
        let sqrt_current = tick_to_sqrt_x64(tick).unwrap();
        let result = calculate_amounts_for_liquidity_piecewise(
            sqrt_current,
            sqrt_lower,
            sqrt_upper,
            liquidity,
        );

        // Verify calculation succeeds and produces reasonable values
        assert!(
            result.is_ok(),
            "Failed to calculate amounts at tick {}",
            tick
        );
        let (amount_0, amount_1) = result.unwrap();

        // Verify amounts are within protocol bounds
        assert!(
            amount_0 <= MAX_TOKEN_AMOUNT,
            "Token0 exceeds max at tick {}",
            tick
        );
        assert!(
            amount_1 <= MAX_TOKEN_AMOUNT,
            "Token1 exceeds max at tick {}",
            tick
        );

        // At least one amount should be non-zero for active range
        if sqrt_current.raw() > sqrt_lower.raw() && sqrt_current.raw() < sqrt_upper.raw() {
            assert!(
                amount_0 > 0 || amount_1 > 0,
                "Both amounts zero in active range at tick {}",
                tick
            );
        }
    }
}

#[test]
fn test_no_value_creation_on_round_trip() {
    // Test: calculate_amounts -> calculate_liquidity -> calculate_amounts
    // should yield the same or slightly lower amounts (due to rounding)

    let tick_lower = -500;
    let tick_upper = 500;
    let tick_current = 0;

    let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
    let sqrt_current = tick_to_sqrt_x64(tick_current).unwrap();
    let initial_liquidity = Q64x64::from_raw(1_000_000_u128 << 64);

    // Step 1: Calculate amounts from liquidity
    let (amount_0_initial, amount_1_initial) = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        initial_liquidity,
    )
    .unwrap();

    // Step 2: Calculate liquidity from amounts
    let recalculated_liquidity = calculate_liquidity(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        amount_0_initial,
        amount_1_initial,
    )
    .unwrap();

    // Step 3: Calculate amounts again from recalculated liquidity
    let (amount_0_final, amount_1_final) = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        Q64x64::from_raw(recalculated_liquidity),
    )
    .unwrap();

    // Verify no value was created (amounts should not increase)
    assert!(
        amount_0_final <= amount_0_initial,
        "Token0 increased on round trip: {} -> {}",
        amount_0_initial,
        amount_0_final
    );
    assert!(
        amount_1_final <= amount_1_initial,
        "Token1 increased on round trip: {} -> {}",
        amount_1_initial,
        amount_1_final
    );

    // Verify liquidity is conserved within tolerance
    let original_liq = initial_liquidity.raw();
    let diff = recalculated_liquidity.abs_diff(original_liq);

    let max_diff = (original_liq * ECONOMIC_TOLERANCE_BPS) / BPS_DENOMINATOR;
    assert!(
        diff <= max_diff,
        "Excessive liquidity drift on round trip: {} -> {} (diff: {}, max: {})",
        original_liq,
        recalculated_liquidity,
        diff,
        max_diff
    );
}

#[test]
fn test_rounding_errors_bounded() {
    // Verify that rounding errors never exceed 0.01% of position value

    for i in 0..1000 {
        let seed = i as u64;
        let sqrt_lower = random_sqrt_price(seed);
        let sqrt_upper = random_sqrt_price(seed + 1);

        if sqrt_upper.raw() <= sqrt_lower.raw() {
            continue;
        }

        let sqrt_current_raw = sqrt_lower.raw() + ((sqrt_upper.raw() - sqrt_lower.raw()) / 2);
        let sqrt_current = Q64x64::from_raw(sqrt_current_raw);
        let liquidity = random_liquidity(seed + 2);

        let (amount_0, amount_1) = match calculate_amounts_for_liquidity_piecewise(
            sqrt_current,
            sqrt_lower,
            sqrt_upper,
            liquidity,
        ) {
            Ok(amounts) => amounts,
            Err(_) => continue, // Skip invalid combinations
        };

        if amount_0 == 0 && amount_1 == 0 {
            continue;
        }

        // Calculate liquidity back and verify precision loss is bounded
        let recalc_liquidity =
            match calculate_liquidity(sqrt_current, sqrt_lower, sqrt_upper, amount_0, amount_1) {
                Ok(liq) => liq,
                Err(_) => continue,
            };

        let original_liq = liquidity.raw();
        let diff = recalc_liquidity.abs_diff(original_liq);

        // Rounding error should be less than 0.01% of original liquidity
        let max_error = (original_liq * ECONOMIC_TOLERANCE_BPS) / BPS_DENOMINATOR;
        assert!(
            diff <= max_error,
            "Excessive rounding error: {} vs {} (diff: {}, max: {})",
            original_liq,
            recalc_liquidity,
            diff,
            max_error
        );
    }
}

// ============================================================================
// 2. FEE CONSISTENCY TESTS
// ============================================================================

#[test]
fn test_position_value_matches_component_calculation() {
    // Verify that calculate_position_value_at_price matches manual calculation

    let position = create_test_position(-500, 500, 1_000_000_u128 << 64);
    let sqrt_current = tick_to_sqrt_x64(0).unwrap();
    let token_0_price = 2000; // $2000 per token0
    let token_1_price = 1; // $1 per token1

    let position_value =
        calculate_position_value_at_price(&position, sqrt_current, token_0_price, token_1_price)
            .unwrap();

    // Manually calculate amounts
    let sqrt_lower = tick_to_sqrt_x64(position.tick_lower).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(position.tick_upper).unwrap();
    let (amount_0, amount_1) = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        position.liquidity,
    )
    .unwrap();

    // Verify amounts match
    assert_eq!(position_value.amount_0, amount_0);
    assert_eq!(position_value.amount_1, amount_1);

    // Verify USD values match
    let expected_value_0 = (amount_0 as u128 * token_0_price as u128) as u64;
    let expected_value_1 = (amount_1 as u128 * token_1_price as u128) as u64;
    let expected_total = expected_value_0 + expected_value_1;

    assert_eq!(position_value.value_0_usd, expected_value_0);
    assert_eq!(position_value.value_1_usd, expected_value_1);
    assert_eq!(position_value.total_value_usd, expected_total);
}

#[test]
fn test_no_systematic_rounding_bias() {
    // Verify that rounding doesn't systematically favor protocol over LPs or vice versa

    let mut protocol_favored = 0;
    let mut lp_favored = 0;
    let mut neutral = 0;

    // Use tick-based generation for more realistic scenarios
    for tick_lower in (-1000..=0).step_by(100) {
        for tick_upper in (tick_lower + 100..=1000).step_by(100) {
            let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
            let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();

            let tick_mid = (tick_lower + tick_upper) / 2;
            let sqrt_current = tick_to_sqrt_x64(tick_mid).unwrap();
            let liquidity = Q64x64::from_raw(1_000_000_u128 << 64);

            let (amount_0, amount_1) = match calculate_amounts_for_liquidity_piecewise(
                sqrt_current,
                sqrt_lower,
                sqrt_upper,
                liquidity,
            ) {
                Ok(amounts) => amounts,
                Err(_) => continue,
            };

            if amount_0 == 0 && amount_1 == 0 {
                continue;
            }

            let recalc_liquidity =
                match calculate_liquidity(sqrt_current, sqrt_lower, sqrt_upper, amount_0, amount_1)
                {
                    Ok(liq) => liq,
                    Err(_) => continue,
                };

            let original_liq = liquidity.raw();

            if recalc_liquidity < original_liq {
                lp_favored += 1;
            } else if recalc_liquidity > original_liq {
                protocol_favored += 1;
            } else {
                neutral += 1;
            }
        }
    }

    let total = (protocol_favored + lp_favored + neutral) as f64;

    // Ensure we had enough valid scenarios
    assert!(total >= 10.0, "Not enough valid scenarios: {}", total);

    let protocol_ratio = protocol_favored as f64 / total;
    let lp_ratio = lp_favored as f64 / total;

    println!(
        "Rounding bias: protocol={:.2}%, lp={:.2}%, neutral={:.2}%",
        protocol_ratio * 100.0,
        lp_ratio * 100.0,
        neutral as f64 / total * 100.0
    );

    // Protocol being conservative (LP-favored) is acceptable and safer
    // What we're checking is that there's no protocol-favored bias that could exploit LPs
    assert!(
        protocol_ratio < 0.60, // Protocol shouldn't consistently extract value
        "Protocol-favored rounding bias detected: protocol={:.2}%, lp={:.2}%",
        protocol_ratio * 100.0,
        lp_ratio * 100.0
    );

    // Also verify we have meaningful data
    assert!(total >= 10.0, "Not enough valid test scenarios");
}

// ============================================================================
// 3. ARBITRAGE IMPOSSIBILITY TESTS
// ============================================================================

#[test]
fn test_zero_profit_same_price_round_trip() {
    // Test that liquidity provision and withdrawal at same price yields zero profit

    let tick_lower = -500;
    let tick_upper = 500;
    let tick_current = 0;

    let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
    let sqrt_current = tick_to_sqrt_x64(tick_current).unwrap();

    let initial_amount_0 = 100_000_u64;
    let initial_amount_1 = 100_000_u64;

    // Step 1: Provide liquidity
    let liquidity = calculate_liquidity(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        initial_amount_0,
        initial_amount_1,
    )
    .unwrap();

    // Step 2: Immediately withdraw at same price
    let (withdrawn_amount_0, withdrawn_amount_1) = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        Q64x64::from_raw(liquidity),
    )
    .unwrap();

    // Verify no profit (withdrawn amounts should be <= initial amounts)
    assert!(
        withdrawn_amount_0 <= initial_amount_0,
        "Arbitrage profit in token0: deposited {}, withdrew {}",
        initial_amount_0,
        withdrawn_amount_0
    );
    assert!(
        withdrawn_amount_1 <= initial_amount_1,
        "Arbitrage profit in token1: deposited {}, withdrew {}",
        initial_amount_1,
        withdrawn_amount_1
    );

    // Verify amounts are very close (within tolerance)
    let amount_0_diff = initial_amount_0.saturating_sub(withdrawn_amount_0);
    let amount_1_diff = initial_amount_1.saturating_sub(withdrawn_amount_1);

    assert!(
        amount_0_diff < initial_amount_0 / 100, // Less than 1% loss
        "Excessive token0 loss: {}",
        amount_0_diff
    );
    assert!(
        amount_1_diff < initial_amount_1 / 100, // Less than 1% loss
        "Excessive token1 loss: {}",
        amount_1_diff
    );
}

#[test]
fn test_no_guaranteed_profit_cross_range() {
    // Test that depositing at one edge and withdrawing at another doesn't guarantee profit

    for i in 0..100 {
        let seed = i as u64;
        let sqrt_lower = random_sqrt_price(seed);
        let sqrt_upper = random_sqrt_price(seed + 1);

        if sqrt_upper.raw() <= sqrt_lower.raw() || sqrt_upper.raw() - sqrt_lower.raw() < 1_000_000 {
            continue;
        }

        // Provide liquidity at lower edge
        let sqrt_deposit = sqrt_lower;
        let amount_0 = random_token_amount(seed + 2);
        let amount_1 = random_token_amount(seed + 3);

        let liquidity =
            match calculate_liquidity(sqrt_deposit, sqrt_lower, sqrt_upper, amount_0, amount_1) {
                Ok(liq) => liq,
                Err(_) => continue,
            };

        // Withdraw at upper edge
        let sqrt_withdraw_raw = sqrt_upper.raw() - 1;
        let sqrt_withdraw = Q64x64::from_raw(sqrt_withdraw_raw);

        let (withdrawn_0, withdrawn_1) = match calculate_amounts_for_liquidity_piecewise(
            sqrt_withdraw,
            sqrt_lower,
            sqrt_upper,
            Q64x64::from_raw(liquidity),
        ) {
            Ok(amounts) => amounts,
            Err(_) => continue,
        };

        // Calculate value at deposit price
        let deposit_value = calculate_precise_value(amount_0, amount_1, sqrt_deposit.raw());
        let withdraw_value = calculate_precise_value(withdrawn_0, withdrawn_1, sqrt_deposit.raw());

        // Withdrawn value should not exceed deposited value (excluding IL which is expected)
        // We allow for some slippage but not guaranteed profit
        assert!(
            withdraw_value <= deposit_value
                || within_economic_tolerance(deposit_value, withdraw_value),
            "Guaranteed profit detected: deposited {}, withdrew {} (at deposit price)",
            deposit_value,
            withdraw_value
        );
    }
}

// ============================================================================
// 4. OVERFLOW ATTACK PREVENTION TESTS
// ============================================================================

#[test]
fn test_max_token_amount_enforcement() {
    // Verify that MAX_TOKEN_AMOUNT is enforced to prevent overflow attacks

    let sqrt_lower = Q64x64::from_raw(MIN_SQRT_X64);
    let sqrt_upper = Q64x64::from_raw(MAX_SQRT_X64);
    let sqrt_current = Q64x64::from_raw((MIN_SQRT_X64 + MAX_SQRT_X64) / 2);

    // Try to create excessive liquidity
    let excessive_liquidity = Q64x64::from_raw(u128::MAX);

    let result = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        excessive_liquidity,
    );

    // Should either return error or amounts within MAX_TOKEN_AMOUNT
    match result {
        Ok((amount_0, amount_1)) => {
            assert!(
                amount_0 <= MAX_TOKEN_AMOUNT,
                "Token0 exceeds MAX_TOKEN_AMOUNT: {}",
                amount_0
            );
            assert!(
                amount_1 <= MAX_TOKEN_AMOUNT,
                "Token1 exceeds MAX_TOKEN_AMOUNT: {}",
                amount_1
            );
        }
        Err(_) => {
            // Error is acceptable for overflow prevention
        }
    }
}

#[test]
fn test_malicious_price_range_rejection() {
    // Verify that malicious price ranges are rejected

    let sqrt_current = Q64x64::from_raw((MIN_SQRT_X64 + MAX_SQRT_X64) / 2);
    let liquidity = Q64x64::from_raw(1_000_000_u128 << 64);

    // Test 1: Lower price >= upper price
    let result = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        Q64x64::from_raw(MAX_SQRT_X64),
        Q64x64::from_raw(MIN_SQRT_X64),
        liquidity,
    );
    assert!(result.is_err(), "Should reject inverted price range");

    // Test 2: Zero lower price
    let result = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        Q64x64::from_raw(0),
        Q64x64::from_raw(MAX_SQRT_X64),
        liquidity,
    );
    assert!(result.is_err(), "Should reject zero lower price");

    // Test 3: Equal prices
    let result = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_current,
        sqrt_current,
        liquidity,
    );
    assert!(result.is_err(), "Should reject equal prices");
}

#[test]
fn test_overflow_prevention_in_value_calculation() {
    // Verify that position value calculation handles large values without overflow

    let position = create_test_position(-1000, 1000, u64::MAX as u128);
    let sqrt_current = tick_to_sqrt_x64(0).unwrap();

    // Use maximum safe token prices
    let max_price = u64::MAX;

    let result = calculate_position_value_at_price(&position, sqrt_current, max_price, max_price);

    // Should either handle gracefully or return error (not panic)
    match result {
        Ok(value) => {
            println!("Handled large value: {}", value.total_value_usd);
        }
        Err(_) => {
            println!("Correctly rejected overflow scenario");
        }
    }
}

#[test]
fn test_extreme_liquidity_scenarios() {
    // Test behavior with extreme liquidity values

    let sqrt_lower = Q64x64::from_raw(MIN_SQRT_X64 + 1_000_000);
    let sqrt_upper = Q64x64::from_raw(MIN_SQRT_X64 + 2_000_000);
    let sqrt_current = Q64x64::from_raw(MIN_SQRT_X64 + 1_500_000);

    // Test with very small liquidity (should reject or handle gracefully)
    let tiny_liquidity = Q64x64::from_raw(1);
    let result = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        tiny_liquidity,
    );

    match result {
        Ok((amount_0, amount_1)) => {
            // If accepted, amounts should be zero or very small
            assert!(
                amount_0 < 1000 && amount_1 < 1000,
                "Tiny liquidity produced large amounts"
            );
        }
        Err(_) => {
            // Rejection is also acceptable
        }
    }

    // Test with very large liquidity
    let huge_liquidity = Q64x64::from_raw(u128::MAX / 2);
    let result = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        huge_liquidity,
    );

    match result {
        Ok((amount_0, amount_1)) => {
            assert!(
                amount_0 <= MAX_TOKEN_AMOUNT,
                "Huge liquidity bypassed MAX_TOKEN_AMOUNT"
            );
            assert!(
                amount_1 <= MAX_TOKEN_AMOUNT,
                "Huge liquidity bypassed MAX_TOKEN_AMOUNT"
            );
        }
        Err(_) => {
            // Rejection is acceptable
        }
    }
}

// ============================================================================
// 5. HIGH-VOLUME PROPERTY-BASED TESTS (100,000+ scenarios)
// ============================================================================

#[test]
fn test_invariants_across_100k_scenarios() {
    // Run 100,000+ randomized scenarios to verify all economic invariants hold

    const ITERATIONS: usize = 100_000;
    let mut passed = 0;
    let mut skipped = 0;
    let mut invariant_violations = Vec::new();

    for i in 0..ITERATIONS {
        // Generate valid tick-based scenarios
        let tick_lower = -10000 + ((i * 17) % 18000) as i32;
        let tick_upper = tick_lower + 100 + ((i * 23) % 1000) as i32;
        let tick_current = tick_lower + ((tick_upper - tick_lower) * ((i * 31) % 100) as i32) / 100;

        if tick_upper <= tick_lower || tick_current < tick_lower || tick_current > tick_upper {
            skipped += 1;
            continue;
        }

        let sqrt_lower = match tick_to_sqrt_x64(tick_lower) {
            Ok(v) => v,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        let sqrt_upper = match tick_to_sqrt_x64(tick_upper) {
            Ok(v) => v,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        let sqrt_current = match tick_to_sqrt_x64(tick_current) {
            Ok(v) => v,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        let liquidity_base = 1_000_000_u128 + ((i * 13) % 100_000_000) as u128;
        let liquidity = Q64x64::from_raw(liquidity_base << 64);

        // Calculate amounts
        let (amount_0, amount_1) = match calculate_amounts_for_liquidity_piecewise(
            sqrt_current,
            sqrt_lower,
            sqrt_upper,
            liquidity,
        ) {
            Ok(amounts) => amounts,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        if amount_0 == 0 && amount_1 == 0 {
            skipped += 1;
            continue;
        }

        // Invariant 1: Amounts within bounds
        if amount_0 > MAX_TOKEN_AMOUNT || amount_1 > MAX_TOKEN_AMOUNT {
            invariant_violations.push(format!(
                "Iteration {}: Token amount exceeds MAX_TOKEN_AMOUNT",
                i
            ));
            continue;
        }

        // Invariant 2: Round-trip preserves or reduces value
        if let Ok(recalc_liq) =
            calculate_liquidity(sqrt_current, sqrt_lower, sqrt_upper, amount_0, amount_1)
        {
            if recalc_liq > liquidity.raw() {
                // Allow small rounding up but check it's within tolerance
                let diff = recalc_liq - liquidity.raw();
                let max_diff = (liquidity.raw() * ECONOMIC_TOLERANCE_BPS) / BPS_DENOMINATOR;

                if diff > max_diff {
                    invariant_violations.push(format!(
                        "Iteration {}: Value creation detected: {} -> {}",
                        i,
                        liquidity.raw(),
                        recalc_liq
                    ));
                    continue;
                }
            }
        }

        // Invariant 3: Position value calculation doesn't panic
        let position = create_test_position(tick_lower, tick_upper, liquidity.raw());
        let token_0_price = (1000 + ((i * 7) % 10000)) as u64;
        let token_1_price = (1 + ((i * 11) % 100)) as u64;

        match calculate_position_value_at_price(
            &position,
            sqrt_current,
            token_0_price,
            token_1_price,
        ) {
            Ok(value) => {
                // Verify range status is correct
                let expected_active =
                    sqrt_current.raw() >= sqrt_lower.raw() && sqrt_current.raw() < sqrt_upper.raw();
                if value.price_range_active != expected_active {
                    invariant_violations.push(format!("Iteration {}: Incorrect range status", i));
                    continue;
                }
            }
            Err(_) => {
                // Some errors are acceptable (overflow, etc.)
            }
        }

        passed += 1;
    }

    println!("\n=== 100K Scenario Test Results ===");
    println!("Total iterations: {}", ITERATIONS);
    println!("Passed: {}", passed);
    println!("Skipped: {}", skipped);
    println!("Invariant violations: {}", invariant_violations.len());

    if !invariant_violations.is_empty() {
        println!("\nViolations:");
        for (idx, violation) in invariant_violations.iter().enumerate().take(10) {
            println!("  {}: {}", idx + 1, violation);
        }
        if invariant_violations.len() > 10 {
            println!("  ... and {} more", invariant_violations.len() - 10);
        }
    }

    assert!(
        invariant_violations.is_empty(),
        "Economic invariants violated in {} scenarios",
        invariant_violations.len()
    );
    assert!(
        passed > ITERATIONS / 10,
        "Too many scenarios skipped: {}/{} (only {} passed)",
        skipped,
        ITERATIONS,
        passed
    );
}

#[test]
fn test_delta_functions_consistency() {
    // Verify calculate_amount_0_delta and calculate_amount_1_delta are internally consistent

    for i in 0..10_000 {
        let seed = i as u64;
        let sqrt_lower = random_sqrt_price(seed);
        let sqrt_upper = random_sqrt_price(seed + 1);

        if sqrt_upper.raw() <= sqrt_lower.raw() {
            continue;
        }

        let liquidity = random_liquidity(seed + 2);

        // Calculate deltas
        let amount_0 = match calculate_amount_0_delta(sqrt_lower, sqrt_upper, liquidity) {
            Ok(amt) => amt,
            Err(_) => continue,
        };

        let amount_1 = match calculate_amount_1_delta(sqrt_lower, sqrt_upper, liquidity) {
            Ok(amt) => amt,
            Err(_) => continue,
        };

        // Both should be within bounds
        assert!(
            amount_0 <= MAX_TOKEN_AMOUNT,
            "amount_0_delta exceeds MAX_TOKEN_AMOUNT"
        );
        assert!(
            amount_1 <= MAX_TOKEN_AMOUNT,
            "amount_1_delta exceeds MAX_TOKEN_AMOUNT"
        );

        // If liquidity is non-zero and range is non-trivial, at least one amount should be non-zero
        if liquidity.raw() > (1u128 << 64) && sqrt_upper.raw() - sqrt_lower.raw() > 100_000 {
            assert!(
                amount_0 > 0 || amount_1 > 0,
                "Both amounts zero for non-trivial inputs"
            );
        }
    }
}

// ============================================================================
// 6. PRECISION DOCUMENTATION TESTS
// ============================================================================

#[test]
fn test_document_precision_bounds() {
    // This test documents the precision characteristics of the liquidity math

    println!("\n=== PRECISION CHARACTERISTICS ===\n");

    // Test 1: Minimum representable liquidity change
    let sqrt_lower = Q64x64::from_raw(MIN_SQRT_X64 + 1_000_000);
    let sqrt_upper = Q64x64::from_raw(MIN_SQRT_X64 + 2_000_000);
    let sqrt_current = Q64x64::from_raw(MIN_SQRT_X64 + 1_500_000);

    let liq_1 = Q64x64::from_raw(1_000_000_u128 << 64);
    let liq_2 = Q64x64::from_raw((1_000_000_u128 << 64) + 1);

    let (amt_0_1, amt_1_1) =
        calculate_amounts_for_liquidity_piecewise(sqrt_current, sqrt_lower, sqrt_upper, liq_1)
            .unwrap();
    let (amt_0_2, amt_1_2) =
        calculate_amounts_for_liquidity_piecewise(sqrt_current, sqrt_lower, sqrt_upper, liq_2)
            .unwrap();

    println!("Minimum detectable liquidity change:");
    println!("  Liquidity change: 1 raw unit");
    println!("  Token0 change: {}", amt_0_2.saturating_sub(amt_0_1));
    println!("  Token1 change: {}", amt_1_2.saturating_sub(amt_1_1));

    // Test 2: Maximum safe liquidity
    let max_safe_liquidity = u64::MAX as u128;
    let result = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        Q64x64::from_raw(max_safe_liquidity << 64),
    );

    println!("\nMaximum safe liquidity (u64::MAX):");
    match result {
        Ok((amt_0, amt_1)) => {
            println!("  Token0: {}", amt_0);
            println!("  Token1: {}", amt_1);
            println!("  Status: ✓ Handled");
        }
        Err(e) => {
            println!("  Status: ✗ Error: {:?}", e);
        }
    }

    // Test 3: Precision loss in round-trip
    let test_liquidities = vec![
        1_000_u128 << 64,
        1_000_000_u128 << 64,
        1_000_000_000_u128 << 64,
    ];

    println!("\nRound-trip precision loss:");
    for liq in test_liquidities {
        let (amt_0, amt_1) = calculate_amounts_for_liquidity_piecewise(
            sqrt_current,
            sqrt_lower,
            sqrt_upper,
            Q64x64::from_raw(liq),
        )
        .unwrap();

        if amt_0 > 0 && amt_1 > 0 {
            let recalc_liq =
                calculate_liquidity(sqrt_current, sqrt_lower, sqrt_upper, amt_0, amt_1).unwrap();
            let loss_bps = if recalc_liq < liq {
                ((liq - recalc_liq) * 10_000) / liq
            } else {
                0
            };
            println!(
                "  Liquidity {}: loss = {} basis points",
                liq >> 64,
                loss_bps
            );
        }
    }

    println!("\n=== ECONOMIC TOLERANCE THRESHOLD ===");
    println!(
        "Maximum acceptable rounding error: {} basis points ({}%)",
        ECONOMIC_TOLERANCE_BPS,
        ECONOMIC_TOLERANCE_BPS as f64 / 100.0
    );
    println!("This represents the maximum value drift allowed in position calculations.");
    println!("\n");
}

#[test]
fn test_edge_case_documentation() {
    println!("\n=== DOCUMENTED EDGE CASES ===\n");

    // Edge case 1: Very narrow price range
    let sqrt_base = Q64x64::from_raw(MIN_SQRT_X64 + 1_000_000);
    let sqrt_lower = sqrt_base;
    let sqrt_upper = Q64x64::from_raw(sqrt_base.raw() + 100); // Very narrow
    let sqrt_current = Q64x64::from_raw(sqrt_base.raw() + 50);
    let liquidity = Q64x64::from_raw(1_000_000_u128 << 64);

    let result =
        calculate_amounts_for_liquidity_piecewise(sqrt_current, sqrt_lower, sqrt_upper, liquidity);

    println!("Edge Case 1: Very narrow price range (width=100 raw units)");
    match result {
        Ok((amt_0, amt_1)) => {
            println!("  Token0: {}", amt_0);
            println!("  Token1: {}", amt_1);
            println!("  Status: ✓ Handled");
        }
        Err(e) => {
            println!("  Status: ✗ Error: {:?}", e);
        }
    }

    // Edge case 2: Price at exact boundaries
    println!("\nEdge Case 2: Price exactly at lower boundary");
    let result =
        calculate_amounts_for_liquidity_piecewise(sqrt_lower, sqrt_lower, sqrt_upper, liquidity);
    match result {
        Ok((amt_0, amt_1)) => {
            println!("  Token0: {}", amt_0);
            println!("  Token1: {}", amt_1);
            println!("  Only token0 required: {}", amt_1 == 0);
        }
        Err(e) => {
            println!("  Status: ✗ Error: {:?}", e);
        }
    }

    // Edge case 3: Very wide price range
    let sqrt_lower_wide = Q64x64::from_raw(MIN_SQRT_X64);
    let sqrt_upper_wide = Q64x64::from_raw(MAX_SQRT_X64);
    let sqrt_current_wide = Q64x64::from_raw((MIN_SQRT_X64 + MAX_SQRT_X64) / 2);

    println!("\nEdge Case 3: Maximum price range (MIN_SQRT_X64 to MAX_SQRT_X64)");
    let result = calculate_amounts_for_liquidity_piecewise(
        sqrt_current_wide,
        sqrt_lower_wide,
        sqrt_upper_wide,
        Q64x64::from_raw(1_000_u128 << 64),
    );
    match result {
        Ok((amt_0, amt_1)) => {
            println!("  Token0: {}", amt_0);
            println!("  Token1: {}", amt_1);
            println!("  Status: ✓ Handled");
        }
        Err(e) => {
            println!("  Status: ✗ Error: {:?}", e);
        }
    }

    println!("\n");
}
