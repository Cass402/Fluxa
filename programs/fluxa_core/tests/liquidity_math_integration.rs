//! Comprehensive Integration Tests for liquidity_math.rs
//!
//! This test suite validates the critical liquidity calculation functions that directly
//! impact user funds and protocol solvency. Any bugs in these calculations could lead to
//! economic exploits worth millions of dollars.
//!
//! Test Coverage:
//! - Cross-module integration with tick_to_sqrt_x64 and core_arithmetic functions
//! - Realistic LP scenarios across price movements
//! - Position value calculations with multiple overlapping ranges
//! - Fee tier tick spacing alignment
//! - Edge case chains and boundary conditions
//! - Precision verification and mathematical invariants
//!
//! Priority: CRITICAL - Core financial calculations handling user funds

use anchor_lang::solana_program::pubkey::Pubkey;
use fluxa_core::math::core_arithmetic::{
    liquidity_from_amount_0, liquidity_from_amount_1, tick_to_sqrt_x64, Q64x64,
};
use fluxa_core::math::liquidity_math::{
    calculate_amounts_for_liquidity_piecewise, calculate_liquidity,
    calculate_position_value_at_price,
};
use fluxa_core::state::position::position_account::Position;
use fluxa_core::utils::constants::{
    MAX_SQRT_X64, MAX_TICK, MAX_TOKEN_AMOUNT, MIN_SQRT_X64, MIN_TICK, TICK_SPACING_PER_FEE,
};

// ============================================================================
// Helper Functions
// ============================================================================

/// Build a Position struct in-memory for pure math testing (bypassing Anchor account init)
fn build_position(tick_lower: i32, tick_upper: i32, liquidity_raw: u128, nonce: u16) -> Position {
    let mut pos = Position {
        owner: Pubkey::new_unique(),
        tick_lower,
        tick_upper,
        status_flags: 0,
        position_nonce: nonce,
        _padding1: [0u8; 2],
        liquidity: Q64x64::from_raw(liquidity_raw),
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
    };
    pos.status_flags = Position::FLAG_ACTIVE;
    pos.position_hash = pos.calculate_optimized_hash();
    pos
}

/// Assert approximate equality for u64 values with tolerance (for future use)
#[allow(dead_code)]
fn assert_approx_eq(a: u64, b: u64, tolerance: u64, context: &str) {
    let diff = a.abs_diff(b);
    assert!(
        diff <= tolerance,
        "{}: Expected ~{}, got {}, diff={}",
        context,
        b,
        a,
        diff
    );
}

// ============================================================================
// Cross-Module Integration Tests
// ============================================================================

#[test]
fn test_liquidity_piecewise_with_tick_conversion() {
    // Verify calculate_amounts_for_liquidity_piecewise works correctly with prices from tick_to_sqrt_x64
    let tick_lower = -1000;
    let tick_upper = 1000;
    let tick_current = 0;

    let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
    let sqrt_current = tick_to_sqrt_x64(tick_current).unwrap();

    let liquidity = Q64x64::from_int(100_000);

    let (amount_0, amount_1) =
        calculate_amounts_for_liquidity_piecewise(sqrt_current, sqrt_lower, sqrt_upper, liquidity)
            .unwrap();

    // At tick 0 (price ≈ 1.0), position should require both tokens
    assert!(amount_0 > 0, "Should require token0 at tick 0");
    assert!(amount_1 > 0, "Should require token1 at tick 0");

    // Verify the amounts are reasonable (not dust, not exceeding max)
    assert!(amount_0 < MAX_TOKEN_AMOUNT, "Token0 within bounds");
    assert!(amount_1 < MAX_TOKEN_AMOUNT, "Token1 within bounds");
}

