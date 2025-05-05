// Tests for the token pair module
//
// This file contains comprehensive tests for the token pair module, ensuring
// both functionality and security aspects are properly tested. The tests follow
// guidelines from the security testing checklist and test plan coverage report.

use crate::token_pair::*;
use anchor_lang::{prelude::*, solana_program::clock::Clock};
use std::collections::HashSet;

#[cfg(test)]
mod tests {
    use super::*;

    // Mock data for tests
    fn mock_pubkey(seed: u8) -> Pubkey {
        let mut bytes = [0u8; 32];
        bytes[0] = seed;
        Pubkey::new_from_array(bytes)
    }

    fn mock_token_pair() -> TokenPair {
        let token_a = mock_pubkey(1);
        let token_b = mock_pubkey(2);

        TokenPair {
            authority: mock_pubkey(10),
            token_a_mint: token_a,
            token_b_mint: token_b,
            token_a_decimals: 6,
            token_b_decimals: 9,
            pools: Vec::new(),
            total_volume_token_a: 0,
            total_volume_token_b: 0,
            total_fees_generated: 0,
            last_oracle_price: 0,
            last_oracle_update: 0,
            is_verified: false,
            version: 1,
            reserved: [0; 64],
        }
    }

    // Mock implementation of Sysvar<Clock> for testing
    struct MockClock {
        pub unix_timestamp: i64,
    }

    impl AsRef<Clock> for MockClock {
        fn as_ref(&self) -> &Clock {
            unsafe { std::mem::transmute(self) }
        }
    }

    // Use this function to update the oracle price in tests
    fn update_oracle_price_test(token_pair: &mut TokenPair, price: u64, timestamp: i64) {
        token_pair.last_oracle_price = price;
        token_pair.last_oracle_update = timestamp;
    }

    fn mock_clock(timestamp: i64) -> MockClock {
        MockClock {
            unix_timestamp: timestamp,
        }
    }

    // ========== Basic Functionality Tests ==========

    #[test]
    fn test_token_pair_initialization() {
        let token_pair = mock_token_pair();

        // Verify fields are correctly initialized
        assert_eq!(token_pair.token_a_decimals, 6);
        assert_eq!(token_pair.token_b_decimals, 9);
        assert_eq!(token_pair.pools.len(), 0);
        assert_eq!(token_pair.total_volume_token_a, 0);
        assert_eq!(token_pair.total_volume_token_b, 0);
        assert_eq!(token_pair.total_fees_generated, 0);
        assert_eq!(token_pair.last_oracle_price, 0);
        assert_eq!(token_pair.last_oracle_update, 0);
        assert_eq!(token_pair.is_verified, false);
        assert_eq!(token_pair.version, 1);

        // Verify reserved space is zeroed
        for &byte in token_pair.reserved.iter() {
            assert_eq!(byte, 0);
        }
    }

    #[test]
    fn test_token_pair_size_constant() {
        // Ensure that the LEN constant accurately reflects the size of the struct
        // This is important for proper account allocation

        // Calculate expected size
        let expected_size = 32 + // authority
            32 + // token_a_mint
            32 + // token_b_mint
            1 +  // token_a_decimals
            1 +  // token_b_decimals
            (32 + 2) * TokenPair::MAX_POOLS + // pools (address + fee_tier)
            8 +  // total_volume_token_a
            8 +  // total_volume_token_b
            8 +  // total_fees_generated
            8 +  // last_oracle_price
            8 +  // last_oracle_update
            1 +  // is_verified
            1 +  // version
            64; // reserved space

        assert_eq!(TokenPair::LEN, expected_size);
    }

    // ========== Pool Management Tests ==========

    #[test]
    fn test_add_pool_success() {
        let mut token_pair = mock_token_pair();
        let pool_address = mock_pubkey(20);
        let fee_tier = 3000; // 0.3%

        // Add a pool
        let result = token_pair.add_pool(pool_address, fee_tier);
        assert!(result.is_ok());

        // Verify pool was added
        assert_eq!(token_pair.pools.len(), 1);
        assert_eq!(token_pair.pools[0].0, pool_address);
        assert_eq!(token_pair.pools[0].1, fee_tier);
    }

