// Tests for the tick bitmap module
//
// This test file provides comprehensive testing for the tick bitmap module, including:
// - Basic bitmap operations (initialization, updates, checks)
// - Edge cases and boundary conditions
// - Security considerations from threat model
// - Performance considerations for critical paths
// - Verification of invariants

use crate::errors::ErrorCode;
use crate::tick_bitmap::*;
use anchor_lang::prelude::*;
use ethereum_types::U256;

#[cfg(test)]
mod tests {
    use super::*;

    // ========== U256Wrapper Tests ==========

    #[test]
    fn test_u256wrapper_creation_and_basics() {
        // Test creation and basic properties
        let zero = U256Wrapper::zero();
        let one = U256Wrapper::from_u32(1);
        let _max = U256Wrapper::max_value(); // Added underscore to avoid unused variable warning

        assert!(zero.is_zero());
        assert!(zero.eq_zero());
        assert!(!one.is_zero());
        assert!(!one.eq_zero());

        // Test value extraction
        assert_eq!(zero.value(), U256::zero());
        assert_eq!(one.value(), U256::from(1u32));

        // Test leading/trailing zeros
        assert_eq!(zero.leading_zeros(), 256);
        assert_eq!(one.trailing_zeros(), 0);
        let two = U256Wrapper::from_u32(2);
        assert_eq!(two.trailing_zeros(), 1);
    }

    #[test]
    fn test_u256wrapper_bitwise_operations() {
        // Test basic bitwise operations
        let a = U256Wrapper::from_u32(0b101);
        let b = U256Wrapper::from_u32(0b110);

        // AND
        let and_result = a & b;
        assert_eq!(and_result.value(), U256::from(0b100u32));

        // OR
        let or_result = a | b;
        assert_eq!(or_result.value(), U256::from(0b111u32));

        // XOR
        let xor_result = a ^ b;
        assert_eq!(xor_result.value(), U256::from(0b011u32));

        // NOT
        let not_result = !a;
        // For a U256, NOT of 5 (101 binary) would set all other bits to 1
        let expected = U256::max_value() - U256::from(0b101u32);
        assert_eq!(not_result.value(), expected);

        // Shift left
        let shift_result = a << 2u8;
        assert_eq!(shift_result.value(), U256::from(0b10100u32));

        // Subtraction
        let sub_result = b - a;
        assert_eq!(sub_result.value(), U256::from(0b001u32));
    }

    #[test]
    fn test_u256wrapper_serialization() {
        // Test serialization/deserialization
        let original = U256Wrapper::from_u32(12345);

        // Serialize
        let mut serialized = Vec::new();
        original
            .serialize(&mut serialized)
            .expect("Serialization failed");
        assert_eq!(serialized.len(), 32); // U256 should serialize to 32 bytes

        // Deserialize
        let mut serialized_slice = serialized.as_slice();
        let deserialized =
            U256Wrapper::deserialize(&mut serialized_slice).expect("Deserialization failed");

        // Verify
        assert_eq!(original.value(), deserialized.value());

        // Test deserialization with reader
        let mut serialized_reader = serialized.as_slice();
        let reader_deserialized = U256Wrapper::deserialize_reader(&mut serialized_reader)
            .expect("Reader deserialization failed");
        assert_eq!(original.value(), reader_deserialized.value());
    }

    #[test]
    fn test_u256wrapper_serialization_error_handling() {
        // Test deserialize with insufficient data
        let too_small = vec![0; 31]; // Only 31 bytes, need 32
        let mut too_small_slice = too_small.as_slice();
        let result = U256Wrapper::deserialize(&mut too_small_slice);
        assert!(result.is_err());
    }

    // ========== TickBitmapWord Tests ==========

    #[test]
    fn test_tickbitmapword_basics() {
        // Test default initialization
        let default_word = TickBitmapWord::default();
        assert!(default_word.bitmap.is_zero());

        // Create a word with single bit set - Fixed initialization
        let word = TickBitmapWord {
            bitmap: U256Wrapper::from_u32(1) << 5, // Set bit at position 5
        };

        // Test is_initialized function
        assert!(is_initialized(&word, 5));
        assert!(!is_initialized(&word, 4));
        assert!(!is_initialized(&word, 6));

        // Test flip_tick function
        let flipped = flip_tick(&word, 5);
        assert!(!is_initialized(&flipped, 5)); // Should be unset now

        let re_flipped = flip_tick(&flipped, 5);
        assert!(is_initialized(&re_flipped, 5)); // Should be set again
    }