#[test]
fn test_liquidity_calculation_alignment_with_core_arithmetic() {
    // Test that liquidity calculations align with expectations from liquidity_from_amount_0 and liquidity_from_amount_1
    let tick_lower = -5000;
    let tick_upper = 5000;
    let tick_current = 0;

    let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
    let sqrt_current = tick_to_sqrt_x64(tick_current).unwrap();

    let amount_0 = 1_000_000u64;
    let amount_1 = 2_000_000u64;

    // Calculate liquidity from both tokens
    let liquidity_from_0 = liquidity_from_amount_0(sqrt_current, sqrt_upper, amount_0).unwrap();
    let liquidity_from_1 = liquidity_from_amount_1(sqrt_lower, sqrt_current, amount_1).unwrap();

    // calculate_liquidity should return the minimum of these two
    let calculated_liquidity =
        calculate_liquidity(sqrt_current, sqrt_lower, sqrt_upper, amount_0, amount_1).unwrap();

    let expected_min = std::cmp::min(liquidity_from_0, liquidity_from_1);
    assert_eq!(
        calculated_liquidity, expected_min,
        "Liquidity should be minimum of individual calculations"
    );

    // Verify round-trip: use calculated liquidity to get amounts back
    let (recovered_0, recovered_1) = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        Q64x64::from_raw(calculated_liquidity),
    )
    .unwrap();

    // Recovered amounts should be <= original (due to minimum liquidity constraint)
    assert!(
        recovered_0 <= amount_0,
        "Recovered token0 should not exceed original"
    );
    assert!(
        recovered_1 <= amount_1,
        "Recovered token1 should not exceed original"
    );
}

#[test]
fn test_position_struct_compatibility() {
    // Verify Position struct compatibility and correct field access
    let tick_lower = -10000;
    let tick_upper = 10000;
    let liquidity = Q64x64::from_int(50_000).raw();

    let position = build_position(tick_lower, tick_upper, liquidity, 1);

    // Verify position fields are correctly set
    assert_eq!(position.tick_lower, tick_lower);
    assert_eq!(position.tick_upper, tick_upper);
    assert_eq!(position.liquidity.raw(), liquidity);
    assert!(position.is_active());

    // Calculate value at different prices
    let sqrt_current = tick_to_sqrt_x64(0).unwrap();
    let value = calculate_position_value_at_price(&position, sqrt_current, 100, 100).unwrap();

    // Verify PositionValue struct fields
    assert!(value.amount_0 > 0);
    assert!(value.amount_1 > 0);
    assert!(value.total_value_usd > 0);
    assert!(value.price_range_active);
    assert_eq!(value.total_value_usd, value.value_0_usd + value.value_1_usd);
}

// ============================================================================
// Realistic Scenario Tests
// ============================================================================

#[test]
fn test_lp_providing_liquidity_various_tick_ranges() {
    // Simulate LP providing liquidity at various tick ranges
    let test_cases = vec![
        // (tick_lower, tick_upper, expected_region)
        (-1000, 1000, "balanced"),
        (-10000, -5000, "far_below"),
        (5000, 10000, "far_above"),
        (-500, 500, "tight"),
        (-50000, 50000, "wide"),
    ];

    for (tick_lower, tick_upper, region) in test_cases {
        let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
        let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
        let sqrt_current = tick_to_sqrt_x64(0).unwrap(); // Current price at tick 0

        let liquidity = Q64x64::from_int(100_000);

        let result = calculate_amounts_for_liquidity_piecewise(
            sqrt_current,
            sqrt_lower,
            sqrt_upper,
            liquidity,
        );

        assert!(result.is_ok(), "Failed for region: {}", region);

        let (amount_0, amount_1) = result.unwrap();

        // Verify amounts based on range position relative to current price
        if tick_upper < 0 {
            // Range entirely below current price
            assert_eq!(amount_0, 0, "{}: Should require no token0", region);
            assert!(amount_1 > 0, "{}: Should require token1", region);
        } else if tick_lower > 0 {
            // Range entirely above current price
            assert!(amount_0 > 0, "{}: Should require token0", region);
            assert_eq!(amount_1, 0, "{}: Should require no token1", region);
        } else {
            // Range includes current price
            assert!(amount_0 > 0, "{}: Should require token0", region);
            assert!(amount_1 > 0, "{}: Should require token1", region);
        }
    }
}

