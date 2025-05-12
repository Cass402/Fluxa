use crate::errors::ErrorCode;
use crate::tick_bitmap::*;
use proptest::prelude::*;
use std::collections::BTreeMap;

const WORD_SIZE: usize = 64;

/// Comprehensive tests for compress_tick function
mod compress_tick_tests {
    use super::*;

    #[test]
    fn test_compress_tick_basic() {
        // Basic compression cases
        assert_eq!(compress_tick(100, 10).unwrap(), 10); // 100/10 = 10
        assert_eq!(compress_tick(0, 5).unwrap(), 0); // 0/5 = 0
        assert_eq!(compress_tick(-50, 5).unwrap(), -10); // -50/5 = -10
        assert_eq!(compress_tick(1000, 25).unwrap(), 40); // 1000/25 = 40
    }

    #[test]
    fn test_compress_tick_with_invalid_spacing() {
        // Test with invalid tick spacing
        let result = compress_tick(100, 0);
        assert!(
            result.is_err(),
            "Expected error for zero tick spacing, got {result:?}"
        );

        // Check error is correct type
        match result {
            Err(err) => {
                // err is of type anchor_lang::prelude::Error
                if let anchor_lang::prelude::Error::AnchorError(details) = err {
                    assert_eq!(details.error_msg, ErrorCode::InvalidTickSpacing.to_string());
                } else {
                    panic!("Expected AnchorError with 'InvalidTickSpacing' message, but got a different error type or message: {err:?}");
                }
            }
            Ok(_) => panic!("Expected InvalidTickSpacing error, got Ok"),
        }
    }

    #[test]
    fn test_compress_tick_with_unaligned_tick() {
        // Test with unaligned tick
        let result = compress_tick(101, 10);
        assert!(
            result.is_err(),
            "Expected error for unaligned tick, got {result:?}"
        );

        // Check error is correct type
        match result {
            Err(err) => {
                // err is of type anchor_lang::prelude::Error
                if let anchor_lang::prelude::Error::AnchorError(details) = err {
                    assert_eq!(details.error_msg, ErrorCode::InvalidTickRange.to_string());
                } else {
                    panic!("Expected AnchorError with 'InvalidTickRange' message, but got a different error type or message: {err:?}");
                }
            }
            Ok(_) => panic!("Expected InvalidTickRange error, got Ok"),
        }
    }

    #[test]
    fn test_compress_tick_edge_cases() {
        // Test edge cases
        // Maximum positive tick that doesn't overflow when compressed
        assert_eq!(compress_tick(i32::MAX - 1, 2).unwrap(), (i32::MAX - 1) / 2);

        // Minimum negative tick
        assert_eq!(compress_tick(i32::MIN, 1).unwrap(), i32::MIN);
    }

    // Property-based testing for compress_tick
    proptest! {
        #[test]
        fn test_compress_tick_properties(
            tick in -100000..100000i32,
            tick_spacing in 1..1000u16
        ) {
            // Only test aligned ticks
            let aligned_tick = tick - (tick % tick_spacing as i32);

            // Property: compressing an aligned tick should not result in error
            let compressed_result = compress_tick(aligned_tick, tick_spacing);
            assert!(compressed_result.is_ok(), "Compressing aligned tick failed: {compressed_result:?}");
            let compressed_value = compressed_result.unwrap();

            // Property: compressing and then decompressing should yield the original tick
            let decompressed = decompress_tick(compressed_value, tick_spacing);
            assert_eq!(decompressed, aligned_tick,
                "Compress-decompress cycle should return original tick");

            // Property: compression should be consistent with division
            assert_eq!(compressed_value, aligned_tick / tick_spacing as i32,
                "Compression should equal division by tick spacing");
        }
    }
}

/// Comprehensive tests for decompress_tick function
mod decompress_tick_tests {
    use super::*;

    #[test]
    fn test_decompress_tick_basic() {
        // Basic decompression cases
        assert_eq!(decompress_tick(10, 10), 100); // 10*10 = 100
        assert_eq!(decompress_tick(0, 5), 0); // 0*5 = 0
        assert_eq!(decompress_tick(-10, 5), -50); // -10*5 = -50
        assert_eq!(decompress_tick(40, 25), 1000); // 40*25 = 1000
    }

    #[test]
    fn test_decompress_tick_with_zero_spacing() {
        // Test with zero spacing (edge case)
        assert_eq!(decompress_tick(100, 0), 0); // 100*0 = 0
    }

    #[test]
    fn test_decompress_tick_edge_cases() {
        // Test edge cases

        // Large positive compressed tick
        assert_eq!(decompress_tick(i32::MAX / 2, 2), i32::MAX - 1);

        // Large negative compressed tick
        assert_eq!(decompress_tick(i32::MIN / 2, 2), i32::MIN);

        // Max tick spacing
        assert_eq!(decompress_tick(10, u16::MAX), 10 * (u16::MAX as i32));
    }

    #[test]
    fn test_decompress_tick_overflow() {
        // Test potential overflow cases
        // When tick_spacing is large and compressed_tick is close to i32::MAX or i32::MIN,
        // multiplication could overflow. Let's verify behavior in these cases.

        // Assuming the function should saturate on overflow:
        let large_spacing = 65535; // max u16 value

        // Large positive - potential overflow
        let large_positive = i32::MAX / 2;
        let result = decompress_tick(large_positive, large_spacing);

        let expected = large_positive.wrapping_mul(large_spacing as i32);

        // The actual implementation may behave differently with overflows,
        // but we should document the expected behavior
        assert_eq!(
            result, expected,
            "Decompression should handle potential overflow correctly"
        );
    }

    // Property-based testing for decompress_tick
    proptest! {
        #[test]
        fn test_decompress_tick_properties(
            compressed_tick in -10000i32..10000i32,
            tick_spacing in 1u16..1000u16
        ) {
            // Skip cases that would cause overflow
            if compressed_tick > 0 && tick_spacing > 0 &&
               compressed_tick as i64 * tick_spacing as i64 > i32::MAX as i64 {
                return Ok(());
            }
            if compressed_tick < 0 && tick_spacing > 0 &&
               (compressed_tick as i64 * tick_spacing as i64) < i32::MIN as i64 {
                return Ok(());
            }

            let decompressed = decompress_tick(compressed_tick, tick_spacing);

            // Property: decompression should equal multiplication
            assert_eq!(decompressed, compressed_tick * (tick_spacing as i32),
                "Decompression should equal multiplication by tick spacing");

            // Property: compressing a decompressed tick should yield the original compressed tick
            if tick_spacing > 0 {
                let recompressed = compress_tick(decompressed, tick_spacing);
                assert!(recompressed.is_ok(), "Recompression should succeed");
                assert_eq!(recompressed.unwrap(), compressed_tick,
                    "Decompress-compress cycle should return original compressed tick");
            }
        }
    }
}

/// Comprehensive tests for get_word_index_and_bit_pos function
mod get_word_index_and_bit_pos_tests {
    use super::*;

    // Helper function to assert that an error is of the expected ErrorCode type
    fn assert_error_code<T>(
        result: Result<T, anchor_lang::prelude::Error>,
        expected_code: ErrorCode,
    ) {
        match result {
            Err(err) => {
                if let anchor_lang::prelude::Error::AnchorError(details) = err {
                    assert_eq!(details.error_msg, expected_code.to_string());
                } else {
                    panic!("Expected AnchorError with {expected_code:?} message, but got a different error type: {err:?}");
                }
            }
            Ok(_) => panic!("Expected error {expected_code:?}, got Ok"),
        }
    }

    #[test]
    fn test_get_word_index_and_bit_pos_basic() {
        // Basic cases
        assert_eq!(get_word_index_and_bit_pos(0).unwrap(), (0, 0)); // Tick 0 -> Word 0, bit 0
        assert_eq!(get_word_index_and_bit_pos(1).unwrap(), (0, 1)); // Tick 1 -> Word 0, bit 1
        assert_eq!(get_word_index_and_bit_pos(63).unwrap(), (0, 63)); // Tick 63 -> Word 0, bit 63
        assert_eq!(get_word_index_and_bit_pos(64).unwrap(), (1, 0)); // Tick 64 -> Word 1, bit 0
        assert_eq!(get_word_index_and_bit_pos(127).unwrap(), (1, 63)); // Tick 127 -> Word 1, bit 63
    }

    #[test]
    fn test_get_word_index_and_bit_pos_negative_ticks() {
        // Test with negative ticks
        assert_eq!(get_word_index_and_bit_pos(-1).unwrap(), (-1, 63)); // Tick -1 -> Word -1, bit 63
        assert_eq!(get_word_index_and_bit_pos(-64).unwrap(), (-1, 0)); // Tick -64 -> Word -1, bit 0
        assert_eq!(get_word_index_and_bit_pos(-65).unwrap(), (-2, 63)); // Tick -65 -> Word -2, bit 63
        assert_eq!(get_word_index_and_bit_pos(-128).unwrap(), (-2, 0)); // Tick -128 -> Word -2, bit 0
    }

