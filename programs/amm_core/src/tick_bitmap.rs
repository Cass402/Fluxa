use crate::errors::ErrorCode;
use anchor_lang::prelude::*;
use std::collections::BTreeMap;
// The size of each word in the bitmap, corresponding to the number of bits in u64
const WORD_SIZE: usize = 64;

/// Compresses a tick index by dividing it by the tick spacing.
///
/// # Arguments
/// * `tick` - The tick index to compress
/// * `tick_spacing` - The spacing between ticks
///
/// # Returns
/// * `Result<i32>` - The compressed tick index or an error if the tick spacing is invalid
///   or the tick is not aligned with the spacing
///
/// # Example
/// ```
/// let compressed_tick = compress_tick(100, 10);
/// assert_eq!(compressed_tick, Ok(10));
/// ```
pub(crate) fn compress_tick(tick: i32, tick_spacing: u16) -> Result<i32> {
    let tick_spacing_i32 = tick_spacing as i32;
    if tick_spacing_i32 <= 0 {
        // This should be validated at pool creation, but good to have a safeguard.
        return Err(ErrorCode::InvalidTickSpacing.into());
    }
    if tick % tick_spacing_i32 != 0 {
        // This indicates an unaligned tick, which should ideally be caught earlier.
        return Err(ErrorCode::InvalidTickRange.into());
    }
    Ok(tick / tick_spacing_i32)
}

/// Decompresses a compressed tick index by multiplying it by the tick spacing.
///
/// # Arguments
/// * `compressed_tick` - The compressed tick index
/// * `tick_spacing` - The spacing between ticks
///
/// # Returns
/// * `i32` - The decompressed tick index
///
/// # Example
///
/// let tick = decompress_tick(10, 10);
/// assert_eq!(tick, 100);
///
pub(crate) fn decompress_tick(compressed_tick: i32, tick_spacing: u16) -> i32 {
    compressed_tick.wrapping_mul(tick_spacing as i32)
}

/// Calculates the word index and bit position for a compressed tick index in the bitmap.
///
/// # Arguments
/// * `compressed_tick` - The compressed tick index
///
/// # Returns
/// * `(i16, u8)` - A tuple containing the word index and bit position within that word
///
/// # Example
///
/// let (word_index, bit_pos) = get_word_index_and_bit_pos(10);
/// assert_eq!(word_index, 0);
/// assert_eq!(bit_pos, 10);
/// # Errors
/// Returns `Err` if the `compressed_tick` results in a word index outside of `i16` bounds.
pub(crate) fn get_word_index_and_bit_pos(compressed_tick: i32) -> Result<(i16, u8)> {
    let word_index_i64 = (compressed_tick as i64).div_euclid(WORD_SIZE as i64);
    let word_index: i16 = word_index_i64
        .try_into()
        .map_err(|_| error!(ErrorCode::TickWordIndexOutOfBounds))?;

    let bit_pos = (compressed_tick - word_index as i32 * WORD_SIZE as i32) as u8;
    Ok((word_index, bit_pos))
}

/// Finds the next initialized bit in a bitmap word, searching either up or down from a starting position.
///
/// # Arguments
/// * `bitmap_word` - The bitmap word to search
/// * `start_bit_pos` - The bit position to start searching from
/// * `search_lte` - If true, search downwards (less than or equal to start_bit_pos),
///   otherwise search upwards (greater than start_bit_pos)
///
/// # Returns
/// * `Option<u8>` - The position of the next initialized bit if found, None otherwise
///
/// # Example
///
/// let bitmap_word = 0b1010;
/// let next_bit = next_initialized_bit_in_word(bitmap_word, 3, true);
/// assert_eq!(next_bit, Some(3));
///
pub(crate) fn next_initialized_bit_in_word(
    bitmap_word: u64,
    start_bit_pos: u8,
    search_lte: bool,
) -> Option<u8> {
    if bitmap_word == 0 {
        return None;
    }

    if search_lte {
        // Search downwards (towards LSB), from start_bit_pos to 0.
        // Ensure start_bit_pos is within bounds [0, WORD_SIZE - 1].
        let search_start = start_bit_pos.min((WORD_SIZE - 1) as u8);
        for i in (0..=search_start).rev() {
            if (bitmap_word & (1u64 << i)) != 0 {
                return Some(i);
            }
        }
    } else {
        // Search upwards (towards MSB), from start_bit_pos to WORD_SIZE - 1.
        // Ensure start_bit_pos is within bounds [0, WORD_SIZE - 1].
        if start_bit_pos >= WORD_SIZE as u8 {
            return None; // start_bit_pos is out of valid range for upward search
        }
        for i in start_bit_pos..(WORD_SIZE as u8) {
            if (bitmap_word & (1u64 << i)) != 0 {
                return Some(i);
            }
        }
    }
    None
}

