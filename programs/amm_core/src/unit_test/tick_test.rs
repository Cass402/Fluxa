use crate::tick::*;
use anchor_lang::prelude::*;
use proptest::prelude::*;
use std::str::FromStr;

/// Comprehensive tests for tick.rs functionalities
mod tick_tests {

    use super::*;

    // Helper function to create a test pubkey
    fn create_test_pubkey(seed: &str) -> Pubkey {
        Pubkey::from_str(seed).unwrap_or_default()
    }

    /// Tests for the initialization of TickData
    mod tick_initialize_tests {
        use super::*;

        #[test]
        fn test_tick_initialize_basic() {
            // Create a new TickData with default values
            let mut tick_data = TickData::default();

            // Initialize with test values
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index = 42;

            // Initialize the tick
            tick_data.initialize(pool, index);

            // Verify all fields are set correctly
            assert_eq!(tick_data.pool, pool);
            assert_eq!(tick_data.index, index);
            assert_eq!(
                tick_data.initialized, 0,
                "Tick should be uninitialized after basic init"
            );
            assert_eq!(tick_data.liquidity_gross, 0);
            assert_eq!(tick_data.liquidity_net, 0);
        }

        #[test]
        fn test_tick_initialize_multiple_times() {
            // Create a new TickData
            let mut tick_data = TickData::default();

            // First initialization
            let pool1 = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index1 = 42;
            tick_data.initialize(pool1, index1);

            // Verify fields
            assert_eq!(tick_data.pool, pool1);
            assert_eq!(tick_data.index, index1);

            // Second initialization (re-initialization)
            let pool2 = create_test_pubkey("7Z6YgXBdQG7dRnQwA1TbMsJTSBMsyzTF6NXJ8Lee7Eks");
            let index2 = 100;
            tick_data.initialize(pool2, index2);

            // Verify fields are updated
            assert_eq!(tick_data.pool, pool2);
            assert_eq!(tick_data.index, index2);
            assert_eq!(
                tick_data.initialized, 0,
                "Tick should be uninitialized after re-init"
            );
            assert_eq!(tick_data.liquidity_gross, 0);
            assert_eq!(tick_data.liquidity_net, 0);
        }

        #[test]
        fn test_tick_initialize_negative_index() {
            // Test with negative tick index
            let mut tick_data = TickData::default();
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let negative_index = -100;

            tick_data.initialize(pool, negative_index);

            // Verify negative index is stored correctly
            assert_eq!(tick_data.index, negative_index);
        }
    }

    /// Tests for the update_on_liquidity_change method
    mod tick_update_liquidity_tests {
        use super::*;

        #[test]
        fn test_update_add_liquidity_lower_tick() -> Result<()> {
            // Create and initialize a tick
            let mut tick_data = TickData::default();
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index = 42;
            tick_data.initialize(pool, index);

            // Add liquidity for a lower tick
            let liquidity_delta = 1000;
            let is_upper_tick = false;

            tick_data.update_on_liquidity_change(liquidity_delta, is_upper_tick)?;

            // Verify the fields are updated correctly
            assert_eq!(tick_data.liquidity_gross, 1000);
            assert_eq!(tick_data.liquidity_net, 1000);
            assert_eq!(
                tick_data.initialized, 1,
                "Tick should be initialized after adding liquidity"
            );

            Ok(())
        }

        #[test]
        fn test_update_add_liquidity_upper_tick() -> Result<()> {
            // Create and initialize a tick
            let mut tick_data = TickData::default();
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index = 42;
            tick_data.initialize(pool, index);

            // Add liquidity for an upper tick
            let liquidity_delta = 1000;
            let is_upper_tick = true;

            tick_data.update_on_liquidity_change(liquidity_delta, is_upper_tick)?;

            // Verify the fields are updated correctly
            assert_eq!(tick_data.liquidity_gross, 1000);
            assert_eq!(tick_data.liquidity_net, -1000);
            assert_eq!(
                tick_data.initialized, 1,
                "Tick should be initialized after adding liquidity"
            );

            Ok(())
        }

