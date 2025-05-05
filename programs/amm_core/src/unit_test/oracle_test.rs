// Tests for the oracle module
//
// This file contains comprehensive tests for the oracle module, ensuring
// both functionality and security aspects are properly tested.
// The tests follow guidelines from the security testing checklist and test plan.

use crate::constants::{MAX_SQRT_PRICE, MAX_TICK, MIN_SQRT_PRICE, MIN_TICK};
use crate::errors::ErrorCode;
use crate::oracle::*;
use anchor_lang::prelude::*;
use std::collections::VecDeque;

#[cfg(test)]
mod tests {
    use super::*;

    // ========== Helper Functions ==========

    /// Create a mock Pubkey for testing
    fn mock_pubkey(seed: u8) -> Pubkey {
        let mut bytes = [0u8; 32];
        bytes[0] = seed;
        Pubkey::new_from_array(bytes)
    }

    /// Create a mock Oracle for testing
    fn create_mock_oracle() -> Oracle {
        Oracle {
            pool: mock_pubkey(1),
            observations: Vec::new(),
            observation_index: 0,
            observation_cardinality: 0,
            observation_cardinality_next: 0,
            base_timestamp: 0,
            base_sqrt_price: 0,
            base_tick_cumulative: 0,
            base_seconds_per_liquidity_cumulative: 0,
            compressed_observations: Vec::new(),
            compression_enabled: false,
        }
    }

    /// Helper to create a sequence of test observations with predictable values
    fn create_observation_sequence(
        start_time: u32,
        count: usize,
        time_step: u32,
        price_step: u128,
        tick_step: i32,
        liquidity: u128,
    ) -> Vec<(u32, u128, i32, u128)> {
        let mut observations = Vec::with_capacity(count);

        for i in 0..count {
            let timestamp = start_time + i as u32 * time_step;
            let sqrt_price = (1 << 96) + i as u128 * price_step;
            let tick = tick_step * i as i32;

            observations.push((timestamp, sqrt_price, tick, liquidity));
        }

        observations
    }

    /// Helper to initialize and populate an Oracle with a sequence of observations
    fn create_populated_oracle(
        observation_sequence: &[(u32, u128, i32, u128)],
        initial_cardinality: u16,
    ) -> Oracle {
        let mut oracle = create_mock_oracle();

        // Initialize the oracle with the first observation
        let (first_time, first_price, _, _) = observation_sequence[0];
        oracle
            .initialize(mock_pubkey(1), first_time, first_price)
            .expect("Failed to initialize oracle");

        // Ensure observations array has capacity
        if oracle.observations.len() < initial_cardinality as usize {
            oracle.observations = vec![Observation::new(); initial_cardinality as usize];
            oracle.observation_cardinality = initial_cardinality;
            oracle.observation_cardinality_next = initial_cardinality;
        }

        // Write all observations except the first (which was used for initialization)
        for i in 1..observation_sequence.len() {
            let (time, price, tick, liquidity) = observation_sequence[i];
            oracle
                .write(time, price, tick, liquidity)
                .expect("Failed to write observation");
        }

        oracle
    }

    // ========== Basic Functionality Tests ==========

    #[test]
    fn test_oracle_initialization() {
        let mut oracle = create_mock_oracle();
        let pool = mock_pubkey(1);
        let timestamp = 1000;
        let sqrt_price = 1 << 96; // 1.0 in Q64.96 format

        // Initialize the oracle
        let result = oracle.initialize(pool, timestamp, sqrt_price);
        assert!(result.is_ok());

        // Verify proper initialization
        assert_eq!(oracle.pool, pool);
        assert_eq!(oracle.observation_index, 0);
        assert_eq!(oracle.observation_cardinality, 1);
        assert_eq!(oracle.observation_cardinality_next, 1);
        assert_eq!(oracle.base_timestamp, timestamp);
        assert_eq!(oracle.base_sqrt_price, sqrt_price);
        assert_eq!(oracle.base_tick_cumulative, 0);
        assert_eq!(oracle.base_seconds_per_liquidity_cumulative, 0);
        assert_eq!(oracle.compression_enabled, false);

        // Verify the first observation was created
        assert_eq!(oracle.observations.len(), 1);
        let observation = &oracle.observations[0];
        assert_eq!(observation.block_timestamp, timestamp);
        assert_eq!(observation.sqrt_price, sqrt_price);
        assert_eq!(observation.tick_cumulative, 0);
        assert_eq!(observation.seconds_per_liquidity_cumulative, 0);
        assert_eq!(observation.initialized, true);
    }