    #[test]
    fn test_next_initialized_tick_within_word() {
        // Create a bitmap with bits set at positions 5, 10, and 200 - Fixed initialization
        let word = TickBitmapWord {
            bitmap: (U256Wrapper::from_u32(1) << 5)
                | (U256Wrapper::from_u32(1) << 10)
                | (U256Wrapper::from_u32(1) << 200),
        };

        // Test searching upward (lte = false)
        let (found, pos) = next_initialized_tick_within_word(&word, 4, false).unwrap();
        assert!(found);
        assert_eq!(pos, 5);

        let (found, pos) = next_initialized_tick_within_word(&word, 5, false).unwrap();
        assert!(found);
        assert_eq!(pos, 10);

        let (found, pos) = next_initialized_tick_within_word(&word, 10, false).unwrap();
        assert!(found);
        assert_eq!(pos, 200);

        let (found, _) = next_initialized_tick_within_word(&word, 200, false).unwrap();
        assert!(!found); // No more initialized ticks above 200

        // Test searching downward (lte = true)
        let (found, pos) = next_initialized_tick_within_word(&word, 201, true).unwrap();
        assert!(found);
        assert_eq!(pos, 200);

        let (found, pos) = next_initialized_tick_within_word(&word, 200, true).unwrap();
        assert!(found);
        assert_eq!(pos, 200);

        let (found, pos) = next_initialized_tick_within_word(&word, 15, true).unwrap();
        assert!(found);
        assert_eq!(pos, 10);

        let (found, _) = next_initialized_tick_within_word(&word, 4, true).unwrap();
        assert!(!found); // No initialized ticks at or below 4
    }

    #[test]
    fn test_next_initialized_tick_within_word_edge_cases() {
        // Test with empty bitmap
        let empty_word = TickBitmapWord::default();
        let (found, _) = next_initialized_tick_within_word(&empty_word, 100, false).unwrap();
        assert!(!found);
        let (found, _) = next_initialized_tick_within_word(&empty_word, 100, true).unwrap();
        assert!(!found);

        // Test with only bit 0 set - Fixed initialization
        let word_bit_0 = TickBitmapWord {
            bitmap: U256Wrapper::from_u32(1), // Bit 0 set
        };
        let (found, pos) = next_initialized_tick_within_word(&word_bit_0, 0, true).unwrap();
        assert!(found);
        assert_eq!(pos, 0);

        // Test with only bit 255 set - Fixed initialization
        let word_bit_255 = TickBitmapWord {
            bitmap: U256Wrapper::from_u32(1) << 255, // Bit 255 set
        };
        let (found, pos) = next_initialized_tick_within_word(&word_bit_255, 254, false).unwrap();
        assert!(found);
        assert_eq!(pos, 255);

        // Test with all bits set - Fixed initialization
        let word_all_bits = TickBitmapWord {
            bitmap: U256Wrapper::max_value(), // All bits set
        };
        let (found, pos) = next_initialized_tick_within_word(&word_all_bits, 127, false).unwrap();
        assert!(found);
        assert_eq!(pos, 128);
        let (found, pos) = next_initialized_tick_within_word(&word_all_bits, 127, true).unwrap();
        assert!(found);
        assert_eq!(pos, 127);
    }

    // ========== Position Calculation Tests ==========