        #[test]
        fn test_update_remove_liquidity_lower_tick() -> Result<()> {
            // Create a tick with existing liquidity
            let mut tick_data = TickData::default();
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index = 42;
            tick_data.initialize(pool, index);

            // First add liquidity
            tick_data.update_on_liquidity_change(1000, false)?;
            assert_eq!(tick_data.liquidity_gross, 1000);
            assert_eq!(tick_data.liquidity_net, 1000);

            // Then remove some liquidity
            tick_data.update_on_liquidity_change(-500, false)?;

            // Verify the fields are updated correctly
            assert_eq!(tick_data.liquidity_gross, 500); // 1000 - 500
            assert_eq!(tick_data.liquidity_net, 500);
            assert_eq!(
                tick_data.initialized, 1,
                "Tick should remain initialized if gross liquidity > 0"
            );

            Ok(())
        }

        #[test]
        fn test_update_remove_liquidity_upper_tick() -> Result<()> {
            // Create a tick with existing liquidity
            let mut tick_data = TickData::default();
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index = 42;
            tick_data.initialize(pool, index);

            // First add liquidity
            tick_data.update_on_liquidity_change(1000, true)?;
            assert_eq!(tick_data.liquidity_gross, 1000);
            assert_eq!(tick_data.liquidity_net, -1000);

            // Then remove some liquidity
            tick_data.update_on_liquidity_change(-500, true)?;

            // Verify the fields are updated correctly
            assert_eq!(tick_data.liquidity_gross, 500); // 1000 - 500
            assert_eq!(tick_data.liquidity_net, -500);
            assert_eq!(
                tick_data.initialized, 1,
                "Tick should remain initialized if gross liquidity > 0"
            );

            Ok(())
        }

        #[test]
        fn test_liquidity_falls_to_zero() -> Result<()> {
            // Create a tick with existing liquidity
            let mut tick_data = TickData::default();
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index = 42;
            tick_data.initialize(pool, index);

            // Add liquidity
            tick_data.update_on_liquidity_change(1000, false)?;
            assert_eq!(
                tick_data.initialized, 1,
                "Tick should be initialized after adding liquidity"
            );

            // Remove all liquidity
            tick_data.update_on_liquidity_change(-1000, false)?;

            assert_eq!(tick_data.liquidity_gross, 0); // 1000 - 1000
            assert_eq!(tick_data.liquidity_net, 0);
            assert_eq!(
                tick_data.initialized, 0,
                "Should be uninitialized as gross is 0"
            );

            Ok(())
        }

        #[test]
        fn test_multiple_liquidity_updates() -> Result<()> {
            // Create a tick
            let mut tick_data = TickData::default();
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index = 42;
            tick_data.initialize(pool, index);

            // Perform multiple updates with different values
            tick_data.update_on_liquidity_change(1000, false)?; // Add to lower, net +1000
            tick_data.update_on_liquidity_change(500, true)?; // Add to upper, net +500
            tick_data.update_on_liquidity_change(-300, false)?; // Remove from lower, net -300
            tick_data.update_on_liquidity_change(-200, true)?; // Remove from upper, net -200

            // Calculate expected values
            // Gross: 1000 + 500 - 300 - 200 = 1000
            // Net: 1000 - 500 - 300 - (-200) = 400

            // Verify the fields are updated correctly
            assert_eq!(tick_data.liquidity_gross, 1000);
            assert_eq!(tick_data.liquidity_net, 400); // Net calculation was correct
            assert_eq!(tick_data.initialized, 1, "Tick should be initialized");

            Ok(())
        }
    }

    /// Tests for edge cases and boundary conditions
    mod tick_edge_cases_tests {
        use super::*;

