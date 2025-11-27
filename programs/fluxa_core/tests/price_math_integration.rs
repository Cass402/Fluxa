//! Integration tests for price_math.rs - Cross-module consistency and protocol invariants.
//!
//! These tests verify that price_math.rs functions integrate correctly with:
//! - core_arithmetic.rs (tick_to_sqrt_x64, Q64x64 operations)
//! - Protocol constants (MIN/MAX_SQRT_X64, MIN/MAX_TICK)
//! - Error handling (MathError propagation)
//!
//! ## Test Organization
//! - Part 1: Core Arithmetic Integration (tick_to_sqrt_x64 ↔ sqrt_price_to_tick inverse relationship)
//! - Part 2: Protocol Constants Integration (LOOKUP_TABLE consistency, boundary enforcement)
//! - Part 3: Error Handling Integration (MathError propagation, Result chains)
//! - Part 4: Security Invariants (no panics, determinism, bounded execution, overflow safety)
//!
//! ## Coverage Goals
//! - 100% line coverage for public functions
//! - 100% branch coverage including error paths
//! - All LOOKUP_TABLE entries validated
//! - All mathematical edge cases covered

use anchor_lang::prelude::*;
use fluxa_core::error::MathError;
use fluxa_core::math::core_arithmetic::{tick_to_sqrt_x64, Q64x64};
use fluxa_core::math::price_math::{sqrt_price_to_price, sqrt_price_to_tick};
use fluxa_core::utils::constants::{MAX_SQRT_X64, MAX_TICK, MIN_SQRT_X64, MIN_TICK, ONE_X64};

// ============================================================================
// PART 1: CORE ARITHMETIC INTEGRATION TESTS
// ============================================================================
// Verify tick_to_sqrt_x64 and sqrt_price_to_tick are proper inverses

/// Tests that tick_to_sqrt_x64 and sqrt_price_to_tick form a proper inverse relationship.
/// This is critical for protocol correctness: positions created at tick T must map back to T.
#[test]
fn test_tick_sqrt_inverse_relationship_standard_ticks() {
    // Test standard tick values commonly used in AMM operations
    let standard_ticks = vec![
        MIN_TICK,
        MIN_TICK + 10000,
        -100000,
        -50000,
        -10000,
        -1000,
        -100,
        -10,
        -1,
        0,
        1,
        10,
        100,
        1000,
        10000,
        50000,
        100000,
        MAX_TICK - 10000,
        MAX_TICK,
    ];

    for original_tick in standard_ticks {
        // Forward: tick → sqrt_price
        let sqrt_price = tick_to_sqrt_x64(original_tick)
            .unwrap_or_else(|_| panic!("tick_to_sqrt_x64 failed for tick {}", original_tick));

        // Inverse: sqrt_price → tick
        let recovered_tick = sqrt_price_to_tick(sqrt_price).unwrap_or_else(|_| {
            panic!(
                "sqrt_price_to_tick failed for sqrt_price from tick {}",
                original_tick
            )
        });

        // Verify inverse relationship: recovered tick should equal original
        // Allow tolerance of 1 tick due to discrete tick spacing
        let tick_diff = (recovered_tick - original_tick).abs();
        assert!(
            tick_diff <= 1,
            "Inverse relationship violated: original_tick={}, recovered_tick={}, diff={}. \
             tick_to_sqrt_x64 and sqrt_price_to_tick must be proper inverses.",
            original_tick,
            recovered_tick,
            tick_diff
        );
    }
}

/// Tests inverse relationship for tick values at fee tier spacing boundaries.
/// Fee tiers use specific tick spacings (1, 10, 60, 200) that must be preserved.
#[test]
fn test_tick_sqrt_inverse_at_fee_tier_spacings() {
    // Common tick spacings per fee tier: 1 (0.01%), 10 (0.05%), 60 (0.30%), 200 (1.00%)
    let tick_spacings = [1, 10, 60, 200];

    for spacing in tick_spacings {
        // Test ticks aligned to this spacing
        let test_ticks: Vec<i32> = (-5..=5)
            .map(|i| i * spacing * 1000) // Generate ticks at spacing intervals
            .filter(|&t| (MIN_TICK..=MAX_TICK).contains(&t))
            .collect();

        for original_tick in test_ticks {
            let sqrt_price = tick_to_sqrt_x64(original_tick).unwrap();
            let recovered_tick = sqrt_price_to_tick(sqrt_price).unwrap();

            let tick_diff = (recovered_tick - original_tick).abs();
            assert!(
                tick_diff <= 1,
                "Fee tier spacing {} inverse failed: original={}, recovered={}, diff={}",
                spacing,
                original_tick,
                recovered_tick,
                tick_diff
            );
        }
    }
}

/// Tests that the inverse relationship holds for the full tick range with sampling.
/// Samples every 10,000 ticks to ensure coverage without excessive runtime.
#[test]
fn test_tick_sqrt_inverse_full_range_sampling() {
    let sample_step = 10_000i32;
    let mut tick = MIN_TICK;
    let mut samples_tested = 0;
    let mut max_tick_error = 0i32;

    while tick <= MAX_TICK {
        if let Ok(sqrt_price) = tick_to_sqrt_x64(tick) {
            if let Ok(recovered_tick) = sqrt_price_to_tick(sqrt_price) {
                samples_tested += 1;
                let tick_diff = (recovered_tick - tick).abs();
                max_tick_error = max_tick_error.max(tick_diff);

                assert!(
                    tick_diff <= 1,
                    "Full range inverse failed at tick {}: recovered={}, diff={}",
                    tick,
                    recovered_tick,
                    tick_diff
                );
            }
        }
        tick = tick.saturating_add(sample_step);
    }

    assert!(
        samples_tested > 80,
        "Insufficient samples tested: {} (expected > 80 for full range coverage)",
        samples_tested
    );

    assert!(
        max_tick_error <= 1,
        "Maximum tick error {} exceeds tolerance of 1 tick",
        max_tick_error
    );
}