    #[test]
    fn test_position_calculation() {
        // Test basic position calculations
        // For tick 0, should be word 0, bit 0
        let (word_pos, bit_pos) = position(0);
        assert_eq!(word_pos, 0);
        assert_eq!(bit_pos, 0);

        // For tick 255, should be word 0, bit 255
        let (word_pos, bit_pos) = position(255);
        assert_eq!(word_pos, 0);
        assert_eq!(bit_pos, 255);

        // For tick 256, should be word 1, bit 0
        let (word_pos, bit_pos) = position(256);
        assert_eq!(word_pos, 1);
        assert_eq!(bit_pos, 0);

        // For tick -1, should be word -1, bit 255
        let (word_pos, bit_pos) = position(-1);
        assert_eq!(word_pos, -1);
        assert_eq!(bit_pos, 255);

        // For tick -256, should be word -1, bit 0
        let (word_pos, bit_pos) = position(-256);
        assert_eq!(word_pos, -1);
        assert_eq!(bit_pos, 0);

        // For tick -257, should be word -2, bit 255
        let (word_pos, bit_pos) = position(-257);
        assert_eq!(word_pos, -2);
        assert_eq!(bit_pos, 255);

        // Test edge cases for large positive and negative values
        let (word_pos, bit_pos) = position(i32::MAX);
        assert_eq!(word_pos, (i32::MAX / WORD_SIZE as i32) as i16);
        assert_eq!(bit_pos, (i32::MAX % WORD_SIZE as i32) as u8);

        let (word_pos, bit_pos) = position(i32::MIN);
        assert_eq!(word_pos, (i32::MIN / WORD_SIZE as i32) as i16);
        assert_eq!(bit_pos, (i32::MIN % WORD_SIZE as i32) as u8);
    }

    // ========== TickBitmap Integration Tests ==========

    #[test]
    fn test_tickbitmap_basic_operations() {
        let mut bitmap = TickBitmap::new();
        let tick_spacing = 10;

        // Test bitmap is initially empty
        assert!(!bitmap.is_tick_initialized(10, tick_spacing));

        // Initialize a tick
        bitmap.update_bitmap(10, tick_spacing, true).unwrap();
        assert!(bitmap.is_tick_initialized(10, tick_spacing));

        // Verify non-initialized ticks return false
        assert!(!bitmap.is_tick_initialized(20, tick_spacing));
        assert!(!bitmap.is_tick_initialized(0, tick_spacing));

        // Uninitialize the tick
        bitmap.update_bitmap(10, tick_spacing, false).unwrap();
        assert!(!bitmap.is_tick_initialized(10, tick_spacing));

        // Test with multiple ticks
        bitmap.update_bitmap(10, tick_spacing, true).unwrap();
        bitmap.update_bitmap(50, tick_spacing, true).unwrap();
        bitmap.update_bitmap(-20, tick_spacing, true).unwrap();

        assert!(bitmap.is_tick_initialized(10, tick_spacing));
        assert!(bitmap.is_tick_initialized(50, tick_spacing));
        assert!(bitmap.is_tick_initialized(-20, tick_spacing));
        assert!(!bitmap.is_tick_initialized(20, tick_spacing));

        // Test that initializing an already initialized tick doesn't change state
        bitmap.update_bitmap(10, tick_spacing, true).unwrap();
        assert!(bitmap.is_tick_initialized(10, tick_spacing));

        // Test that uninitializing an already uninitialized tick doesn't change state
        bitmap.update_bitmap(20, tick_spacing, false).unwrap();
        assert!(!bitmap.is_tick_initialized(20, tick_spacing));
    }

    #[test]
    fn test_tickbitmap_next_initialized_tick() {
        let mut bitmap = TickBitmap::new();
        let tick_spacing = 10;

        // Initialize some ticks
        bitmap.update_bitmap(10, tick_spacing, true).unwrap();
        bitmap.update_bitmap(50, tick_spacing, true).unwrap();
        bitmap.update_bitmap(100, tick_spacing, true).unwrap();

        // Test next_initialized_tick (upward search)
        let next_tick = bitmap.next_initialized_tick(0, tick_spacing).unwrap();
        assert_eq!(next_tick, 10);

        let next_tick = bitmap.next_initialized_tick(10, tick_spacing).unwrap();
        assert_eq!(next_tick, 50);

        let next_tick = bitmap.next_initialized_tick(50, tick_spacing).unwrap();
        assert_eq!(next_tick, 100);

        // Test when we're above all initialized ticks
        let next_tick = bitmap.next_initialized_tick(100, tick_spacing).unwrap();
        assert_eq!(next_tick, 0x7FFFFFFF); // Should return MAX_TICK

        // Test prev_initialized_tick (downward search)
        let prev_tick = bitmap.prev_initialized_tick(120, tick_spacing).unwrap();
        assert_eq!(prev_tick, 100);

        let prev_tick = bitmap.prev_initialized_tick(100, tick_spacing).unwrap();
        assert_eq!(prev_tick, 100);

        let prev_tick = bitmap.prev_initialized_tick(70, tick_spacing).unwrap();
        assert_eq!(prev_tick, 50);

        // Test when we're below all initialized ticks
        let prev_tick = bitmap.prev_initialized_tick(0, tick_spacing).unwrap();
        assert_eq!(prev_tick, -0x80000000); // Should return MIN_TICK
    }

