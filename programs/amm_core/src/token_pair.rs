/// Fluxa Token Pair Module
///
/// This module defines the token pair data structure and related functionality for the Fluxa AMM.
/// A token pair represents a tradable pair of tokens and maintains references to all liquidity pools
/// created for this pair, along with statistics and configuration data.
///
/// Token pairs in Fluxa are created as Program Derived Addresses (PDAs) to ensure deterministic
/// discovery and uniqueness, with seeds derived from both token mint addresses in a canonically
/// ordered way to prevent duplicate pairs.
use anchor_lang::prelude::*;

/// Data structure representing a token pair in the Fluxa AMM
///
/// A token pair is the fundamental trading relationship in the protocol, connecting
/// two tokens that can be exchanged. Each token pair can have multiple liquidity pools
/// with different fee tiers to accommodate different trading strategies and volatility profiles.
///
/// # Features
/// - Maintains references to all pools for this token pair
/// - Tracks trading statistics and fees
/// - Integrates oracle price data
/// - Supports governance verification for trusted pairs
/// - Versioned for future protocol upgrades
///
/// # Storage
/// The account is created as a Program Derived Address (PDA) with seeds:
/// `["token_pair", token_a_mint, token_b_mint]`
#[account]
#[derive(Debug)]
pub struct TokenPair {
    /// The authority that controls this token pair (typically the DAO or governance)
    /// This address has the ability to modify verification status and other settings.
    pub authority: Pubkey,

    /// Token A mint address
    /// The first token in the pair, canonically ordered to be less than token B.
    pub token_a_mint: Pubkey,

    /// Token B mint address
    /// The second token in the pair, canonically ordered to be greater than token A.
    pub token_b_mint: Pubkey,

    /// Token A decimals
    /// Number of decimal places used by the first token, cached for efficiency.
    pub token_a_decimals: u8,

    /// Token B decimals
    /// Number of decimal places used by the second token, cached for efficiency.
    pub token_b_decimals: u8,

    /// Pools associated with this token pair
    /// Each element is a tuple of (pool_address, fee_tier) where fee_tier is in basis points.
    /// For example, 3000 represents a 0.3% fee tier.
    pub pools: Vec<(Pubkey, u16)>,

    /// Total trading volume denominated in Token A
    /// Cumulative volume tracked for analytics and potential fee sharing.
    pub total_volume_token_a: u64,

    /// Total trading volume denominated in Token B
    /// Cumulative volume tracked for analytics and potential fee sharing.
    pub total_volume_token_b: u64,

    /// Total fees generated across all pools for this token pair
    /// Denominated in the protocol's native token (SOL).
    pub total_fees_generated: u64,

    /// Last known exchange price, used as an oracle data source
    /// Price expressed as a fixed-point number with 6 decimal places (1_000_000 = 1.0)
    pub last_oracle_price: u64,

    /// Timestamp of the last oracle price update (Unix timestamp)
    /// Helps determine the freshness and reliability of the price data.
    pub last_oracle_update: i64,

    /// Verification status flag
    /// When true, the pair has been reviewed and approved by governance.
    /// Verified pairs may receive benefits like reduced fees or highlighted UI placement.
    pub is_verified: bool,

    /// Version of the token pair structure for future protocol upgrades
    /// Allows backward compatibility as the protocol evolves.
    pub version: u8,

    /// Reserved space for future extensions
    /// Prevents expensive account reallocation when new fields are added.
    pub reserved: [u8; 64],
}

impl TokenPair {
    /// Maximum number of pools allowed per token pair
    /// This limit prevents excessive fragmentation of liquidity.
    pub const MAX_POOLS: usize = 10;

    /// Total serialized size of the TokenPair account in bytes
    pub const LEN: usize = 32 + // authority
        32 + // token_a_mint
        32 + // token_b_mint
        1 +  // token_a_decimals
        1 +  // token_b_decimals
        (32 + 2) * Self::MAX_POOLS + // pools (address + fee_tier)
        8 +  // total_volume_token_a
        8 +  // total_volume_token_b
        8 +  // total_fees_generated
        8 +  // last_oracle_price
        8 +  // last_oracle_update
        1 +  // is_verified
        1 +  // version
        64; // reserved space