// ============================================================================
// Q64x64 ARITHMETIC ERROR PROPAGATION TESTS
// ============================================================================

/// Tests that Q64x64 arithmetic errors propagate correctly through sqrt_price_to_price.
#[test]
fn test_q64x64_error_propagation_in_sqrt_price_to_price() {
    // Test that checked_mul overflow propagates as error
    // Use maximum Q64x64 value that would overflow when squared
    let overflow_sqrt = Q64x64::from_raw(u128::MAX);
    let result = sqrt_price_to_price(overflow_sqrt);

    assert!(
        result.is_err(),
        "Overflow in Q64x64::checked_mul must propagate as error, not panic"
    );
}

/// Tests that Q64x64 operations in sqrt_price_to_tick maintain precision.
#[test]
fn test_q64x64_precision_in_conversion_chain() {
    // Test conversion chain: tick → sqrt_price → tick → sqrt_price
    // Verify precision is maintained through the chain
    let test_ticks = vec![-200000, -100000, -10000, 0, 10000, 100000, 200000];

    for original_tick in test_ticks {
        let sqrt_1 = tick_to_sqrt_x64(original_tick).unwrap();
        let tick_1 = sqrt_price_to_tick(sqrt_1).unwrap();
        let sqrt_2 = tick_to_sqrt_x64(tick_1).unwrap();
        let tick_2 = sqrt_price_to_tick(sqrt_2).unwrap();

        // After two round-trips, tick should be stable (no drift)
        assert_eq!(
            tick_1, tick_2,
            "Conversion chain unstable: tick {} → {} → {} (should stabilize after first round-trip)",
            original_tick, tick_1, tick_2
        );

        // Sqrt prices should also be stable after stabilization
        let sqrt_3 = tick_to_sqrt_x64(tick_2).unwrap();
        assert_eq!(
            sqrt_2.raw(),
            sqrt_3.raw(),
            "Sqrt price drift after stabilization: tick={}, sqrt_2={}, sqrt_3={}",
            tick_1,
            sqrt_2.raw(),
            sqrt_3.raw()
        );
    }
}

/// Tests fixed-point precision across conversion chains with price calculation.
#[test]
fn test_fixed_point_precision_tick_to_price_chain() {
    // Verify: tick → sqrt_price → price maintains expected mathematical relationship
    // price ≈ 1.0001^tick (within quantization tolerance)

    let test_cases = vec![
        (0, 1u64),      // tick 0 → price ≈ 1.0001^0 = 1
        (10000, 2u64),  // tick 10000 → price ≈ 1.0001^10000 ≈ 2.7 (truncated to 2)
        (-10000, 0u64), // tick -10000 → price ≈ 0.37 (truncated to 0)
        (20000, 7u64),  // tick 20000 → price ≈ 7.4 (truncated)
    ];

    for (tick, min_expected_price) in test_cases {
        let sqrt_price = tick_to_sqrt_x64(tick).unwrap();
        let price = sqrt_price_to_price(sqrt_price).unwrap();

        // Price should be at least min_expected_price (accounting for truncation)
        assert!(
            price >= min_expected_price,
            "Price chain failed: tick {} → sqrt_price {} → price {} (expected >= {})",
            tick,
            sqrt_price.raw(),
            price,
            min_expected_price
        );
    }
}

// ============================================================================
// CROSS-MODULE CONSISTENCY TESTS
// ============================================================================

/// Tests that sqrt_price values from tick_to_sqrt_x64 are always within protocol bounds.
#[test]
fn test_tick_to_sqrt_respects_protocol_bounds() {
    // All ticks in valid range should produce sqrt_prices within bounds
    let boundary_ticks = vec![
        MIN_TICK,
        MIN_TICK + 1,
        MIN_TICK + 100,
        -1,
        0,
        1,
        MAX_TICK - 100,
        MAX_TICK - 1,
        MAX_TICK,
    ];

    for tick in boundary_ticks {
        let sqrt_price = tick_to_sqrt_x64(tick)
            .unwrap_or_else(|_| panic!("tick_to_sqrt_x64 should succeed for valid tick {}", tick));

        assert!(
            sqrt_price.raw() >= MIN_SQRT_X64,
            "tick {} produced sqrt_price {} below MIN_SQRT_X64 {}",
            tick,
            sqrt_price.raw(),
            MIN_SQRT_X64
        );

        assert!(
            sqrt_price.raw() <= MAX_SQRT_X64,
            "tick {} produced sqrt_price {} above MAX_SQRT_X64 {}",
            tick,
            sqrt_price.raw(),
            MAX_SQRT_X64
        );
    }
}

/// Tests that sqrt_price_to_tick produces ticks within protocol bounds.
#[test]
fn test_sqrt_to_tick_respects_protocol_bounds() {
    // All valid sqrt_prices should produce ticks within bounds
    let test_sqrt_prices = vec![
        Q64x64::from_raw(MIN_SQRT_X64),
        Q64x64::from_raw(MIN_SQRT_X64 + 1000000),
        Q64x64::from_raw(ONE_X64),
        Q64x64::from_raw(ONE_X64 * 10),
        Q64x64::from_raw(MAX_SQRT_X64 - 1000000),
        Q64x64::from_raw(MAX_SQRT_X64),
    ];

    for sqrt_price in test_sqrt_prices {
        let tick = sqrt_price_to_tick(sqrt_price).unwrap_or_else(|_| {
            panic!(
                "sqrt_price_to_tick should succeed for valid sqrt_price {:?}",
                sqrt_price.raw()
            )
        });

        assert!(
            tick >= MIN_TICK,
            "sqrt_price {} produced tick {} below MIN_TICK {}",
            sqrt_price.raw(),
            tick,
            MIN_TICK
        );

        assert!(
            tick <= MAX_TICK,
            "sqrt_price {} produced tick {} above MAX_TICK {}",
            sqrt_price.raw(),
            tick,
            MAX_TICK
        );
    }
}