/// Flips the initialization status of a tick in the bitmap.
///
/// # Arguments
/// * `tick_bitmap` - The bitmap storing tick initialization status
/// * `tick` - The tick to update
/// * `tick_spacing` - The spacing between ticks
/// * `set_as_initialized` - If true, set the tick as initialized; if false, set as uninitialized
///
/// # Returns
/// * `Result<()>` - Success if the operation completed, error otherwise
///
/// # Example
///
///
/// let mut bitmap = BTreeMap::new();
/// flip_tick_initialized_status(&mut bitmap, 100, 10, true)?; // Initialize tick 100
/// flip_tick_initialized_status(&mut bitmap, 100, 10, false)?; // Uninitialize tick 100
///
pub fn flip_tick_initialized_status(
    tick_bitmap: &mut BTreeMap<i16, u64>,
    tick: i32,
    tick_spacing: u16,
    set_as_initialized: bool,
) -> Result<()> {
    let compressed_tick = compress_tick(tick, tick_spacing)?;
    let (word_idx, bit_pos) = get_word_index_and_bit_pos(compressed_tick)?;

    let bit_mask = 1u64 << bit_pos;

    if set_as_initialized {
        let bitmap_word = tick_bitmap.entry(word_idx).or_insert(0);
        *bitmap_word |= bit_mask;
    } else if let Some(bitmap_word) = tick_bitmap.get_mut(&word_idx) {
        *bitmap_word &= !bit_mask;
        if *bitmap_word == 0 {
            tick_bitmap.remove(&word_idx);
        }
    }
    Ok(())
}

/// Checks if a tick is initialized in the bitmap.
///
/// # Arguments
/// * `tick_bitmap` - The bitmap storing tick initialization status
/// * `tick` - The tick to check
/// * `tick_spacing` - The spacing between ticks
///
/// # Returns
/// * `Result<bool>` - True if the tick is initialized, false otherwise
///
/// # Example
///
///
/// let bitmap = BTreeMap::new();
/// let is_initialized = is_tick_initialized(&bitmap, 100, 10)?;
///
pub fn is_tick_initialized(
    tick_bitmap: &BTreeMap<i16, u64>,
    tick: i32,
    tick_spacing: u16,
) -> Result<bool> {
    let compressed_tick = compress_tick(tick, tick_spacing)?;
    let (word_idx, bit_pos) = get_word_index_and_bit_pos(compressed_tick)?;

    match tick_bitmap.get(&word_idx) {
        Some(bitmap_word) => Ok((bitmap_word & (1u64 << bit_pos)) != 0),
        None => Ok(false),
    }
}

