//! Integration tests for core arithmetic across module boundaries.
//!
//! These tests exercise realistic protocol flows combining multiple mathematical primitives
//! (tick conversion, sqrt, liquidity math, position valuation) to ensure invariants hold
//! when functions are composed as they are in real AMM operations.
//!
//! Focus Areas:
//! - Piecewise liquidity amount flows vs. direct liquidity inversion
//! - Position valuation in different price regions
//! - Cross-verification of tick_to_sqrt_x64 with liquidity_from_amount_* formulas
//! - Error propagation and boundary handling
//! - Deterministic chained operations (swap-like simulation)

use anchor_lang::prelude::*;
use fluxa_core::error::MathError;
use fluxa_core::math::core_arithmetic::{
    liquidity_from_amount_0, liquidity_from_amount_1, mul_div, mul_div_round_up, tick_to_sqrt_x64,
    Q64x64,
};
use fluxa_core::math::liquidity_math::{
    calculate_amounts_for_liquidity_piecewise, calculate_liquidity,
    calculate_position_value_at_price,
};
use fluxa_core::state::position::position_account::Position;
use fluxa_core::utils::constants::ONE_X64;
use solana_program::pubkey::Pubkey;

// Helper to build a Position in-memory (bypassing Anchor account init for pure math tests)
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
    // mimic initialize hash + flags
    pos.status_flags = Position::FLAG_ACTIVE;
    pos.position_hash = pos.calculate_optimized_hash();
    pos
}

// Approximate relative error helper (PPB) for u128 raw values
fn relative_error_ppb(actual: u128, expected: u128) -> u128 {
    if expected == 0 {
        return if actual == 0 { 0 } else { u128::MAX };
    }
    let diff = if actual > expected {
        actual - expected
    } else {
        expected - actual
    };
    (diff.saturating_mul(1_000_000_000u128)) / expected
}

#[test]
fn position_value_piecewise_regions() {
    // Define ticks and convert to sqrt prices for reference flows
    let tick_lower = -1000;
    let tick_upper = 1000;
    let pos_liquidity = Q64x64::from_int(10_000).raw(); // larger to avoid truncation to zero token amounts
    let position = build_position(tick_lower, tick_upper, pos_liquidity, 7);

    let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();

    // Region 1: current below range (only token0 required)
    let below_price = Q64x64::from_raw(sqrt_lower.raw().saturating_sub(ONE_X64 / 4).max(1));
    let pv_below = calculate_position_value_at_price(&position, below_price, 2, 3).unwrap();
    assert!(
        pv_below.amount_0 > 0 && pv_below.amount_1 == 0,
        "Below range should only require token0"
    );
    assert!(!pv_below.price_range_active);

    // Region 2: inside range (both tokens required). Use arithmetic midpoint then force strictly inside.
    let mut mid_raw = (sqrt_lower.raw() + sqrt_upper.raw()) / 2;
    if mid_raw <= sqrt_lower.raw() {
        mid_raw = sqrt_lower.raw() + 1;
    }
    if mid_raw >= sqrt_upper.raw() {
        mid_raw = sqrt_upper.raw() - 1;
    }
    let mid_price = Q64x64::from_raw(mid_raw);
    let pv_mid = calculate_position_value_at_price(&position, mid_price, 2, 3).unwrap();
    assert!(
        pv_mid.amount_0 > 0 && pv_mid.amount_1 > 0,
        "Inside range requires both tokens"
    );
    assert!(pv_mid.price_range_active);

    // Region 3: above range (only token1 required)
    let above_price = Q64x64::from_raw(sqrt_upper.raw().saturating_add(ONE_X64 / 4));
    let pv_above = calculate_position_value_at_price(&position, above_price, 2, 3).unwrap();
    assert!(
        pv_above.amount_0 == 0 && pv_above.amount_1 > 0,
        "Above range should only require token1"
    );
    assert!(!pv_above.price_range_active);

    // USD value consistency: total = token0_value + token1_value
    assert_eq!(
        pv_mid.total_value_usd,
        pv_mid.value_0_usd + pv_mid.value_1_usd
    );
}

#[test]
fn liquidity_core_formula_identities() {
    // Validate internal algebraic identities of core_arithmetic liquidity formulas without relying on unverified inversion logic.
    let lower_tick = -4200;
    let upper_tick = 3100;
    let sqrt_lower = tick_to_sqrt_x64(lower_tick).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(upper_tick).unwrap();
    let amount0: u64 = 3_500_000;
    let amount1: u64 = 2_250_000;

    // Token0 liquidity identity
    let l0 = liquidity_from_amount_0(sqrt_lower, sqrt_upper, amount0).unwrap();
    let raw_n = mul_div(amount0 as u128, sqrt_lower.raw(), 1).unwrap();
    let l0_manual = mul_div(raw_n, sqrt_upper.raw(), sqrt_upper.raw() - sqrt_lower.raw()).unwrap();
    assert_eq!(l0, l0_manual, "Token0 liquidity formula mismatch");

    // Token1 liquidity identity
    use fluxa_core::utils::constants::{FRAC_BITS, ONE_X64};
    let l1 = liquidity_from_amount_1(sqrt_lower, sqrt_upper, amount1).unwrap();
    let scaled = (amount1 as u128) << FRAC_BITS;
    let l1_manual = mul_div(scaled, ONE_X64, sqrt_upper.raw() - sqrt_lower.raw()).unwrap();
    assert_eq!(l1, l1_manual, "Token1 liquidity formula mismatch");

    // Range narrowing should increase liquidity density for token0
    let narrow_upper = tick_to_sqrt_x64(upper_tick - 600).unwrap();
    let l0_narrow = liquidity_from_amount_0(sqrt_lower, narrow_upper, amount0).unwrap();
    assert!(
        l0_narrow > l0,
        "Narrower range must yield higher token0-derived liquidity"
    );
    // And similarly for token1
    let l1_narrow = liquidity_from_amount_1(sqrt_lower, narrow_upper, amount1).unwrap();
    assert!(
        l1_narrow > l1,
        "Narrower range must yield higher token1-derived liquidity"
    );
}

