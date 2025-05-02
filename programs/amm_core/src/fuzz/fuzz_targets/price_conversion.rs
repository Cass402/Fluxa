#![no_main]

use amm_core::constants::MIN_SQRT_PRICE;
use amm_core::math::*;
use libfuzzer_sys::fuzz_target;

// Fuzz test for price to sqrt price conversion
//
// This test ensures that:
// 1. When price_to_sqrt_price succeeds, the resulting sqrt_price is within valid bounds
// 2. The sqrt_price is not unexpectedly large (exceeds 2^96)
// 3. The sqrt_price is not too small (below MIN_SQRT_PRICE), unless price is zero
fuzz_target!(|price: u64| {
    // Test price to sqrt price conversion
    if let Ok(sqrt_price) = price_to_sqrt_price(price) {
        // If conversion succeeds, ensure it's a reasonable value
        assert!(sqrt_price <= (1u128 << 96), "sqrt_price unexpectedly large");
        assert!(
            sqrt_price >= MIN_SQRT_PRICE || price == 0,
            "sqrt_price too small"
        );
    }
});