    /// Registers a new liquidity pool with this token pair
    ///
    /// Adds a new pool to the token pair's registry, enabling it to be discovered
    /// by traders and liquidity providers. Each pool has a specific fee tier.
    ///
    /// # Parameters
    /// * `pool_address` - The address of the pool account to register
    /// * `fee_tier` - The fee tier of the pool in basis points (e.g. 3000 for 0.3%)
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    ///
    /// # Errors
    /// * `TokenPairError::TooManyPools` - If the maximum number of pools is exceeded
    /// * `TokenPairError::PoolAlreadyExists` - If the pool is already registered
    pub fn add_pool(&mut self, pool_address: Pubkey, fee_tier: u16) -> Result<()> {
        require!(
            self.pools.len() < Self::MAX_POOLS,
            TokenPairError::TooManyPools
        );

        // Check if the pool already exists
        require!(
            !self.pools.iter().any(|(addr, _)| *addr == pool_address),
            TokenPairError::PoolAlreadyExists
        );

        self.pools.push((pool_address, fee_tier));
        Ok(())
    }

    /// Deregisters a liquidity pool from this token pair
    ///
    /// Removes a pool from the token pair's registry when it's decommissioned
    /// or no longer relevant. This helps maintain an accurate list of active pools.
    ///
    /// # Parameters
    /// * `pool_address` - The address of the pool account to deregister
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    ///
    /// # Errors
    /// * `TokenPairError::PoolNotFound` - If the specified pool doesn't exist
    pub fn remove_pool(&mut self, pool_address: Pubkey) -> Result<()> {
        let initial_len = self.pools.len();
        self.pools.retain(|(addr, _)| *addr != pool_address);

        require!(self.pools.len() < initial_len, TokenPairError::PoolNotFound);

        Ok(())
    }

    /// Updates trading statistics for the token pair
    ///
    /// Accumulates volume and fee data for analytics, fee sharing calculations,
    /// and protocol governance decisions. Uses saturating addition to prevent overflows.
    ///
    /// # Parameters
    /// * `volume_a` - The additional volume in token A to record
    /// * `volume_b` - The additional volume in token B to record
    /// * `fees` - The additional fees generated to record
    pub fn update_statistics(&mut self, volume_a: u64, volume_b: u64, fees: u64) {
        self.total_volume_token_a = self.total_volume_token_a.saturating_add(volume_a);
        self.total_volume_token_b = self.total_volume_token_b.saturating_add(volume_b);
        self.total_fees_generated = self.total_fees_generated.saturating_add(fees);
    }

    /// Updates the oracle price data for the token pair
    ///
    /// Records the latest price information for use as a price oracle.
    /// Includes a timestamp to track data freshness.
    ///
    /// # Parameters
    /// * `price` - The latest price as a fixed-point number
    /// * `clock` - Reference to the Solana clock sysvar for timestamp
    pub fn update_oracle_price(&mut self, price: u64, clock: &Sysvar<Clock>) {
        self.last_oracle_price = price;
        self.last_oracle_update = clock.unix_timestamp;
    }

    /// Sets the verification status of the token pair
    ///
    /// Used by governance to mark token pairs as verified after review.
    /// Verified pairs may receive benefits like reduced fees or UI prominence.
    ///
    /// # Parameters
    /// * `is_verified` - The new verification status
    pub fn set_verification(&mut self, is_verified: bool) {
        self.is_verified = is_verified;
    }
}

/// Derives the deterministic PDA address for a token pair
///
/// This utility function computes the canonical address for a token pair based on
/// the two token mint addresses. It ensures tokens are ordered consistently to
/// prevent duplicate pairs (e.g., A-B and B-A would yield the same address).
///
/// # Parameters
/// * `token_a_mint` - The mint address of the first token
/// * `token_b_mint` - The mint address of the second token
/// * `program_id` - The AMM program ID
///
/// # Returns
/// * `(Pubkey, u8)` - Tuple containing the derived address and bump seed
pub fn find_token_pair_address(
    token_a_mint: &Pubkey,
    token_b_mint: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    // Canonical ordering of token mints to ensure consistent results
    let (mint_1, mint_2) = if token_a_mint < token_b_mint {
        (token_a_mint, token_b_mint)
    } else {
        (token_b_mint, token_a_mint)
    };

    Pubkey::find_program_address(
        &[b"token_pair", mint_1.as_ref(), mint_2.as_ref()],
        program_id,
    )
}

/// Error codes specific to token pair operations
///
/// These error codes provide detailed information about failures in
/// token pair-related operations.
#[error_code]
pub enum TokenPairError {
    /// Returned when attempting to add more pools than the maximum allowed
    #[msg("Too many pools for this token pair")]
    TooManyPools,

    /// Returned when attempting to add a pool that already exists
    #[msg("Pool already exists for this token pair")]
    PoolAlreadyExists,

    /// Returned when attempting to remove a pool that doesn't exist
    #[msg("Pool not found for this token pair")]
    PoolNotFound,

    /// Returned when an unauthorized account attempts to modify a token pair
    #[msg("Only the authority can modify this token pair")]
    UnauthorizedAccess,

    /// Returned when invalid token mints are provided
    #[msg("Invalid token mints")]
    InvalidTokenMints,
}