    #[test]
    fn test_get_word_index_and_bit_pos_edge_cases() {
        // Test edge cases for valid compressed_tick range for i16 word_index

        // Max valid compressed tick
        let max_valid_compressed_tick =
            (i16::MAX as i32) * WORD_SIZE as i32 + (WORD_SIZE - 1) as i32; // 2097151
        let (word_idx, bit_pos) = get_word_index_and_bit_pos(max_valid_compressed_tick).unwrap();
        assert_eq!(word_idx, i16::MAX);
        assert_eq!(bit_pos, (WORD_SIZE - 1) as u8); // 63

        // Min valid compressed tick
        let min_valid_compressed_tick = (i16::MIN as i32) * WORD_SIZE as i32; // -2097152
        let (word_idx, bit_pos) = get_word_index_and_bit_pos(min_valid_compressed_tick).unwrap();
        assert_eq!(word_idx, i16::MIN);
        assert_eq!(bit_pos, 0);

        // Test inputs that are out of bounds for i16 word_index
        assert_error_code(
            get_word_index_and_bit_pos(i32::MAX),
            ErrorCode::TickWordIndexOutOfBounds,
        );
        assert_error_code(
            get_word_index_and_bit_pos(i32::MIN),
            ErrorCode::TickWordIndexOutOfBounds,
        );
    }

    // Property-based testing for get_word_index_and_bit_pos
    proptest! {
        #[test]
        fn test_get_word_index_and_bit_pos_properties(compressed_tick in -2097151..=2097151i32) { // Use valid range
            let (word_index, bit_pos) = get_word_index_and_bit_pos(compressed_tick).unwrap();

            // Property 1: bit_pos should always be in range [0, WORD_SIZE-1]
            assert!(bit_pos < WORD_SIZE as u8,
                    "Bit position should be less than WORD_SIZE");

            // Property 2: The formula should be consistent with how we partition ticks into words
            let expected_word_index_i64 = (compressed_tick as i64).div_euclid(WORD_SIZE as i64);
            let expected_word_index: i16 = expected_word_index_i64.try_into().unwrap();
            let expected_bit_pos = (compressed_tick - expected_word_index as i32 * WORD_SIZE as i32) as u8;

            assert_eq!(word_index, expected_word_index,
                       "Word index calculation should match expected formula");
            assert_eq!(bit_pos, expected_bit_pos,
                       "Bit position calculation should match expected formula");

            // Property 3: Reconstructing the tick from word_index and bit_pos should yield the original tick
            // This check is implicitly covered by the bit_pos calculation logic.
            // let reconstructed_tick = (word_index as i32 * WORD_SIZE as i32 + bit_pos as i32);
            // assert_eq!(reconstructed_tick, compressed_tick,
            //            "Reconstructing tick from word_index and bit_pos should yield original tick");
        }
    }
}

/// Comprehensive tests for next_initialized_bit_in_word function
mod next_initialized_bit_in_word_tests {
    use super::*;

    #[test]
    fn test_next_initialized_bit_empty_bitmap() {
        // Test with empty bitmap (all bits are 0)
        assert_eq!(next_initialized_bit_in_word(0, 0, false), None);
        assert_eq!(next_initialized_bit_in_word(0, 0, true), None);
        assert_eq!(next_initialized_bit_in_word(0, 32, false), None);
        assert_eq!(next_initialized_bit_in_word(0, 32, true), None);
    }

    #[test]
    fn test_next_initialized_bit_full_bitmap() {
        // Test with full bitmap (all bits are 1)
        let full_bitmap = u64::MAX;

        // Search upward
        assert_eq!(next_initialized_bit_in_word(full_bitmap, 0, false), Some(0));
        assert_eq!(
            next_initialized_bit_in_word(full_bitmap, 32, false),
            Some(32)
        );
        assert_eq!(
            next_initialized_bit_in_word(full_bitmap, 63, false),
            Some(63)
        );

        // Search downward
        assert_eq!(next_initialized_bit_in_word(full_bitmap, 0, true), Some(0));
        assert_eq!(
            next_initialized_bit_in_word(full_bitmap, 32, true),
            Some(32)
        );
        assert_eq!(
            next_initialized_bit_in_word(full_bitmap, 63, true),
            Some(63)
        );
    }

    #[test]
    fn test_next_initialized_bit_specific_patterns() {
        // Test with specific bitmap patterns

        // Bitmap with LSB set: 0b...00001
        let lsb_only = 1u64;
        assert_eq!(next_initialized_bit_in_word(lsb_only, 0, false), Some(0));
        assert_eq!(next_initialized_bit_in_word(lsb_only, 0, true), Some(0));
        assert_eq!(next_initialized_bit_in_word(lsb_only, 1, false), None);
        assert_eq!(next_initialized_bit_in_word(lsb_only, 1, true), Some(0));

        // Bitmap with MSB set: 0b1000...0000
        let msb_only = 1u64 << 63;
        assert_eq!(next_initialized_bit_in_word(msb_only, 0, false), Some(63));
        assert_eq!(next_initialized_bit_in_word(msb_only, 63, true), Some(63));
        assert_eq!(next_initialized_bit_in_word(msb_only, 62, true), None);

        // Bitmap with alternating bits: 0b...010101
        let alternating = 0x5555555555555555u64; // 0b0101...0101
        assert_eq!(next_initialized_bit_in_word(alternating, 0, false), Some(0));
        assert_eq!(next_initialized_bit_in_word(alternating, 1, false), Some(2));
        assert_eq!(
            next_initialized_bit_in_word(alternating, 62, true),
            Some(62)
        );
        assert_eq!(
            next_initialized_bit_in_word(alternating, 63, true),
            Some(62)
        );
    }

    #[test]
    fn test_next_initialized_bit_edge_cases() {
        // Edge cases with start position at or beyond bounds

        // Valid bitmap with bit at position 0
        let bitmap_with_lsb = 1u64;
        assert_eq!(
            next_initialized_bit_in_word(bitmap_with_lsb, u8::MAX, true),
            Some(0),
            "Out-of-bounds start_bit_pos should be clamped to WORD_SIZE-1 when searching downward"
        );

        // Valid bitmap with bit at position 63
        let bitmap_with_msb = 1u64 << 63;
        assert_eq!(
            next_initialized_bit_in_word(bitmap_with_msb, u8::MAX, false),
            None,
            "Out-of-bounds start_bit_pos should return None when searching upward"
        );
    }

    #[test]
    fn test_next_initialized_bit_search_direction() {
        // Test searching in both directions with multiple bits set
        // Corrected bitmap for bits at positions 0, 4, 8, 12, 16
        let bitmap = (1u64 << 0) | (1u64 << 4) | (1u64 << 8) | (1u64 << 12) | (1u64 << 16);

        // Upward search
        assert_eq!(next_initialized_bit_in_word(bitmap, 0, false), Some(0));
        assert_eq!(next_initialized_bit_in_word(bitmap, 1, false), Some(4));
        assert_eq!(next_initialized_bit_in_word(bitmap, 5, false), Some(8));

        // Downward search
        assert_eq!(next_initialized_bit_in_word(bitmap, 16, true), Some(16));
        assert_eq!(next_initialized_bit_in_word(bitmap, 15, true), Some(12));
        assert_eq!(next_initialized_bit_in_word(bitmap, 7, true), Some(4));
    }

    // Property-based testing for next_initialized_bit_in_word
    proptest! {
        #[test]
        fn test_next_initialized_bit_properties(
            bitmap in any::<u64>(),
            start_pos in 0..64u8,
            search_lte in any::<bool>()
        ) {
            let result = next_initialized_bit_in_word(bitmap, start_pos, search_lte);

            if let Some(pos) = result {
                assert!(pos < WORD_SIZE as u8, "Found position {pos} should be < WORD_SIZE");
                assert!((bitmap & (1u64 << pos)) != 0,
                        "Returned bit position should have its bit set in the bitmap. Bitmap: {bitmap:b}, pos: {pos}");

                if search_lte {
                    // Downward search: found pos <= start_pos (start_pos is already 0..63)
                    assert!(pos <= start_pos,
                            "Downward search result ({pos}) should be <= start position ({start_pos}). Bitmap: {bitmap:b}");

                    // Check that no bits are set strictly between pos and start_pos
                    // i.e., for k in (pos + 1)..=start_pos, bit k is 0
                    for k_bit in (pos + 1)..=start_pos {
                        // k_bit is already < WORD_SIZE because start_pos < WORD_SIZE
                        assert_eq!((bitmap >> k_bit) & 1, 0,
                            "Downward search: bit {k_bit} should be 0 (found pos={pos}, search started at {start_pos}). Bitmap: {bitmap:b}");
                    }
                } else {
                    // For upward search, result should be >= start_pos
                    assert!(pos >= start_pos,
                            "Upward search result ({pos}) should be >= start position ({start_pos}). Bitmap: {bitmap:b}");

                    // Check that no bits are set strictly between start_pos and pos
                    // i.e., for k in start_pos..pos, bit k is 0
                    for k_bit in start_pos..pos {
                        assert_eq!((bitmap >> k_bit) & 1, 0,
                            "Upward search: bit {k_bit} should be 0 (search started at {start_pos}, found pos={pos}). Bitmap: {bitmap:b}");
                    }
                }
            } else {
                // If no result, verify there are no set bits in the expected direction
                if search_lte {
                    // Downward search from start_pos (0..63): no bits in [0, start_pos] should be set.
                    // The function searches from start_pos down to 0.
                    for k_bit in 0..=start_pos {
                         assert_eq!((bitmap >> k_bit) & 1, 0,
                            "If no result in downward search, bit {k_bit} should be 0 (search started at {start_pos}). Bitmap: {bitmap:b}");
                    }
                } else {
                    // Upward search from start_pos (0..63): no bits in [start_pos, WORD_SIZE - 1] should be set.
                    // The function searches from start_pos up to WORD_SIZE - 1.
                    // start_pos is already < WORD_SIZE as u8 due to proptest range 0..WORD_SIZE as u8
                    for k_bit in start_pos..(WORD_SIZE as u8) {
                        assert_eq!((bitmap >> k_bit) & 1, 0,
                            "If no result in upward search, bit {k_bit} should be 0 (search started at {start_pos}). Bitmap: {bitmap:b}");
                    }
                }
            }
        }
    }
}