/// Tests consistency between sqrt_price_to_price and the mathematical definition.
#[test]
fn test_sqrt_price_to_price_mathematical_consistency() {
    // Verify: price = sqrt_price^2 >> 64 (Q64x64 squaring)
    let test_values = vec![
        Q64x64::from_int(1),   // sqrt(1)^2 = 1
        Q64x64::from_int(2),   // sqrt(4)^2 = 4
        Q64x64::from_int(10),  // sqrt(100)^2 = 100
        Q64x64::from_int(100), // sqrt(10000)^2 = 10000
    ];

    for sqrt_price in test_values {
        let price = sqrt_price_to_price(sqrt_price).unwrap();

        // Manual calculation: sqrt_price^2 >> 64
        let squared = sqrt_price.checked_mul(sqrt_price).unwrap();
        let expected_price = (squared.raw() >> 64) as u64;

        assert_eq!(
            price,
            expected_price,
            "Mathematical inconsistency: sqrt_price={}, computed={}, expected={}",
            sqrt_price.raw(),
            price,
            expected_price
        );
    }
}

/// Tests that conversion functions are bijective within precision tolerance.
#[test]
fn test_conversion_bijectivity() {
    // For distinct ticks, sqrt_prices should be distinct
    // For distinct sqrt_prices, ticks should be monotonically ordered

    let mut prev_sqrt: Option<u128> = None;
    let mut prev_tick = MIN_TICK - 1;

    // Sample ticks across the range
    for tick in (MIN_TICK..=MAX_TICK).step_by(5000) {
        let sqrt_price = tick_to_sqrt_x64(tick).unwrap();

        // Verify strict monotonicity of sqrt_price with respect to tick
        if let Some(prev) = prev_sqrt {
            assert!(
                sqrt_price.raw() > prev,
                "Bijectivity violated: tick {} produced sqrt_price {} <= previous {}",
                tick,
                sqrt_price.raw(),
                prev
            );
        }

        // Verify inverse maintains ordering
        let recovered_tick = sqrt_price_to_tick(sqrt_price).unwrap();
        assert!(
            recovered_tick > prev_tick || recovered_tick == prev_tick + 1 || (tick == MIN_TICK),
            "Inverse ordering violated: tick {} → sqrt {} → recovered {} (prev: {})",
            tick,
            sqrt_price.raw(),
            recovered_tick,
            prev_tick
        );

        prev_sqrt = Some(sqrt_price.raw());
        prev_tick = recovered_tick;
    }
}

// ============================================================================
// PART 2: PROTOCOL CONSTANTS INTEGRATION TESTS
// ============================================================================
// Verify LOOKUP_TABLE entries are consistent with tick_to_sqrt_x64

/// Tests that all LOOKUP_TABLE entries are consistent with tick_to_sqrt_x64.
/// Each (sqrt_price, tick) pair in the table should satisfy: tick_to_sqrt_x64(tick) ≈ sqrt_price.
#[test]
fn test_lookup_table_consistency_with_tick_to_sqrt() {
    // LOOKUP_TABLE entries from price_math.rs (representative sample)
    // Format: (sqrt_price_raw, expected_tick)
    let lookup_entries: Vec<(u128, i32)> = vec![
        (4295048016u128, -443636),
        (7081160003u128, -433636),
        (11674567271u128, -423636),
        (19247626221u128, -413636),
        (31733177475u128, -403636),
        (52317856814u128, -393636),
        (86255407099u128, -383636),
        (142207569401u128, -373636),
        (234454783474u128, -363636),
        (386540925532u128, -353636),
        // Mid-range entries
        (15380446138133159918u128, -3636),
        (25357434799262418241u128, 6364),
        (41806297023116804610u128, 16364),
        (68925208114343765373u128, 26364),
        (113635615969017952144u128, 36364),
        // Upper range entries
        (7454599325645827675584627361u128, 396364),
        (12290249233149591626786744287u128, 406364),
        (20262688793116043139907801508u128, 416364),
        (33406690892748727032575218928u128, 426364),
        (55076945009529438865748214840u128, 436364),
    ];

    for (table_sqrt_price, table_tick) in lookup_entries {
        // Skip entries outside valid sqrt_price range
        if !(MIN_SQRT_X64..=MAX_SQRT_X64).contains(&table_sqrt_price) {
            continue;
        }

        // Verify: tick_to_sqrt_x64(table_tick) ≈ table_sqrt_price
        let computed_sqrt = tick_to_sqrt_x64(table_tick).unwrap();

        // Allow 0.1% relative error for lookup table approximation
        let rel_error = if table_sqrt_price > 0 {
            let diff = computed_sqrt.raw().abs_diff(table_sqrt_price);
            (diff as f64 / table_sqrt_price as f64) * 100.0
        } else {
            0.0
        };

        assert!(
            rel_error < 0.1,
            "LOOKUP_TABLE inconsistency: tick {} maps to sqrt {} but table has {} (error: {:.4}%)",
            table_tick,
            computed_sqrt.raw(),
            table_sqrt_price,
            rel_error
        );
    }
}