    #[test]
    fn test_tickbitmap_sparse_ticks() {
        let mut bitmap = TickBitmap::new();
        let tick_spacing = 100;

        // Initialize ticks in different words (far apart)
        bitmap.update_bitmap(-50000, tick_spacing, true).unwrap();
        bitmap.update_bitmap(0, tick_spacing, true).unwrap();
        bitmap.update_bitmap(50000, tick_spacing, true).unwrap();

        // Test traversal across words
        let next_tick = bitmap.next_initialized_tick(-60000, tick_spacing).unwrap();
        assert_eq!(next_tick, -50000);

        let next_tick = bitmap.next_initialized_tick(-50000, tick_spacing).unwrap();
        assert_eq!(next_tick, 0);

        let prev_tick = bitmap.prev_initialized_tick(60000, tick_spacing).unwrap();
        assert_eq!(prev_tick, 50000);

        let prev_tick = bitmap.prev_initialized_tick(50000, tick_spacing).unwrap();
        assert_eq!(prev_tick, 50000);

        let prev_tick = bitmap.prev_initialized_tick(40000, tick_spacing).unwrap();
        assert_eq!(prev_tick, 0);
    }

    #[test]
    fn test_tickbitmap_tick_spacing_validation() {
        let mut bitmap = TickBitmap::new();
        let tick_spacing = 10;

        // Test invalid tick spacing (tick not a multiple of spacing)
        let result = bitmap.update_bitmap(15, tick_spacing, true);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), ErrorCode::InvalidTickSpacing.to_string());

        // Valid tick spacing should work
        let result = bitmap.update_bitmap(20, tick_spacing, true);
        assert!(result.is_ok());

        // Check that non-multiple ticks are not considered initialized
        assert!(!bitmap.is_tick_initialized(15, tick_spacing));
    }

    #[test]
    fn test_tickbitmap_boundary_conditions() {
        let mut bitmap = TickBitmap::new();
        let tick_spacing = 1;

        // Test with extremes of tick range
        bitmap
            .update_bitmap(
                i32::MAX - (i32::MAX % tick_spacing as i32),
                tick_spacing,
                true,
            )
            .unwrap();
        bitmap
            .update_bitmap(
                i32::MIN - (i32::MIN % tick_spacing as i32),
                tick_spacing,
                true,
            )
            .unwrap();

        assert!(
            bitmap.is_tick_initialized(i32::MAX - (i32::MAX % tick_spacing as i32), tick_spacing)
        );
        assert!(
            bitmap.is_tick_initialized(i32::MIN - (i32::MIN % tick_spacing as i32), tick_spacing)
        );

        // Test traversal near boundaries
        let next_tick = bitmap
            .next_initialized_tick(i32::MIN + 1, tick_spacing)
            .unwrap();
        assert_eq!(next_tick, i32::MAX - (i32::MAX % tick_spacing as i32));

        let prev_tick = bitmap
            .prev_initialized_tick(i32::MAX - 1, tick_spacing)
            .unwrap();
        assert_eq!(prev_tick, i32::MAX - (i32::MAX % tick_spacing as i32));
    }

    #[test]
    fn test_bitmap_word_boundaries() {
        let mut bitmap = TickBitmap::new();
        let tick_spacing = 1;

        // Initialize ticks at word boundaries
        bitmap.update_bitmap(0, tick_spacing, true).unwrap();
        bitmap
            .update_bitmap(WORD_SIZE as i32 - 1, tick_spacing, true)
            .unwrap();
        bitmap
            .update_bitmap(WORD_SIZE as i32, tick_spacing, true)
            .unwrap();
        bitmap
            .update_bitmap(WORD_SIZE as i32 + 1, tick_spacing, true)
            .unwrap();

        // Verify positions are correct
        assert_eq!(position(0), (0, 0));
        assert_eq!(position(WORD_SIZE as i32 - 1), (0, 255));
        assert_eq!(position(WORD_SIZE as i32), (1, 0));
        assert_eq!(position(WORD_SIZE as i32 + 1), (1, 1));

        // Test traversing across word boundaries
        let next_tick = bitmap.next_initialized_tick(0, tick_spacing).unwrap();
        assert_eq!(next_tick, WORD_SIZE as i32 - 1);

        let next_tick = bitmap
            .next_initialized_tick(WORD_SIZE as i32 - 1, tick_spacing)
            .unwrap();
        assert_eq!(next_tick, WORD_SIZE as i32);

        let prev_tick = bitmap
            .prev_initialized_tick(WORD_SIZE as i32 + 1, tick_spacing)
            .unwrap();
        assert_eq!(prev_tick, WORD_SIZE as i32 + 1);

        let prev_tick = bitmap
            .prev_initialized_tick(WORD_SIZE as i32, tick_spacing)
            .unwrap();
        assert_eq!(prev_tick, WORD_SIZE as i32);

        let prev_tick = bitmap
            .prev_initialized_tick(WORD_SIZE as i32 - 1, tick_spacing)
            .unwrap();
        assert_eq!(prev_tick, WORD_SIZE as i32 - 1);
    }

    // ========== Security Tests ==========

    #[test]
    fn test_bitmap_traversal_security() {
        // This test ensures traversal doesn't cause infinite loops or excessive computation
        let mut bitmap = TickBitmap::new();
        let tick_spacing = 10;

        // Create a sparse bitmap with ticks very far apart
        bitmap.update_bitmap(-1000000, tick_spacing, true).unwrap();
        bitmap.update_bitmap(1000000, tick_spacing, true).unwrap();

        // Ensure traversal works efficiently
        let next_tick = bitmap.next_initialized_tick(0, tick_spacing).unwrap();
        assert_eq!(next_tick, 1000000);

        let prev_tick = bitmap.prev_initialized_tick(0, tick_spacing).unwrap();
        assert_eq!(prev_tick, -1000000);
    }

    #[test]
    fn test_multiple_updates_consistency() {
        // Test that multiple updates maintain consistency
        let mut bitmap = TickBitmap::new();
        let tick_spacing = 10;

        // Initialize and uninitialize the same tick multiple times
        for _ in 0..10 {
            bitmap.update_bitmap(100, tick_spacing, true).unwrap();
            assert!(bitmap.is_tick_initialized(100, tick_spacing));

            bitmap.update_bitmap(100, tick_spacing, false).unwrap();
            assert!(!bitmap.is_tick_initialized(100, tick_spacing));
        }
    }

    #[test]
    fn test_bitmap_data_integrity() {
        // Ensure bitmap data isn't corrupted by operations
        let mut bitmap = TickBitmap::new();
        let tick_spacing = 10;

        // Initialize several ticks
        let test_ticks = vec![-1000, -500, 0, 500, 1000];
        for &tick in &test_ticks {
            bitmap.update_bitmap(tick, tick_spacing, true).unwrap();
        }

        // Verify all ticks are initialized
        for &tick in &test_ticks {
            assert!(bitmap.is_tick_initialized(tick, tick_spacing));
        }

        // Update middle tick
        bitmap.update_bitmap(0, tick_spacing, false).unwrap();

        // Verify only the middle tick is uninitialized
        for &tick in &test_ticks {
            if tick == 0 {
                assert!(!bitmap.is_tick_initialized(tick, tick_spacing));
            } else {
                assert!(bitmap.is_tick_initialized(tick, tick_spacing));
            }
        }

        // Re-initialize middle tick
        bitmap.update_bitmap(0, tick_spacing, true).unwrap();

        // Verify all ticks are initialized again
        for &tick in &test_ticks {
            assert!(bitmap.is_tick_initialized(tick, tick_spacing));
        }
    }

    #[test]
    fn test_high_traffic_simulation() {
        // Test performance and correctness with many tick updates (simulating high activity)
        let mut bitmap = TickBitmap::new();
        let tick_spacing = 1;

        // Initialize a range of ticks
        let start_tick = -1000;
        let end_tick = 1000;

        for tick in (start_tick..=end_tick).step_by(tick_spacing as usize) {
            bitmap.update_bitmap(tick, tick_spacing, true).unwrap();
        }

        // Verify each tick is initialized
        for tick in (start_tick..=end_tick).step_by(tick_spacing as usize) {
            assert!(bitmap.is_tick_initialized(tick, tick_spacing));
        }

        // Test traversal through densely populated bitmap
        let mut current = start_tick - 1;

        while current < end_tick {
            let next = bitmap.next_initialized_tick(current, tick_spacing).unwrap();

            // Verify the next tick is within our range and is the expected next value
            assert!(next > current);
            assert!(next <= end_tick);

            if current >= start_tick - 1 && current < end_tick {
                assert_eq!(next, current + tick_spacing as i32);
            }

            current = next;
        }
    }

    #[test]
    fn test_random_access_patterns() {
        let mut bitmap = TickBitmap::new();
        let tick_spacing = 10;

        // Create a pattern of ticks with specific gaps to test traversal
        let ticks = vec![-1000, -700, -300, 0, 400, 750, 1200];

        for &tick in &ticks {
            bitmap.update_bitmap(tick, tick_spacing, true).unwrap();
        }

        // Test forward traversal from various points
        assert_eq!(
            bitmap.next_initialized_tick(-2000, tick_spacing).unwrap(),
            -1000
        );
        assert_eq!(
            bitmap.next_initialized_tick(-1000, tick_spacing).unwrap(),
            -700
        );
        assert_eq!(
            bitmap.next_initialized_tick(-500, tick_spacing).unwrap(),
            -300
        );
        assert_eq!(
            bitmap.next_initialized_tick(200, tick_spacing).unwrap(),
            400
        );
        assert_eq!(
            bitmap.next_initialized_tick(800, tick_spacing).unwrap(),
            1200
        );

        // Test backward traversal from various points
        assert_eq!(
            bitmap.prev_initialized_tick(2000, tick_spacing).unwrap(),
            1200
        );
        assert_eq!(
            bitmap.prev_initialized_tick(1000, tick_spacing).unwrap(),
            750
        );
        assert_eq!(
            bitmap.prev_initialized_tick(500, tick_spacing).unwrap(),
            400
        );
        assert_eq!(
            bitmap.prev_initialized_tick(-100, tick_spacing).unwrap(),
            -300
        );
        assert_eq!(
            bitmap.prev_initialized_tick(-800, tick_spacing).unwrap(),
            -1000
        );
    }

    // ========== Edge Case and Error Handling Tests ==========

    #[test]
    fn test_deserialize_error_handling() {
        // Test error handling in deserialization with malformed data
        let too_small = vec![0; 16]; // Only 16 bytes, need 32
        let mut too_small_slice = too_small.as_slice();
        let result = U256Wrapper::deserialize(&mut too_small_slice);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_tick_spacing() {
        let mut bitmap = TickBitmap::new();

        // Test with tick spacing of 0 (should be invalid)
        let result = bitmap.update_bitmap(100, 0, true);
        assert!(result.is_err());

        // Test with tick not a multiple of spacing
        let result = bitmap.update_bitmap(105, 10, true);
        assert!(result.is_err());

        // Valid tick and spacing should succeed
        let result = bitmap.update_bitmap(100, 10, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_overflow_underflow_protection() {
        let mut bitmap = TickBitmap::new();
        let tick_spacing = 1;

        // Test operations near i32 boundaries
        let result = bitmap.update_bitmap(i32::MAX, tick_spacing, true);
        assert!(result.is_ok());

        let result = bitmap.update_bitmap(i32::MIN, tick_spacing, true);
        assert!(result.is_ok());

        // These should work without overflows/underflows
        let next_tick = bitmap
            .next_initialized_tick(i32::MIN, tick_spacing)
            .unwrap();
        assert_eq!(next_tick, i32::MAX);

        let prev_tick = bitmap
            .prev_initialized_tick(i32::MAX, tick_spacing)
            .unwrap();
        assert_eq!(prev_tick, i32::MAX);
    }
}