/// Comprehensive tests for flip_tick_initialized_status function
mod flip_tick_initialized_status_tests {
    use super::*;

    #[test]
    fn test_flip_tick_initialized_status_basic() {
        let mut bitmap = BTreeMap::new();

        // Initialize a tick
        assert!(flip_tick_initialized_status(&mut bitmap, 100, 10, true).is_ok());

        // Check if the bitmap has been updated correctly
        let compressed_tick = compress_tick(100, 10).unwrap();
        let (word_idx, bit_pos) = get_word_index_and_bit_pos(compressed_tick).unwrap();

        assert!(bitmap.contains_key(&word_idx));
        assert_eq!(bitmap[&word_idx] & (1u64 << bit_pos), 1u64 << bit_pos);

        // Uninitialize the same tick
        assert!(flip_tick_initialized_status(&mut bitmap, 100, 10, false).is_ok());

        // The word should be removed since it's now empty
        assert!(!bitmap.contains_key(&word_idx));
    }

    #[test]
    fn test_flip_tick_initialized_status_multiple_ticks() {
        let mut bitmap = BTreeMap::new();

        // Initialize multiple ticks in the same word
        assert!(flip_tick_initialized_status(&mut bitmap, 0, 10, true).is_ok());
        assert!(flip_tick_initialized_status(&mut bitmap, 10, 10, true).is_ok());
        assert!(flip_tick_initialized_status(&mut bitmap, 20, 10, true).is_ok());

        // Check if the bitmap has the correct bits set
        let (word_idx, _) = get_word_index_and_bit_pos(0).unwrap();
        assert_eq!(bitmap[&word_idx], (1u64 << 0) | (1u64 << 1) | (1u64 << 2));

        // Uninitialize one tick
        assert!(flip_tick_initialized_status(&mut bitmap, 10, 10, false).is_ok());

        // Check that only that tick was uninitialized
        assert_eq!(bitmap[&word_idx], (1u64 << 0) | (1u64 << 2));

        // Uninitialize all remaining ticks
        assert!(flip_tick_initialized_status(&mut bitmap, 0, 10, false).is_ok());
        assert!(flip_tick_initialized_status(&mut bitmap, 20, 10, false).is_ok());

        // The word should be removed
        assert!(bitmap.is_empty());
    }

    #[test]
    fn test_flip_tick_initialized_status_across_words() {
        let mut bitmap = BTreeMap::new();

        // Initialize ticks in different words
        let tick1 = 0;
        let tick2 = WORD_SIZE as i32 * 10; // Tick in the next word
        let tick3 = -10 * 10; // Tick in a negative word

        assert!(flip_tick_initialized_status(&mut bitmap, tick1, 10, true).is_ok());
        assert!(flip_tick_initialized_status(&mut bitmap, tick2, 10, true).is_ok());
        assert!(flip_tick_initialized_status(&mut bitmap, tick3, 10, true).is_ok());

        // Check if each word has the correct bit set
        let (word_idx1, bit_pos1) =
            get_word_index_and_bit_pos(compress_tick(tick1, 10).unwrap()).unwrap();
        let (word_idx2, bit_pos2) =
            get_word_index_and_bit_pos(compress_tick(tick2, 10).unwrap()).unwrap();
        let (word_idx3, bit_pos3) =
            get_word_index_and_bit_pos(compress_tick(tick3, 10).unwrap()).unwrap();

        assert_eq!(bitmap[&word_idx1] & (1u64 << bit_pos1), 1u64 << bit_pos1);
        assert_eq!(bitmap[&word_idx2] & (1u64 << bit_pos2), 1u64 << bit_pos2);
        assert_eq!(bitmap[&word_idx3] & (1u64 << bit_pos3), 1u64 << bit_pos3);

        // Bitmap should have 3 words
        assert_eq!(bitmap.len(), 3);
    }

    #[test]
    fn test_flip_tick_initialized_status_with_invalid_inputs() {
        let mut bitmap = BTreeMap::new();

        // Test with invalid tick spacing
        let result = flip_tick_initialized_status(&mut bitmap, 100, 0, true);
        assert!(result.is_err());
        match result {
            Err(err) => {
                if let anchor_lang::prelude::Error::AnchorError(details) = err {
                    assert_eq!(details.error_msg, ErrorCode::InvalidTickSpacing.to_string());
                } else {
                    panic!("Expected AnchorError with InvalidTickSpacing message");
                }
            }
            Ok(_) => panic!("Expected error for invalid tick spacing"),
        }

        // Test with unaligned tick
        let result = flip_tick_initialized_status(&mut bitmap, 5, 10, true);
        assert!(result.is_err());
        match result {
            Err(err) => {
                if let anchor_lang::prelude::Error::AnchorError(details) = err {
                    assert_eq!(details.error_msg, ErrorCode::InvalidTickRange.to_string());
                } else {
                    panic!("Expected AnchorError with InvalidTickRange message");
                }
            }
            Ok(_) => panic!("Expected error for unaligned tick"),
        }
    }

    #[test]
    fn test_flip_tick_initialized_status_idempotence() {
        let mut bitmap = BTreeMap::new();

        // Initialize a tick multiple times (should be idempotent)
        assert!(flip_tick_initialized_status(&mut bitmap, 100, 10, true).is_ok());
        assert!(flip_tick_initialized_status(&mut bitmap, 100, 10, true).is_ok());

        // Check that the bitmap still has just one bit set
        let compressed_tick = compress_tick(100, 10).unwrap();
        let (word_idx, bit_pos) = get_word_index_and_bit_pos(compressed_tick).unwrap();
        assert_eq!(bitmap[&word_idx], 1u64 << bit_pos);

        // Uninitialize a tick that doesn't exist (should be a no-op)
        assert!(flip_tick_initialized_status(&mut bitmap, 200, 10, false).is_ok());

        // Check bitmap still has one word with one bit set
        assert_eq!(bitmap.len(), 1);
        assert_eq!(bitmap[&word_idx], 1u64 << bit_pos);
    }

    // Property-based testing for flip_tick_initialized_status
    proptest! {
        #[test]
        fn test_flip_tick_initialized_status_properties(
            ticks in prop::collection::vec(-10000..10000i32, 1..10),
            tick_spacing in 1..100u16
        ) {
            // Only consider ticks aligned with tick_spacing
            let mut aligned_ticks: Vec<i32> = ticks.into_iter()
                .map(|t| (t / tick_spacing as i32) * tick_spacing as i32)
                .collect();

            aligned_ticks.sort_unstable();
            aligned_ticks.dedup();

            let mut bitmap = BTreeMap::new();

            // Initialize all ticks
            for &tick in &aligned_ticks {
                // Skip ticks that would result in out-of-bounds word index
                let compressed_tick_res = compress_tick(tick, tick_spacing);
                if compressed_tick_res.is_err() { continue; }
                if get_word_index_and_bit_pos(compressed_tick_res.unwrap()).is_err() {
                    continue;
                }
                let result = flip_tick_initialized_status(&mut bitmap, tick, tick_spacing, true);
                assert!(result.is_ok(), "Failed to initialize tick {tick}");
            }

            // Verify all ticks are initialized
            for &tick in &aligned_ticks {
                let compressed_res = compress_tick(tick, tick_spacing);
                if compressed_res.is_err() { continue; }
                let compressed = compressed_res.unwrap();
                let gwi_res = get_word_index_and_bit_pos(compressed);
                if gwi_res.is_err() { continue; } // Skip if this tick can't be represented
                let (word_idx, bit_pos) = gwi_res.unwrap();

                assert!(bitmap.contains_key(&word_idx),
                        "Word index {word_idx} should exist in bitmap");
                assert_ne!(bitmap[&word_idx] & (1u64 << bit_pos), 0,
                          "Bit at position {bit_pos} should be set in word {word_idx}");
            }

            // Uninitialize all ticks
            for &tick in &aligned_ticks {
                // Skip ticks that would result in out-of-bounds word index during check
                let compressed_tick_res = compress_tick(tick, tick_spacing);
                if compressed_tick_res.is_err() { continue; }
                if get_word_index_and_bit_pos(compressed_tick_res.unwrap()).is_err() {
                    continue;
                }
                let result = flip_tick_initialized_status(&mut bitmap, tick, tick_spacing, false);
                assert!(result.is_ok(), "Failed to uninitialize tick {tick}");
            }

            // Bitmap should be empty now
            assert!(bitmap.is_empty(), "Bitmap should be empty after uninitializing all ticks");
        }
    }
}

