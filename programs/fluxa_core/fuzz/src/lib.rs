use ethnum::U256;

pub const REL_PPB_STRICT: u128 = 1; // 1e-9
pub const REL_PPB_SQUARED: u128 = 10; // 1e-8
pub const ULP_TIGHT: u128 = 2;
pub const ULP_SAFE: u128 = 4;
pub const ULP_SQUARED_STRICT: u128 = 20;

#[inline]
pub fn assert_rel_close_raw(actual: u128, expected: u128, rel_ppb: u128, ulps: u128, context: &str) {
    if actual == expected {
        return;
    }

    let ulp_diff = actual.abs_diff(expected);
    if ulp_diff <= ulps {
        return;
    }

    if expected == 0 {
        panic!(
            "Precision assertion failed in {}: actual={:032x}, expected={:032x}, ulp_diff={}, rel_ppb_limit={}, expected_raw=0",
            context, actual, expected, ulp_diff, rel_ppb
        );
    }

    let num = U256::from(ulp_diff) * U256::from(1_000_000_000u128);
    let den = U256::from(expected);
    let rel_error_ppb = (num / den).as_u128();

    if rel_error_ppb <= rel_ppb {
        return;
    }

    panic!(
        "Precision assertion failed in {}: actual={:032x}, expected={:032x}, ulp_diff={}, rel_ppb_limit={}, actual_rel_ppb={}",
        context, actual, expected, ulp_diff, rel_ppb, rel_error_ppb
    );
}

#[inline]
pub fn within_ulp(actual: u128, expected: u128, ulps: u128) -> bool {
    actual.abs_diff(expected) <= ulps
}