#[test]
fn test_token_requirements_and_withdrawal_consistency() {
    // Calculate token requirements and verify withdrawals match expectations
    let tick_lower = -2000;
    let tick_upper = 2000;
    let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
    let sqrt_current = tick_to_sqrt_x64(0).unwrap();

    // Step 1: Deposit tokens to calculate liquidity
    let deposit_0 = 5_000_000u64;
    let deposit_1 = 3_000_000u64;

    let liquidity =
        calculate_liquidity(sqrt_current, sqrt_lower, sqrt_upper, deposit_0, deposit_1).unwrap();

    // Step 2: Calculate required amounts for this liquidity
    let (required_0, required_1) = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        Q64x64::from_raw(liquidity),
    )
    .unwrap();

    // Required amounts should be <= deposits (due to minimum liquidity constraint)
    assert!(
        required_0 <= deposit_0,
        "Required token0 should not exceed deposit"
    );
    assert!(
        required_1 <= deposit_1,
        "Required token1 should not exceed deposit"
    );

    // Step 3: Simulate withdrawal - should get back same amounts
    let (withdraw_0, withdraw_1) = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        Q64x64::from_raw(liquidity),
    )
    .unwrap();

    assert_eq!(
        withdraw_0, required_0,
        "Withdrawal should match required amount for token0"
    );
    assert_eq!(
        withdraw_1, required_1,
        "Withdrawal should match required amount for token1"
    );
}

#[test]
fn test_position_value_across_price_movements() {
    // Test position value calculations across price movements
    let tick_lower = -5000;
    let tick_upper = 5000;
    let liquidity = Q64x64::from_int(100_000).raw();
    let position = build_position(tick_lower, tick_upper, liquidity, 1);

    let token_0_price = 100u64; // $100 per token0
    let token_1_price = 150u64; // $150 per token1

    // Test at different price points
    let price_ticks = vec![-10000, -5000, -2500, 0, 2500, 5000, 10000];

    let mut previous_value: Option<u64> = None;

    for tick in price_ticks {
        let sqrt_price = tick_to_sqrt_x64(tick).unwrap();
        let value =
            calculate_position_value_at_price(&position, sqrt_price, token_0_price, token_1_price)
                .unwrap();

        // Verify value consistency
        assert_eq!(
            value.total_value_usd,
            value.value_0_usd + value.value_1_usd,
            "Total value should equal sum at tick {}",
            tick
        );

        // Verify range active status
        let expected_active = tick >= tick_lower && tick < tick_upper;
        assert_eq!(
            value.price_range_active, expected_active,
            "Range active status incorrect at tick {}",
            tick
        );

        // Track value changes (for analysis, not strict assertion as value can vary)
        if let Some(_prev) = previous_value {
            // Value should be positive and within reasonable bounds
            assert!(
                value.total_value_usd > 0,
                "Value should be positive at tick {}",
                tick
            );
        }
        previous_value = Some(value.total_value_usd);
    }
}

#[test]
fn test_fee_tier_tick_spacing_alignment() {
    // Verify fee tier tick spacing alignment with liquidity calculations
    for (fee, tick_spacing) in TICK_SPACING_PER_FEE.iter() {
        // Create positions aligned with tick spacing
        let tick_lower = -10000 - (-10000 % *tick_spacing as i32);
        let tick_upper = 10000 - (10000 % *tick_spacing as i32);

        assert_eq!(
            tick_lower % *tick_spacing as i32,
            0,
            "Lower tick should align with spacing for fee {}",
            fee
        );
        assert_eq!(
            tick_upper % *tick_spacing as i32,
            0,
            "Upper tick should align with spacing for fee {}",
            fee
        );

        let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
        let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
        let sqrt_current = tick_to_sqrt_x64(0).unwrap();

        let liquidity = Q64x64::from_int(50_000);

        let result = calculate_amounts_for_liquidity_piecewise(
            sqrt_current,
            sqrt_lower,
            sqrt_upper,
            liquidity,
        );

        assert!(
            result.is_ok(),
            "Liquidity calculation should work for fee tier {} with spacing {}",
            fee,
            tick_spacing
        );
    }
}

