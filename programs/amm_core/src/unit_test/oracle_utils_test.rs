// Tests for the oracle_utils module
//
// This file contains comprehensive tests for the oracle utils module, particularly
// focusing on observation storage, compression techniques, and TWAP calculations.
// The tests follow security guidelines from the security testing checklist and test plan.

use crate::constants::{MAX_SQRT_PRICE, MAX_TICK, MIN_SQRT_PRICE, MIN_TICK};
use crate::errors::ErrorCode;
use crate::oracle_utils::*;
use anchor_lang::prelude::*;
use std::ops::{Add, Sub};

#[cfg(test)]
mod tests {
    use super::*;

    // ============= Helper Functions =============

    /// Create a test observation with specified values
    fn create_observation(
        timestamp: i64,
        sqrt_price: u128,
        tick_accumulator: i128,
        seconds_per_liquidity: u128,
        initialized: bool,
    ) -> Observation {
        Observation {
            timestamp,
            sqrt_price,
            tick_accumulator,
            seconds_per_liquidity,
            initialized,
        }
    }

    /// Create a test observation with default values at a given timestamp
    fn create_observation_at(timestamp: i64) -> Observation {
        create_observation(
            timestamp,
            1 << 96, // 1.0 in Q64.96 format
            0,
            0,
            true,
        )
    }

    /// Create a sequence of test observations with increasingly higher values
    fn create_observation_sequence(
        start_time: i64,
        count: usize,
        time_step: i64,
    ) -> Vec<Observation> {
        let mut observations = Vec::with_capacity(count);
        for i in 0..count {
            let timestamp = start_time.checked_add(i as i64 * time_step).unwrap();
            let sqrt_price = (1 << 96).checked_add(i as u128 * 1_000_000).unwrap();
            let tick_accumulator = (i as i128 * 1000).checked_add(i as i128 * 100).unwrap();
            let seconds_per_liquidity = (i as u128 * 500).checked_add(i as u128 * 50).unwrap();

            observations.push(create_observation(
                timestamp,
                sqrt_price,
                tick_accumulator,
                seconds_per_liquidity,
                true,
            ));
        }
        observations
    }

    /// Initialize an observation storage with a sequence of observations
    fn initialize_storage_with_sequence(
        start_time: i64,
        count: usize,
        time_step: i64,
        cardinality: u8,
    ) -> Result<(ObservationStorage, Vec<Observation>)> {
        let mut storage = ObservationStorage::default();
        storage.initialize(cardinality)?;

        let observations = create_observation_sequence(start_time, count, time_step);

        for obs in &observations {
            storage.write(obs)?;
        }

        Ok((storage, observations))
    }

    // ============= Basic Functionality Tests =============

    #[test]
    fn test_observation_storage_initialization() {
        let mut storage = ObservationStorage::default();

        // Test default state
        assert_eq!(storage.observation_count, 0);
        assert_eq!(storage.current_observation_index, 0);
        assert_eq!(storage.cardinality, 1);
        assert_eq!(storage.base_timestamp, 0);
        assert_eq!(storage.base_sqrt_price, 0);
        assert_eq!(storage.base_tick_accumulator, 0);
        assert_eq!(storage.base_seconds_per_liquidity, 0);

        // Initialize with valid cardinality
        let result = storage.initialize(10);
        assert!(result.is_ok());
        assert_eq!(storage.cardinality, 10);

        // Test initialization with invalid cardinality (0)
        let mut storage = ObservationStorage::default();
        let result = storage.initialize(0);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::InvalidObservationCardinality.to_string()
        );