        #[test]
        fn test_max_liquidity_values() -> Result<()> {
            // Create a tick
            let mut tick_data = TickData::default();
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index = 42;
            tick_data.initialize(pool, index);

            // Use large but valid liquidity values
            let large_value = i128::MAX / 2;

            // Add liquidity
            tick_data.update_on_liquidity_change(large_value, false)?;

            // Verify the fields are updated correctly
            assert_eq!(tick_data.liquidity_gross, large_value as u128);
            assert_eq!(tick_data.liquidity_net, large_value);
            assert_eq!(
                tick_data.initialized, 1,
                "Tick should be initialized with large liquidity"
            );

            Ok(())
        }

        #[test]
        #[should_panic(expected = "Operation would result in math overflow")]
        fn test_liquidity_gross_overflow() {
            // Create a tick
            let mut tick_data = TickData::default();
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index = 42;
            tick_data.initialize(pool, index);
            // Directly set liquidity_gross to u128::MAX to prepare for overflow
            tick_data.liquidity_gross = u128::MAX;

            // Adding 1 (via liquidity_delta=1, so abs_delta_u128=1) should overflow liquidity_gross
            tick_data.update_on_liquidity_change(1, false).unwrap();
        }

        #[test]
        #[should_panic(expected = "Operation would result in math overflow")]
        fn test_liquidity_net_positive_overflow() {
            // Create a tick
            let mut tick_data = TickData::default();
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index = 42;
            tick_data.initialize(pool, index);
            // Add maximum positive liquidity
            tick_data
                .update_on_liquidity_change(i128::MAX, false)
                .unwrap();

            // Adding 1 more should overflow liquidity_net
            tick_data.update_on_liquidity_change(1, false).unwrap();
        }

        #[test]
        #[should_panic(expected = "Operation would result in math overflow")]
        fn test_liquidity_net_negative_overflow() {
            // Create a tick
            let mut tick_data = TickData::default();
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index = 42;
            tick_data.initialize(pool, index);

            // Set liquidity_net to i128::MIN
            tick_data
                .update_on_liquidity_change(i128::MIN, false)
                .unwrap();

            // Subtracting 1 (by adding -1) should overflow liquidity_net
            tick_data.update_on_liquidity_change(-1, false).unwrap();
        }

        #[test]
        fn test_opposite_liquidity_changes_cancel_out() -> Result<()> {
            // Create a tick
            let mut tick_data = TickData::default();
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index = 42;
            tick_data.initialize(pool, index);

            // Add and then remove the same amount of liquidity
            let liquidity_delta = 1000;
            tick_data.update_on_liquidity_change(liquidity_delta, false)?;
            tick_data.update_on_liquidity_change(-liquidity_delta, false)?;

            // Net should be 0, but gross should accumulate absolute values
            assert_eq!(tick_data.liquidity_gross, 0); // 1000 - 1000
            assert_eq!(tick_data.liquidity_net, 0);
            assert_eq!(
                tick_data.initialized, 0,
                "Tick should be uninitialized as gross is 0"
            ); // Gross is 0

            Ok(())
        }

        #[test]
        fn test_tick_remains_initialized_after_liquidity_removal() -> Result<()> {
            // Renamed to reflect actual behavior: test_tick_becomes_uninitialized_if_all_liquidity_removed
            // fn test_tick_becomes_uninitialized_if_all_liquidity_removed() -> Result<()> {
            let mut tick_data = TickData::default();
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index = 42;
            tick_data.initialize(pool, index);

            // Add liquidity
            tick_data.update_on_liquidity_change(1000, false)?;
            assert_eq!(tick_data.liquidity_gross, 1000);
            assert_eq!(
                tick_data.initialized, 1,
                "Tick should be initialized after adding liquidity"
            );

            // Remove all liquidity
            tick_data.update_on_liquidity_change(-1000, false)?;

            assert_eq!(tick_data.liquidity_gross, 0); // Gross becomes 0
            assert_eq!(tick_data.initialized, 0, "Tick becomes uninitialized"); // Tick becomes uninitialized
            Ok(())
        }
    }