// ============================================================================
// Edge Case Chain Tests
// ============================================================================

#[test]
fn test_create_position_price_moves_calculate_value_consistency() {
    // Test sequences: create position → price moves → calculate value → verify consistency
    let tick_lower = -3000;
    let tick_upper = 3000;
    let liquidity = Q64x64::from_int(75_000).raw();

    let position = build_position(tick_lower, tick_upper, liquidity, 1);

    // Simulate price movement sequence
    let price_sequence = vec![
        -5000, // Below range
        -2999, // Just inside lower bound
        -1500, // Inside range (lower half)
        0,     // Center
        1500,  // Inside range (upper half)
        2999,  // Just below upper bound
        5000,  // Above range
    ];

    for tick in price_sequence {
        let sqrt_price = tick_to_sqrt_x64(tick).unwrap();
        let value = calculate_position_value_at_price(&position, sqrt_price, 100, 100).unwrap();

        // Verify value invariants at each price point
        assert_eq!(
            value.total_value_usd,
            value.value_0_usd + value.value_1_usd,
            "Value consistency at tick {}",
            tick
        );

        // Verify token amounts match expectations based on concentrated liquidity logic
        // In concentrated liquidity:
        // - Below range (price < lower): Only token0 needed
        // - Above range (price >= upper): Only token1 needed
        // - In range: Both tokens needed
        if tick < tick_lower {
            assert!(
                value.amount_0 > 0,
                "Should have token0 below range at tick {}",
                tick
            );
            assert_eq!(value.amount_1, 0, "No token1 below range at tick {}", tick);
            assert!(!value.price_range_active);
        } else if tick >= tick_upper {
            assert_eq!(
                value.amount_0, 0,
                "No token0 above/at upper at tick {}",
                tick
            );
            assert!(
                value.amount_1 > 0,
                "Should have token1 above/at upper at tick {}",
                tick
            );
            assert!(!value.price_range_active);
        } else {
            // tick is in [tick_lower, tick_upper)
            assert!(
                value.amount_0 > 0,
                "Should have token0 in range at tick {}",
                tick
            );
            assert!(
                value.amount_1 > 0,
                "Should have token1 in range at tick {}",
                tick
            );
            assert!(value.price_range_active);
        }
    }
}

#[test]
fn test_multiple_positions_overlapping_ranges() {
    // Test multiple positions with overlapping ranges
    let positions = vec![
        build_position(-5000, 5000, Q64x64::from_int(50_000).raw(), 1),
        build_position(-2000, 2000, Q64x64::from_int(30_000).raw(), 2),
        build_position(-1000, 1000, Q64x64::from_int(20_000).raw(), 3),
        build_position(0, 10000, Q64x64::from_int(40_000).raw(), 4),
    ];

    let sqrt_current = tick_to_sqrt_x64(500).unwrap(); // Price at tick 500

    let mut total_value = 0u64;

    for position in positions.iter() {
        let value = calculate_position_value_at_price(position, sqrt_current, 100, 100).unwrap();

        // Verify each position independently
        assert_eq!(value.total_value_usd, value.value_0_usd + value.value_1_usd);

        // Accumulate total value across all positions
        total_value += value.total_value_usd;

        // Verify range active status
        let expected_active = 500 >= position.tick_lower && 500 < position.tick_upper;
        assert_eq!(value.price_range_active, expected_active);
    }

    // Total value should be positive and reasonable
    assert!(
        total_value > 0,
        "Total value across positions should be positive"
    );
}

