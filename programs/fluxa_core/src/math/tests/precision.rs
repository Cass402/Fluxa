use crate::math::core_arithmetic::Q64x64;
use ethnum::U256;

/// Relative error tolerance in parts per billion (PPB) for strict mathematical operations.
/// Used for functions requiring maximum precision like tick-to-price conversions.
pub const REL_PPB_STRICT: u128 = 1; // 1e-9 relative error

/// Relative error tolerance for squared-back verification checks.
/// Accounts for error amplification in operations like sqrt(x)² ≈ x.
pub const REL_PPB_SQUARED: u128 = 10; // 1e-8 relative error

/// Tight ULP (Unit in Last Place) tolerance for high-precision operations.
/// Used when mathematical result should be very close to expected value.
pub const ULP_TIGHT: u128 = 2; // 2 raw Q64.64 ULPs

/// Safe ULP tolerance for operations in challenging numerical domains.
/// Provides margin for operations near boundaries or with complex calculations.
pub const ULP_SAFE: u128 = 4; // 4 ULPs

/// Asserts that two Q64x64 values are approximately equal within specified tolerances.
///
/// The assertion passes if EITHER condition is met:
/// 1. Relative error ≤ `rel_ppb`
/// 2. Absolute ULP difference ≤ `ulps`
///
/// This dual-tolerance approach handles the full Q64.64 range correctly.
pub fn assert_rel_close(
    actual: Q64x64,
    expected: Q64x64,
    rel_ppb: u128,
    ulps: u128,
    context: &str,
) {
    let actual_raw = actual.raw();
    let expected_raw = expected.raw();

    if actual_raw == expected_raw {
        return;
    }

    let ulp_diff = actual_raw.abs_diff(expected_raw);

    if ulp_diff <= ulps {
        return;
    }

    if expected_raw > 0 {
        let num = U256::from(ulp_diff) * U256::from(1_000_000_000u128);
        let den = U256::from(expected_raw);
        let rel_error_ppb = (num / den).as_u128();

        if rel_error_ppb <= rel_ppb {
            return;
        }

        panic!(
            "Precision assertion failed in {}: actual={:032x}, expected={:032x}, ulp_diff={}, rel_ppb_limit={}, actual_rel_ppb={}",
            context, actual_raw, expected_raw, ulp_diff, rel_ppb, rel_error_ppb
        );
    }

    panic!(
        "Precision assertion failed in {}: actual={:032x}, expected={:032x}, ulp_diff={}, rel_ppb_limit={}, expected_raw=0 (no relative error calculation)",
        context, actual_raw, expected_raw, ulp_diff, rel_ppb
    );
}