    #[test]
    fn test_oracle_write_observation() {
        let mut oracle = create_mock_oracle();
        let pool = mock_pubkey(1);

        // Initialize with first observation
        let initial_time = 1000;
        let initial_price = 1 << 96;
        oracle
            .initialize(pool, initial_time, initial_price)
            .unwrap();

        // Create a second observation
        let second_time = 1060; // 1 minute later
        let second_price = (1 << 96) + 1000000;
        let tick = 10;
        let liquidity = 1000000;

        // Write the second observation
        let result = oracle.write(second_time, second_price, tick, liquidity);
        assert!(result.is_ok());

        // Verify oracle state updates
        assert_eq!(oracle.observation_index, 1);
        assert_eq!(oracle.observation_cardinality, 1);
        assert_eq!(oracle.observations.len(), 1);

        // Create larger capacity and write more observations
        oracle.observation_cardinality = 4;
        oracle.observation_cardinality_next = 4;
        oracle.observations = vec![Observation::new(); 4];
        oracle.observations[0] = Observation {
            block_timestamp: initial_time,
            sqrt_price: initial_price,
            tick_cumulative: 0,
            seconds_per_liquidity_cumulative: 0,
            initialized: true,
        };

        // Write the second observation again
        let result = oracle.write(second_time, second_price, tick, liquidity);
        assert!(result.is_ok());

        // Verify the second observation was written properly
        assert_eq!(oracle.observation_index, 1);
        let observation = &oracle.observations[1];
        assert_eq!(observation.block_timestamp, second_time);
        assert_eq!(observation.sqrt_price, second_price);

        // Calculate expected tick_cumulative
        // Time elapsed = 60 seconds, tick = 10
        // tick_cumulative = 0 + (10 * 60) = 600
        assert_eq!(observation.tick_cumulative, 600);

        // Write a third observation
        let third_time = 1120; // Another minute later
        let third_price = (1 << 96) + 2000000;
        let tick = 20;

        let result = oracle.write(third_time, third_price, tick, liquidity);
        assert!(result.is_ok());

        // Verify the third observation
        assert_eq!(oracle.observation_index, 2);
        let observation = &oracle.observations[2];
        assert_eq!(observation.block_timestamp, third_time);
        assert_eq!(observation.sqrt_price, third_price);

        // Calculate expected tick_cumulative
        // Previous tick_cumulative = 600
        // Time elapsed = 60 seconds, tick = 20
        // tick_cumulative = 600 + (20 * 60) = 1800
        assert_eq!(observation.tick_cumulative, 1800);
    }

    #[test]
    fn test_get_last_observation() {
        // Create a sequence of observations
        let observations = create_observation_sequence(1000, 5, 60, 1000000, 10, 1000000);

        // Initialize oracle with the observations
        let oracle = create_populated_oracle(&observations, 8);

        // Get the most recent observation
        let last_observation = oracle.get_last_observation().unwrap();

        // Verify it matches the last observation we wrote
        let (expected_time, expected_price, _, _) = observations.last().unwrap();
        assert_eq!(last_observation.block_timestamp, *expected_time);
        assert_eq!(last_observation.sqrt_price, *expected_price);
    }

    #[test]
    fn test_calculate_twap() {
        // Create a sequence of observations with consistent price growth
        let start_time = 1000;
        let time_step = 60; // 1 minute between observations
        let price_step = 1000000; // Consistent price increase
        let tick_step = 10; // Consistent tick increase
        let liquidity = 1000000;

        let observations =
            create_observation_sequence(start_time, 5, time_step, price_step, tick_step, liquidity);

        // Initialize oracle with the observations
        let oracle = create_populated_oracle(&observations, 8);

        // Calculate TWAP for the entire period
        let start = start_time;
        let end = start_time + 4 * time_step;

        let twap = oracle.calculate_twap(end, end - start).unwrap();

        // We can't exactly verify the TWAP value due to the tick_to_sqrt_price approximation,
        // but we can check it's a reasonable value between the starting and ending prices
        let (_, start_price, _, _) = observations[0];
        let (_, end_price, _, _) = observations[4];

        assert!(twap >= start_price);
        assert!(twap <= end_price);

        // Test TWAP over a smaller window
        let partial_twap = oracle
            .calculate_twap(start + 3 * time_step, time_step)
            .unwrap();

        // Should be close to the price at time start + 2 * time_step
        let (_, expected_price, _, _) = observations[2];

        // Allow for some approximation error
        let diff = if partial_twap > expected_price {
            partial_twap - expected_price
        } else {
            expected_price - partial_twap
        };

        // Verify difference is reasonably small (less than 1%)
        assert!(diff < expected_price / 100);
    }