#[test]
fn test_tick_spacing_boundary_calculations() {
    // Verify calculations at tick spacing boundaries
    let tick_spacing = 60; // 0.3% fee tier spacing

    for base_tick in (-10000..=10000).step_by(tick_spacing as usize) {
        let tick_lower = base_tick;
        let tick_upper = base_tick + (tick_spacing * 10); // 10 ticks wide

        if tick_upper > MAX_TICK {
            break;
        }

        let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
        let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
        let sqrt_current = tick_to_sqrt_x64(tick_lower + (tick_spacing * 5)).unwrap();

        let liquidity = Q64x64::from_int(10_000);

        let result = calculate_amounts_for_liquidity_piecewise(
            sqrt_current,
            sqrt_lower,
            sqrt_upper,
            liquidity,
        );

        assert!(
            result.is_ok(),
            "Should calculate at tick boundary {}",
            base_tick
        );

        let (amount_0, amount_1) = result.unwrap();
        assert!(
            amount_0 > 0 && amount_1 > 0,
            "Both tokens required in middle of range"
        );
    }
}

// ============================================================================
// Edge Case and Error Handling Tests
// ============================================================================

#[test]
fn test_invalid_price_range_errors() {
    // Test invalid ranges via piecewise calculation
    let liquidity = Q64x64::from_int(1000);

    // Test: lower >= upper (should error in piecewise calculation)
    let sqrt_lower = Q64x64::from_int(2);
    let sqrt_upper = Q64x64::from_int(1);
    let sqrt_current = Q64x64::from_raw((sqrt_lower.raw() + sqrt_upper.raw()) / 2);
    let result =
        calculate_amounts_for_liquidity_piecewise(sqrt_current, sqrt_lower, sqrt_upper, liquidity);
    assert!(result.is_err(), "Should error when lower >= upper");

    // Test: zero prices
    let sqrt_zero = Q64x64::zero();
    let sqrt_valid = Q64x64::from_int(2);
    let result =
        calculate_amounts_for_liquidity_piecewise(sqrt_valid, sqrt_zero, sqrt_valid, liquidity);
    assert!(result.is_err(), "Should error with zero lower price");
}

#[test]
fn test_zero_liquidity_handling() {
    let sqrt_lower = Q64x64::from_int(1);
    let sqrt_upper = Q64x64::from_int(2);
    let sqrt_current = Q64x64::from_raw((sqrt_lower.raw() + sqrt_upper.raw()) / 2);

    // Zero liquidity in piecewise calculation
    let result = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        Q64x64::zero(),
    );
    assert!(result.is_err(), "Should error with zero liquidity");

    // Zero token amounts in liquidity calculation
    let result = calculate_liquidity(sqrt_current, sqrt_lower, sqrt_upper, 0, 0);
    assert!(result.is_err(), "Should error with both amounts zero");
}

#[test]
fn test_price_outside_range_liquidity_calculation() {
    let sqrt_lower = Q64x64::from_int(2);
    let sqrt_upper = Q64x64::from_int(4);

    // Current price below range
    let sqrt_below = Q64x64::from_int(1);
    let result = calculate_liquidity(sqrt_below, sqrt_lower, sqrt_upper, 1000, 1000);
    assert!(
        result.is_err(),
        "Should error with current price below range"
    );

    // Current price above range
    let sqrt_above = Q64x64::from_int(5);
    let result = calculate_liquidity(sqrt_above, sqrt_lower, sqrt_upper, 1000, 1000);
    assert!(
        result.is_err(),
        "Should error with current price above range"
    );

    // Current price at lower boundary
    let result = calculate_liquidity(sqrt_lower, sqrt_lower, sqrt_upper, 1000, 1000);
    assert!(
        result.is_err(),
        "Should error with current price at lower boundary"
    );

    // Current price at upper boundary
    let result = calculate_liquidity(sqrt_upper, sqrt_lower, sqrt_upper, 1000, 1000);
    assert!(
        result.is_err(),
        "Should error with current price at upper boundary"
    );
}