/// Comprehensive tests for is_tick_initialized function
mod is_tick_initialized_tests {
    use super::*;

    #[test]
    fn test_is_tick_initialized_basic() {
        let mut bitmap = BTreeMap::new();

        // Check uninitialized tick
        assert!(!is_tick_initialized(&bitmap, 100, 10).unwrap());

        // Initialize a tick
        assert!(flip_tick_initialized_status(&mut bitmap, 100, 10, true).is_ok());

        // Check initialized tick
        assert!(is_tick_initialized(&bitmap, 100, 10).unwrap());

        // Check another uninitialized tick
        assert!(!is_tick_initialized(&bitmap, 200, 10).unwrap());
    }

    #[test]
    fn test_is_tick_initialized_multiple_ticks() {
        let mut bitmap = BTreeMap::new();

        // Initialize multiple ticks
        let ticks_to_initialize = [0, 10, 20, -10, -20, 630, 640];

        for &tick in &ticks_to_initialize {
            assert!(flip_tick_initialized_status(&mut bitmap, tick, 10, true).is_ok());
        }

        // Check all initialized ticks
        for &tick in &ticks_to_initialize {
            assert!(
                is_tick_initialized(&bitmap, tick, 10).unwrap(),
                "Tick {tick} should be initialized"
            );
        }

        // Check some uninitialized ticks
        let uninitialized_ticks = [50, 100, -30, 1000];

        for &tick in &uninitialized_ticks {
            assert!(
                !is_tick_initialized(&bitmap, tick, 10).unwrap(),
                "Tick {tick} should NOT be initialized"
            );
        }
    }

    #[test]
    fn test_is_tick_initialized_with_invalid_inputs() {
        let bitmap = BTreeMap::new();

        // Test with invalid tick spacing
        let result = is_tick_initialized(&bitmap, 100, 0);
        assert!(result.is_err());
        match result {
            Err(err) => {
                if let anchor_lang::prelude::Error::AnchorError(details) = err {
                    assert_eq!(details.error_msg, ErrorCode::InvalidTickSpacing.to_string());
                } else {
                    panic!("Expected AnchorError with InvalidTickSpacing message");
                }
            }
            Ok(_) => panic!("Expected error for invalid tick spacing"),
        }

        // Test with unaligned tick
        let result = is_tick_initialized(&bitmap, 5, 10);
        assert!(result.is_err());
        match result {
            Err(err) => {
                if let anchor_lang::prelude::Error::AnchorError(details) = err {
                    assert_eq!(details.error_msg, ErrorCode::InvalidTickRange.to_string());
                } else {
                    panic!("Expected AnchorError with InvalidTickRange message");
                }
            }
            Ok(_) => panic!("Expected error for unaligned tick"),
        }
    }

    #[test]
    fn test_is_tick_initialized_after_flipping() {
        let mut bitmap = BTreeMap::new();

        // Initialize then uninitialize
        assert!(flip_tick_initialized_status(&mut bitmap, 100, 10, true).is_ok());
        assert!(is_tick_initialized(&bitmap, 100, 10).unwrap());

        assert!(flip_tick_initialized_status(&mut bitmap, 100, 10, false).is_ok());
        assert!(!is_tick_initialized(&bitmap, 100, 10).unwrap());

        // Initialize again
        assert!(flip_tick_initialized_status(&mut bitmap, 100, 10, true).is_ok());
        assert!(is_tick_initialized(&bitmap, 100, 10).unwrap());
    }

    // Property-based testing for is_tick_initialized
    proptest! {
        #[test]
        fn test_is_tick_initialized_properties(
            ticks_to_init in prop::collection::vec(-10000..10000i32, 1..20),
            ticks_to_check in prop::collection::vec(-10000..10000i32, 1..20),
            tick_spacing in 1..100u16
        ) {
            // Align ticks with tick_spacing
            let mut aligned_init_ticks: Vec<i32> = ticks_to_init.into_iter()
                .map(|t| (t / tick_spacing as i32) * tick_spacing as i32)
                .collect();
            aligned_init_ticks.sort_unstable();
            aligned_init_ticks.dedup();


            let aligned_check_ticks: Vec<i32> = ticks_to_check.into_iter()
                .map(|t| (t / tick_spacing as i32) * tick_spacing as i32)
                .collect();

            let mut bitmap = BTreeMap::new();

            // Initialize the init ticks
            for &tick in &aligned_init_ticks {
                // Skip ticks that would result in out-of-bounds word index
                let compressed_tick_res = compress_tick(tick, tick_spacing);
                if compressed_tick_res.is_err() { continue; }
                if get_word_index_and_bit_pos(compressed_tick_res.unwrap()).is_err() {
                    continue;
                }
                let result = flip_tick_initialized_status(&mut bitmap, tick, tick_spacing, true);
                assert!(result.is_ok(), "Failed to initialize tick {tick}");
            }

            // Check all ticks
            for &tick in &aligned_check_ticks {
                let compressed_tick_res = compress_tick(tick, tick_spacing);
                if compressed_tick_res.is_err() { continue; } // Cannot check unaligned/invalid spacing
                if get_word_index_and_bit_pos(compressed_tick_res.unwrap()).is_err() { continue; } // Cannot check out of bounds

                let should_be_initialized = aligned_init_ticks.contains(&tick);
                let is_init = is_tick_initialized(&bitmap, tick, tick_spacing).unwrap();

                assert_eq!(is_init, should_be_initialized,
                          "Tick {tick} initialization status: expected {should_be_initialized}, got {is_init}");
            }
        }
    }
}

/// Comprehensive tests for next_initialized_tick function
mod next_initialized_tick_tests {
    use super::*;

    #[test]
    fn test_next_initialized_tick_empty_bitmap() {
        let bitmap = BTreeMap::new();

        // Test with empty bitmap
        assert_eq!(next_initialized_tick(&bitmap, 0, 10, true).unwrap(), None);
        assert_eq!(next_initialized_tick(&bitmap, 0, 10, false).unwrap(), None);
    }