    #[test]
    fn test_oracle_cardinality_management() {
        let mut oracle = create_mock_oracle();

        // Initialize with small cardinality
        oracle.initialize(mock_pubkey(1), 1000, 1 << 96).unwrap();
        assert_eq!(oracle.observation_cardinality, 1);

        // Increase target cardinality
        let result = oracle.increase_cardinality(4);
        assert!(result.is_ok());
        assert_eq!(oracle.observation_cardinality, 1); // Hasn't changed yet
        assert_eq!(oracle.observation_cardinality_next, 4); // Target updated

        // Grow observations to match target
        let result = oracle.grow_observations();
        assert!(result.is_ok());
        assert_eq!(oracle.observation_cardinality, 4); // Now matches target
        assert_eq!(oracle.observations.len(), 4);

        // Try to grow again (should be no-op since already at target)
        let result = oracle.grow_observations();
        assert!(result.is_ok());
        assert_eq!(oracle.observation_cardinality, 4); // Unchanged

        // Try to increase beyond maximum
        let result = oracle.increase_cardinality(MAX_ORACLE_OBSERVATIONS as u16 + 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            ErrorCode::OracleCardinalityTooLarge.to_string()
        );

        // Try to increase to same or smaller value (should be no-op)
        let result = oracle.increase_cardinality(4);
        assert!(result.is_ok());
        assert_eq!(oracle.observation_cardinality_next, 4); // Unchanged

        let result = oracle.increase_cardinality(2);
        assert!(result.is_ok());
        assert_eq!(oracle.observation_cardinality_next, 4); // Unchanged
    }

    // ========== Compression Tests ==========

    #[test]
    fn test_enable_compression() {
        // Create sequence of observations
        let observations = create_observation_sequence(1000, 5, 60, 1000000, 10, 1000000);

        // Initialize oracle with the observations
        let mut oracle = create_populated_oracle(&observations, 8);

        // Verify compression is disabled initially
        assert_eq!(oracle.compression_enabled, false);
        assert_eq!(oracle.compressed_observations.len(), 0);

        // Enable compression
        let result = oracle.enable_compression();
        assert!(result.is_ok());

        // Verify compression is now enabled
        assert_eq!(oracle.compression_enabled, true);
        assert_eq!(oracle.compressed_observations.len(), 8);

        // Check that the compressed observations were created
        let last_index = oracle.observation_index as usize;
        let last_compressed = &oracle.compressed_observations[last_index];

        // We can't easily check the exact compression values without duplicating the logic,
        // but we can at least verify the first observation has the initialized flag set
        assert_eq!(last_compressed.flags & 1, 1);

        // Calculate compression ratio
        let ratio = oracle.calculate_compression_ratio();
        assert!(ratio > 0.0); // Should achieve some compression
    }

    #[test]
    fn test_write_with_compression_enabled() {
        // Create sequence of observations
        let observations = create_observation_sequence(1000, 3, 60, 1000000, 10, 1000000);

        // Initialize oracle with the observations
        let mut oracle = create_populated_oracle(&observations, 8);

        // Enable compression
        oracle.enable_compression().unwrap();

        // Write a new observation
        let new_time = 1180;
        let new_price = (1 << 96) + 3000000;
        let new_tick = 30;
        let liquidity = 1000000;

        let result = oracle.write(new_time, new_price, new_tick, liquidity);
        assert!(result.is_ok());

        // Verify both regular and compressed observations were updated
        let new_index = oracle.observation_index as usize;
        let new_observation = &oracle.observations[new_index];
        let new_compressed = &oracle.compressed_observations[new_index];

        assert_eq!(new_observation.block_timestamp, new_time);
        assert_eq!(new_observation.sqrt_price, new_price);
        assert_eq!(new_compressed.flags & 1, 1);
    }

    // ========== Security Tests ==========

    #[test]
    fn test_observation_timestamp_ordering() {
        let mut oracle = create_mock_oracle();

        // Initialize oracle
        oracle.initialize(mock_pubkey(1), 1000, 1 << 96).unwrap();

        // Try to write an observation with the same timestamp
        let result = oracle.write(1000, (1 << 96) + 1000, 10, 1000000);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            ErrorCode::OracleInvalidTimestamp.to_string()
        );