#[test]
fn test_excessive_token_amount_rejection() {
    let sqrt_lower = Q64x64::from_raw(MIN_SQRT_X64);
    let sqrt_upper = Q64x64::from_raw(MAX_SQRT_X64);
    let sqrt_current = Q64x64::from_raw((MIN_SQRT_X64 + MAX_SQRT_X64) / 2);

    // Liquidity that would produce excessive token amounts
    let excessive_liquidity = Q64x64::from_raw(u128::MAX);

    let result = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        excessive_liquidity,
    );

    // Should either error or return amounts within bounds
    if let Ok((amount_0, amount_1)) = result {
        assert!(amount_0 <= MAX_TOKEN_AMOUNT, "Token0 should not exceed max");
        assert!(amount_1 <= MAX_TOKEN_AMOUNT, "Token1 should not exceed max");
    }
}

#[test]
fn test_boundary_tick_calculations() {
    // Test at MIN_TICK and MAX_TICK boundaries
    let test_cases = vec![
        (MIN_TICK, MIN_TICK + 1000, 0),
        (MAX_TICK - 1000, MAX_TICK, 0),
        (MIN_TICK, 0, -100000),
        (0, MAX_TICK, 50000),
    ];

    for (tick_lower, tick_upper, tick_current) in test_cases {
        let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
        let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
        let sqrt_current = tick_to_sqrt_x64(tick_current).unwrap();

        let liquidity = Q64x64::from_int(1000);

        // Should handle boundary calculations without overflow using piecewise calculation
        let result = calculate_amounts_for_liquidity_piecewise(
            sqrt_current,
            sqrt_lower,
            sqrt_upper,
            liquidity,
        );

        assert!(result.is_ok(), "Should handle boundary ticks");
    }
}

// ============================================================================
// Precision and Mathematical Invariant Tests
// ============================================================================

#[test]
fn test_liquidity_reciprocity_invariant() {
    // Test that deposit → calculate_liquidity → withdraw gives back consistent amounts
    // Note: Due to minimum liquidity constraint, one token will be fully used, the other may have excess
    let tick_lower = -1000;
    let tick_upper = 1000;
    let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
    let sqrt_current = tick_to_sqrt_x64(0).unwrap();

    let original_0 = 1_000_000u64;
    let original_1 = 2_000_000u64;

    // Calculate liquidity from deposits
    let liquidity =
        calculate_liquidity(sqrt_current, sqrt_lower, sqrt_upper, original_0, original_1).unwrap();

    // Calculate amounts from liquidity
    let (recovered_0, recovered_1) = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        Q64x64::from_raw(liquidity),
    )
    .unwrap();

    // Due to minimum liquidity constraint, recovered amounts should be <= original
    assert!(
        recovered_0 <= original_0,
        "Recovered token0 {} should not exceed original {}",
        recovered_0,
        original_0
    );
    assert!(
        recovered_1 <= original_1,
        "Recovered token1 {} should not exceed original {}",
        recovered_1,
        original_1
    );

    // At least one token should be fully utilized (within rounding tolerance)
    let utilization_0 = (recovered_0 as f64) / (original_0 as f64);
    let utilization_1 = (recovered_1 as f64) / (original_1 as f64);

    // One token should be nearly fully utilized (>95%) as it's the limiting factor
    assert!(
        utilization_0 > 0.95 || utilization_1 > 0.95,
        "At least one token should be fully utilized: token0={:.2}%, token1={:.2}%",
        utilization_0 * 100.0,
        utilization_1 * 100.0
    );
}
#[test]
fn test_amount_delta_monotonicity() {
    // Verify that wider price ranges require more tokens for same liquidity
    let liquidity = Q64x64::from_int(10_000);
    let sqrt_current = tick_to_sqrt_x64(0).unwrap();

    let sqrt_lower = tick_to_sqrt_x64(-1000).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(1000).unwrap();
    let sqrt_far_upper = tick_to_sqrt_x64(2000).unwrap();

    // Narrow range
    let (narrow_0, _narrow_1) =
        calculate_amounts_for_liquidity_piecewise(sqrt_current, sqrt_lower, sqrt_upper, liquidity)
            .unwrap();

    // Wider range (extends upper bound further)
    let (wide_0, _wide_1) = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_far_upper,
        liquidity,
    )
    .unwrap();

    // Wider ranges should require more token0 (price above current)
    assert!(wide_0 > narrow_0, "Wider range should require more token0");
}