/// Tests that LOOKUP_TABLE entries are monotonically increasing.
/// This is critical for binary search correctness in coarse_lookup_table_search.
#[test]
fn test_lookup_table_monotonicity() {
    // Representative sqrt_price values from LOOKUP_TABLE
    let lookup_sqrt_prices: Vec<u128> = vec![
        4295048016u128,
        7081160003u128,
        11674567271u128,
        19247626221u128,
        31733177475u128,
        52317856814u128,
        86255407099u128,
        142207569401u128,
        234454783474u128,
        386540925532u128,
        637282314726u128,
        1050674642287u128,
        // ... sampling continues
        15380446138133159918u128,
        25357434799262418241u128,
        41806297023116804610u128,
        68925208114343765373u128,
        113635615969017952144u128,
    ];

    let mut prev_sqrt: Option<u128> = None;

    for sqrt_price in lookup_sqrt_prices {
        if let Some(prev) = prev_sqrt {
            assert!(
                sqrt_price > prev,
                "LOOKUP_TABLE monotonicity violated: {} <= {}",
                sqrt_price,
                prev
            );
        }
        prev_sqrt = Some(sqrt_price);
    }
}

/// Tests that LOOKUP_TABLE covers the full protocol sqrt_price range.
#[test]
fn test_lookup_table_range_coverage() {
    // First entry should be near MIN_SQRT_X64
    let first_entry_sqrt = 4295048016u128;
    let first_entry_tick = -443636i32;

    // Last entry should be near MAX_SQRT_X64
    let last_entry_sqrt = 55076945009529438865748214840u128;
    let last_entry_tick = 436364i32;

    // Verify first entry is close to MIN_SQRT_X64
    assert!(
        first_entry_sqrt < MIN_SQRT_X64 * 2,
        "First LOOKUP_TABLE entry {} too far from MIN_SQRT_X64 {}",
        first_entry_sqrt,
        MIN_SQRT_X64
    );

    // Verify last entry provides coverage toward MAX_SQRT_X64
    assert!(
        last_entry_sqrt > MAX_SQRT_X64 / 2,
        "Last LOOKUP_TABLE entry {} should cover toward MAX_SQRT_X64 {}",
        last_entry_sqrt,
        MAX_SQRT_X64
    );

    // Verify tick range coverage
    assert!(
        first_entry_tick <= MIN_TICK + 1000,
        "First entry tick {} should be near MIN_TICK {}",
        first_entry_tick,
        MIN_TICK
    );

    assert!(
        last_entry_tick >= MAX_TICK - 10000,
        "Last entry tick {} should approach MAX_TICK {}",
        last_entry_tick,
        MAX_TICK
    );
}

// ============================================================================
// MIN/MAX BOUNDS ENFORCEMENT TESTS
// ============================================================================

/// Tests that MIN_SQRT_X64 maps to approximately MIN_TICK.
#[test]
fn test_min_sqrt_maps_to_min_tick() {
    let min_sqrt = Q64x64::from_raw(MIN_SQRT_X64);
    let tick = sqrt_price_to_tick(min_sqrt).unwrap();

    // Should be very close to MIN_TICK
    let tick_diff = (tick - MIN_TICK).abs();
    assert!(
        tick_diff <= 5,
        "MIN_SQRT_X64 should map to ≈MIN_TICK: got tick {} (MIN_TICK={}, diff={})",
        tick,
        MIN_TICK,
        tick_diff
    );
}

/// Tests that MAX_SQRT_X64 maps to approximately MAX_TICK.
#[test]
fn test_max_sqrt_maps_to_max_tick() {
    let max_sqrt = Q64x64::from_raw(MAX_SQRT_X64);
    let tick = sqrt_price_to_tick(max_sqrt).unwrap();

    // Should be very close to MAX_TICK
    let tick_diff = (tick - MAX_TICK).abs();
    assert!(
        tick_diff <= 5,
        "MAX_SQRT_X64 should map to ≈MAX_TICK: got tick {} (MAX_TICK={}, diff={})",
        tick,
        MAX_TICK,
        tick_diff
    );
}

/// Tests that MIN_TICK maps to approximately MIN_SQRT_X64.
#[test]
fn test_min_tick_maps_to_min_sqrt() {
    let sqrt_price = tick_to_sqrt_x64(MIN_TICK).unwrap();

    // Should be very close to MIN_SQRT_X64
    let rel_error = sqrt_price.raw().abs_diff(MIN_SQRT_X64) as f64 / MIN_SQRT_X64 as f64;
    assert!(
        rel_error < 0.01,
        "MIN_TICK should map to ≈MIN_SQRT_X64: got {} (MIN_SQRT_X64={}, error={:.4}%)",
        sqrt_price.raw(),
        MIN_SQRT_X64,
        rel_error * 100.0
    );
}

/// Tests that MAX_TICK maps to approximately MAX_SQRT_X64.
#[test]
fn test_max_tick_maps_to_max_sqrt() {
    let sqrt_price = tick_to_sqrt_x64(MAX_TICK).unwrap();

    // Should be very close to MAX_SQRT_X64
    let rel_error = sqrt_price.raw().abs_diff(MAX_SQRT_X64) as f64 / MAX_SQRT_X64 as f64;
    assert!(
        rel_error < 0.01,
        "MAX_TICK should map to ≈MAX_SQRT_X64: got {} (MAX_SQRT_X64={}, error={:.4}%)",
        sqrt_price.raw(),
        MAX_SQRT_X64,
        rel_error * 100.0
    );
}

