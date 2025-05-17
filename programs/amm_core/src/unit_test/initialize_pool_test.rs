use crate::constants::MAX_SQRT_PRICE; // Assuming MIN_SQRT_PRICE is handled or not strictly needed for these checks
use crate::errors::ErrorCode;
use crate::math;
use crate::state::pool::{InitializePoolParams, Pool};
use anchor_lang::prelude::*; // For sqrt_price_q64_to_tick
use std::collections::BTreeMap;

// Q64.64 fixed-point constants for readability, similar to math_test.rs
const Q64_ONE: u128 = 1u128 << 64; // 1.0
const Q64_TWO: u128 = 2u128 << 64; // 2.0
const Q64_HALF: u128 = 1u128 << 63; // 0.5

/// Helper to create unique Pubkeys for testing.
fn new_pubkey(val: u8) -> Pubkey {
    let mut arr = [0u8; 32];
    arr[0] = val;
    Pubkey::new_from_array(arr)
}

mod pool_initialize_tests {
    use super::*;

    // Helper function to get default InitializePoolParams for tests
    fn get_default_params() -> InitializePoolParams {
        InitializePoolParams {
            bump: 255,
            factory: new_pubkey(1),
            token0_mint: new_pubkey(2), // Typically mint_a (smaller key)
            token1_mint: new_pubkey(3), // Typically mint_b (larger key)
            token0_vault: new_pubkey(4),
            token1_vault: new_pubkey(5),
            initial_sqrt_price_q64: Q64_ONE, // Corresponds to price 1.0
            fee_rate: 30,                    // e.g., 0.3%
            tick_spacing: 60,
        }
    }

    #[test]
    fn test_pool_initialize_success() {
        let mut pool = Pool::default();
        let params = get_default_params();
        let expected_tick = math::sqrt_price_q64_to_tick(params.initial_sqrt_price_q64)
            .expect("Tick calculation failed for default params");

        let result = pool.initialize(params);
        assert!(result.is_ok(), "Initialization failed: {:?}", result.err());

        assert_eq!(pool.bump, 255);
        assert_eq!(pool.factory, new_pubkey(1));
        assert_eq!(pool.token0_mint, new_pubkey(2));
        assert_eq!(pool.token1_mint, new_pubkey(3));
        assert_eq!(pool.token0_vault, new_pubkey(4));
        assert_eq!(pool.token1_vault, new_pubkey(5));
        assert_eq!(pool.fee_rate, 30);
        assert_eq!(pool.tick_spacing, 60);
        assert_eq!(pool.sqrt_price_q64, Q64_ONE);
        assert_eq!(pool.current_tick, expected_tick);
        assert_eq!(pool.liquidity, 0);
        let deserialized_bitmap: BTreeMap<i16, u64> =
            borsh::BorshDeserialize::try_from_slice(&pool.tick_bitmap_data)
                .expect("Deserialization failed");
        assert!(deserialized_bitmap.is_empty());
    }

    #[test]
    fn test_pool_initialize_error_mints_must_differ() {
        let mut pool = Pool::default();
        let mut params = get_default_params();
        params.token1_mint = params.token0_mint; // Make mints the same

        let result = pool.initialize(params);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), ErrorCode::MintsMustDiffer.into());
    }

    #[test]
    fn test_pool_initialize_error_invalid_initial_price_zero() {
        let mut pool = Pool::default();
        let mut params = get_default_params();
        params.initial_sqrt_price_q64 = 0;

        let result = pool.initialize(params);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), ErrorCode::InvalidInitialPrice.into());
    }

    #[test]
    fn test_pool_initialize_error_invalid_initial_price_too_high() {
        let mut pool = Pool::default();
        let mut params = get_default_params();
        params.initial_sqrt_price_q64 = MAX_SQRT_PRICE + 1;

        let result = pool.initialize(params);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), ErrorCode::InvalidInitialPrice.into());
    }

    #[test]
    fn test_pool_initialize_valid_boundary_prices() {
        // Test with a very small, valid sqrt_price (e.g., 1, which is > 0)
        let mut pool_min_price = Pool::default();
        let mut params_min_price = get_default_params();
        params_min_price.initial_sqrt_price_q64 = 1; // Smallest non-zero u128
        let result_min = pool_min_price.initialize(params_min_price);
        assert!(
            result_min.is_ok(),
            "Expected OK for minimal valid price (1), got {:?}",
            result_min.err()
        );
        let expected_tick_min = math::sqrt_price_q64_to_tick(1).unwrap();
        assert_eq!(pool_min_price.current_tick, expected_tick_min);

        // Test with MAX_SQRT_PRICE
        let mut pool_max_price = Pool::default();
        let mut params_max_price = get_default_params();
        params_max_price.initial_sqrt_price_q64 = MAX_SQRT_PRICE;
        let result_max = pool_max_price.initialize(params_max_price);
        assert!(
            result_max.is_ok(),
            "Expected OK for MAX_SQRT_PRICE, got {:?}",
            result_max.err()
        );
        let expected_tick_max = math::sqrt_price_q64_to_tick(MAX_SQRT_PRICE).unwrap();
        assert_eq!(pool_max_price.current_tick, expected_tick_max);
    }

    #[test]
    fn test_pool_initialize_error_invalid_tick_spacing_zero() {
        let mut pool = Pool::default();
        let mut params = get_default_params();
        params.tick_spacing = 0;

        let result = pool.initialize(params);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), ErrorCode::InvalidTickSpacing.into());
    }

    #[test]
    fn test_pool_initialize_current_tick_calculation() {
        let mut pool = Pool::default();
        let mut params = get_default_params();

        // Example 1: sqrt_price = 2.0 (price = 4.0)
        params.initial_sqrt_price_q64 = Q64_TWO;
        let expected_tick_price_4 = math::sqrt_price_q64_to_tick(Q64_TWO).unwrap();
        assert!(pool.initialize(params.clone()).is_ok()); // Use clone if params is modified or reused
        assert_eq!(pool.current_tick, expected_tick_price_4);

        // Example 2: sqrt_price = 0.5 (price = 0.25)
        let mut pool2 = Pool::default(); // fresh pool instance
        params.initial_sqrt_price_q64 = Q64_HALF;
        let expected_tick_price_0_25 = math::sqrt_price_q64_to_tick(Q64_HALF).unwrap();
        assert!(pool2.initialize(params).is_ok());
        assert_eq!(pool2.current_tick, expected_tick_price_0_25);
    }
}