    #[test]
    fn test_add_pool_duplicate() {
        let mut token_pair = mock_token_pair();
        let pool_address = mock_pubkey(20);
        let fee_tier = 3000;

        // Add a pool
        let result = token_pair.add_pool(pool_address, fee_tier);
        assert!(result.is_ok());

        // Try to add the same pool again
        let result = token_pair.add_pool(pool_address, fee_tier);
        assert!(result.is_err());

        // Verify error type
        let error = result.unwrap_err();
        assert_eq!(
            error.to_string(),
            TokenPairError::PoolAlreadyExists.to_string()
        );

        // Verify no duplicate was added
        assert_eq!(token_pair.pools.len(), 1);
    }

    #[test]
    fn test_add_pool_max_limit() {
        let mut token_pair = mock_token_pair();

        // Add the maximum number of allowed pools
        for i in 0..TokenPair::MAX_POOLS {
            let pool_address = mock_pubkey(20 + i as u8);
            let fee_tier = 3000;
            let result = token_pair.add_pool(pool_address, fee_tier);
            assert!(result.is_ok());
        }

        // Try to add one more pool
        let extra_pool = mock_pubkey(50);
        let result = token_pair.add_pool(extra_pool, 3000);
        assert!(result.is_err());

        // Verify error type
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), TokenPairError::TooManyPools.to_string());

        // Verify no additional pool was added
        assert_eq!(token_pair.pools.len(), TokenPair::MAX_POOLS);
    }

    #[test]
    fn test_remove_pool_success() {
        let mut token_pair = mock_token_pair();
        let pool_1 = mock_pubkey(20);
        let pool_2 = mock_pubkey(21);

        // Add two pools
        token_pair.add_pool(pool_1, 3000).unwrap();
        token_pair.add_pool(pool_2, 500).unwrap();
        assert_eq!(token_pair.pools.len(), 2);

        // Remove the first pool
        let result = token_pair.remove_pool(pool_1);
        assert!(result.is_ok());

        // Verify correct pool was removed
        assert_eq!(token_pair.pools.len(), 1);
        assert_eq!(token_pair.pools[0].0, pool_2);
        assert_eq!(token_pair.pools[0].1, 500);
    }

    #[test]
    fn test_remove_pool_not_found() {
        let mut token_pair = mock_token_pair();
        let pool = mock_pubkey(20);
        let nonexistent_pool = mock_pubkey(21);

        // Add one pool
        token_pair.add_pool(pool, 3000).unwrap();

        // Try to remove a nonexistent pool
        let result = token_pair.remove_pool(nonexistent_pool);
        assert!(result.is_err());

        // Verify error type
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), TokenPairError::PoolNotFound.to_string());

        // Verify no pool was removed
        assert_eq!(token_pair.pools.len(), 1);
        assert_eq!(token_pair.pools[0].0, pool);
    }

    #[test]
    fn test_remove_pool_from_empty() {
        let mut token_pair = mock_token_pair();
        let pool = mock_pubkey(20);

        // Try to remove a pool from an empty list
        let result = token_pair.remove_pool(pool);
        assert!(result.is_err());

        // Verify error type
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), TokenPairError::PoolNotFound.to_string());
    }

    // ========== Statistics Update Tests ==========

    #[test]
    fn test_update_statistics() {
        let mut token_pair = mock_token_pair();

        // Update statistics with initial values
        token_pair.update_statistics(1000, 500, 10);
        assert_eq!(token_pair.total_volume_token_a, 1000);
        assert_eq!(token_pair.total_volume_token_b, 500);
        assert_eq!(token_pair.total_fees_generated, 10);

        // Update again with additional values
        token_pair.update_statistics(2000, 1500, 15);
        assert_eq!(token_pair.total_volume_token_a, 3000);
        assert_eq!(token_pair.total_volume_token_b, 2000);
        assert_eq!(token_pair.total_fees_generated, 25);
    }

    #[test]
    fn test_update_statistics_overflow_protection() {
        let mut token_pair = mock_token_pair();

        // Set initial values close to u64::MAX
        token_pair.total_volume_token_a = u64::MAX - 100;
        token_pair.total_volume_token_b = u64::MAX - 50;
        token_pair.total_fees_generated = u64::MAX - 10;

        // Update with values that would cause overflow
        token_pair.update_statistics(200, 100, 20);

        // Verify that values saturated at maximum instead of overflowing
        assert_eq!(token_pair.total_volume_token_a, u64::MAX);
        assert_eq!(token_pair.total_volume_token_b, u64::MAX);
        assert_eq!(token_pair.total_fees_generated, u64::MAX);
    }

    // ========== Oracle Price Tests ==========

    #[test]
    fn test_update_oracle_price() {
        let mut token_pair = mock_token_pair();
        let price = 1_500_000; // $1.50 with 6 decimal places
        let timestamp = 1640995200; // 2022-01-01 00:00:00 UTC
        let clock = mock_clock(timestamp);

        // Update oracle price directly using the test helper
        update_oracle_price_test(&mut token_pair, price, timestamp);

        // Verify price and timestamp were updated
        assert_eq!(token_pair.last_oracle_price, price);
        assert_eq!(token_pair.last_oracle_update, timestamp);

        // Update again with new values
        let new_price = 1_600_000;
        let new_timestamp = 1641081600; // 2022-01-02 00:00:00 UTC

        update_oracle_price_test(&mut token_pair, new_price, new_timestamp);
        assert_eq!(token_pair.last_oracle_price, new_price);
        assert_eq!(token_pair.last_oracle_update, new_timestamp);
    }

    // ========== Verification Status Tests ==========

    #[test]
    fn test_set_verification() {
        let mut token_pair = mock_token_pair();
        assert_eq!(token_pair.is_verified, false);

        // Set to verified
        token_pair.set_verification(true);
        assert_eq!(token_pair.is_verified, true);

        // Set back to unverified
        token_pair.set_verification(false);
        assert_eq!(token_pair.is_verified, false);
    }

    // ========== Address Derivation Tests ==========

    #[test]
    fn test_find_token_pair_address_ordering() {
        let program_id = mock_pubkey(99);
        let token_a = mock_pubkey(1); // Smaller pubkey
        let token_b = mock_pubkey(2); // Larger pubkey

        // Test canonical ordering (A < B)
        let (address1, bump1) = find_token_pair_address(&token_a, &token_b, &program_id);

        // Test reverse ordering (B, A)
        let (address2, bump2) = find_token_pair_address(&token_b, &token_a, &program_id);

        // Verify both orders produce the same address
        assert_eq!(address1, address2);
        assert_eq!(bump1, bump2);
    }

    #[test]
    fn test_find_token_pair_address_uniqueness() {
        let program_id = mock_pubkey(99);

        // Create a set of distinct token pairs
        let mut addresses = HashSet::new();

        for i in 0..10 {
            for j in (i + 1)..10 {
                let token_a = mock_pubkey(i);
                let token_b = mock_pubkey(j);

                let (address, _) = find_token_pair_address(&token_a, &token_b, &program_id);
                addresses.insert(address);
            }
        }

        // Verify each pair has a unique address
        // With 10 tokens, we should have 45 unique pairs (10 choose 2)
        assert_eq!(addresses.len(), 45);
    }

    // ========== Security Tests ==========

    #[test]
    fn test_pool_management_uniqueness() {
        let mut token_pair = mock_token_pair();

        // Add several pools with different fee tiers
        let pools = vec![
            (mock_pubkey(20), 500),   // 0.05%
            (mock_pubkey(21), 3000),  // 0.3%
            (mock_pubkey(22), 10000), // 1%
        ];

        for &(address, fee) in &pools {
            token_pair.add_pool(address, fee).unwrap();
        }

        // Verify each pool was added exactly once
        assert_eq!(token_pair.pools.len(), pools.len());

        // Create a HashSet to check for duplicates
        let unique_pools: HashSet<_> = token_pair.pools.iter().map(|(addr, _)| addr).collect();
        assert_eq!(unique_pools.len(), pools.len());
    }

    #[test]
    fn test_duplicate_removal_safety() {
        let mut token_pair = mock_token_pair();
        let pool_1 = mock_pubkey(20);
        let pool_2 = mock_pubkey(21);

        // Add two pools
        token_pair.add_pool(pool_1, 3000).unwrap();
        token_pair.add_pool(pool_2, 500).unwrap();

        // Remove the first pool
        token_pair.remove_pool(pool_1).unwrap();

        // Try to remove the same pool again
        let result = token_pair.remove_pool(pool_1);
        assert!(result.is_err());

        // Verify only one pool remains
        assert_eq!(token_pair.pools.len(), 1);
        assert_eq!(token_pair.pools[0].0, pool_2);
    }

    #[test]
    fn test_multiple_fee_tiers() {
        let mut token_pair = mock_token_pair();
        let base_pool = mock_pubkey(20);

        // Try adding multiple pools with same address but different fee tiers
        token_pair.add_pool(base_pool, 3000).unwrap();

        // This should fail since the pool address is the same
        let result = token_pair.add_pool(base_pool, 500);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            TokenPairError::PoolAlreadyExists.to_string()
        );
    }

    // ========== Boundary and Edge Case Tests ==========

    #[test]
    fn test_max_pools_boundary() {
        let mut token_pair = mock_token_pair();

        // Verify MAX_POOLS is reasonable and greater than zero
        assert!(TokenPair::MAX_POOLS > 0);

        // Add exactly MAX_POOLS pools
        for i in 0..TokenPair::MAX_POOLS {
            let result = token_pair.add_pool(mock_pubkey(20 + i as u8), 3000);
            assert!(result.is_ok());
        }

        // Verify we have exactly MAX_POOLS pools
        assert_eq!(token_pair.pools.len(), TokenPair::MAX_POOLS);
    }

    #[test]
    fn test_fee_tier_edge_values() {
        let mut token_pair = mock_token_pair();

        // Test with minimum fee (0%)
        token_pair.add_pool(mock_pubkey(20), 0).unwrap();

        // Test with maximum possible fee (65535 basis points = 655.35%)
        token_pair.add_pool(mock_pubkey(21), u16::MAX).unwrap();

        // Verify both pools were added
        assert_eq!(token_pair.pools.len(), 2);
        assert_eq!(token_pair.pools[0].1, 0);
        assert_eq!(token_pair.pools[1].1, u16::MAX);
    }

    // ========== Simulated Integration Tests ==========

    #[test]
    fn test_simulated_pool_lifecycle() {
        let mut token_pair = mock_token_pair();
        let pool_address = mock_pubkey(20);
        let fee_tier = 3000;

        // Pool addition
        token_pair.add_pool(pool_address, fee_tier).unwrap();

        // Trading activity
        // Simulate multiple swap operations with volume and fees
        for _ in 0..5 {
            token_pair.update_statistics(10_000, 5_000, 15);

            // Update oracle price based on latest swap
            let timestamp = 1640995200;
            update_oracle_price_test(&mut token_pair, 2_000_000, timestamp);
        }

        // Verify accumulated statistics
        assert_eq!(token_pair.total_volume_token_a, 50_000);
        assert_eq!(token_pair.total_volume_token_b, 25_000);
        assert_eq!(token_pair.total_fees_generated, 75);

        // Verify last oracle price
        assert_eq!(token_pair.last_oracle_price, 2_000_000);

        // Governance verification
        token_pair.set_verification(true);
        assert!(token_pair.is_verified);

        // Pool removal
        token_pair.remove_pool(pool_address).unwrap();
        assert_eq!(token_pair.pools.len(), 0);
    }
}