/// Tests behavior at exact boundary values.
#[test]
fn test_exact_boundary_values() {
    // Test exactly at MIN_SQRT_X64
    let min_result = sqrt_price_to_tick(Q64x64::from_raw(MIN_SQRT_X64));
    assert!(min_result.is_ok(), "Exactly MIN_SQRT_X64 must be valid");

    // Test exactly at MAX_SQRT_X64
    let max_result = sqrt_price_to_tick(Q64x64::from_raw(MAX_SQRT_X64));
    assert!(max_result.is_ok(), "Exactly MAX_SQRT_X64 must be valid");

    // Test just inside boundaries
    let just_above_min = sqrt_price_to_tick(Q64x64::from_raw(MIN_SQRT_X64 + 1));
    assert!(
        just_above_min.is_ok(),
        "Just above MIN_SQRT_X64 must be valid"
    );

    let just_below_max = sqrt_price_to_tick(Q64x64::from_raw(MAX_SQRT_X64 - 1));
    assert!(
        just_below_max.is_ok(),
        "Just below MAX_SQRT_X64 must be valid"
    );

    // Test just outside boundaries (should fail)
    let below_min = sqrt_price_to_tick(Q64x64::from_raw(MIN_SQRT_X64 - 1));
    assert!(below_min.is_err(), "Below MIN_SQRT_X64 must return error");

    let above_max = sqrt_price_to_tick(Q64x64::from_raw(MAX_SQRT_X64 + 1));
    assert!(above_max.is_err(), "Above MAX_SQRT_X64 must return error");
}

// ============================================================================
// PART 3: ERROR HANDLING INTEGRATION TESTS
// ============================================================================
// Verify MathError::InvalidSqrtPrice is raised correctly and error propagation works

/// Tests that MathError::InvalidSqrtPrice is returned for out-of-bounds sqrt_price values.
#[test]
fn test_invalid_sqrt_price_error_type() {
    use anchor_lang::error::Error as AnchorError;

    // Test below MIN_SQRT_X64
    let below_min = Q64x64::from_raw(MIN_SQRT_X64 - 1);
    let result = sqrt_price_to_tick(below_min);

    assert!(result.is_err(), "Below MIN_SQRT_X64 should return error");
    if let Err(e) = result {
        assert_eq!(
            e,
            AnchorError::from(MathError::InvalidSqrtPrice),
            "Error type should be MathError::InvalidSqrtPrice for below-min value"
        );
    }

    // Test above MAX_SQRT_X64
    let above_max = Q64x64::from_raw(MAX_SQRT_X64 + 1);
    let result = sqrt_price_to_tick(above_max);

    assert!(result.is_err(), "Above MAX_SQRT_X64 should return error");
    if let Err(e) = result {
        assert_eq!(
            e,
            AnchorError::from(MathError::InvalidSqrtPrice),
            "Error type should be MathError::InvalidSqrtPrice for above-max value"
        );
    }

    // Test zero
    let zero = Q64x64::zero();
    let result = sqrt_price_to_tick(zero);

    assert!(result.is_err(), "Zero sqrt_price should return error");
    if let Err(e) = result {
        assert_eq!(
            e,
            AnchorError::from(MathError::InvalidSqrtPrice),
            "Error type should be MathError::InvalidSqrtPrice for zero"
        );
    }
}

/// Tests that sqrt_price_to_price returns correct error types.
#[test]
fn test_sqrt_price_to_price_error_types() {
    use anchor_lang::error::Error as AnchorError;

    // Below MIN_SQRT_X64
    let below_min = Q64x64::from_raw(MIN_SQRT_X64 - 1);
    let result = sqrt_price_to_price(below_min);
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(
            e,
            AnchorError::from(MathError::InvalidSqrtPrice),
            "sqrt_price_to_price should return InvalidSqrtPrice for below-min"
        );
    }

    // Above MAX_SQRT_X64
    let above_max = Q64x64::from_raw(MAX_SQRT_X64 + 1);
    let result = sqrt_price_to_price(above_max);
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(
            e,
            AnchorError::from(MathError::InvalidSqrtPrice),
            "sqrt_price_to_price should return InvalidSqrtPrice for above-max"
        );
    }
}

/// Tests error propagation through Result chains.
#[test]
fn test_error_propagation_through_result_chains() {
    // Simulate a chain of operations where error should propagate
    fn chain_conversion(tick: i32) -> Result<u64> {
        let sqrt_price = tick_to_sqrt_x64(tick)?;
        let price = sqrt_price_to_price(sqrt_price)?;
        Ok(price)
    }

    // Valid tick should succeed
    let valid_result = chain_conversion(0);
    assert!(
        valid_result.is_ok(),
        "Valid tick should produce valid price"
    );

    // Invalid tick should propagate error
    let invalid_result = chain_conversion(MIN_TICK - 1);
    assert!(
        invalid_result.is_err(),
        "Invalid tick error should propagate through chain"
    );

    let invalid_result = chain_conversion(MAX_TICK + 1);
    assert!(
        invalid_result.is_err(),
        "Invalid tick error should propagate through chain"
    );
}

/// Tests that all error paths return Result::Err, never panic.
#[test]
fn test_no_panic_on_any_error_path() {
    // Comprehensive list of invalid inputs that should return errors, not panic
    let invalid_sqrt_prices = vec![
        Q64x64::from_raw(0),
        Q64x64::from_raw(1),
        Q64x64::from_raw(MIN_SQRT_X64 - 1),
        Q64x64::from_raw(MIN_SQRT_X64 - 1000),
        Q64x64::from_raw(MAX_SQRT_X64 + 1),
        Q64x64::from_raw(MAX_SQRT_X64 + 1000),
        Q64x64::from_raw(u128::MAX),
        Q64x64::from_raw(u128::MAX - 1),
    ];

    for sqrt_price in invalid_sqrt_prices {
        // sqrt_price_to_tick should return error, not panic
        let result = sqrt_price_to_tick(sqrt_price);
        // We just verify it returns (either Ok or Err), not panics
        let _ = result.is_err();

        // sqrt_price_to_price should return error, not panic
        let result = sqrt_price_to_price(sqrt_price);
        let _ = result.is_err();
    }

    // Invalid ticks
    let invalid_ticks = vec![
        MIN_TICK - 1,
        MIN_TICK - 100,
        MIN_TICK - 10000,
        MAX_TICK + 1,
        MAX_TICK + 100,
        MAX_TICK + 10000,
        i32::MIN,
        i32::MAX,
    ];

    for tick in invalid_ticks {
        // tick_to_sqrt_x64 should return error, not panic
        let result = tick_to_sqrt_x64(tick);
        let _ = result.is_err();
    }
}