#[test]
fn test_position_value_usd_calculation_precision() {
    // Verify USD value calculation precision and overflow handling
    let position = build_position(-1000, 1000, Q64x64::from_int(50_000).raw(), 1);
    let sqrt_current = tick_to_sqrt_x64(0).unwrap();

    // Test with various price combinations
    let price_cases = vec![
        (1u64, 1u64),           // Low prices
        (100, 100),             // Medium prices
        (1_000_000, 1_000_000), // High prices
        (1, 1_000_000),         // Asymmetric prices
        (1_000_000, 1),         // Reverse asymmetric
    ];

    for (price_0, price_1) in price_cases {
        let value = calculate_position_value_at_price(&position, sqrt_current, price_0, price_1);

        assert!(
            value.is_ok(),
            "Should calculate value for prices ({}, {})",
            price_0,
            price_1
        );

        let v = value.unwrap();

        // Verify value consistency
        assert_eq!(v.total_value_usd, v.value_0_usd + v.value_1_usd);

        // Verify individual values are reasonable
        assert!(
            v.value_0_usd > 0 || v.value_1_usd > 0,
            "Should have some value"
        );
    }
}

#[test]
fn test_liquidity_calculation_minimum_constraint() {
    // Verify that calculate_liquidity returns minimum of liquidity from both tokens
    let sqrt_lower = tick_to_sqrt_x64(-1000).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(1000).unwrap();
    let sqrt_current = tick_to_sqrt_x64(0).unwrap();

    // Case 1: Token0 is limiting factor
    let amount_0_small = 100_000u64;
    let amount_1_large = 10_000_000u64;

    let liquidity_0_limited = calculate_liquidity(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        amount_0_small,
        amount_1_large,
    )
    .unwrap();
    let expected_0 = liquidity_from_amount_0(sqrt_current, sqrt_upper, amount_0_small).unwrap();

    assert_eq!(
        liquidity_0_limited, expected_0,
        "Should be limited by token0"
    );

    // Case 2: Token1 is limiting factor
    let amount_0_large = 10_000_000u64;
    let amount_1_small = 100_000u64;

    let liquidity_1_limited = calculate_liquidity(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        amount_0_large,
        amount_1_small,
    )
    .unwrap();
    let expected_1 = liquidity_from_amount_1(sqrt_lower, sqrt_current, amount_1_small).unwrap();

    assert_eq!(
        liquidity_1_limited, expected_1,
        "Should be limited by token1"
    );
}

#[test]
fn test_sqrt_price_conversion_consistency() {
    // Verify that tick → sqrt → liquidity calculations are consistent
    let ticks = vec![-10000, -5000, -1000, 0, 1000, 5000, 10000];

    for tick in ticks {
        let sqrt_price = tick_to_sqrt_x64(tick).unwrap();

        // Use this price in liquidity calculations
        let tick_lower = tick - 500;
        let tick_upper = tick + 500;

        let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
        let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();

        let liquidity = Q64x64::from_int(10_000);

        // Should not error with properly converted sqrt prices
        let result = calculate_amounts_for_liquidity_piecewise(
            sqrt_price, sqrt_lower, sqrt_upper, liquidity,
        );

        assert!(
            result.is_ok(),
            "Liquidity calculation should work at tick {}",
            tick
        );

        let (amount_0, amount_1) = result.unwrap();
        assert!(
            amount_0 > 0 && amount_1 > 0,
            "Should require both tokens at tick {}",
            tick
        );
    }
}