        // Try to write an observation with an earlier timestamp
        let result = oracle.write(999, (1 << 96) + 1000, 10, 1000000);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            ErrorCode::OracleInvalidTimestamp.to_string()
        );

        // Write with a later timestamp should succeed
        let result = oracle.write(1001, (1 << 96) + 1000, 10, 1000000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_oracle_overflow_protection() {
        let mut oracle = create_mock_oracle();

        // Initialize oracle with large initial values
        let initial_time = 1000;
        let initial_price = 1 << 96;
        oracle
            .initialize(mock_pubkey(1), initial_time, initial_price)
            .unwrap();

        // Ensure the observations array has capacity
        oracle.observation_cardinality = 4;
        oracle.observation_cardinality_next = 4;
        oracle.observations = vec![Observation::new(); 4];
        oracle.observations[0] = Observation {
            block_timestamp: initial_time,
            sqrt_price: initial_price,
            tick_cumulative: 0,
            seconds_per_liquidity_cumulative: 0,
            initialized: true,
        };

        // Try to create an observation that would cause tick_cumulative overflow
        let huge_time_delta = u32::MAX;
        let huge_tick = i32::MAX;

        let result = oracle.write(
            initial_time + huge_time_delta,
            initial_price,
            huge_tick,
            1000000,
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), ErrorCode::MathOverflow.to_string());

        // Try with max liquidity (would cause overflow in seconds_per_liquidity calculation)
        let result = oracle.write(initial_time + 100, initial_price, 10, u128::MAX);
        assert!(result.is_ok()); // Should handle max liquidity gracefully

        // Try with zero liquidity (special case)
        let result = oracle.write(initial_time + 200, initial_price, 10, 0);
        assert!(result.is_ok());
        // When liquidity is zero, seconds_per_liquidity_delta should be 0
        let observation = &oracle.observations[oracle.observation_index as usize];
        let prev_observation = &oracle.observations[0];
        assert_eq!(
            observation.seconds_per_liquidity_cumulative,
            prev_observation.seconds_per_liquidity_cumulative
        );
    }

    #[test]
    fn test_surrounding_observations_logic() {
        // Create observations with significant time gaps
        let mut observations = Vec::new();

        // Create widely spaced observations
        observations.push((1000, 1 << 96, 10, 1000000)); // t=1000
        observations.push((2000, (1 << 96) + 1000000, 20, 1000000)); // t=2000
        observations.push((4000, (1 << 96) + 2000000, 30, 1000000)); // t=4000
        observations.push((8000, (1 << 96) + 3000000, 40, 1000000)); // t=8000

        // Initialize oracle with the observations
        let oracle = create_populated_oracle(&observations, 8);

        // Test exact timestamp matches
        let (observation_before, observation_after) =
            oracle.get_surrounding_observations(1000).unwrap();
        assert_eq!(observation_before.block_timestamp, 1000);
        assert_eq!(observation_after.block_timestamp, 2000);

        // Test timestamp between observations
        let (observation_before, observation_after) =
            oracle.get_surrounding_observations(3000).unwrap();
        assert_eq!(observation_before.block_timestamp, 2000);
        assert_eq!(observation_after.block_timestamp, 4000);

        // Test timestamp before earliest observation
        let (observation_before, observation_after) =
            oracle.get_surrounding_observations(500).unwrap();
        assert_eq!(observation_before.block_timestamp, 1000);
        assert_eq!(observation_after.block_timestamp, 2000);

        // Test timestamp after latest observation
        let (observation_before, observation_after) =
            oracle.get_surrounding_observations(9000).unwrap();
        assert_eq!(observation_before.block_timestamp, 4000);
        assert_eq!(observation_after.block_timestamp, 8000);
    }

    #[test]
    fn test_twap_manipulation_resistance() {
        let mut oracle = create_mock_oracle();

        // Initialize with a consistent price
        oracle.initialize(mock_pubkey(1), 1000, 1 << 96).unwrap();

        // Ensure the observations array has capacity
        oracle.observation_cardinality = 10;
        oracle.observation_cardinality_next = 10;
        oracle.observations = vec![Observation::new(); 10];
        oracle.observations[0] = Observation {
            block_timestamp: 1000,
            sqrt_price: 1 << 96,
            tick_cumulative: 0,
            seconds_per_liquidity_cumulative: 0,
            initialized: true,
        };

        // Write several observations with stable price
        let base_price = 1 << 96;
        let base_tick = 0;
        let liquidity = 1000000;

        for i in 1..5 {
            let time = 1000 + i * 60;
            oracle
                .write(time, base_price, base_tick, liquidity)
                .unwrap();
        }

        // Try to manipulate with a price spike
        let manipulation_time = 1300;
        let manipulation_price = base_price * 10; // 10x price spike
        let manipulation_tick = 23026; // ln(10)/ln(1.0001) ≈ 23026

        oracle
            .write(
                manipulation_time,
                manipulation_price,
                manipulation_tick,
                liquidity,
            )
            .unwrap();

        // Quickly revert to normal price
        let reversion_time = 1310; // Just 10 seconds later
        oracle
            .write(reversion_time, base_price, base_tick, liquidity)
            .unwrap();

        // Calculate TWAP over the entire period
        let twap = oracle.calculate_twap(1310, 310).unwrap(); // 310 seconds = entire period

        // The TWAP should be close to the base price since the manipulation was brief
        // Since the manipulation lasted 10 out of 310 seconds, the impact should be limited
        let max_expected_deviation = base_price / 10; // Allow up to 10% deviation

        let diff = if twap > base_price {
            twap - base_price
        } else {
            base_price - twap
        };

        assert!(diff < max_expected_deviation);
    }

    #[test]
    fn test_tick_to_sqrt_price() {
        // Test conversion from tick to sqrt price

        // Tick 0 should return 1.0 in Q64.96 format
        let result = tick_to_sqrt_price(0).unwrap();
        assert_eq!(result, 1 << 96);

        // Test positive ticks
        let result = tick_to_sqrt_price(1).unwrap();
        // 1.0001^0.5 ≈ 1.00005 in Q64.96
        assert!(result > 1 << 96);

        // Test negative ticks
        let result = tick_to_sqrt_price(-1).unwrap();
        // 1.0001^-0.5 ≈ 0.99995 in Q64.96
        assert!(result < 1 << 96);

        // Test boundary ticks
        let min_result = tick_to_sqrt_price(MIN_TICK).unwrap();
        assert!(min_result > 0);

        let max_result = tick_to_sqrt_price(MAX_TICK).unwrap();
        assert!(max_result > 0);
    }

    // ========== Edge Case Tests ==========

    #[test]
    fn test_oracle_size_calculation() {
        // Verify size calculation is accurate
        let size_for_1 = Oracle::size(1);
        let size_for_10 = Oracle::size(10);
        let size_difference = size_for_10 - size_for_1;

        // The difference should be 9 times the size of an Observation plus 9 times the size of a CompressedObservation
        let expected_difference =
            9 * (std::mem::size_of::<Observation>() + std::mem::size_of::<CompressedObservation>());
        assert_eq!(size_difference, expected_difference);
    }

    #[test]
    fn test_circular_buffer_behavior() {
        let mut oracle = create_mock_oracle();

        // Initialize with small cardinality
        oracle.initialize(mock_pubkey(1), 1000, 1 << 96).unwrap();

        // Set cardinality to 4
        oracle.observation_cardinality = 4;
        oracle.observation_cardinality_next = 4;
        oracle.observations = vec![Observation::new(); 4];
        oracle.observations[0] = Observation {
            block_timestamp: 1000,
            sqrt_price: 1 << 96,
            tick_cumulative: 0,
            seconds_per_liquidity_cumulative: 0,
            initialized: true,
        };

        // Write 8 observations (more than cardinality)
        let base_price = 1 << 96;
        let base_tick = 0;
        let liquidity = 1000000;

        for i in 1..8 {
            let time = 1000 + i * 60;
            let price = base_price + i as u128 * 1000;
            oracle.write(time, price, base_tick, liquidity).unwrap();
        }

        // Verify the circular buffer behavior
        // After writing 8 observations to a 4-capacity buffer,
        // we should have the most recent 4 observations (indices 4,5,6,7)

        // Get most recent observation
        let last_observation = oracle.get_last_observation().unwrap();
        assert_eq!(last_observation.block_timestamp, 1000 + 7 * 60);

        // Try to calculate TWAP
        let twap = oracle.calculate_twap(1000 + 7 * 60, 4 * 60).unwrap();
        assert!(twap > 0); // Should be able to calculate TWAP over the observable window
    }

    #[test]
    fn test_insufficient_data_handling() {
        let mut oracle = create_mock_oracle();

        // Initialize with just one observation
        oracle.initialize(mock_pubkey(1), 1000, 1 << 96).unwrap();

        // Try to calculate TWAP with insufficient data
        let result = oracle.calculate_twap(1060, 60);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            ErrorCode::OracleInsufficientData.to_string()
        );

        // Add another observation
        oracle.observation_cardinality = 2;
        oracle.observation_cardinality_next = 2;
        oracle.observations = vec![Observation::new(); 2];
        oracle.observations[0] = Observation {
            block_timestamp: 1000,
            sqrt_price: 1 << 96,
            tick_cumulative: 0,
            seconds_per_liquidity_cumulative: 0,
            initialized: true,
        };

        oracle.write(1060, (1 << 96) + 1000, 10, 1000000).unwrap();

        // Now we should be able to calculate TWAP
        let result = oracle.calculate_twap(1060, 60);
        assert!(result.is_ok());
    }

    // ========== Integration-Style Tests ==========

    #[test]
    fn test_oracle_lifecycle() {
        // Test the full lifecycle of an oracle
        let mut oracle = create_mock_oracle();

        // 1. Initialize
        oracle.initialize(mock_pubkey(1), 1000, 1 << 96).unwrap();

        // 2. Increase cardinality
        oracle.increase_cardinality(4).unwrap();
        oracle.grow_observations().unwrap();

        // 3. Add observations with varying prices
        let liquidity = 1000000;

        // Rising price trend
        oracle.write(1060, (1 << 96) + 1000, 1, liquidity).unwrap();
        oracle.write(1120, (1 << 96) + 2000, 2, liquidity).unwrap();
        oracle.write(1180, (1 << 96) + 3000, 3, liquidity).unwrap();

        // 4. Calculate TWAP during price rise
        let twap_rising = oracle.calculate_twap(1180, 180).unwrap();

        // 5. Enable compression
        oracle.enable_compression().unwrap();

        // 6. Continue adding observations with falling prices
        oracle.write(1240, (1 << 96) + 2500, 2, liquidity).unwrap();
        oracle.write(1300, (1 << 96) + 2000, 1, liquidity).unwrap();
        oracle.write(1360, (1 << 96) + 1500, 0, liquidity).unwrap();

        // 7. Calculate TWAP during price fall
        let twap_falling = oracle.calculate_twap(1360, 180).unwrap();

        // 8. Increase cardinality again
        oracle.increase_cardinality(8).unwrap();
        oracle.grow_observations().unwrap();

        // 9. Add more observations with stable price
        oracle.write(1420, (1 << 96) + 1500, 0, liquidity).unwrap();
        oracle.write(1480, (1 << 96) + 1500, 0, liquidity).unwrap();

        // 10. Calculate TWAP during stable period
        let twap_stable = oracle.calculate_twap(1480, 120).unwrap();

        // Verify TWAPs reflect the appropriate trends
        assert!(twap_rising > 1 << 96); // Rising TWAP should be above 1.0
        assert!(twap_falling < twap_rising); // Falling TWAP should be lower than rising TWAP
        assert_eq!(twap_stable, (1 << 96) + 1500); // Stable TWAP should equal the stable price

        // Check compression ratio
        let compression_ratio = oracle.calculate_compression_ratio();
        assert!(compression_ratio > 0.0);
    }

    #[test]
    fn test_oracle_with_extreme_values() {
        let mut oracle = create_mock_oracle();

        // Initialize with normal values
        oracle.initialize(mock_pubkey(1), 1000, 1 << 96).unwrap();
        oracle.observation_cardinality = 4;
        oracle.observation_cardinality_next = 4;
        oracle.observations = vec![Observation::new(); 4];
        oracle.observations[0] = Observation {
            block_timestamp: 1000,
            sqrt_price: 1 << 96,
            tick_cumulative: 0,
            seconds_per_liquidity_cumulative: 0,
            initialized: true,
        };

        // Test with extreme price values
        let min_price = MIN_SQRT_PRICE;
        let max_price = MAX_SQRT_PRICE;

        // Write observation with minimum price
        oracle.write(1060, min_price, MIN_TICK, 1000000).unwrap();

        // Write observation with maximum price
        oracle.write(1120, max_price, MAX_TICK, 1000000).unwrap();

        // Calculate TWAP over extreme price movement
        let twap = oracle.calculate_twap(1120, 120).unwrap();

        // TWAP should be between min and max price
        assert!(twap >= min_price);
        assert!(twap <= max_price);

        // Test with zero liquidity
        oracle.write(1180, 1 << 96, 0, 0).unwrap();

        // Final TWAP should still work
        let final_twap = oracle.calculate_twap(1180, 60).unwrap();
        assert!(final_twap > 0);
    }

    #[test]
    fn test_oracle_with_high_frequency_updates() {
        let mut oracle = create_mock_oracle();

        // Initialize
        oracle.initialize(mock_pubkey(1), 1000, 1 << 96).unwrap();
        oracle.observation_cardinality = 64; // Use maximum cardinality
        oracle.observation_cardinality_next = 64;
        oracle.observations = vec![Observation::new(); 64];
        oracle.observations[0] = Observation {
            block_timestamp: 1000,
            sqrt_price: 1 << 96,
            tick_cumulative: 0,
            seconds_per_liquidity_cumulative: 0,
            initialized: true,
        };

        // Write many observations with small time gaps (high frequency)
        let base_price = 1 << 96;
        let liquidity = 1000000;

        for i in 1..100 {
            let time = 1000 + i; // 1 second intervals
            let price = base_price + i as u128 * 100;
            let tick = i as i32 / 10; // Small tick changes

            let result = oracle.write(time, price, tick, liquidity);
            assert!(result.is_ok());
        }

        // Check that we can calculate TWAP over different windows
        let short_window_twap = oracle.calculate_twap(1090, 10).unwrap(); // 10 second window
        let medium_window_twap = oracle.calculate_twap(1090, 50).unwrap(); // 50 second window
        let long_window_twap = oracle.calculate_twap(1090, 90).unwrap(); // 90 second window

        // Verify TWAPs are reasonable (increasing with time window since price is rising)
        assert!(short_window_twap > medium_window_twap);
        assert!(medium_window_twap > long_window_twap);
    }

    #[test]
    fn test_twap_calculation_with_different_windows() {
        // Create a sequence with varying price trends
        let mut oracle = create_mock_oracle();

        // Initialize
        oracle.initialize(mock_pubkey(1), 1000, 1 << 96).unwrap();
        oracle.observation_cardinality = 10;
        oracle.observation_cardinality_next = 10;
        oracle.observations = vec![Observation::new(); 10];
        oracle.observations[0] = Observation {
            block_timestamp: 1000,
            sqrt_price: 1 << 96,
            tick_cumulative: 0,
            seconds_per_liquidity_cumulative: 0,
            initialized: true,
        };

        let liquidity = 1000000;
        let base_price = 1 << 96;

        // Initial rising trend
        oracle.write(1100, base_price + 1000, 1, liquidity).unwrap();
        oracle.write(1200, base_price + 2000, 2, liquidity).unwrap();

        // Sudden price drop
        oracle
            .write(1300, base_price - 1000, -1, liquidity)
            .unwrap();
        oracle
            .write(1400, base_price - 2000, -2, liquidity)
            .unwrap();

        // Recovery
        oracle.write(1500, base_price, 0, liquidity).unwrap();

        // Calculate TWAPs over different windows

        // 1. Short window during drop: 1300-1400
        let drop_twap = oracle.calculate_twap(1400, 100).unwrap();

        // 2. Medium window including drop and some recovery: 1300-1500
        let recovery_twap = oracle.calculate_twap(1500, 200).unwrap();

        // 3. Long window over the entire period: 1000-1500
        let full_twap = oracle.calculate_twap(1500, 500).unwrap();

        // Verify the TWAPs reflect the corresponding trends
        assert!(drop_twap < base_price); // Should show price drop
        assert!(recovery_twap < drop_twap); // Should show continuing overall decline
        assert!(full_twap > recovery_twap); // Should be higher due to including initial rise
    }

    #[test]
    fn test_oracle_with_varying_liquidity() {
        let mut oracle = create_mock_oracle();

        // Initialize
        oracle.initialize(mock_pubkey(1), 1000, 1 << 96).unwrap();
        oracle.observation_cardinality = 4;
        oracle.observation_cardinality_next = 4;
        oracle.observations = vec![Observation::new(); 4];
        oracle.observations[0] = Observation {
            block_timestamp: 1000,
            sqrt_price: 1 << 96,
            tick_cumulative: 0,
            seconds_per_liquidity_cumulative: 0,
            initialized: true,
        };

        let base_price = 1 << 96;

        // Write observations with varying liquidity
        // High liquidity
        oracle.write(1100, base_price + 1000, 1, 10000000).unwrap();

        // Medium liquidity
        oracle.write(1200, base_price + 2000, 2, 1000000).unwrap();

        // Low liquidity
        oracle.write(1300, base_price + 3000, 3, 100000).unwrap();

        // Calculate TWAP
        let twap = oracle.calculate_twap(1300, 300).unwrap();

        // TWAP should be based on time and ticks, not directly affected by liquidity
        assert!(twap > base_price); // Should show overall rising trend

        // But seconds_per_liquidity should be affected by liquidity
        let last_observation = oracle.get_last_observation().unwrap();
        let first_observation = &oracle.observations[0];

        // Lower liquidity should lead to higher seconds_per_liquidity_cumulative
        assert!(
            last_observation.seconds_per_liquidity_cumulative
                > first_observation.seconds_per_liquidity_cumulative
        );
    }

    // ========== Security-Focused Tests ==========

    #[test]
    fn test_oracle_data_consistency() {
        let mut oracle = create_mock_oracle();

        // Initialize with stable values
        oracle.initialize(mock_pubkey(1), 1000, 1 << 96).unwrap();
        oracle.observation_cardinality = 4;
        oracle.observation_cardinality_next = 4;
        oracle.observations = vec![Observation::new(); 4];
        oracle.observations[0] = Observation {
            block_timestamp: 1000,
            sqrt_price: 1 << 96,
            tick_cumulative: 0,
            seconds_per_liquidity_cumulative: 0,
            initialized: true,
        };

        // Write a sequence of observations
        let liquidity = 1000000;

        for i in 1..4 {
            let time = 1000 + i * 100;
            let price = (1 << 96) + i as u128 * 1000;
            let tick = i as i32;

            oracle.write(time, price, tick, liquidity).unwrap();
        }

        // Verify cumulative values are increasing monotonically
        let mut last_tick_cumulative = 0;
        let mut last_seconds_cumulative = 0;

        for i in 0..4 {
            let observation = &oracle.observations[i];
            if observation.initialized {
                assert!(observation.tick_cumulative >= last_tick_cumulative);
                assert!(observation.seconds_per_liquidity_cumulative >= last_seconds_cumulative);

                last_tick_cumulative = observation.tick_cumulative;
                last_seconds_cumulative = observation.seconds_per_liquidity_cumulative;
            }
        }
    }

    #[test]
    fn test_oracle_manipulation_resistance_over_time() {
        let mut oracle = create_mock_oracle();

        // Initialize
        oracle.initialize(mock_pubkey(1), 1000, 1 << 96).unwrap();
        oracle.observation_cardinality = 64; // Large cardinality for more history
        oracle.observation_cardinality_next = 64;
        oracle.observations = vec![Observation::new(); 64];
        oracle.observations[0] = Observation {
            block_timestamp: 1000,
            sqrt_price: 1 << 96,
            tick_cumulative: 0,
            seconds_per_liquidity_cumulative: 0,
            initialized: true,
        };

        let base_price = 1 << 96;
        let liquidity = 1000000;

        // Create a long sequence of stable price observations
        for i in 1..50 {
            let time = 1000 + i * 60; // 1 minute intervals

            // Add a small random component to price to simulate natural market movement
            let random_component = (i % 5) as u128 * 10;
            let price = base_price + random_component;

            oracle.write(time, price, 0, liquidity).unwrap();
        }

        // Calculate baseline TWAP
        let baseline_twap = oracle.calculate_twap(3940, 2940).unwrap(); // 49 minute period

        // Now simulate a price manipulation attack
        // Attacker pushes price to 10x for a short period

        // First, store the current oracle state
        let original_observation_index = oracle.observation_index;
        let original_observation = oracle.observations[original_observation_index as usize].clone();

        // Add manipulation spike
        oracle
            .write(4000, base_price * 10, 23026, liquidity)
            .unwrap();

        // Quick return to normal
        oracle.write(4060, base_price, 0, liquidity).unwrap();

        // Continue normal market activity
        for i in 1..10 {
            let time = 4060 + i * 60;
            let random_component = (i % 5) as u128 * 10;
            let price = base_price + random_component;

            oracle.write(time, price, 0, liquidity).unwrap();
        }

        // Calculate TWAP for different windows

        // 1. Short window including manipulation
        let short_twap = oracle.calculate_twap(4060, 120).unwrap(); // 2 minute window

        // 2. Medium window diluting manipulation
        let medium_twap = oracle.calculate_twap(4600, 600).unwrap(); // 10 minute window

        // 3. Long window making manipulation negligible
        let long_twap = oracle.calculate_twap(4600, 3600).unwrap(); // 60 minute window

        // Short window should show significant impact
        assert!(short_twap > base_price * 2); // Substantial deviation

        // Medium window should show moderate impact
        assert!(medium_twap > base_price);
        assert!(medium_twap < base_price * 2); // Moderate deviation

        // Long window should show minimal impact
        assert!(long_twap > base_price);
        let long_window_deviation = long_twap - base_price;
        let short_window_deviation = short_twap - base_price;
        assert!(long_window_deviation < short_window_deviation / 5); // Much smaller deviation
    }

    #[test]
    fn test_error_handling_and_input_validation() {
        let mut oracle = create_mock_oracle();

        // Test uninitialized oracle
        let result = oracle.get_last_observation();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), ErrorCode::OracleNotInitialized.to_string());

        // Initialize oracle
        oracle.initialize(mock_pubkey(1), 1000, 1 << 96).unwrap();

        // Test invalid timestamp (same as last)
        let result = oracle.write(1000, 1 << 96, 0, 1000000);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            ErrorCode::OracleInvalidTimestamp.to_string()
        );

        // Test invalid timestamp (earlier than last)
        let result = oracle.write(999, 1 << 96, 0, 1000000);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            ErrorCode::OracleInvalidTimestamp.to_string()
        );

        // Test insufficient observations for TWAP
        let result = oracle.calculate_twap(1100, 100);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            ErrorCode::OracleInsufficientData.to_string()
        );

        // Write a valid observation
        oracle.write(1100, 1 << 96, 0, 1000000).unwrap();

        // Test invalid TWAP parameters (zero seconds ago)
        let result = oracle.calculate_twap(1100, 0);
        assert!(result.is_ok()); // This should be fine - returns current price

        // Test calculate_twap with insufficient history for requested window
        let result = oracle.calculate_twap(1100, 200); // 200 seconds ago would be t=900, before our first observation
        assert!(result.is_ok()); // Should use the earliest available observation
    }

    #[test]
    fn test_protocol_boundaries_and_constraints() {
        // Test oracle with protocol boundary conditions
        let mut oracle = create_mock_oracle();

        // Initialize with standard values
        oracle.initialize(mock_pubkey(1), 1000, 1 << 96).unwrap();
        oracle.observation_cardinality = 4;
        oracle.observation_cardinality_next = 4;
        oracle.observations = vec![Observation::new(); 4];
        oracle.observations[0] = Observation {
            block_timestamp: 1000,
            sqrt_price: 1 << 96,
            tick_cumulative: 0,
            seconds_per_liquidity_cumulative: 0,
            initialized: true,
        };

        // Test with maximum cardinality
        let result = oracle.increase_cardinality(MAX_ORACLE_OBSERVATIONS as u16);
        assert!(result.is_ok());

        // Test with exceeding maximum cardinality
        let result = oracle.increase_cardinality(MAX_ORACLE_OBSERVATIONS as u16 + 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            ErrorCode::OracleCardinalityTooLarge.to_string()
        );

        // Test with minimum and maximum protocol tick values
        // Write observation at MIN_TICK
        oracle
            .write(1100, MIN_SQRT_PRICE, MIN_TICK, 1000000)
            .unwrap();

        // Write observation at MAX_TICK
        oracle
            .write(1200, MAX_SQRT_PRICE, MAX_TICK, 1000000)
            .unwrap();

        // Calculate TWAP across extreme price range
        let twap = oracle.calculate_twap(1200, 100).unwrap();

        // TWAP should be between MIN and MAX sqrt prices
        assert!(twap >= MIN_SQRT_PRICE);
        assert!(twap <= MAX_SQRT_PRICE);
    }
}
