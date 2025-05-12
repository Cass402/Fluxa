/// Defines the state for a user's concentrated liquidity position.
///
/// In Fluxa's concentrated liquidity model, users provide liquidity within specific
/// price ranges, defined by a lower and upper tick. The `PositionData` account
/// stores all relevant information for a single user's position in a particular pool.
use anchor_lang::prelude::*;

use crate::errors::ErrorCode;

/// Represents the state of a user's concentrated liquidity position.
///
/// For the MVP, this struct focuses on the core attributes of a position:
/// ownership, the associated pool, the tick boundaries, and the amount of liquidity.
/// Fields related to NFT representation, fee growth snapshots, and owed tokens
/// are omitted for simplification as per the MVP scope.
///
/// Accounts of this type are typically PDAs derived from elements like the owner's
/// key, the pool key, and tick indices to ensure uniqueness.
#[account]
#[derive(Default, Debug)]
pub struct PositionData {
    /// The public key of the account that owns this position.
    pub owner: Pubkey,
    /// The public key of the liquidity pool this position belongs to.
    pub pool: Pubkey,
    /// The lower tick boundary of this position. Liquidity is active when the
    /// pool's current tick is at or above this value.
    pub tick_lower_index: i32,
    /// The upper tick boundary of this position. Liquidity is active when the
    /// pool's current tick is below this value.
    pub tick_upper_index: i32,
    /// The amount of liquidity provided by this position.
    /// This is an abstract measure and its relation to token amounts depends
    /// on the price range (tick_lower_index to tick_upper_index).
    pub liquidity: u128,
    // MVP Simplification:
    // - nft_id: Pubkey (or u64 if it's an ID for an off-chain NFT)
    // - fee_growth_inside_0_last_x64: u128
    // - fee_growth_inside_1_last_x64: u128
    // - tokens_owed_0: u64
    // - tokens_owed_1: u64
}

impl PositionData {
    /// Discriminator (8) + owner (32) + pool (32) + tick_lower_index (4) + tick_upper_index (4) + liquidity (16)
    /// Note: Anchor adds 8 bytes for the discriminator.
    pub const LEN: usize = 8 + 32 + 32 + 4 + 4 + 16;

    /// Initializes a new position with the provided parameters.
    ///
    /// # Arguments
    /// * `owner` - The public key of the position's owner.
    /// * `pool` - The public key of the pool this position is for.
    /// * `tick_lower_index` - The lower tick of the position's range.
    /// * `tick_upper_index` - The upper tick of the position's range.
    /// * `liquidity` - The amount of liquidity to initialize this position with.
    pub fn initialize(
        &mut self,
        owner: Pubkey,
        pool: Pubkey,
        tick_lower_index: i32,
        tick_upper_index: i32,
        liquidity: u128,
    ) -> Result<()> {
        if tick_lower_index >= tick_upper_index {
            return err!(ErrorCode::InvalidTickRange);
        }
        // Further validation, e.g., checking against MIN_TICK/MAX_TICK from constants.rs
        // and ensuring ticks align with pool's tick_spacing, would typically be done
        // in the instruction handler calling this, or could be added here if desired.

        self.owner = owner;
        self.pool = pool;
        self.tick_lower_index = tick_lower_index;
        self.tick_upper_index = tick_upper_index;
        self.liquidity = liquidity;
        Ok(())
    }
}
