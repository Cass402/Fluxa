/// Tick Bitmap Module
///
/// This module implements a space-efficient bitmap for tracking initialized ticks.
/// It allows for fast traversal of initialized ticks during swap operations without
/// needing to explicitly check every possible tick value.
///
/// Each bit in the bitmap represents whether a particular tick is initialized,
/// enabling efficient binary search for the next initialized tick during swaps.
use crate::errors::ErrorCode;
use anchor_lang::prelude::*;
use std::collections::HashMap;
use std::ops::{BitAnd, BitOr, BitXor, Not, Shl};

// Import the U256 type from ethereum_types
use ethereum_types::U256;

/// Number of bits in a word
pub const WORD_SIZE: usize = 256;

/// A wrapper around U256 that implements AnchorSerialize and AnchorDeserialize
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct U256Wrapper(U256);

impl U256Wrapper {
    /// Create a new U256Wrapper
    pub fn new(value: U256) -> Self {
        Self(value)
    }

    /// Convert from u32
    pub fn from_u32(value: u32) -> Self {
        Self(U256::from(value))
    }

    /// Get the inner U256 value
    pub fn value(&self) -> U256 {
        self.0
    }

    /// Check if the wrapper is zero
    pub fn is_zero(&self) -> bool {
        self.0 == U256::zero()
    }

    /// Get leading zeros
    pub fn leading_zeros(&self) -> u32 {
        self.0.leading_zeros()
    }

    /// Get trailing zeros
    pub fn trailing_zeros(&self) -> u32 {
        self.0.trailing_zeros()
    }

    /// Check equality with zero
    pub fn eq_zero(&self) -> bool {
        self.0 == U256::zero()
    }

    /// Get max value
    pub fn max_value() -> Self {
        Self(U256::max_value())
    }

    /// Get zero value
    pub fn zero() -> Self {
        Self(U256::zero())
    }
}

// Implement BitAnd for U256Wrapper
impl BitAnd for U256Wrapper {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

// Implement BitOr for U256Wrapper
impl BitOr for U256Wrapper {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

// Implement BitXor for U256Wrapper
impl BitXor for U256Wrapper {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

// Implement Not for U256Wrapper
impl Not for U256Wrapper {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

// Implement Shl for U256Wrapper and u8
impl Shl<u8> for U256Wrapper {
    type Output = Self;

    fn shl(self, rhs: u8) -> Self::Output {
        Self(self.0 << rhs)
    }
}

// Implement Sub for U256Wrapper
impl std::ops::Sub for U256Wrapper {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

// Implement AnchorSerialize for U256Wrapper
impl AnchorSerialize for U256Wrapper {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        // Convert U256 to bytes and serialize
        let mut bytes = [0u8; 32];
        self.0.to_big_endian(&mut bytes);
        writer.write_all(&bytes)
    }
}

// Implement AnchorDeserialize for U256Wrapper
impl AnchorDeserialize for U256Wrapper {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        if buf.len() < 32 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "buffer too small for U256",
            ));
        }

        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&buf[..32]);
        *buf = &buf[32..];

        Ok(Self(U256::from_big_endian(&bytes)))
    }

    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut bytes = [0u8; 32];
        reader.read_exact(&mut bytes)?;
        Ok(Self(U256::from_big_endian(&bytes)))
    }
}

/// Represents a single bitmap word that tracks 256 adjacent ticks
#[derive(Debug, Default, Copy, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct TickBitmapWord {
    /// The bitmap data - each bit represents an initialized tick
    pub bitmap: U256Wrapper,
}

/// Wrapper struct for managing the tick bitmap
/// This struct provides methods for finding initialized ticks efficiently
#[derive(Debug, Default)]
pub struct TickBitmap {
    /// Map from word positions to bitmap words
    pub bitmap_map: HashMap<i16, TickBitmapWord>,
}

impl TickBitmap {
    /// Creates a new empty tick bitmap
    pub fn new() -> Self {
        Self {
            bitmap_map: HashMap::new(),
        }
    }

    /// Finds the next initialized tick in the given direction
    ///
    /// # Parameters
    /// * `tick` - The current tick
    /// * `tick_spacing` - The spacing between ticks
    ///
    /// # Returns
    /// * `Result<i32>` - The next initialized tick, or MAX_TICK if not found
    pub fn next_initialized_tick(&self, tick: i32, tick_spacing: u16) -> Result<i32> {
        let (next_tick, initialized) = next_initialized_tick_in_direction(
            &self.bitmap_map,
            tick,
            tick_spacing,
            false, // search upward
        )?;

        if initialized {
            Ok(next_tick)
        } else {
            // Not found, return the boundary tick
            // In production, this would typically be crate::constants::MAX_TICK
            Ok(next_tick)
        }
    }