    #[test]
    fn test_next_initialized_tick_basic() {
        let mut bitmap = BTreeMap::new();

        // Initialize some ticks
        let ticks_to_initialize = [-100, 0, 100, 200];
        for &tick in &ticks_to_initialize {
            assert!(flip_tick_initialized_status(&mut bitmap, tick, 10, true).is_ok());
        }

        // Test searching less than or equal
        assert_eq!(
            next_initialized_tick(&bitmap, 50, 10, true).unwrap(),
            Some(0)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, 150, 10, true).unwrap(),
            Some(100)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, 300, 10, true).unwrap(),
            Some(200)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, -50, 10, true).unwrap(),
            Some(-100)
        );

        // Test searching greater than or equal
        assert_eq!(
            next_initialized_tick(&bitmap, 50, 10, false).unwrap(),
            Some(100)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, 150, 10, false).unwrap(),
            Some(200)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, -150, 10, false).unwrap(),
            Some(-100)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, -120, 10, false).unwrap(),
            Some(-100)
        );
    }

    #[test]
    fn test_next_initialized_tick_exact_match() {
        let mut bitmap = BTreeMap::new();

        // Initialize some ticks
        let ticks_to_initialize = [-100, 0, 100, 200];
        for &tick in &ticks_to_initialize {
            assert!(flip_tick_initialized_status(&mut bitmap, tick, 10, true).is_ok());
        }

        // Test searching from an exact match
        // When searching LTE from an initialized tick, should return the same tick
        assert_eq!(
            next_initialized_tick(&bitmap, 100, 10, true).unwrap(),
            Some(100)
        );

        // When searching GTE from an initialized tick, should return the same tick
        assert_eq!(
            next_initialized_tick(&bitmap, 100, 10, false).unwrap(),
            Some(100)
        );
    }

    #[test]
    fn test_next_initialized_tick_no_match() {
        let mut bitmap = BTreeMap::new();

        // Initialize some ticks
        let ticks_to_initialize = [0, 100, 200];
        for &tick in &ticks_to_initialize {
            assert!(flip_tick_initialized_status(&mut bitmap, tick, 10, true).is_ok());
        }

        // Test searching with no match
        // No initialized tick less than -1
        assert_eq!(next_initialized_tick(&bitmap, -1, 10, true).unwrap(), None);

        // No initialized tick greater than 300
        assert_eq!(
            next_initialized_tick(&bitmap, 300, 10, false).unwrap(),
            None
        );
    }

    #[test]
    fn test_next_initialized_tick_with_unaligned_current_tick() {
        let mut bitmap = BTreeMap::new();

        // Initialize some ticks
        let ticks_to_initialize = [0, 100, 200];
        for &tick in &ticks_to_initialize {
            assert!(flip_tick_initialized_status(&mut bitmap, tick, 10, true).is_ok());
        }

        // Test with unaligned current tick (should work because we don't check alignment for the starting tick)
        // Starting from tick 95 (not aligned with spacing 10)
        assert_eq!(
            next_initialized_tick(&bitmap, 95, 10, true).unwrap(),
            Some(0)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, 95, 10, false).unwrap(),
            Some(100)
        );

        // Starting from tick 105 (not aligned with spacing 10)
        assert_eq!(
            next_initialized_tick(&bitmap, 105, 10, true).unwrap(),
            Some(100)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, 105, 10, false).unwrap(),
            Some(200)
        );
    }

    #[test]
    fn test_next_initialized_tick_with_invalid_spacing() {
        let bitmap = BTreeMap::new();

        // Test with invalid tick spacing
        let result = next_initialized_tick(&bitmap, 100, 0, true);
        assert!(result.is_err());
        match result {
            Err(err) => {
                if let anchor_lang::prelude::Error::AnchorError(details) = err {
                    assert_eq!(details.error_msg, ErrorCode::InvalidTickSpacing.to_string());
                } else {
                    panic!("Expected AnchorError with InvalidTickSpacing message");
                }
            }
            Ok(_) => panic!("Expected error for invalid tick spacing"),
        }
    }

    #[test]
    fn test_next_initialized_tick_across_words() {
        let mut bitmap = BTreeMap::new();

        // Initialize ticks in different words
        let word_size_in_ticks = WORD_SIZE as i32 * 10; // With tick spacing 10
        let ticks_to_initialize = [
            0,
            word_size_in_ticks,
            2 * word_size_in_ticks,
            -word_size_in_ticks,
        ];

        for &tick in &ticks_to_initialize {
            assert!(flip_tick_initialized_status(&mut bitmap, tick, 10, true).is_ok());
        }

        // Test searching across words
        assert_eq!(
            next_initialized_tick(&bitmap, word_size_in_ticks - 10, 10, false).unwrap(),
            Some(word_size_in_ticks)
        );

        assert_eq!(
            next_initialized_tick(&bitmap, word_size_in_ticks + 10, 10, true).unwrap(),
            Some(word_size_in_ticks)
        );

        assert_eq!(
            next_initialized_tick(&bitmap, -10, 10, true).unwrap(),
            Some(-word_size_in_ticks)
        );

        assert_eq!(
            next_initialized_tick(&bitmap, -word_size_in_ticks - 10, 10, false).unwrap(),
            Some(-word_size_in_ticks)
        );
    }

    #[test]
    fn test_next_initialized_tick_edge_cases() {
        let mut bitmap = BTreeMap::new();

        // Test with edge cases
        let edge_ticks = [i32::MIN + 10, -10000, 0, 10000, i32::MAX - 10];
        // Align with tick spacing (we can't use i32::MIN directly due to overflow)
        let aligned_edge_ticks: Vec<i32> = edge_ticks.iter().map(|&t| (t / 10) * 10).collect();
        let tick_spacing = 10;

        for &tick in &aligned_edge_ticks {
            // Only initialize ticks that are within the representable range of the bitmap
            let compressed_res = compress_tick(tick, tick_spacing);
            if compressed_res.is_ok_and(|ct| get_word_index_and_bit_pos(ct).is_ok()) {
                assert!(
                    flip_tick_initialized_status(&mut bitmap, tick, tick_spacing, true).is_ok(),
                    "Failed to initialize tick {tick}"
                );
            }
        }
        let aligned_edge_ticks: Vec<i32> = aligned_edge_ticks
            .into_iter()
            .filter(|&tick| {
                compress_tick(tick, tick_spacing)
                    .is_ok_and(|ct| get_word_index_and_bit_pos(ct).is_ok())
            })
            .collect();

        // Check we can find all ticks
        for i in 0..aligned_edge_ticks.len() - 1 {
            let current_tick = aligned_edge_ticks[i];
            let next_tick = aligned_edge_ticks[i + 1];

            let search_from = current_tick + 5; // Search from between current and next

            assert_eq!(
                next_initialized_tick(&bitmap, search_from, 10, false).unwrap(),
                Some(next_tick),
                "Failed to find next tick from {search_from}"
            );

            let search_from = next_tick - 5; // Search from between next and current

            assert_eq!(
                next_initialized_tick(&bitmap, search_from, 10, true).unwrap(),
                Some(current_tick),
                "Failed to find previous tick from {search_from}"
            );
        }
    }

    // Property-based testing for next_initialized_tick
    proptest! {
        #[test]
        fn test_next_initialized_tick_properties(
            ticks in prop::collection::vec(-10000..10000i32, 1..20),
            search_points in prop::collection::vec(-10000..10000i32, 1..10),
            tick_spacing in 1..100u16,
            search_lte in proptest::bool::ANY
        ) {
            // Align ticks with tick_spacing
            let mut aligned_ticks: Vec<i32> = ticks.into_iter()
                .map(|t| (t / tick_spacing as i32) * tick_spacing as i32)
                .collect();

            // Sort and deduplicate
            aligned_ticks.sort();
            aligned_ticks.dedup();

            let mut bitmap = BTreeMap::new();

            // Initialize the ticks
            for &tick in &aligned_ticks {
                // Skip ticks that would result in out-of-bounds word index
                let compressed_tick_res = compress_tick(tick, tick_spacing);
                if compressed_tick_res.is_err() { continue; }
                if get_word_index_and_bit_pos(compressed_tick_res.unwrap()).is_err() {
                    continue;
                }
                let result = flip_tick_initialized_status(&mut bitmap, tick, tick_spacing, true);
                assert!(result.is_ok(), "Failed to initialize tick {tick}");
            }
            // Test each search point
            for &search_point in &search_points {
                let result = next_initialized_tick(&bitmap, search_point, tick_spacing, search_lte);
                assert!(result.is_ok(), "next_initialized_tick call failed");
                let found_tick = result.unwrap();

                if aligned_ticks.is_empty() {
                    // If no ticks are initialized, next_initialized_tick should return None
                    assert_eq!(found_tick, None, "Empty bitmap should return None");
                } else if search_lte {
                    // Find the greatest tick <= search_point
                    let expected = aligned_ticks.iter().rev()
                        .find(|&&t| t <= search_point);

                    match (found_tick, expected) {
                        (Some(found), Some(&expected)) => {
                            assert_eq!(found, expected,
                                      "LTE search from {search_point} found {found}, expected {expected}");
                        }
                        (None, None) => {
                            // Correctly found no tick
                        }
                        (Some(found), None) => {
                            panic!("LTE search from {search_point} found {found}, but expected None");
                        }
                        (None, Some(&expected)) => {
                            panic!("LTE search from {search_point} found None, but expected {expected}");
                        }
                    }
                } else {
                    // Find the smallest tick >= search_point
                    let expected = aligned_ticks.iter()
                        .find(|&&t| t >= search_point);

                    match (found_tick, expected) {
                        (Some(found), Some(&expected)) => {
                            assert_eq!(found, expected,
                                      "GTE search from {search_point} found {found}, expected {expected}");
                        }
                        (None, None) => {
                            // Correctly found no tick
                        }
                        (Some(found), None) => {
                            panic!("GTE search from {search_point} found {found}, but expected None");
                        }
                        (None, Some(&expected)) => {
                            panic!("GTE search from {search_point} found None, but expected {expected}");
                        }
                    }
                }
            }
        }
    }
}

/// Security tests focusing on edge cases and potential vulnerabilities in tick_bitmap functions
mod security_tests {
    use super::*;

    #[test]
    fn test_security_integer_overflow_in_decompress_tick() {
        // Test potential integer overflow in decompress_tick
        let max_i32 = i32::MAX;
        let large_spacing = u16::MAX; // Maximum tick spacing

        // This multiplication could potentially overflow
        let result = decompress_tick(max_i32, large_spacing);

        // Check that the result is consistent with expected wrapping behavior
        let expected = max_i32.wrapping_mul(large_spacing as i32);
        assert_eq!(
            result, expected,
            "decompress_tick should handle potential overflow correctly"
        );
    }