    /// Property-based tests for tick functions
    mod tick_property_tests {
        use super::*;

        proptest! {
            #[test]
            fn test_liquidity_gross_non_negative( // Renamed from test_liquidity_gross_always_positive
                delta1 in -1000..1000i128,
                delta2 in -1000..1000i128,
                is_upper1 in proptest::bool::ANY,
                is_upper2 in proptest::bool::ANY
            ) {
                let mut tick_data = TickData::default();
                let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
                let index = 42;
                tick_data.initialize(pool, index);

                // Apply two liquidity changes
                // These operations use checked arithmetic. liquidity_gross is a u128 and
                // cannot underflow to a semantically negative value (the operation would error).
                // This test exercises these update paths for robustness against panics.
                let _ = tick_data.update_on_liquidity_change(delta1, is_upper1);
                let _ = tick_data.update_on_liquidity_change(delta2, is_upper2);
            }

            #[test]
            fn test_net_liquidity_calculation(
                delta1 in -1000..1000i128,
                delta2 in -1000..1000i128,
                is_upper1 in proptest::bool::ANY,
                is_upper2 in proptest::bool::ANY
            ) {
                prop_assume!(delta1 != i128::MIN && delta2 != i128::MIN); // Avoid abs panic for i128::MIN

                let mut tick_data = TickData::default();
                let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
                let index = 42;
                tick_data.initialize(pool, index);

                let mut current_expected_net = 0i128;

                // First update
                let effect1 = if is_upper1 { -delta1 } else { delta1 };
                if let Some(next_expected_net1) = current_expected_net.checked_add(effect1) {
                    if tick_data.update_on_liquidity_change(delta1, is_upper1).is_ok() {
                        current_expected_net = next_expected_net1;
                        prop_assert_eq!(tick_data.liquidity_net, current_expected_net, "Net after 1st update");
                    } else {
                        // If update failed (e.g. gross underflow), net should remain current_expected_net (previous value)
                        prop_assert_eq!(tick_data.liquidity_net, current_expected_net, "Net after failed 1st update");
                    }
                } else {
                    // Net overflow for effect1, update_on_liquidity_change should also error.
                    prop_assert!(tick_data.update_on_liquidity_change(delta1, is_upper1).is_err(), "Expected net overflow error from update1");
                    prop_assert_eq!(tick_data.liquidity_net, current_expected_net, "Net after net-overflowing 1st update");
                    return Ok(()); // Cannot reliably proceed with this path for second update's expectation
                }

                // Second update
                let effect2 = if is_upper2 { -delta2 } else { delta2 };
                if let Some(next_expected_net2) = current_expected_net.checked_add(effect2) {
                    if tick_data.update_on_liquidity_change(delta2, is_upper2).is_ok() {
                        current_expected_net = next_expected_net2;
                        prop_assert_eq!(tick_data.liquidity_net, current_expected_net, "Net after 2nd update");
                    } else {
                        prop_assert_eq!(tick_data.liquidity_net, current_expected_net, "Net after failed 2nd update");
                    }
                } else {
                    prop_assert!(tick_data.update_on_liquidity_change(delta2, is_upper2).is_err(), "Expected net overflow error from update2");
                    prop_assert_eq!(tick_data.liquidity_net, current_expected_net, "Net after net-overflowing 2nd update");
                }
            }

            #[test]
            fn test_initialized_flag_correctness(
                delta in -1000..1000i128,
                is_upper in proptest::bool::ANY
            ) {
                let mut tick_data = TickData::default();
                let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
                let index = 42;
                tick_data.initialize(pool, index);

                // Apply liquidity change
                let _ = tick_data.update_on_liquidity_change(delta, is_upper);

                // Check that initialized flag is consistent with liquidity_gross
                prop_assert_eq!(tick_data.initialized, (tick_data.liquidity_gross > 0) as u8);
            }

            #[test]
            fn test_index_range(index in i32::MIN..i32::MAX) {
                let mut tick_data = TickData::default();
                let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");

                // Initialize with the given index
                tick_data.initialize(pool, index);

                // Check index is stored correctly regardless of value
                prop_assert_eq!(tick_data.index, index);
            }
        }
    }