    pub fn prev_initialized_tick(&self, tick: i32, tick_spacing: u16) -> Result<i32> {
        // 1. Protect against overflow
        if tick == i32::MAX {
            return Ok(i32::MAX);
        }

        // 2. Now do the usual downward search
        let (next_tick, initialized) = next_initialized_tick_in_direction(
            &self.bitmap_map,
            tick,
            tick_spacing,
            true, // searching downward
        )?;

        if initialized {
            Ok(next_tick)
        } else {
            // No bit found at all: return the natural boundary
            Ok(next_tick)
        }
    }

    /// Updates the bitmap when a tick becomes initialized or uninitialized
    ///
    /// # Parameters
    /// * `tick` - The tick to update
    /// * `tick_spacing` - The spacing between ticks
    /// * `initialized` - Whether to mark the tick as initialized or uninitialized
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub fn update_bitmap(&mut self, tick: i32, tick_spacing: u16, initialized: bool) -> Result<()> {
        update_tick_bitmap(&mut self.bitmap_map, tick, tick_spacing, initialized)
    }

    /// Checks if a specific tick is initialized in the bitmap
    ///
    /// # Parameters
    /// * `tick` - The tick to check
    /// * `tick_spacing` - The spacing between ticks
    ///
    /// # Returns
    /// * `bool` - Whether the tick is initialized
    pub fn is_tick_initialized(&self, tick: i32, tick_spacing: u16) -> bool {
        // Ensure the tick is a multiple of tick spacing
        if tick % tick_spacing as i32 != 0 {
            return false;
        }

        // Calculate word and bit position
        let compressed_tick = tick / tick_spacing as i32;
        let (word_pos, bit_pos) = position(compressed_tick);

        // Check if the word exists and the bit is set
        if let Some(word) = self.bitmap_map.get(&word_pos) {
            is_initialized(word, bit_pos)
        } else {
            false
        }
    }
}

/// Calculates the position in the tick bitmap for a given tick
///
/// # Parameters
/// * `tick` - The tick index to find the position for
///
/// # Returns
/// * `(i16, u8)` - The word index and bit position within that word
pub fn position(tick: i32) -> (i16, u8) {
    let word_pos = tick / WORD_SIZE as i32;
    let bit_pos = (tick % WORD_SIZE as i32) as u8;

    (word_pos as i16, bit_pos)
}

/// Checks if a tick is initialized in the bitmap
///
/// # Parameters
/// * `bitmap` - The bitmap word to check
/// * `bit_pos` - The bit position within the word
///
/// # Returns
/// * `bool` - Whether the tick is initialized
pub fn is_initialized(bitmap: &TickBitmapWord, bit_pos: u8) -> bool {
    (bitmap.bitmap & (U256Wrapper::from_u32(1) << bit_pos)) != U256Wrapper::zero()
}

/// Flips the bit for a tick in the bitmap to mark it as initialized or uninitialized
///
/// # Parameters
/// * `bitmap` - The bitmap word to modify
/// * `bit_pos` - The bit position within the word
///
/// # Returns
/// * `TickBitmapWord` - The updated bitmap
pub fn flip_tick(bitmap: &TickBitmapWord, bit_pos: u8) -> TickBitmapWord {
    TickBitmapWord {
        bitmap: bitmap.bitmap ^ (U256Wrapper::from_u32(1) << bit_pos),
    }
}

/// Finds the next initialized tick in the bitmap
///
/// # Parameters
/// * `bitmap` - The bitmap word to search
/// * `bit_pos` - The starting bit position
/// * `lte` - If true, search for ticks less than or equal to bit_pos;
///   if false, search for ticks greater than bit_pos
///
/// # Returns
/// * `Result<(bool, u8)>` - Tuple of (found, position) where position is the bit position
///   of the next initialized tick, or an error if not found
pub fn next_initialized_tick_within_word(
    bitmap: &TickBitmapWord,
    bit_pos: u8,
    lte: bool,
) -> Result<(bool, u8)> {
    let bitmap_data = bitmap.bitmap;

    // If searching for ticks less than or equal to the current position
    if lte {
        // Create a mask for all bits at positions <= bit_pos
        let mask = if bit_pos == 255 {
            U256Wrapper::max_value() // All bits set
        } else {
            (U256Wrapper::from_u32(1) << (bit_pos + 1)) - U256Wrapper::from_u32(1)
        };

        let masked_bitmap = bitmap_data & mask;

        // If no initialized ticks <= bit_pos
        if masked_bitmap.is_zero() {
            return Ok((false, 0));
        }

        // Find the most significant (highest) bit that is set
        let msb_pos = 255 - masked_bitmap.leading_zeros() as u8;
        Ok((true, msb_pos))
    }
    // If searching for ticks greater than the current position
    else {
        // Create a mask for all bits at positions > bit_pos
        let mask = !((U256Wrapper::from_u32(1) << (bit_pos + 1)) - U256Wrapper::from_u32(1));

        let masked_bitmap = bitmap_data & mask;

        // If no initialized ticks > bit_pos
        if masked_bitmap.is_zero() {
            return Ok((false, 0));
        }

        // Find the least significant (lowest) bit that is set
        let lsb_pos = masked_bitmap.trailing_zeros() as u8;
        Ok((true, lsb_pos))
    }
}