/// Finds the next initialized tick in the bitmap.
///
/// # Arguments
/// * `tick_bitmap` - The bitmap storing tick initialization status
/// * `current_tick_approx` - The tick to start searching from
/// * `tick_spacing` - The spacing between ticks
/// * `search_lte` - If true, search for ticks less than or equal to current_tick_approx.
///   If false, search for ticks greater than or equal to current_tick_approx.
///
/// # Returns
/// * `Result<Option<i32>>` - The next initialized tick if found, None otherwise
///
/// # Errors
/// * Returns an error if tick_spacing is invalid (zero or negative)
///
/// # Example
/// ```
/// let bitmap = BTreeMap::new();
/// let next_tick = next_initialized_tick(&bitmap, 100, 10, true)?;
/// assert_eq!(next_tick, None);
/// ```
/// # Note
/// This function searches for the next initialized tick in the bitmap.
pub fn next_initialized_tick(
    tick_bitmap: &BTreeMap<i16, u64>,
    current_tick_approx: i32,
    tick_spacing: u16,
    search_lte: bool,
) -> Result<Option<i32>> {
    let tick_spacing_i32 = tick_spacing as i32;
    if tick_spacing_i32 <= 0 {
        return Err(ErrorCode::InvalidTickSpacing.into());
    }

    if tick_bitmap.is_empty() {
        return Ok(None);
    }

    // Determine the compressed tick to start searching from, relative to current_tick_approx.
    // For LTE, start from floor(current_tick_approx / tick_spacing).
    // For GTE, start from ceil(current_tick_approx / tick_spacing).
    let compressed_search_start_tick_ref = if search_lte {
        current_tick_approx.div_euclid(tick_spacing_i32)
    } else {
        // Calculate ceil(current_tick_approx / tick_spacing_i32)
        // This handles positive, negative, and zero current_tick_approx correctly.
        let q = current_tick_approx / tick_spacing_i32; // Truncating division
        let r = current_tick_approx % tick_spacing_i32;
        if r == 0 {
            q
        } else if current_tick_approx > 0 {
            // e.g., current_tick_approx=7, spacing=10. q=0, r=7. Returns 0+1=1 (correct, for tick 10).
            q + 1
        } else {
            // e.g., current_tick_approx=-7, spacing=10. q=0, r=-7. Returns 0 (correct, for tick 0).
            // e.g., current_tick_approx=-17, spacing=10. q=-1, r=-7. Returns -1 (correct, for tick -10).
            q
        }
    };

    // Ensure the compressed search reference tick maps to a word index within i16 bounds
    // The valid range for compressed ticks is [i16::MIN * WORD_SIZE, (i16::MAX + 1) * WORD_SIZE - 1]
    let max_compressed_tick_for_i16_word =
        (i16::MAX as i32) * WORD_SIZE as i32 + (WORD_SIZE - 1) as i32;
    let min_compressed_tick_for_i16_word = (i16::MIN as i32) * WORD_SIZE as i32;
    let compressed_search_start_tick_ref = compressed_search_start_tick_ref.clamp(
        min_compressed_tick_for_i16_word,
        max_compressed_tick_for_i16_word,
    );

    let (search_ref_word_idx, search_ref_bit_pos) =
        get_word_index_and_bit_pos(compressed_search_start_tick_ref)?;

    if search_lte {
        // 1. Search current word, downwards from current_bit_pos
        if let Some(word_val) = tick_bitmap.get(&search_ref_word_idx) {
            if let Some(found_bit_pos) =
                next_initialized_bit_in_word(*word_val, search_ref_bit_pos, true)
            {
                let found_compressed_tick =
                    search_ref_word_idx as i32 * WORD_SIZE as i32 + found_bit_pos as i32;
                return Ok(Some(decompress_tick(found_compressed_tick, tick_spacing)));
            }
        }

        // 2. Search preceding words (lower word indices)
        // BTreeMap iterators go from smallest key to largest.
        // `range(..current_word_idx).rev()` gets keys < current_word_idx, in descending order.
        for (&word_idx, &word_val) in tick_bitmap.range(..search_ref_word_idx).rev() {
            // word_val cannot be 0 because we remove zero words in flip_tick.
            // Search the entire word from MSB (WORD_SIZE - 1) downwards.
            if let Some(found_bit_pos) =
                next_initialized_bit_in_word(word_val, (WORD_SIZE - 1) as u8, true)
            {
                let found_compressed_tick =
                    word_idx as i32 * WORD_SIZE as i32 + found_bit_pos as i32;
                return Ok(Some(decompress_tick(found_compressed_tick, tick_spacing)));
            }
        }
    } else {
        // search_gte (search upwards)
        // 1. Search current word, upwards from current_bit_pos
        if let Some(word_val) = tick_bitmap.get(&search_ref_word_idx) {
            if let Some(found_bit_pos) =
                next_initialized_bit_in_word(*word_val, search_ref_bit_pos, false)
            {
                let found_compressed_tick =
                    search_ref_word_idx as i32 * WORD_SIZE as i32 + found_bit_pos as i32;
                return Ok(Some(decompress_tick(found_compressed_tick, tick_spacing)));
            }
        }

        // 2. Search succeeding words (higher word indices)
        // `range((current_word_idx + 1)..)` gets keys > current_word_idx, in ascending order.
        let start_next_word_idx = match search_ref_word_idx.checked_add(1) {
            Some(idx) => idx,
            None => return Ok(None), // current_word_idx is i16::MAX, no succeeding words
        };

        for (&word_idx, &word_val) in tick_bitmap.range(start_next_word_idx..) {
            // word_val cannot be 0.
            // Search the entire word from LSB (0) upwards.
            if let Some(found_bit_pos) = next_initialized_bit_in_word(word_val, 0, false) {
                let found_compressed_tick =
                    word_idx as i32 * WORD_SIZE as i32 + found_bit_pos as i32;
                return Ok(Some(decompress_tick(found_compressed_tick, tick_spacing)));
            }
        }
    }

    Ok(None) // No initialized tick found in the search direction
}