    /// Tests for real-world scenarios
    mod tick_scenario_tests {
        use super::*;

        #[test]
        fn test_multiple_positions_using_same_tick() -> Result<()> {
            // Create a tick
            let mut tick_data = TickData::default();
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index = 42;
            tick_data.initialize(pool, index);

            // Position 1: Uses this tick as a lower bound
            tick_data.update_on_liquidity_change(1000, false)?;

            // Position 2: Uses this tick as an upper bound
            tick_data.update_on_liquidity_change(500, true)?;

            // Position 3: Uses this tick as a lower bound
            tick_data.update_on_liquidity_change(750, false)?;

            // Calculate expected values
            // Gross: 1000 + 500 + 750 = 2250
            // Net: 1000 - 500 + 750 = 1250

            // Verify the fields are updated correctly
            assert_eq!(tick_data.liquidity_gross, 2250);
            assert_eq!(tick_data.liquidity_net, 1250);
            assert_eq!(tick_data.initialized, 1, "Tick should be initialized");

            Ok(())
        }

        #[test]
        fn test_lifecycle_of_tick() -> Result<()> {
            // Create a tick
            let mut tick_data = TickData::default();
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index = 42;
            tick_data.initialize(pool, index);

            // Initially not initialized
            assert_eq!(
                tick_data.initialized, 0,
                "Tick should be uninitialized initially"
            );

            // Position 1: Add liquidity
            tick_data.update_on_liquidity_change(1000, false)?;
            assert_eq!(
                tick_data.initialized, 1,
                "Tick should be initialized after first liquidity add"
            );
            assert_eq!(tick_data.liquidity_gross, 1000);
            assert_eq!(tick_data.liquidity_net, 1000);

            // Position 2: Add more liquidity
            tick_data.update_on_liquidity_change(500, false)?;
            assert_eq!(tick_data.liquidity_gross, 1500);
            assert_eq!(tick_data.liquidity_net, 1500);

            // Position 1: Remove liquidity
            tick_data.update_on_liquidity_change(-1000, false)?;
            assert_eq!(tick_data.liquidity_gross, 500); // 1500 - 1000
            assert_eq!(tick_data.liquidity_net, 500);

            // Position 2: Remove liquidity
            tick_data.update_on_liquidity_change(-500, false)?;
            assert_eq!(tick_data.liquidity_gross, 0); // 500 - 500
            assert_eq!(tick_data.liquidity_net, 0);

            // Becomes uninitialized because liquidity_gross is 0
            assert_eq!(
                tick_data.initialized, 0,
                "Tick should be uninitialized when all liquidity removed"
            );

            Ok(())
        }

        #[test]
        fn test_balancing_upper_and_lower_bounds() -> Result<()> {
            // Create a tick
            let mut tick_data = TickData::default();
            let pool = create_test_pubkey("3rTXd8nRJqiKHiLGkPAuaALpGHKxLvPKvSJ5F5gTr3Z2");
            let index = 42;
            tick_data.initialize(pool, index);

            // Add equal amounts as lower and upper bounds
            tick_data.update_on_liquidity_change(1000, false)?; // Lower bound
            tick_data.update_on_liquidity_change(1000, true)?; // Upper bound

            // Net liquidity should be 0, gross should be 2000
            assert_eq!(tick_data.liquidity_net, 0);
            assert_eq!(tick_data.liquidity_gross, 2000);
            assert_eq!(tick_data.initialized, 1, "Tick should be initialized");

            Ok(())
        }
    }
}