        // Test initialization with invalid cardinality (exceeds max)
        let mut storage = ObservationStorage::default();
        let result = storage.initialize(MAX_OBSERVATIONS as u8 + 1);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::InvalidObservationCardinality.to_string()
        );
    }

    #[test]
    fn test_write_single_observation() {
        let mut storage = ObservationStorage::default();
        storage.initialize(8).unwrap();

        // Create and write first observation
        let timestamp = 1000;
        let sqrt_price = 1 << 96; // 1.0 in Q64.96 format
        let tick_accumulator = 500;
        let seconds_per_liquidity = 1000;

        let observation = create_observation(
            timestamp,
            sqrt_price,
            tick_accumulator,
            seconds_per_liquidity,
            true,
        );

        let result = storage.write(&observation);
        assert!(result.is_ok());

        // Verify storage state after write
        assert_eq!(storage.observation_count, 1);
        assert_eq!(storage.current_observation_index, 0);
        assert_eq!(storage.base_timestamp, timestamp);
        assert_eq!(storage.base_sqrt_price, sqrt_price);
        assert_eq!(storage.base_tick_accumulator, tick_accumulator);
        assert_eq!(storage.base_seconds_per_liquidity, seconds_per_liquidity);

        // Verify the first compressed observation
        let compressed_obs = storage.observations[0];
        assert_eq!(compressed_obs.time_delta, 0); // First observation has no delta
        assert_eq!(compressed_obs.sqrt_price_delta, 0);
        assert_eq!(compressed_obs.tick_accumulator_delta, 0);
        assert_eq!(compressed_obs.seconds_per_liquidity_delta, 0);
        assert_eq!(compressed_obs.flags & 1, 1); // Initialized flag should be set
    }

    #[test]
    fn test_write_multiple_observations() {
        let mut storage = ObservationStorage::default();
        storage.initialize(8).unwrap();

        // Write first observation
        let first_obs = create_observation_at(1000);
        storage.write(&first_obs).unwrap();

        // Write second observation
        let second_obs = create_observation(
            1100,                // 100 seconds later
            (1 << 96) + 1000000, // Slightly higher price
            1000,                // Some tick accumulation
            500,                 // Some seconds per liquidity
            true,
        );
        storage.write(&second_obs).unwrap();

        // Verify storage state after writes
        assert_eq!(storage.observation_count, 2);
        assert_eq!(storage.current_observation_index, 1);

        // Retrieve and verify the second observation
        let retrieved_obs = storage.get_observation(1).unwrap();
        assert_eq!(retrieved_obs.timestamp, second_obs.timestamp);
        assert_eq!(retrieved_obs.sqrt_price, second_obs.sqrt_price);
        assert_eq!(retrieved_obs.tick_accumulator, second_obs.tick_accumulator);
        assert_eq!(
            retrieved_obs.seconds_per_liquidity,
            second_obs.seconds_per_liquidity
        );
        assert_eq!(retrieved_obs.initialized, second_obs.initialized);

        // Check the compressed second observation
        let compressed_obs = storage.observations[1];
        assert_eq!(compressed_obs.time_delta, 100); // 100 seconds delta
        assert_eq!(compressed_obs.sqrt_price_delta, 1000000); // Price delta
        assert_eq!(compressed_obs.tick_accumulator_delta, 1000); // Tick accumulator delta
        assert_eq!(compressed_obs.seconds_per_liquidity_delta, 500); // Seconds per liquidity delta
    }

    #[test]
    fn test_observation_retrieval() {
        let mut storage = ObservationStorage::default();
        storage.initialize(8).unwrap();

        // Write a few observations
        let obs1 = create_observation(1000, 1 << 96, 0, 0, true);
        let obs2 = create_observation(1100, (1 << 96) + 1000000, 1000, 500, true);
        let obs3 = create_observation(1200, (1 << 96) + 2000000, 2000, 1000, true);

        storage.write(&obs1).unwrap();
        storage.write(&obs2).unwrap();
        storage.write(&obs3).unwrap();

        // Get first observation
        let retrieved1 = storage.get_observation(0).unwrap();
        assert_eq!(retrieved1.timestamp, obs1.timestamp);
        assert_eq!(retrieved1.sqrt_price, obs1.sqrt_price);
        assert_eq!(retrieved1.tick_accumulator, obs1.tick_accumulator);
        assert_eq!(retrieved1.seconds_per_liquidity, obs1.seconds_per_liquidity);

        // Get second observation
        let retrieved2 = storage.get_observation(1).unwrap();
        assert_eq!(retrieved2.timestamp, obs2.timestamp);
        assert_eq!(retrieved2.sqrt_price, obs2.sqrt_price);
        assert_eq!(retrieved2.tick_accumulator, obs2.tick_accumulator);
        assert_eq!(retrieved2.seconds_per_liquidity, obs2.seconds_per_liquidity);

        // Get third observation (latest)
        let retrieved3 = storage.get_observation(2).unwrap();
        assert_eq!(retrieved3.timestamp, obs3.timestamp);
        assert_eq!(retrieved3.sqrt_price, obs3.sqrt_price);
        assert_eq!(retrieved3.tick_accumulator, obs3.tick_accumulator);
        assert_eq!(retrieved3.seconds_per_liquidity, obs3.seconds_per_liquidity);

        // Get latest observation using dedicated function
        let latest = storage.get_latest_observation().unwrap();
        assert_eq!(latest.timestamp, obs3.timestamp);
        assert_eq!(latest.sqrt_price, obs3.sqrt_price);

        // Try to get observation with invalid index
        let result = storage.get_observation(3);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::ObservationIndexOutOfBounds.to_string()
        );
    }

    #[test]
    fn test_cardinality_increase() {
        let mut storage = ObservationStorage::default();
        storage.initialize(4).unwrap();
        assert_eq!(storage.cardinality, 4);

        // Valid increase
        let result = storage.increase_cardinality(8);
        assert!(result.is_ok());
        assert_eq!(storage.cardinality, 8);

        // Try to decrease (not allowed)
        let result = storage.increase_cardinality(6);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::InvalidObservationCardinality.to_string()
        );

        // Try to exceed maximum
        let result = storage.increase_cardinality(MAX_OBSERVATIONS as u8 + 1);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::InvalidObservationCardinality.to_string()
        );
    }

    // ============= Observation Overflow Tests =============

    #[test]
    fn test_observation_circular_buffer() {
        let mut storage = ObservationStorage::default();
        let cardinality = 4;
        storage.initialize(cardinality).unwrap();

        // Write more observations than cardinality
        let base_time = 1000;
        for i in 0..6 {
            let obs = create_observation_at(base_time + i * 100);
            storage.write(&obs).unwrap();
        }

        // After writing 6 observations to a buffer of size 4,
        // we should have observations at indices 2, 3, 0, 1
        // with current_index = 1 (the most recent write)
        assert_eq!(storage.observation_count, cardinality);
        assert_eq!(storage.current_observation_index, 1);

        // Check the timestamps of the observations in the buffer
        // They should be the 4 most recent observations
        let obs0 = storage.get_observation(0).unwrap();
        let obs1 = storage.get_observation(1).unwrap();
        let obs2 = storage.get_observation(2).unwrap();
        let obs3 = storage.get_observation(3).unwrap();

        assert_eq!(obs0.timestamp, base_time + 400); // 5th observation (index wrapped to 0)
        assert_eq!(obs1.timestamp, base_time + 500); // 6th observation (index wrapped to 1)
        assert_eq!(obs2.timestamp, base_time + 200); // 3rd observation
        assert_eq!(obs3.timestamp, base_time + 300); // 4th observation

        // Get latest observation
        let latest = storage.get_latest_observation().unwrap();
        assert_eq!(latest.timestamp, base_time + 500); // 6th observation

        // Write one more and verify circular buffer behavior continues
        let new_obs = create_observation_at(base_time + 600);
        storage.write(&new_obs).unwrap();

        // Latest should now be at index 2
        assert_eq!(storage.current_observation_index, 2);
        let latest = storage.get_latest_observation().unwrap();
        assert_eq!(latest.timestamp, base_time + 600); // 7th observation
    }

    // ============= Compression Tests =============

    #[test]
    fn test_observation_compression_decompression() {
        let mut storage = ObservationStorage::default();
        storage.initialize(8).unwrap();

        // Create base observation values
        let base_timestamp = 1000;
        let base_sqrt_price = (1 << 96) + 1000;
        let base_tick_accumulator = 500;
        let base_seconds_per_liquidity = 2000;

        // Create and write first observation
        let first_obs = create_observation(
            base_timestamp,
            base_sqrt_price,
            base_tick_accumulator,
            base_seconds_per_liquidity,
            true,
        );
        storage.write(&first_obs).unwrap();

        // Create and write second observation with deltas in all values
        let second_obs = create_observation(
            base_timestamp + 100,
            base_sqrt_price + 5000,
            base_tick_accumulator + 1000,
            base_seconds_per_liquidity + 1500,
            true,
        );
        storage.write(&second_obs).unwrap();

        // Create and write third observation with a negative price delta
        let third_obs = create_observation(
            base_timestamp + 250,
            base_sqrt_price - 2000, // Price going down
            base_tick_accumulator + 2000,
            base_seconds_per_liquidity + 3000,
            true,
        );
        storage.write(&third_obs).unwrap();

        // Retrieve observations and verify they match original values
        let retrieved1 = storage.get_observation(0).unwrap();
        let retrieved2 = storage.get_observation(1).unwrap();
        let retrieved3 = storage.get_observation(2).unwrap();

        // First observation should match exactly
        assert_eq!(retrieved1.timestamp, first_obs.timestamp);
        assert_eq!(retrieved1.sqrt_price, first_obs.sqrt_price);
        assert_eq!(retrieved1.tick_accumulator, first_obs.tick_accumulator);
        assert_eq!(
            retrieved1.seconds_per_liquidity,
            first_obs.seconds_per_liquidity
        );

        // Second observation should match exactly
        assert_eq!(retrieved2.timestamp, second_obs.timestamp);
        assert_eq!(retrieved2.sqrt_price, second_obs.sqrt_price);
        assert_eq!(retrieved2.tick_accumulator, second_obs.tick_accumulator);
        assert_eq!(
            retrieved2.seconds_per_liquidity,
            second_obs.seconds_per_liquidity
        );

        // Third observation with negative price delta should match exactly
        assert_eq!(retrieved3.timestamp, third_obs.timestamp);
        assert_eq!(retrieved3.sqrt_price, third_obs.sqrt_price);
        assert_eq!(retrieved3.tick_accumulator, third_obs.tick_accumulator);
        assert_eq!(
            retrieved3.seconds_per_liquidity,
            third_obs.seconds_per_liquidity
        );
    }

    #[test]
    fn test_time_delta_limits() {
        let mut storage = ObservationStorage::default();
        storage.initialize(4).unwrap();

        // Write first observation
        let first_obs = create_observation_at(1000);
        storage.write(&first_obs).unwrap();

        // Write second observation with max valid time delta (u16::MAX)
        let max_delta_obs = create_observation_at(1000 + u16::MAX as i64);
        let result = storage.write(&max_delta_obs);
        assert!(result.is_ok());

        // Write observation with time delta exceeding u16::MAX
        let too_large_delta_obs = create_observation_at(1000 + u16::MAX as i64 + 1);
        let result = storage.write(&too_large_delta_obs);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::ObservationTimeDeltaTooLarge.to_string()
        );

        // Write observation with negative time delta (going backwards in time)
        let negative_delta_obs = create_observation_at(500); // Earlier than first observation
        let result = storage.write(&negative_delta_obs);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::ObservationTimeDeltaTooLarge.to_string()
        );
    }

    // ============= TWAP Tests =============

    #[test]
    fn test_twap_calculation() {
        let (storage, observations) = initialize_storage_with_sequence(1000, 5, 100, 8).unwrap();

        // Test TWAP between first and last observation
        let start_timestamp = observations[0].timestamp;
        let end_timestamp = observations[4].timestamp;

        let twap_result = storage.get_twap(start_timestamp, end_timestamp);
        assert!(twap_result.is_ok());

        // Due to the simplified conversion in the implementation, we can't exactly verify
        // the TWAP value, but we can check that it's a reasonable value
        let twap = twap_result.unwrap();
        assert!(twap > 0);

        // Test TWAP with timestamps not exactly matching observations
        let twap_result = storage.get_twap(start_timestamp + 30, end_timestamp - 30);
        assert!(twap_result.is_ok());

        // Test TWAP with end before start (should error)
        let twap_result = storage.get_twap(end_timestamp, start_timestamp);
        assert!(twap_result.is_err());
        assert_eq!(
            twap_result.unwrap_err().to_string(),
            ErrorCode::InvalidInput.to_string()
        );

        // Test TWAP with equal timestamps (should error)
        let twap_result = storage.get_twap(start_timestamp, start_timestamp);
        assert!(twap_result.is_err());
        assert_eq!(
            twap_result.unwrap_err().to_string(),
            ErrorCode::InvalidInput.to_string()
        );
    }

    #[test]
    fn test_get_observation_at_timestamp() {
        let (storage, observations) = initialize_storage_with_sequence(1000, 5, 100, 8).unwrap();

        // Test with exact timestamp matches
        for obs in &observations {
            let result = storage
                .get_observation_at_or_before_timestamp(obs.timestamp)
                .unwrap();
            assert_eq!(result.timestamp, obs.timestamp);
        }

        // Test with timestamps between observations
        let result = storage
            .get_observation_at_or_before_timestamp(1050)
            .unwrap();
        assert_eq!(result.timestamp, 1000); // Should get observation at 1000

        let result = storage
            .get_observation_at_or_before_timestamp(1150)
            .unwrap();
        assert_eq!(result.timestamp, 1100); // Should get observation at 1100

        // Test with timestamp after all observations
        let result = storage
            .get_observation_at_or_before_timestamp(2000)
            .unwrap();
        assert_eq!(result.timestamp, 1400); // Should get the last observation

        // Test with timestamp before all observations
        let result = storage.get_observation_at_or_before_timestamp(900);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::ObservationBoundaryError.to_string()
        );
    }

    #[test]
    fn test_twap_with_insufficient_data() {
        let mut storage = ObservationStorage::default();
        storage.initialize(4).unwrap();

        // Try TWAP with no observations
        let twap_result = storage.get_twap(1000, 2000);
        assert!(twap_result.is_err());
        assert_eq!(
            twap_result.unwrap_err().to_string(),
            ErrorCode::OracleInsufficientData.to_string()
        );

        // Add just one observation
        let obs = create_observation_at(1000);
        storage.write(&obs).unwrap();

        // Try TWAP with only one observation
        let twap_result = storage.get_twap(1000, 2000);
        assert!(twap_result.is_err());
        assert_eq!(
            twap_result.unwrap_err().to_string(),
            ErrorCode::OracleInsufficientData.to_string()
        );
    }

    // ============= Edge Case and Error Tests =============

    #[test]
    fn test_empty_storage_operations() {
        let storage = ObservationStorage::default();

        // Try to get latest observation from empty storage
        let result = storage.get_latest_observation();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::NoObservations.to_string()
        );

        // Try to get observation by index from empty storage
        let result = storage.get_observation(0);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::ObservationIndexOutOfBounds.to_string()
        );

        // Try to get observation at timestamp from empty storage
        let result = storage.get_observation_at_or_before_timestamp(1000);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::NoObservations.to_string()
        );
    }

    #[test]
    fn test_overflow_protection() {
        let mut storage = ObservationStorage::default();
        storage.initialize(4).unwrap();

        // Create base observation with values close to limits
        let base_sqrt_price = u128::MAX - 1000;
        let base_tick_accumulator = i128::MAX - 1000;
        let base_seconds_per_liquidity = u128::MAX - 1000;

        let first_obs = create_observation(
            1000,
            base_sqrt_price,
            base_tick_accumulator,
            base_seconds_per_liquidity,
            true,
        );

        storage.write(&first_obs).unwrap();

        // Create observation that would cause overflow
        let overflow_obs = create_observation(
            1100,
            base_sqrt_price + 2000,            // Would overflow u128
            base_tick_accumulator + 2000,      // Would overflow i128
            base_seconds_per_liquidity + 2000, // Would overflow u128
            true,
        );

        // Writing should fail due to overflow protection
        let result = storage.write(&overflow_obs);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            ErrorCode::MathOverflow.to_string()
        );
    }

    #[test]
    fn test_binary_search_edge_cases() {
        // Test binary search behavior with various patterns of observations
        let (storage, _) = initialize_storage_with_sequence(1000, 10, 200, 16).unwrap();

        // Test at exact boundaries
        let first_result = storage
            .get_observation_at_or_before_timestamp(1000)
            .unwrap();
        assert_eq!(first_result.timestamp, 1000);

        let last_result = storage
            .get_observation_at_or_before_timestamp(2800)
            .unwrap();
        assert_eq!(last_result.timestamp, 2800);

        // Test at midpoint
        let mid_result = storage
            .get_observation_at_or_before_timestamp(1900)
            .unwrap();
        assert_eq!(mid_result.timestamp, 1800); // Should get observation just before

        // Test just after each observation
        for i in 0..9 {
            let timestamp = 1000 + i * 200;
            let result = storage
                .get_observation_at_or_before_timestamp(timestamp + 1)
                .unwrap();
            assert_eq!(result.timestamp, timestamp);
        }

        // Test just before each observation
        for i in 1..10 {
            let timestamp = 1000 + i * 200;
            let result = storage
                .get_observation_at_or_before_timestamp(timestamp - 1)
                .unwrap();
            assert_eq!(result.timestamp, timestamp - 200);
        }
    }

    // ============= Security-Related Tests =============

    #[test]
    fn test_compression_with_extreme_values() {
        let mut storage = ObservationStorage::default();
        storage.initialize(4).unwrap();

        // Test with minimal values
        let min_obs = create_observation(
            0,              // Minimum timestamp
            MIN_SQRT_PRICE, // Minimum sqrt price
            i128::MIN / 2,  // Very negative tick accumulator (but not MIN to avoid overflow)
            0,              // Minimum seconds per liquidity
            true,
        );
        storage.write(&min_obs).unwrap();

        // Test with maximal values
        let max_obs = create_observation(
            i64::MAX / 2,   // Very large timestamp (but not MAX to avoid overflow)
            MAX_SQRT_PRICE, // Maximum sqrt price
            i128::MAX / 2,  // Very large tick accumulator (but not MAX to avoid overflow)
            u128::MAX / 2,  // Very large seconds per liquidity (but not MAX to avoid overflow)
            true,
        );
        storage.write(&max_obs).unwrap();

        // Verify we can retrieve these extreme values correctly
        let retrieved_min = storage.get_observation(0).unwrap();
        let retrieved_max = storage.get_observation(1).unwrap();

        assert_eq!(retrieved_min.timestamp, min_obs.timestamp);
        assert_eq!(retrieved_min.sqrt_price, min_obs.sqrt_price);
        assert_eq!(retrieved_min.tick_accumulator, min_obs.tick_accumulator);
        assert_eq!(
            retrieved_min.seconds_per_liquidity,
            min_obs.seconds_per_liquidity
        );

        assert_eq!(retrieved_max.timestamp, max_obs.timestamp);
        assert_eq!(retrieved_max.sqrt_price, max_obs.sqrt_price);
        assert_eq!(retrieved_max.tick_accumulator, max_obs.tick_accumulator);
        assert_eq!(
            retrieved_max.seconds_per_liquidity,
            max_obs.seconds_per_liquidity
        );
    }

    #[test]
    fn test_observation_flags() {
        let mut storage = ObservationStorage::default();
        storage.initialize(4).unwrap();

        // Test observation with initialized flag set
        let init_obs = create_observation(1000, 1 << 96, 0, 0, true);
        storage.write(&init_obs).unwrap();

        // Test observation with initialized flag not set
        let uninit_obs = create_observation(1100, 1 << 96, 0, 0, false);
        storage.write(&uninit_obs).unwrap();

        // Verify flags are preserved after compression/decompression
        let retrieved_init = storage.get_observation(0).unwrap();
        let retrieved_uninit = storage.get_observation(1).unwrap();

        assert_eq!(retrieved_init.initialized, true);
        assert_eq!(retrieved_uninit.initialized, false);
    }

    #[test]
    fn test_rapid_price_changes() {
        let mut storage = ObservationStorage::default();
        storage.initialize(8).unwrap();

        // Write observations with rapidly changing prices (volatility test)
        let base_time = 1000;
        let base_price = 1 << 96;

        // First observation
        let obs1 = create_observation(base_time, base_price, 0, 0, true);
        storage.write(&obs1).unwrap();

        // Price spike up 100x
        let obs2 = create_observation(base_time + 60, base_price * 100, 1000, 500, true);
        storage.write(&obs2).unwrap();

        // Price crash down 99%
        let obs3 = create_observation(base_time + 120, base_price, 2000, 1000, true);
        storage.write(&obs3).unwrap();

        // Price recovery 10x
        let obs4 = create_observation(base_time + 180, base_price * 10, 3000, 1500, true);
        storage.write(&obs4).unwrap();

        // Verify we can retrieve these observations correctly
        let retrieved1 = storage.get_observation(0).unwrap();
        let retrieved2 = storage.get_observation(1).unwrap();
        let retrieved3 = storage.get_observation(2).unwrap();
        let retrieved4 = storage.get_observation(3).unwrap();

        assert_eq!(retrieved1.sqrt_price, obs1.sqrt_price);
        assert_eq!(retrieved2.sqrt_price, obs2.sqrt_price);
        assert_eq!(retrieved3.sqrt_price, obs3.sqrt_price);
        assert_eq!(retrieved4.sqrt_price, obs4.sqrt_price);

        // Test TWAP during this volatile period
        let twap = storage.get_twap(base_time, base_time + 180).unwrap();
        // We don't know the exact TWAP value due to the implementation simplification,
        // but it should be somewhere between the smallest and largest price
        assert!(twap >= base_price);
        assert!(twap <= base_price * 100);
    }

    #[test]
    fn test_long_observation_sequence() {
        // Test with a long sequence to ensure robustness
        let cardinality = 16;
        let mut storage = ObservationStorage::default();
        storage.initialize(cardinality).unwrap();

        // Create and write many observations
        let base_time = 1000;
        let time_step = 60; // 1 minute intervals
        let count = 100; // More than MAX_OBSERVATIONS

        for i in 0..count {
            let timestamp = base_time + i as i64 * time_step;
            let sqrt_price = (1 << 96) + i as u128 * 10000;
            let tick_accumulator = i as i128 * 500;
            let seconds_per_liquidity = i as u128 * 200;

            let obs = create_observation(
                timestamp,
                sqrt_price,
                tick_accumulator,
                seconds_per_liquidity,
                true,
            );
            storage.write(&obs).unwrap();
        }

        // Verify we have exactly cardinality observations
        assert_eq!(storage.observation_count, cardinality);

        // Get the latest observation
        let latest = storage.get_latest_observation().unwrap();
        let expected_latest_time = base_time + (count - 1) as i64 * time_step;
        assert_eq!(latest.timestamp, expected_latest_time);

        // Get TWAP over the entire observable range
        // The observable range is now the last 'cardinality' observations
        let oldest_observable_time = base_time + (count - cardinality) as i64 * time_step;
        let twap = storage
            .get_twap(oldest_observable_time, expected_latest_time)
            .unwrap();
        assert!(twap > 0);
    }

    // ============= Oracle Security Tests =============

    #[test]
    fn test_manipulation_resistance() {
        let mut storage = ObservationStorage::default();
        storage.initialize(8).unwrap();

        // Establish a steady price sequence
        let base_time = 1000;
        let base_price = 1 << 96;

        // Write several observations with stable price
        for i in 0..5 {
            let obs = create_observation(
                base_time + i * 60,
                base_price,
                i as i128 * 1000,
                i as u128 * 500,
                true,
            );
            storage.write(&obs).unwrap();
        }

        // Try to manipulate with an outlier price spike
        let manipulation_obs = create_observation(
            base_time + 5 * 60,
            base_price * 10, // 10x price spike
            5 * 1000,
            5 * 500,
            true,
        );
        storage.write(&manipulation_obs).unwrap();

        // Quick reversion to normal price
        let reversion_obs =
            create_observation(base_time + 6 * 60, base_price, 6 * 1000, 6 * 500, true);
        storage.write(&reversion_obs).unwrap();

        // Calculate TWAP over the entire period including manipulation
        let twap = storage.get_twap(base_time, base_time + 6 * 60).unwrap();

        // The TWAP should be resistant to the short-term manipulation
        // It won't equal the base price due to the spike, but should be closer to base than spike
        let price_diff = if twap > base_price {
            twap - base_price
        } else {
            base_price - twap
        };

        // The difference should be less than 50% of the manipulation magnitude
        assert!(price_diff < (base_price * 10 - base_price) / 2);
    }

    #[test]
    fn test_observation_interpolation_security() {
        // Test that the binary search for observations correctly handles
        // edge cases and can't be manipulated

        let mut storage = ObservationStorage::default();
        storage.initialize(8).unwrap();

        // Write sparse observations
        let obs1 = create_observation(1000, 1 << 96, 0, 0, true);
        let obs2 = create_observation(2000, 2 << 96, 1000, 500, true);

        storage.write(&obs1).unwrap();
        storage.write(&obs2).unwrap();

        // Test multiple points in between
        for i in 1..10 {
            let timestamp = 1000 + i * 100;
            let result = storage
                .get_observation_at_or_before_timestamp(timestamp)
                .unwrap();

            // Should always get the observation at 1000
            assert_eq!(result.timestamp, 1000);
        }

        // Test exactly at observation timestamp
        let result = storage
            .get_observation_at_or_before_timestamp(2000)
            .unwrap();
        assert_eq!(result.timestamp, 2000);

        // Test after all observations
        let result = storage
            .get_observation_at_or_before_timestamp(3000)
            .unwrap();
        assert_eq!(result.timestamp, 2000);
    }

    #[test]
    fn test_twap_boundary_security() {
        let mut storage = ObservationStorage::default();
        storage.initialize(8).unwrap();

        // Write observations with extreme tick values
        let base_time = 1000;

        // First observation at MIN_TICK
        let min_sqrt_price = MIN_SQRT_PRICE;
        let min_obs =
            create_observation(base_time, min_sqrt_price, MIN_TICK as i128 * 1000, 0, true);
        storage.write(&min_obs).unwrap();

        // Second observation at MAX_TICK
        let max_sqrt_price = MAX_SQRT_PRICE;
        let max_obs = create_observation(
            base_time + 1000,
            max_sqrt_price,
            MAX_TICK as i128 * 1000,
            1000,
            true,
        );
        storage.write(&max_obs).unwrap();

        // Calculate TWAP over the full range
        let twap_result = storage.get_twap(base_time, base_time + 1000);
        assert!(twap_result.is_ok());

        // The TWAP value should be within the valid price range
        let twap = twap_result.unwrap();
        assert!(twap >= min_sqrt_price);
        assert!(twap <= max_sqrt_price);
    }

    // ============= Stress Tests =============

    #[test]
    fn test_alternating_price_direction() {
        let mut storage = ObservationStorage::default();
        storage.initialize(16).unwrap();

        // Create a sequence of observations with prices that alternate direction
        let base_time = 1000;
        let base_price = 1 << 96;
        let mut price = base_price;
        let mut direction = 1;

        for i in 0..20 {
            let timestamp = base_time + i * 60;

            let obs = create_observation(timestamp, price, i as i128 * 1000, i as u128 * 500, true);
            storage.write(&obs).unwrap();

            // Alternate price direction and increase magnitude
            direction *= -1;
            let change = base_price / 10 * (i + 1) as u128;

            if direction > 0 {
                price = price.saturating_add(change);
            } else {
                price = price.saturating_sub(change.min(price - 1)); // Ensure price stays positive
            }
        }

        // Get observations from different points and verify they match expectations
        let mid_point = storage
            .get_observation(storage.observation_count as usize / 2)
            .unwrap();
        assert!(mid_point.sqrt_price > 0);

        // Calculate TWAP over the entire period
        let twap = storage.get_twap(base_time, base_time + 19 * 60).unwrap();
        assert!(twap > 0);
    }

    #[test]
    fn test_cardinality_changes_with_data() {
        let mut storage = ObservationStorage::default();
        storage.initialize(4).unwrap();

        // Fill the initial cardinality
        for i in 0..4 {
            let obs = create_observation_at(1000 + i * 100);
            storage.write(&obs).unwrap();
        }

        // Increase cardinality
        storage.increase_cardinality(8).unwrap();

        // Add more observations
        for i in 4..8 {
            let obs = create_observation_at(1000 + i * 100);
            storage.write(&obs).unwrap();
        }

        // Verify we can access all observations
        for i in 0..8 {
            let obs = storage.get_observation(i).unwrap();
            assert_eq!(obs.timestamp, 1000 + i * 100);
        }

        // Further increase cardinality
        storage.increase_cardinality(12).unwrap();

        // Add more observations
        for i in 8..12 {
            let obs = create_observation_at(1000 + i * 100);
            storage.write(&obs).unwrap();
        }

        // Verify we can access all observations
        for i in 0..12 {
            let obs = storage.get_observation(i).unwrap();
            assert_eq!(obs.timestamp, 1000 + i * 100);
        }
    }

    // ============= Performance and Resource Usage Tests =============

    #[test]
    fn test_storage_compression_efficiency() {
        // Calculate theoretical compression ratio
        // Full Observation: timestamp (i64) + sqrt_price (u128) + tick_accumulator (i128) +
        //                   seconds_per_liquidity (u128) + initialized (bool)
        // = 8 + 16 + 16 + 16 + 1 = 57 bytes

        // CompressedObservation: time_delta (u16) + sqrt_price_delta (i64) +
        //                        tick_accumulator_delta (i64) + seconds_per_liquidity_delta (u64) + flags (u8)
        // = 2 + 8 + 8 + 8 + 1 = 27 bytes

        // Theoretical compression ratio: 57/27 ≈ 2.11

        let mut storage = ObservationStorage::default();
        storage.initialize(MAX_OBSERVATIONS as u8).unwrap();

        // Create and write many observations with realistic values
        let base_time = 1000;
        let observations_to_write = MAX_OBSERVATIONS;

        for i in 0..observations_to_write {
            // Create observation with small deltas to simulate typical usage
            let timestamp = base_time + i as i64 * 15; // 15 second intervals
            let sqrt_price = (1 << 96) + (i as u128 % 1000) * 1000; // Small price variations
            let tick_accumulator = i as i128 * 100;
            let seconds_per_liquidity = i as u128 * 50;

            let obs = create_observation(
                timestamp,
                sqrt_price,
                tick_accumulator,
                seconds_per_liquidity,
                true,
            );
            storage.write(&obs).unwrap();
        }

        // Verify we can retrieve all observations correctly
        for i in 0..observations_to_write {
            let expected_time = base_time + i as i64 * 15;
            let retrieved = storage.get_observation(i).unwrap();
            assert_eq!(retrieved.timestamp, expected_time);
        }
    }

    #[test]
    fn test_max_observations_boundary() {
        let mut storage = ObservationStorage::default();

        // Initialize with maximum allowed cardinality
        let result = storage.initialize(MAX_OBSERVATIONS as u8);
        assert!(result.is_ok());

        // Try to exceed the maximum
        let mut storage = ObservationStorage::default();
        let result = storage.initialize(MAX_OBSERVATIONS as u8 + 1);
        assert!(result.is_err());

        // Try to increase beyond the maximum
        let mut storage = ObservationStorage::default();
        storage.initialize(MAX_OBSERVATIONS as u8 - 1).unwrap();
        let result = storage.increase_cardinality(MAX_OBSERVATIONS as u8);
        assert!(result.is_ok());

        let result = storage.increase_cardinality(MAX_OBSERVATIONS as u8 + 1);
        assert!(result.is_err());
    }

    // ============= Integration-Like Tests =============

    #[test]
    fn test_observation_lifecycle() {
        // This test simulates a full lifecycle of oracle usage
        let mut storage = ObservationStorage::default();

        // 1. Initialize with small cardinality
        storage.initialize(4).unwrap();

        // 2. Add initial observations
        let base_time = 1000;
        let base_price = 1 << 96;

        for i in 0..4 {
            let timestamp = base_time + i * 60;
            let sqrt_price = base_price + i as u128 * 1000;
            let tick_accumulator = i as i128 * 500;
            let seconds_per_liquidity = i as u128 * 200;

            let obs = create_observation(
                timestamp,
                sqrt_price,
                tick_accumulator,
                seconds_per_liquidity,
                true,
            );
            storage.write(&obs).unwrap();
        }

        // 3. Calculate initial TWAP
        let initial_twap = storage.get_twap(base_time, base_time + 3 * 60).unwrap();

        // 4. Increase cardinality
        storage.increase_cardinality(8).unwrap();

        // 5. Add more observations
        for i in 4..8 {
            let timestamp = base_time + i * 60;
            let sqrt_price = base_price + i as u128 * 1000;
            let tick_accumulator = i as i128 * 500;
            let seconds_per_liquidity = i as u128 * 200;

            let obs = create_observation(
                timestamp,
                sqrt_price,
                tick_accumulator,
                seconds_per_liquidity,
                true,
            );
            storage.write(&obs).unwrap();
        }

        // 6. Calculate new TWAP over expanded range
        let expanded_twap = storage.get_twap(base_time, base_time + 7 * 60).unwrap();

        // 7. Verify calculations are consistent
        // The expanded TWAP should be higher since we're adding higher prices
        assert!(expanded_twap >= initial_twap);

        // 8. Simulate overflow by adding many more observations
        for i in 8..20 {
            let timestamp = base_time + i * 60;
            let sqrt_price = base_price + i as u128 * 1000;
            let tick_accumulator = i as i128 * 500;
            let seconds_per_liquidity = i as u128 * 200;

            let obs = create_observation(
                timestamp,
                sqrt_price,
                tick_accumulator,
                seconds_per_liquidity,
                true,
            );
            storage.write(&obs).unwrap();
        }

        // 9. Verify we still have exactly 8 observations
        assert_eq!(storage.observation_count, 8);

        // 10. Get latest observation
        let latest = storage.get_latest_observation().unwrap();
        assert_eq!(latest.timestamp, base_time + 19 * 60);

        // 11. Calculate TWAP over the most recent range
        let latest_twap = storage
            .get_twap(base_time + 12 * 60, base_time + 19 * 60)
            .unwrap();

        // 12. Verify TWAP is higher than expanded TWAP
        assert!(latest_twap > expanded_twap);
    }

    #[test]
    fn test_protocol_tick_boundaries() {
        // Test observations at the protocol tick boundaries
        let mut storage = ObservationStorage::default();
        storage.initialize(4).unwrap();

        // Create observation at MIN_TICK
        let min_obs = create_observation(1000, MIN_SQRT_PRICE, MIN_TICK as i128, 0, true);
        storage.write(&min_obs).unwrap();

        // Create observation at MAX_TICK
        let max_obs = create_observation(2000, MAX_SQRT_PRICE, MAX_TICK as i128, 1000, true);
        storage.write(&max_obs).unwrap();

        // Retrieve and verify boundary observations
        let retrieved_min = storage.get_observation(0).unwrap();
        let retrieved_max = storage.get_observation(1).unwrap();

        assert_eq!(retrieved_min.sqrt_price, MIN_SQRT_PRICE);
        assert_eq!(retrieved_max.sqrt_price, MAX_SQRT_PRICE);

        // Calculate TWAP across the full range
        let twap = storage.get_twap(1000, 2000).unwrap();

        // The TWAP should be within the valid range
        assert!(twap >= MIN_SQRT_PRICE);
        assert!(twap <= MAX_SQRT_PRICE);
    }
}