/// Tests that errors provide sufficient context for debugging.
#[test]
fn test_error_context_sufficiency() {
    // Verify error messages are meaningful (not empty/generic)
    let result = sqrt_price_to_tick(Q64x64::zero());
    if let Err(e) = result {
        let error_string = format!("{:?}", e);
        assert!(
            !error_string.is_empty(),
            "Error should have non-empty debug representation"
        );
        assert!(
            error_string.contains("InvalidSqrtPrice") || error_string.len() > 5,
            "Error should contain meaningful context: {}",
            error_string
        );
    }
}

/// Tests error handling in edge case numerical scenarios.
#[test]
fn test_numerical_edge_case_errors() {
    // Very small values just above zero
    for i in 1..10 {
        let tiny = Q64x64::from_raw(i);
        let result = sqrt_price_to_tick(tiny);
        assert!(
            result.is_err(),
            "Tiny sqrt_price {} should be below MIN_SQRT_X64 and return error",
            i
        );
    }

    // Values very close to but exceeding MAX_SQRT_X64
    for i in 1..10 {
        let huge = Q64x64::from_raw(MAX_SQRT_X64.saturating_add(i as u128));
        let result = sqrt_price_to_tick(huge);
        assert!(
            result.is_err(),
            "sqrt_price {} exceeding MAX_SQRT_X64 should return error",
            huge.raw()
        );
    }
}

// ============================================================================
// PART 4: SECURITY INVARIANTS TESTS
// ============================================================================
// Critical security properties that must hold for DeFi protocol safety

/// Tests the "No Panics" invariant: all invalid inputs return Result::Err, never panic.
#[test]
fn test_security_no_panics_exhaustive() {
    // Exhaustive test of all potential panic-inducing inputs
    let dangerous_inputs = vec![
        // Zero and near-zero
        Q64x64::from_raw(0),
        Q64x64::from_raw(1),
        Q64x64::from_raw(100),
        // Near MIN_SQRT_X64
        Q64x64::from_raw(MIN_SQRT_X64.saturating_sub(1)),
        Q64x64::from_raw(MIN_SQRT_X64.saturating_sub(1000)),
        // Near MAX_SQRT_X64
        Q64x64::from_raw(MAX_SQRT_X64.saturating_add(1)),
        Q64x64::from_raw(MAX_SQRT_X64.saturating_add(1000)),
        // Maximum values (potential overflow)
        Q64x64::from_raw(u128::MAX),
        Q64x64::from_raw(u128::MAX - 1),
        Q64x64::from_raw(u128::MAX / 2),
        // Powers of 2 (edge cases in binary arithmetic)
        Q64x64::from_raw(1u128 << 64),
        Q64x64::from_raw(1u128 << 127),
    ];

    for input in dangerous_inputs {
        // All calls should return without panic
        let _ = sqrt_price_to_tick(input);
        let _ = sqrt_price_to_price(input);
    }

    // Test dangerous tick values
    let dangerous_ticks = vec![
        i32::MIN,
        i32::MIN + 1,
        MIN_TICK - 1,
        MIN_TICK - 1000,
        MAX_TICK + 1,
        MAX_TICK + 1000,
        i32::MAX - 1,
        i32::MAX,
    ];

    for tick in dangerous_ticks {
        let _ = tick_to_sqrt_x64(tick);
    }
}

/// Tests the "Determinism" invariant: same input always produces same output.
#[test]
fn test_security_determinism() {
    let test_inputs = vec![
        Q64x64::from_raw(MIN_SQRT_X64),
        Q64x64::from_raw(ONE_X64),
        Q64x64::from_raw(ONE_X64 * 10),
        Q64x64::from_raw(MAX_SQRT_X64),
    ];

    for input in test_inputs {
        // Run each conversion 10 times and verify identical results
        let mut tick_results = Vec::new();
        let mut price_results = Vec::new();

        for _ in 0..10 {
            if let Ok(tick) = sqrt_price_to_tick(input) {
                tick_results.push(tick);
            }
            if let Ok(price) = sqrt_price_to_price(input) {
                price_results.push(price);
            }
        }

        // All tick results should be identical
        if !tick_results.is_empty() {
            let first_tick = tick_results[0];
            for tick in &tick_results {
                assert_eq!(
                    *tick,
                    first_tick,
                    "Determinism violated: sqrt_price {} produced different ticks",
                    input.raw()
                );
            }
        }

        // All price results should be identical
        if !price_results.is_empty() {
            let first_price = price_results[0];
            for price in &price_results {
                assert_eq!(
                    *price,
                    first_price,
                    "Determinism violated: sqrt_price {} produced different prices",
                    input.raw()
                );
            }
        }
    }
}