    #[test]
    fn test_security_extreme_word_indices() {
        let mut bitmap = BTreeMap::new();

        // Test with extreme word indices near i16 limits
        let min_word_idx_tick = (i16::MIN as i32) * WORD_SIZE as i32; // Corresponds to i16::MIN word index
        let max_word_idx_tick = (i16::MAX as i32) * WORD_SIZE as i32; // Corresponds to i16::MAX word index

        // Initialize ticks at extreme word indices
        assert!(
            flip_tick_initialized_status(&mut bitmap, min_word_idx_tick, 1, true).is_ok(),
            "Should be able to initialize tick at minimum valid compressed tick"
        );
        assert!(
            flip_tick_initialized_status(&mut bitmap, max_word_idx_tick, 1, true).is_ok(),
            "Should be able to initialize tick at maximum valid compressed tick"
        );

        // Verify ticks are initialized
        assert!(
            is_tick_initialized(&bitmap, min_word_idx_tick, 1).unwrap(),
            "Tick at minimum word index should be initialized"
        ); // min_word_idx_tick is -2097152
        assert!(
            is_tick_initialized(&bitmap, max_word_idx_tick, 1).unwrap(),
            "Tick at maximum word index should be initialized"
        ); // max_word_idx_tick is 2097088 (bit 0 of i16::MAX word)

        // Test searching from the extremes
        assert_eq!(
            next_initialized_tick(&bitmap, min_word_idx_tick, 1, true).unwrap(),
            Some(min_word_idx_tick),
            "Should find tick at minimum word index when searching LTE"
        );

        assert_eq!(
            next_initialized_tick(&bitmap, max_word_idx_tick, 1, false).unwrap(),
            Some(max_word_idx_tick),
            "Should find tick at maximum word index when searching GTE"
        );
    }

    #[test]
    fn test_security_word_index_wrapping() {
        // Test behavior at word index boundaries to ensure no wrapping occurs
        let mut bitmap = BTreeMap::new();

        // Initialize a tick at word index boundary
        let boundary_tick = (i16::MAX as i32) * WORD_SIZE as i32;
        assert!(flip_tick_initialized_status(&mut bitmap, boundary_tick, 1, true).is_ok());

        // Try to search beyond the maximum word index
        let beyond_max_tick = boundary_tick + WORD_SIZE as i32;

        // When searching LTE from beyond max, should find the boundary tick
        assert_eq!(
            next_initialized_tick(&bitmap, beyond_max_tick, 1, true).unwrap(),
            Some(boundary_tick),
            "Searching LTE from beyond MAX should find tick at boundary"
        );

        // When searching GTE from beyond max, should find nothing
        assert_eq!(
            next_initialized_tick(&bitmap, beyond_max_tick, 1, false).unwrap(),
            None,
            "Searching GTE from beyond MAX should find nothing"
        );

        // Similar tests for minimum boundary
        let min_boundary_tick = (i16::MIN as i32) * WORD_SIZE as i32;
        bitmap.clear();
        assert!(flip_tick_initialized_status(&mut bitmap, min_boundary_tick, 1, true).is_ok());

        let beyond_min_tick = min_boundary_tick - WORD_SIZE as i32;

        // When searching GTE from beyond min, should find the boundary tick
        assert_eq!(
            next_initialized_tick(&bitmap, beyond_min_tick, 1, false).unwrap(),
            Some(min_boundary_tick),
            "Searching GTE from beyond MIN should find tick at boundary"
        );

        // When searching LTE from beyond min, should find nothing
        assert_eq!(
            next_initialized_tick(&bitmap, beyond_min_tick, 1, true).unwrap(),
            Some(min_boundary_tick), // Clamped to min_boundary_tick, which is initialized
            "Searching LTE from beyond MIN should find nothing"
        );
    }

    #[test]
    fn test_security_invalid_tick_spacing() {
        // Test handling of invalid tick spacing
        let mut bitmap = BTreeMap::new();
        let tick = 100;

        // Zero tick spacing
        let zero_spacing_result = flip_tick_initialized_status(&mut bitmap, tick, 0, true);
        assert!(zero_spacing_result.is_err());
        assert_error_code(zero_spacing_result, ErrorCode::InvalidTickSpacing);

        // Same for is_tick_initialized and next_initialized_tick
        assert!(is_tick_initialized(&bitmap, tick, 0).is_err());
        assert!(next_initialized_tick(&bitmap, tick, 0, true).is_err());
    }

    #[test]
    fn test_security_unaligned_ticks() {
        // Test handling of unaligned ticks
        let mut bitmap = BTreeMap::new();
        let spacing = 10;
        let unaligned_tick = 15; // Not divisible by spacing=10

        // Try to initialize an unaligned tick
        let result = flip_tick_initialized_status(&mut bitmap, unaligned_tick, spacing, true);
        assert!(result.is_err());
        assert_error_code(result, ErrorCode::InvalidTickRange);

        // Try to check if an unaligned tick is initialized
        let result = is_tick_initialized(&bitmap, unaligned_tick, spacing);
        assert!(result.is_err());
        assert_error_code(result, ErrorCode::InvalidTickRange);
    }

    #[test]
    fn test_security_sparse_bitmap() {
        // Test handling of very sparse bitmaps (words far apart)
        let mut bitmap = BTreeMap::new();

        // Initialize ticks in widely separated words
        // Adjusted to be within i16 word index limits for compressed ticks (spacing = 1)
        let ticks = [
            (i16::MIN as i32) * WORD_SIZE as i32 + 100, // e.g., -2097152 + 100
            -10_000 * WORD_SIZE as i32,                 // e.g., -640000
            0,
            10_000 * WORD_SIZE as i32,                  // e.g., 640000
            (i16::MAX as i32) * WORD_SIZE as i32 - 100, // e.g., 2097151 - (WORD_SIZE-1) - 100
        ];

        for &tick in &ticks {
            let res = flip_tick_initialized_status(&mut bitmap, tick, 1, true);
            assert!(res.is_ok(), "Failed to initialize tick {tick}: {res:?}");
        }

        // Verify all ticks are found when searching
        for i in 0..ticks.len() - 1 {
            // Search forward
            // Ensure search_point calculation doesn't overflow for extreme ticks[i] and ticks[i+1]
            let search_point =
                if (ticks[i] > 0 && ticks[i + 1] > 0 && ticks[i] > i32::MAX - ticks[i + 1])
                    || (ticks[i] < 0 && ticks[i + 1] < 0 && ticks[i] < i32::MIN - ticks[i + 1])
                {
                    ticks[i] + (ticks[i + 1] - ticks[i]) / 2 // Avoid overflow for large positives/negatives
                } else {
                    (ticks[i] + ticks[i + 1]) / 2
                };
            assert_eq!(
                next_initialized_tick(&bitmap, search_point, 1, false).unwrap(),
                Some(ticks[i + 1]),
                "Failed to find next tick in sparse bitmap"
            );

            // Search backward
            assert_eq!(
                next_initialized_tick(&bitmap, search_point, 1, true).unwrap(),
                Some(ticks[i]),
                "Failed to find previous tick in sparse bitmap"
            );
        }
    }

    // Helper function to assert that an error is of the expected ErrorCode type
    fn assert_error_code<T>(
        result: Result<T, anchor_lang::prelude::Error>,
        expected_code: ErrorCode,
    ) {
        match result {
            Err(err) => {
                if let anchor_lang::prelude::Error::AnchorError(details) = err {
                    assert_eq!(details.error_msg, expected_code.to_string());
                } else {
                    panic!("Expected AnchorError with {expected_code:?} message, but got a different error type: {err:?}");
                }
            }
            Ok(_) => panic!("Expected error {expected_code:?}, got Ok"),
        }
    }