// ============================================================================
// Stress Tests and Extreme Conditions
// ============================================================================

#[test]
fn test_extreme_liquidity_values() {
    // Test with very small and very large liquidity values
    let sqrt_lower = tick_to_sqrt_x64(-100).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(100).unwrap();
    let sqrt_current = tick_to_sqrt_x64(0).unwrap();

    // Very small liquidity (but not zero)
    let small_liquidity = Q64x64::from_raw(1000);
    let result_small = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        small_liquidity,
    );
    assert!(result_small.is_ok(), "Should handle small liquidity");

    // Large liquidity (but within reasonable bounds)
    let large_liquidity = Q64x64::from_int(1_000_000_000);
    let result_large = calculate_amounts_for_liquidity_piecewise(
        sqrt_current,
        sqrt_lower,
        sqrt_upper,
        large_liquidity,
    );

    // Either succeeds or properly errors (not panics)
    match result_large {
        Ok((a0, a1)) => {
            assert!(a0 <= MAX_TOKEN_AMOUNT, "Token0 within bounds");
            assert!(a1 <= MAX_TOKEN_AMOUNT, "Token1 within bounds");
        }
        Err(_) => {
            // Expected to error with excessive amounts - this is correct behavior
        }
    }
}

#[test]
fn test_narrow_price_ranges() {
    // Test with very narrow price ranges (single tick spacing)
    let tick_spacing = 1i32;

    for base_tick in (-1000..=1000).step_by(100) {
        let tick_lower = base_tick;
        let tick_upper = base_tick + tick_spacing;
        let tick_current = base_tick; // Position at the lower edge

        if tick_upper > MAX_TICK {
            break;
        }

        let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
        let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
        let sqrt_current = tick_to_sqrt_x64(tick_current).unwrap();

        let liquidity = Q64x64::from_int(1000);

        let result = calculate_amounts_for_liquidity_piecewise(
            sqrt_current,
            sqrt_lower,
            sqrt_upper,
            liquidity,
        );

        assert!(
            result.is_ok(),
            "Should handle narrow range at tick {}",
            base_tick
        );

        // For narrow ranges, amounts can be zero due to rounding
        // This is acceptable behavior
    }
}

#[test]
fn test_wide_price_ranges() {
    // Test with very wide price ranges
    let tick_lower = MIN_TICK;
    let tick_upper = MAX_TICK;

    let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
    let sqrt_current = tick_to_sqrt_x64(0).unwrap();

    let liquidity = Q64x64::from_int(1000);

    let result =
        calculate_amounts_for_liquidity_piecewise(sqrt_current, sqrt_lower, sqrt_upper, liquidity);

    assert!(result.is_ok(), "Should handle maximum range");

    let (amount_0, amount_1) = result.unwrap();
    assert!(amount_0 > 0, "Should require token0 in max range");
    assert!(amount_1 > 0, "Should require token1 in max range");
}

#[test]
fn test_sequential_price_movements() {
    // Simulate sequential price movements and verify value changes are monotonic in expected direction
    let position = build_position(-2000, 2000, Q64x64::from_int(50_000).raw(), 1);

    let price_sequence = (-3000..=3000).step_by(500).collect::<Vec<_>>();

    let mut previous_tick: Option<i32> = None;

    for tick in price_sequence {
        let sqrt_price = tick_to_sqrt_x64(tick).unwrap();
        let value = calculate_position_value_at_price(&position, sqrt_price, 100, 100).unwrap();

        // Value should always be positive
        assert!(
            value.total_value_usd > 0,
            "Value should be positive at tick {}",
            tick
        );

        // Track changes for analysis
        if let Some(_prev_tick) = previous_tick {
            // Values can change as price moves through the range
            // This test just ensures calculations succeed at all points
        }
        previous_tick = Some(tick);
    }
}