/// Finds the next initialized tick across multiple bitmap words
///
/// # Parameters
/// * `tick_current` - The current tick index
/// * `tick_spacing` - The spacing between ticks
/// * `lte` - If true, search for a tick less than or equal to the current tick;
///   if false, search for a tick greater than the current tick
///
/// # Returns
/// * `Result<(i32, bool)>` - Tuple of (next_tick, initialized)
///   - If initialized is false, next_tick will be a boundary tick
pub fn next_initialized_tick_in_direction(
    tick_bitmap_map: &std::collections::HashMap<i16, TickBitmapWord>,
    tick_current: i32,
    tick_spacing: u16,
    lte: bool,
) -> Result<(i32, bool)> {
    // Ensure the current tick is a multiple of tick spacing
    let compressed = (tick_current / tick_spacing as i32) * tick_spacing as i32;
    let (mut word_pos, bit_pos) = position(compressed / tick_spacing as i32);

    // Get the bitmap word for the current position
    let mut bitmap_word = match tick_bitmap_map.get(&word_pos) {
        Some(word) => *word,
        None => TickBitmapWord::default(), // Empty bitmap if word doesn't exist
    };

    // Search for the next initialized tick within the current word
    let (mut initialized, mut next_bit_pos) =
        next_initialized_tick_within_word(&bitmap_word, bit_pos, lte)?;

    // If not found in the current word, search in subsequent words
    if !initialized {
        // Direction to move when searching for the next word with initialized ticks
        let word_delta: i16 = if lte { -1 } else { 1 };

        // Keep searching until we find an initialized tick or reach the boundary
        loop {
            word_pos = word_pos
                .checked_add(word_delta)
                .ok_or(ErrorCode::InvalidTickRange)?;

            // Check boundaries based on direction
            if (lte && word_pos == i16::MIN) || (!lte && word_pos == i16::MAX) {
                // Return boundary ticks based on direction
                return if lte {
                    Ok((-0x80000000, false))
                } else {
                    Ok((0x7FFFFFFF, false))
                };
            }

            // Get the bitmap for the next word
            bitmap_word = match tick_bitmap_map.get(&word_pos) {
                Some(word) => *word,
                None => TickBitmapWord::default(), // Empty bitmap if word doesn't exist
            };

            // Skip if the word is empty (no initialized ticks)
            if bitmap_word.bitmap == U256Wrapper::zero() {
                continue;
            }

            // Find the next initialized tick in this word
            let bit_pos_to_use = if lte { 255 } else { 0 };
            let (found, bit_pos_found) =
                next_initialized_tick_within_word(&bitmap_word, bit_pos_to_use, lte)?;

            if found {
                initialized = true;
                next_bit_pos = bit_pos_found;
                break;
            }
        }
    }

    // Calculate the actual tick index
    let next_tick =
        ((word_pos as i32) * WORD_SIZE as i32 + next_bit_pos as i32) * tick_spacing as i32;

    Ok((next_tick, initialized))
}

/// Updates the bitmap when a tick becomes initialized or uninitialized
///
/// # Parameters
/// * `tick_bitmap_map` - Map of word positions to bitmap words
/// * `tick` - The tick being updated
/// * `tick_spacing` - The spacing between ticks
/// * `initialized` - Whether the tick is being initialized (true) or uninitialized (false)
///
/// # Returns
/// * `Result<()>` - Success or error
pub fn update_tick_bitmap(
    tick_bitmap_map: &mut std::collections::HashMap<i16, TickBitmapWord>,
    tick: i32,
    tick_spacing: u16,
    initialized: bool,
) -> Result<()> {
    // Ensure the tick is a multiple of tick spacing
    if tick % tick_spacing as i32 != 0 {
        return Err(ErrorCode::InvalidTickSpacing.into());
    }

    // Calculate word and bit position
    let compressed_tick = tick / tick_spacing as i32;
    let (word_pos, bit_pos) = position(compressed_tick);

    // Get or create the bitmap word
    let bitmap_word = tick_bitmap_map.entry(word_pos).or_default();

    // Check current status
    let is_already_initialized = is_initialized(bitmap_word, bit_pos);

    // Only update if necessary
    if initialized != is_already_initialized {
        // Flip the bit
        *bitmap_word = flip_tick(bitmap_word, bit_pos);
    }

    Ok(())
}