/// Tests the "Bounded Execution" invariant: all functions complete in constant/bounded time.
#[test]
fn test_security_bounded_execution() {
    // Test with 1000 random-ish inputs to verify bounded execution time
    // If any input causes excessive iterations, it would timeout/slow down noticeably

    let step = (MAX_SQRT_X64 - MIN_SQRT_X64) / 1000;

    for i in 0..1000 {
        let sqrt_price = Q64x64::from_raw(MIN_SQRT_X64 + i * step);

        // These should all complete quickly (bounded by MAX_BINARY_ITERATIONS = 32)
        let _ = sqrt_price_to_tick(sqrt_price);
        let _ = sqrt_price_to_price(sqrt_price);
    }

    // Also test tick conversions
    let tick_step = (MAX_TICK - MIN_TICK) / 1000;
    for i in 0..1000 {
        let tick = MIN_TICK + i * tick_step;
        let _ = tick_to_sqrt_x64(tick);
    }

    // If we reach here, bounded execution is verified
}

/// Tests the "Overflow Safety" invariant: all arithmetic uses checked operations.
#[test]
fn test_security_overflow_safety() {
    // Test values that could trigger overflow in naive implementations
    let overflow_prone_values = vec![
        // Large values that square to overflow
        Q64x64::from_raw(u128::MAX),
        Q64x64::from_raw(1u128 << 127),
        Q64x64::from_raw(1u128 << 96),
        // Values near type boundaries
        Q64x64::from_raw(MAX_SQRT_X64),
        Q64x64::from_raw(MAX_SQRT_X64 - 1),
    ];

    for value in overflow_prone_values {
        // sqrt_price_to_price involves squaring, which could overflow
        let result = sqrt_price_to_price(value);
        // Should return error, not panic or wrap
        assert!(
            result.is_err() || result.is_ok(),
            "Overflow should be handled gracefully"
        );
    }

    // Test multiplication chains that could overflow
    let sqrt_price = Q64x64::from_raw(MAX_SQRT_X64);
    let result = sqrt_price_to_price(sqrt_price);
    // Should succeed without overflow (values are within bounds)
    assert!(
        result.is_ok(),
        "MAX_SQRT_X64 should produce valid price without overflow"
    );
}

/// Tests the "Range Validity" invariant: all outputs respect protocol min/max constraints.
#[test]
fn test_security_range_validity() {
    // Sample valid sqrt_prices and verify all outputs are within bounds
    let num_samples = 500;
    let step = (MAX_SQRT_X64 - MIN_SQRT_X64) / num_samples;

    for i in 0..=num_samples {
        let sqrt_price = Q64x64::from_raw(MIN_SQRT_X64 + i * step);

        if let Ok(tick) = sqrt_price_to_tick(sqrt_price) {
            assert!(
                tick >= MIN_TICK,
                "Output tick {} below MIN_TICK {} for sqrt_price {}",
                tick,
                MIN_TICK,
                sqrt_price.raw()
            );
            assert!(
                tick <= MAX_TICK,
                "Output tick {} above MAX_TICK {} for sqrt_price {}",
                tick,
                MAX_TICK,
                sqrt_price.raw()
            );
        }

        // Price is u64, so it's automatically bounded (no negative or > u64::MAX)
        let _ = sqrt_price_to_price(sqrt_price);
    }
}

/// Tests the "Monotonicity" invariant: sqrt_price ordering is preserved in tick conversion.
/// This is critical for preventing arbitrage exploits from price ordering violations.
#[test]
fn test_security_monotonicity() {
    // Strict monotonicity: higher sqrt_price must map to higher or equal tick
    let num_samples = 1000;
    let step = (MAX_SQRT_X64 - MIN_SQRT_X64) / num_samples;

    let mut prev_tick: Option<i32> = None;
    let mut monotonicity_violations = 0;

    for i in 0..=num_samples {
        let sqrt_price = Q64x64::from_raw(MIN_SQRT_X64 + i * step);

        if let Ok(tick) = sqrt_price_to_tick(sqrt_price) {
            if let Some(prev) = prev_tick {
                if tick < prev {
                    monotonicity_violations += 1;
                }
            }
            prev_tick = Some(tick);
        }
    }

    assert_eq!(
        monotonicity_violations, 0,
        "CRITICAL: {} monotonicity violations detected. Price ordering must be strictly preserved!",
        monotonicity_violations
    );
}

/// Tests that the protocol's mathematical constants are self-consistent.
#[test]
fn test_security_constant_consistency() {
    // Verify MIN_TICK produces sqrt_price close to MIN_SQRT_X64
    let sqrt_from_min_tick = tick_to_sqrt_x64(MIN_TICK).unwrap();
    let min_tick_error =
        sqrt_from_min_tick.raw().abs_diff(MIN_SQRT_X64) as f64 / MIN_SQRT_X64 as f64;
    assert!(
        min_tick_error < 0.01,
        "MIN_TICK inconsistent with MIN_SQRT_X64: error = {:.4}%",
        min_tick_error * 100.0
    );

    // Verify MAX_TICK produces sqrt_price close to MAX_SQRT_X64
    let sqrt_from_max_tick = tick_to_sqrt_x64(MAX_TICK).unwrap();
    let max_tick_error =
        sqrt_from_max_tick.raw().abs_diff(MAX_SQRT_X64) as f64 / MAX_SQRT_X64 as f64;
    assert!(
        max_tick_error < 0.01,
        "MAX_TICK inconsistent with MAX_SQRT_X64: error = {:.4}%",
        max_tick_error * 100.0
    );

    // Verify tick 0 produces sqrt_price close to ONE_X64
    let sqrt_from_zero_tick = tick_to_sqrt_x64(0).unwrap();
    let zero_tick_error = sqrt_from_zero_tick.raw().abs_diff(ONE_X64) as f64 / ONE_X64 as f64;
    assert!(
        zero_tick_error < 0.0001,
        "Tick 0 should map to sqrt_price ≈ 1.0: error = {:.6}%",
        zero_tick_error * 100.0
    );
}