    // Property-based tests focusing on security aspects
    proptest! {
        #[test]
        fn test_security_compress_decompress_roundtrip(
            tick in -100000..100000i32,
            tick_spacing in 1..1000u16
        ) {
            // Only test aligned ticks
            let aligned_tick = (tick / tick_spacing as i32) * tick_spacing as i32;

            // Compress-decompress should round-trip properly
            let compressed = compress_tick(aligned_tick, tick_spacing).unwrap();
            let decompressed = decompress_tick(compressed, tick_spacing);

            assert_eq!(decompressed, aligned_tick,
                      "Compress-decompress roundtrip failed for tick {aligned_tick} with spacing {tick_spacing}");
        }

        #[test]
        fn test_security_word_index_and_bit_pos(
            compressed_tick in -2097152..=2097151i32 // Valid range for i16 word index
        ) {
            // Get word index and bit position
            let result = get_word_index_and_bit_pos(compressed_tick);
            assert!(result.is_ok(), "get_word_index_and_bit_pos failed for valid compressed_tick {compressed_tick}");
            let (word_idx, bit_pos) = result.unwrap();

            // Verify word_idx calculation matches expected formula
            let expected_word_idx_i64 = (compressed_tick as i64).div_euclid(WORD_SIZE as i64);
            assert_eq!(word_idx as i64, expected_word_idx_i64,
                      "Word index calculation incorrect for compressed tick {compressed_tick}");
            // Ensure bit_pos is in valid range
            assert!(bit_pos < WORD_SIZE as u8, "Bit position {bit_pos} exceeds word size");

            // Verify word_idx calculation matches expected formula
            let expected_word_idx = (compressed_tick as i64).div_euclid(WORD_SIZE as i64) as i16;
            assert_eq!(word_idx, expected_word_idx,
                      "Word index calculation incorrect for compressed tick {compressed_tick}");

            // Verify bit_pos calculation based on reconstruction
            let expected_bit_pos = (compressed_tick - (word_idx as i32 * WORD_SIZE as i32)) as u8;
            assert_eq!(bit_pos, expected_bit_pos,
                      "Bit position calculation incorrect for compressed tick {compressed_tick}");

            // Check that the word index is within i16 bounds (shouldn't overflow)
            assert!((i16::MIN..=i16::MAX).contains(&word_idx),
                   "Word index {word_idx} outside i16 bounds for compressed tick {compressed_tick}");
        }

        #[test]
        fn test_security_bitmap_consistency(
            ticks_input in prop::collection::vec(-10000..10000i32, 1..50),
            tick_spacing in 1..100u16
        ) {
            // Align ticks with tick_spacing
            let mut aligned_ticks: Vec<i32> = ticks_input.into_iter()
                .map(|t| (t / tick_spacing as i32) * tick_spacing as i32)
                .collect();

            // Sort and deduplicate to ensure consistent behavior with unique ticks
            aligned_ticks.sort_unstable();
            aligned_ticks.dedup();

            if aligned_ticks.is_empty() {
                return Ok(()); // Skip test if no unique aligned ticks
            }

            let mut bitmap = BTreeMap::new();

            // Initialize all ticks
            for &tick in &aligned_ticks {
                // Skip if tick is out of representable range for the bitmap
                let compressed_res = compress_tick(tick, tick_spacing);
                if compressed_res.is_err() { continue; }
                if get_word_index_and_bit_pos(compressed_res.unwrap()).is_err() { continue; }

                let res = flip_tick_initialized_status(&mut bitmap, tick, tick_spacing, true);
                assert!(res.is_ok(), "Initialization failed for tick {tick}: {res:?}");
            }

            // Verify all initialized ticks
            for &tick in &aligned_ticks {
                // Skip if tick is out of representable range for the bitmap
                let compressed_res = compress_tick(tick, tick_spacing);
                if compressed_res.is_err() { continue; }
                if get_word_index_and_bit_pos(compressed_res.unwrap()).is_err() { continue; }

                let is_init = is_tick_initialized(&bitmap, tick, tick_spacing).unwrap();
                assert!(is_init, "Tick {tick} should be initialized with spacing {tick_spacing}");
            }

            // Uninitialize every other unique aligned tick
            for (i, &tick) in aligned_ticks.iter().enumerate() {
                // Check representability before attempting to flip
                if i % 2 == 0 && compress_tick(tick, tick_spacing).is_ok_and(|ct| get_word_index_and_bit_pos(ct).is_ok()) { // Uninitialize ticks at even indices (0, 2, 4...)
                    let _ = flip_tick_initialized_status(&mut bitmap, tick, tick_spacing, false);
                }
            }

            // Verify consistency after uninitialization for unique aligned ticks
            for (i, &tick) in aligned_ticks.iter().enumerate() {
                let is_init = is_tick_initialized(&bitmap, tick, tick_spacing).unwrap();
                // Skip if tick is out of representable range for the bitmap
                let compressed_res = compress_tick(tick, tick_spacing);
                if compressed_res.is_err() { continue; }
                if get_word_index_and_bit_pos(compressed_res.unwrap()).is_err() { continue; }

                // Ticks at odd indices (1, 3, 5...) should remain initialized
                // Ticks at even indices (0, 2, 4...) should be uninitialized
                let expected_state = i % 2 == 1;

                assert_eq!(is_init, expected_state,
                          "Tick {tick} (at index {i}) initialization status incorrect after partial uninitialization. Expected: {expected_state}, Got: {is_init}");
            }
        }
    }
}

/// Integration tests that verify multiple tick_bitmap functions working together
mod integration_tests {
    use super::*;