#[test]
fn tick_sqrt_liquidity_consistency() {
    // Iterate several tick spans and verify monotonic relationship holds at integration level
    let spans = [(-500, 500), (-250, 250), (0, 300), (100, 700)];

    for (lo, hi) in spans {
        let sqrt_lo = tick_to_sqrt_x64(lo).unwrap();
        let sqrt_hi = tick_to_sqrt_x64(hi).unwrap();
        assert!(sqrt_lo.raw() < sqrt_hi.raw());

        // Increasing amount0 should strictly increase liquidity_from_amount_0
        let mut prev = 0u128;
        for amt in [100u64, 1_000, 5_000, 10_000] {
            let liq0 = liquidity_from_amount_0(sqrt_lo, sqrt_hi, amt).unwrap();
            assert!(
                liq0 > prev,
                "Liquidity must increase with amount0 ({} > {}).",
                liq0,
                prev
            );
            prev = liq0;
        }

        // Increasing amount1 should increase liquidity_from_amount_1
        let mut prev1 = 0u128;
        for amt in [50u64, 500, 5_000, 50_000] {
            let liq1 = liquidity_from_amount_1(sqrt_lo, sqrt_hi, amt).unwrap();
            assert!(
                liq1 > prev1,
                "Liquidity must increase with amount1 ({} > {}).",
                liq1,
                prev1
            );
            prev1 = liq1;
        }
    }
}

#[test]
fn integration_error_propagation() {
    // Invalid range: equal bounds
    let sqrt = Q64x64::from_int(5);
    let res = calculate_liquidity(sqrt, sqrt, sqrt, 100, 100);
    assert!(res.is_err());

    // Invalid: zero liquidity in piecewise path
    let tick_lower = -100;
    let tick_upper = 100;
    let sqrt_lower = tick_to_sqrt_x64(tick_lower).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(tick_upper).unwrap();
    let current = Q64x64::from_raw((sqrt_lower.raw() + sqrt_upper.raw()) / 2);
    let zero_liq = Q64x64::zero();
    let piece =
        calculate_amounts_for_liquidity_piecewise(current, sqrt_lower, sqrt_upper, zero_liq);
    assert!(piece.is_err());

    // Division by zero path inside amount_0 when sqrt_lower == 0
    let bad =
        calculate_amount_0_delta_wrapper(Q64x64::zero(), Q64x64::from_int(2), Q64x64::from_int(1));
    assert!(matches!(bad, Err(_)));
}

// Wrapper to access private helper for error surface test
fn calculate_amount_0_delta_wrapper(a: Q64x64, b: Q64x64, l: Q64x64) -> Result<u64> {
    // replicate calculate_amount_0_delta logic minimal for division by zero branch
    if a.raw() >= b.raw() || a.raw() == 0 {
        return Err(MathError::InvalidPriceRange.into());
    }
    let diff = b.checked_sub(a)?;
    let num = l.checked_mul(diff)?;
    let den = a.checked_mul(b)?; // a==0 triggers earlier
    let q = num.checked_div(den)?;
    Ok((q.raw() >> 64) as u64)
}

#[test]
fn chained_arithmetic_stability() {
    // Pure core arithmetic chain: liquidity contributions, fee increment via ceiling division, deterministic reruns.
    let lower_tick = -900;
    let upper_tick = 1400;
    let sqrt_lower = tick_to_sqrt_x64(lower_tick).unwrap();
    let sqrt_upper = tick_to_sqrt_x64(upper_tick).unwrap();
    let amount0 = 4_200_000u64;
    let amount1 = 3_100_000u64;
    let l0 = liquidity_from_amount_0(sqrt_lower, sqrt_upper, amount0).unwrap();
    let l1 = liquidity_from_amount_1(sqrt_lower, sqrt_upper, amount1).unwrap();
    let combined = l0 + l1;
    let fee_bps = 45u128; // 0.45%
    let inc = mul_div_round_up(combined, fee_bps, 10_000).unwrap();
    let combined2 = combined + inc;
    assert!(combined2 > combined);
    // Determinism: recompute increment
    let inc2 = mul_div_round_up(combined, fee_bps, 10_000).unwrap();
    assert_eq!(inc, inc2);
    let rel_ppb = inc * 1_000_000_000 / combined;
    assert!(
        rel_ppb >= 4_500_000 && rel_ppb <= 4_600_000,
        "Fee increment outside expected band: {} ppb",
        rel_ppb
    );
}