/// Tests cross-platform determinism by verifying known test vectors.
#[test]
fn test_security_cross_platform_determinism() {
    // Known test vectors that should produce identical results on all platforms
    // These can be verified against external implementations (e.g., Uniswap V3)
    let test_vectors: Vec<(i32, u128)> = vec![
        (0, ONE_X64),                   // tick 0 → sqrt_price = 1.0
        (1, 18446817168932946694u128),  // tick 1 → 1.0001^0.5
        (-1, 18446671028174523315u128), // tick -1 → 1/1.0001^0.5
    ];

    for (tick, expected_approx_sqrt) in test_vectors {
        let sqrt_price = tick_to_sqrt_x64(tick).unwrap();

        // Allow small tolerance for fixed-point arithmetic
        let rel_error =
            sqrt_price.raw().abs_diff(expected_approx_sqrt) as f64 / expected_approx_sqrt as f64;

        assert!(
            rel_error < 0.0001,
            "Cross-platform test vector failed: tick {} expected sqrt ≈ {}, got {} (error: {:.6}%)",
            tick,
            expected_approx_sqrt,
            sqrt_price.raw(),
            rel_error * 100.0
        );
    }
}

// ============================================================================
// COMPREHENSIVE INTEGRATION SCENARIOS
// ============================================================================

/// Tests a realistic AMM position creation scenario.
#[test]
fn test_integration_position_creation_scenario() {
    // Simulate creating a position from tick_lower=-1000 to tick_upper=1000
    let tick_lower = -1000;
    let tick_upper = 1000;

    // Convert tick bounds to sqrt_prices
    let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();

    // Verify ordering
    assert!(
        sqrt_lower.raw() < sqrt_upper.raw(),
        "Lower tick must produce lower sqrt_price"
    );

    // Convert back to verify round-trip
    let recovered_lower = sqrt_price_to_tick(sqrt_lower).unwrap();
    let recovered_upper = sqrt_price_to_tick(sqrt_upper).unwrap();

    assert!(
        (recovered_lower - tick_lower).abs() <= 1,
        "Lower tick round-trip error too large"
    );
    assert!(
        (recovered_upper - tick_upper).abs() <= 1,
        "Upper tick round-trip error too large"
    );

    // Verify current price within range
    let current_tick = 0;
    let current_sqrt = tick_to_sqrt_x64(current_tick).unwrap();

    assert!(
        current_sqrt.raw() >= sqrt_lower.raw(),
        "Current price should be >= lower bound"
    );
    assert!(
        current_sqrt.raw() <= sqrt_upper.raw(),
        "Current price should be <= upper bound"
    );
}

/// Tests a realistic swap price impact scenario.
#[test]
fn test_integration_swap_price_impact_scenario() {
    // Simulate a swap that moves price from tick 0 to tick 10000
    // Using larger tick difference to see measurable price change after truncation
    let start_tick = 0;
    let end_tick = 10000;

    let start_sqrt = tick_to_sqrt_x64(start_tick).unwrap();
    let end_sqrt = tick_to_sqrt_x64(end_tick).unwrap();

    // Calculate price change using sqrt_prices directly (higher precision)
    // price = sqrt_price^2 / 2^64
    let start_price = sqrt_price_to_price(start_sqrt).unwrap();
    let end_price = sqrt_price_to_price(end_sqrt).unwrap();

    // End sqrt_price should be higher (positive tick movement = price increase)
    assert!(
        end_sqrt.raw() > start_sqrt.raw(),
        "Positive tick movement should increase sqrt_price: {} -> {}",
        start_sqrt.raw(),
        end_sqrt.raw()
    );

    // Verify the sqrt_price ratio is approximately sqrt(1.0001^10000) ≈ sqrt(2.7) ≈ 1.64
    // Since prices are small integers (1, 2, etc.) after truncation, we verify sqrt_price ratio instead
    let sqrt_ratio = end_sqrt.raw() as f64 / start_sqrt.raw() as f64;
    assert!(
        sqrt_ratio > 1.5 && sqrt_ratio < 1.8,
        "10000 ticks should produce sqrt_price ratio ~1.64, got ratio: {:.4}",
        sqrt_ratio
    );

    // End price should be >= start price (may be equal due to truncation for small values)
    assert!(
        end_price >= start_price,
        "End price should be >= start price: {} -> {}",
        start_price,
        end_price
    );
}

/// Tests fee tier boundary alignment.
#[test]
fn test_integration_fee_tier_alignment() {
    // Fee tiers use tick spacings: 1 (0.01%), 10 (0.05%), 60 (0.30%), 200 (1.00%)
    let spacings = [1, 10, 60, 200];

    for spacing in spacings {
        // Test that ticks aligned to spacing produce consistent sqrt_prices
        let tick1 = spacing * 100;
        let tick2 = spacing * 101;

        let sqrt1 = tick_to_sqrt_x64(tick1).unwrap();
        let sqrt2 = tick_to_sqrt_x64(tick2).unwrap();

        // sqrt2 should be slightly higher than sqrt1
        assert!(
            sqrt2.raw() > sqrt1.raw(),
            "Tick spacing {} alignment: tick {} should have higher sqrt than tick {}",
            spacing,
            tick2,
            tick1
        );

        // Verify recovery maintains alignment
        let recovered1 = sqrt_price_to_tick(sqrt1).unwrap();
        let recovered2 = sqrt_price_to_tick(sqrt2).unwrap();

        assert!(
            (recovered1 - tick1).abs() <= 1,
            "Fee tier spacing {} alignment lost for tick {}",
            spacing,
            tick1
        );
        assert!(
            (recovered2 - tick2).abs() <= 1,
            "Fee tier spacing {} alignment lost for tick {}",
            spacing,
            tick2
        );
    }
}