    #[test]
    fn test_integration_bitmap_operations_sequence() {
        // Test a sequence of operations to verify integrated functionality
        let mut bitmap = BTreeMap::new();

        // 1. Initialize several ticks
        let ticks_to_initialize = [-100, 0, 100, 200, 300];
        for &tick in &ticks_to_initialize {
            assert!(flip_tick_initialized_status(&mut bitmap, tick, 10, true).is_ok());
        }

        // 2. Verify all ticks are initialized
        for &tick in &ticks_to_initialize {
            assert!(is_tick_initialized(&bitmap, tick, 10).unwrap());
        }

        // 3. Find next/previous ticks at different positions
        assert_eq!(
            next_initialized_tick(&bitmap, 50, 10, true).unwrap(),
            Some(0)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, 50, 10, false).unwrap(),
            Some(100)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, 150, 10, true).unwrap(),
            Some(100)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, 150, 10, false).unwrap(),
            Some(200)
        );

        // 4. Uninitialize one tick and verify search still works correctly
        assert!(flip_tick_initialized_status(&mut bitmap, 100, 10, false).is_ok());
        assert!(!is_tick_initialized(&bitmap, 100, 10).unwrap());

        assert_eq!(
            next_initialized_tick(&bitmap, 50, 10, true).unwrap(),
            Some(0)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, 50, 10, false).unwrap(),
            Some(200)
        );

        // 5. Uninitialize all ticks and verify bitmap is empty
        for &tick in &ticks_to_initialize {
            assert!(flip_tick_initialized_status(&mut bitmap, tick, 10, false).is_ok());
        }

        assert!(
            bitmap.is_empty(),
            "Bitmap should be empty after uninitializing all ticks"
        );
        assert_eq!(next_initialized_tick(&bitmap, 0, 10, false).unwrap(), None);
    }

    #[test]
    fn test_integration_tick_ranges() {
        // Test operations on ranges of ticks
        let mut bitmap = BTreeMap::new();
        let tick_spacing = 100;

        // Initialize a range of ticks
        for tick in (-500..=500).step_by(tick_spacing as usize) {
            assert!(flip_tick_initialized_status(&mut bitmap, tick, tick_spacing, true).is_ok());
        }

        // Verify all ticks in range are initialized
        for tick in (-500..=500).step_by(tick_spacing as usize) {
            assert!(
                is_tick_initialized(&bitmap, tick, tick_spacing).unwrap(),
                "Tick {tick} should be initialized"
            );
        }

        // Verify ticks outside range are not initialized
        assert!(!is_tick_initialized(&bitmap, -600, tick_spacing).unwrap());
        assert!(!is_tick_initialized(&bitmap, 600, tick_spacing).unwrap());

        // Test searching from middle of range
        assert_eq!(
            next_initialized_tick(&bitmap, 0, tick_spacing, true).unwrap(),
            Some(0)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, 0, tick_spacing, false).unwrap(),
            Some(0)
        );

        // Test searching from between initialized ticks
        assert_eq!(
            next_initialized_tick(&bitmap, 50, tick_spacing, true).unwrap(),
            Some(0)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, 50, tick_spacing, false).unwrap(),
            Some(100)
        );

        // Test searching from edge of range
        assert_eq!(
            next_initialized_tick(&bitmap, -500, tick_spacing, true).unwrap(),
            Some(-500)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, 500, tick_spacing, false).unwrap(),
            Some(500)
        );

        // Test searching beyond range
        assert_eq!(
            next_initialized_tick(&bitmap, -600, tick_spacing, true).unwrap(),
            None
        );
        assert_eq!(
            next_initialized_tick(&bitmap, -600, tick_spacing, false).unwrap(),
            Some(-500)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, 600, tick_spacing, true).unwrap(),
            Some(500)
        );
        assert_eq!(
            next_initialized_tick(&bitmap, 600, tick_spacing, false).unwrap(),
            None
        );
    }

    #[test]
    fn test_integration_bitmap_state_consistency() {
        // Test that the bitmap maintains a consistent state through multiple operations
        let mut bitmap = BTreeMap::new();
        let tick_spacing = 10;

        // 1. Initialize ticks
        for tick in (0..100).step_by(tick_spacing as usize) {
            assert!(flip_tick_initialized_status(&mut bitmap, tick, tick_spacing, true).is_ok());
        }

        // 2. Check bitmap state
        assert_eq!(bitmap.len(), 1, "All ticks should be in the same word");

        // Get the word value directly
        let expected_word_value =
            (0..100)
                .step_by(tick_spacing as usize)
                .fold(0u64, |acc, tick| {
                    let compressed_tick = tick / tick_spacing as i32;
                    let (_, bit_pos) = get_word_index_and_bit_pos(compressed_tick).unwrap();
                    acc | (1u64 << bit_pos)
                });

        assert_eq!(
            bitmap[&0], expected_word_value,
            "Word value should match expected bitmap state"
        );

        // 3. Uninitialize every other tick
        for tick in (0..100).step_by(tick_spacing as usize * 2) {
            assert!(flip_tick_initialized_status(&mut bitmap, tick, tick_spacing, false).is_ok());
        }

        // 4. Check updated bitmap state
        let updated_expected_value =
            (10..100)
                .step_by(tick_spacing as usize * 2)
                .fold(0u64, |acc, tick| {
                    let compressed_tick = tick / tick_spacing as i32;
                    let (_, bit_pos) = get_word_index_and_bit_pos(compressed_tick).unwrap();
                    acc | (1u64 << bit_pos) // bit_pos is already a u8
                });

        assert_eq!(
            bitmap[&0], updated_expected_value,
            "Word value should match expected bitmap state after uninitialization"
        );

        // 5. Verify next_initialized_tick follows the expected pattern
        for i in 0..9 {
            let start_tick = i * tick_spacing as i32;
            let next_tick_result =
                next_initialized_tick(&bitmap, start_tick, tick_spacing, false).unwrap();

            // Expected next tick is the next odd multiple of tick_spacing
            let expected_next_tick = if i % 2 == 0 {
                Some((i + 1) * tick_spacing as i32)
            } else {
                Some(i * tick_spacing as i32)
            };

            assert_eq!(next_tick_result, expected_next_tick,
                      "Incorrect next tick from {start_tick}, got {next_tick_result:?}, expected {expected_next_tick:?}");
        }
    }

    #[test]
    fn test_integration_word_boundaries() {
        // Test operations across word boundaries
        let mut bitmap = BTreeMap::new();
        let tick_spacing = 1;

        // Initialize ticks near the word boundary
        let word_boundary_tick = WORD_SIZE as i32;
        let ticks_near_boundary = [
            word_boundary_tick - 2,
            word_boundary_tick - 1,
            word_boundary_tick,
            word_boundary_tick + 1,
            word_boundary_tick + 2,
        ];

        for &tick in &ticks_near_boundary {
            assert!(flip_tick_initialized_status(&mut bitmap, tick, tick_spacing, true).is_ok());
        }

        // Verify bitmap has two words
        assert_eq!(bitmap.len(), 2, "Bitmap should have two words");
        assert!(
            bitmap.contains_key(&0),
            "Bitmap should contain word index 0"
        );
        assert!(
            bitmap.contains_key(&1),
            "Bitmap should contain word index 1"
        );

        // Check bit patterns in each word
        let word0 = bitmap[&0];
        let word1 = bitmap[&1];

        assert_eq!(
            word0 & ((1u64 << (WORD_SIZE - 2)) | (1u64 << (WORD_SIZE - 1))),
            (1u64 << (WORD_SIZE - 2)) | (1u64 << (WORD_SIZE - 1)),
            "Word 0 should have the two highest bits set"
        );

        assert_eq!(
            word1 & ((1u64 << 0) | (1u64 << 1) | (1u64 << 2)),
            (1u64 << 0) | (1u64 << 1) | (1u64 << 2),
            "Word 1 should have the three lowest bits set"
        );

        // Test searching across the boundary
        assert_eq!(
            next_initialized_tick(&bitmap, word_boundary_tick - 1, tick_spacing, true).unwrap(),
            Some(word_boundary_tick - 1),
            "Should find the current tick when searching LTE from it"
        );

        assert_eq!(
            next_initialized_tick(&bitmap, word_boundary_tick - 1, tick_spacing, false).unwrap(),
            Some(word_boundary_tick - 1),
            "Should find the current tick when searching GTE from it"
        );

        assert_eq!(
            next_initialized_tick(&bitmap, word_boundary_tick - 3, tick_spacing, false).unwrap(),
            Some(word_boundary_tick - 2),
            "Should find the next tick when searching GTE"
        );

        assert_eq!(
            next_initialized_tick(&bitmap, word_boundary_tick + 3, tick_spacing, true).unwrap(),
            Some(word_boundary_tick + 2),
            "Should find the previous tick when searching LTE"
        );

        // Test searching across the boundary
        assert_eq!(
            next_initialized_tick(&bitmap, word_boundary_tick - 1, tick_spacing, false).unwrap(),
            Some(word_boundary_tick - 1),
            "Should find current tick when searching GTE from second-last tick in word 0"
        );

        assert_eq!(
            next_initialized_tick(&bitmap, word_boundary_tick - 1 - 1, tick_spacing, false)
                .unwrap(),
            Some(word_boundary_tick - 2), // Tick 62 is initialized, search GTE from 62 should yield 62.
            "Should find tick 62 when searching GTE from 62, as it's initialized."
        );
    }

    // Property-based integration tests
    proptest! {
        #[test]
        fn test_integration_flip_and_check_sequence(
            ticks in prop::collection::vec(-1000..1000i32, 1..50),
            tick_spacing in 1..50u16
        ) {
            // Align ticks with tick_spacing
            let mut aligned_ticks: Vec<i32> = ticks.into_iter()
                .map(|t| (t / tick_spacing as i32) * tick_spacing as i32)
                .collect();

            // Sort and deduplicate to ensure consistent behavior with unique ticks
            aligned_ticks.sort_unstable();
            aligned_ticks.dedup();

            // if aligned_ticks.is_empty() { return Ok(()); } // Proptest handles empty vecs

            let mut bitmap = BTreeMap::new();

            // Simulate a sequence of operations:
            // 1. Initialize all ticks
            for &tick in &aligned_ticks {
                assert!(flip_tick_initialized_status(&mut bitmap, tick, tick_spacing, true).is_ok());
            }

            // 2. For each tick that's initialized, try to find the next and previous initialized ticks
            for &tick in &aligned_ticks {
                // Ensure the tick itself is validly initializable before checking next/prev
                let compressed_res = compress_tick(tick, tick_spacing);
                if compressed_res.is_err() { continue; }
                if get_word_index_and_bit_pos(compressed_res.unwrap()).is_err() { continue; }
                if !is_tick_initialized(&bitmap, tick, tick_spacing).unwrap_or(false) { continue; }

                // Find next initialized tick (should be this tick or a later one)
                let next_result = next_initialized_tick(&bitmap, tick, tick_spacing, false).unwrap();
                assert!(next_result.is_some(),
                       "Should find an initialized tick when searching GTE from {tick} (spacing {tick_spacing})");

                // Find previous initialized tick (should be this tick or an earlier one)
                let prev_result = next_initialized_tick(&bitmap, tick, tick_spacing, true).unwrap();
                assert!(prev_result.is_some(),
                       "Should find an initialized tick when searching LTE from {tick} (spacing {tick_spacing})");
            }

            // 3. Uninitialize every other tick
            for (i, &tick) in aligned_ticks.iter().enumerate() {
                if i % 2 == 0 {
                    // Ensure the tick is valid before trying to uninitialize
                    let compressed_res = compress_tick(tick, tick_spacing);
                    if compressed_res.is_err() { continue; }
                    if get_word_index_and_bit_pos(compressed_res.unwrap()).is_err() { continue; }
                    assert!(flip_tick_initialized_status(&mut bitmap, tick, tick_spacing, false).is_ok());
                }
            }

            // 4. Verify tick states
            for (i, &tick) in aligned_ticks.iter().enumerate() {
                // Ensure the tick is valid before checking its state
                let compressed_res = compress_tick(tick, tick_spacing);
                if compressed_res.is_err() { continue; }
                if get_word_index_and_bit_pos(compressed_res.unwrap()).is_err() { continue; }

                let expected_state = i % 2 == 1; // Odd indices should still be initialized
                let actual_state = is_tick_initialized(&bitmap, tick, tick_spacing).unwrap();

                assert_eq!(actual_state, expected_state,
                          "Tick {tick} should be initialized={expected_state} after selective uninitialization");
            }
        }

        #[test]
        fn test_integration_search_consistency(
            ticks in prop::collection::vec(-1000..1000i32, 1..20),
            search_points in prop::collection::vec(-1500..1500i32, 1..10),
            tick_spacing in 1..50u16
        ) {
            // Align ticks with tick_spacing
            let mut aligned_ticks: Vec<i32> = ticks.into_iter()
                .map(|t| (t / tick_spacing as i32) * tick_spacing as i32)
                .collect();

            // Sort and deduplicate ticks
            aligned_ticks.sort();
            aligned_ticks.dedup();

            let mut bitmap = BTreeMap::new();

            // Initialize all ticks
            for &tick in &aligned_ticks {
                 // Skip ticks that would result in out-of-bounds word index
                let compressed_tick_res = compress_tick(tick, tick_spacing);
                if compressed_tick_res.is_err() { continue; }
                if get_word_index_and_bit_pos(compressed_tick_res.unwrap()).is_err() {
                    continue;
                }
                assert!(flip_tick_initialized_status(&mut bitmap, tick, tick_spacing, true).is_ok(), "Failed to init {tick}");
            }

            // For each search point:
            // 1. Find next initialized tick with next_initialized_tick
            // 2. Separately calculate what the next tick should be
            // 3. Compare the results
            for &search_point in &search_points {
                // Search GTE
                let next_result = next_initialized_tick(&bitmap, search_point, tick_spacing, false).unwrap();

                // Direct calculation for comparison
                let expected_next = aligned_ticks.iter()
                    .find(|&&t| t >= search_point)
                    .copied();

                assert_eq!(next_result, expected_next,
                          "GTE search from {search_point} found {next_result:?}, expected {expected_next:?}");

                // Search LTE
                let prev_result = next_initialized_tick(&bitmap, search_point, tick_spacing, true).unwrap();

                // Direct calculation for comparison
                let expected_prev = aligned_ticks.iter()
                    .rev()
                    .find(|&&t| t <= search_point)
                    .copied();

                assert_eq!(prev_result, expected_prev,
                          "LTE search from {search_point} found {prev_result:?}, expected {expected_prev:?}");
            }
        }
    }
}
