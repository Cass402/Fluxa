// Oracle Module for Time-Weighted Average Price (TWAP) calculations
//
// This module implements the Oracle functionality described in the Core Protocol Technical Design.
// It tracks historical price observations and provides methods to calculate time-weighted average
// prices, which are essential for various protocol features and external integrations.
//
// It also utilizes compression techniques from oracle_utils.rs to optimize on-chain storage.

use anchor_lang::prelude::*;
use crate::errors::ErrorCode;

/// Maximum number of observations that can be stored in the oracle
pub const MAX_ORACLE_OBSERVATIONS: usize = 64;

/// Represents a single price observation point
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct Observation {
    /// Timestamp of the observation
    pub block_timestamp: u32,
    
    /// Observed square root price in Q64.96 format
    pub sqrt_price: u128,
    
    /// Cumulative tick value since pool initialization
    pub tick_cumulative: i64,
    
    /// Cumulative seconds per liquidity value (for liquidity-weighted calculations)
    pub seconds_per_liquidity_cumulative: u128,
    
    /// Whether this observation slot has been initialized
    pub initialized: bool,
}

/// Represents a single compressed price observation
#[derive(Debug, Default, Copy, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct CompressedObservation {
    /// Time delta from previous observation (seconds)
    /// Using u16 saves space and allows for up to ~18 hours between observations
    pub time_delta: u16,
    
    /// Compressed sqrt price (uses a delta from a base price and reduced precision)
    /// Delta from the prior observation or from a base value for the first observation
    pub sqrt_price_delta: i64,
    
    /// Compressed tick accumulator delta
    pub tick_accumulator_delta: i64,
    
    /// Seconds per liquidity accumulator delta (compressed)
    pub seconds_per_liquidity_delta: u64,
    
    /// Flags for observation - includes validation bits and compression parameters
    pub flags: u8,
}

impl Observation {
    /// Create a new uninitialized observation
    pub fn new() -> Self {
        Observation {
            block_timestamp: 0,
            sqrt_price: 0,
            tick_cumulative: 0,
            seconds_per_liquidity_cumulative: 0,
            initialized: false,
        }
    }
}

impl Default for Observation {
    fn default() -> Self {
        Self::new()
    }
}

/// Oracle account structure for storing and managing observations
#[account]
pub struct Oracle {
    /// The pool this oracle belongs to
    pub pool: Pubkey,
    
    /// Array of price observations
    pub observations: Vec<Observation>,
    
    /// Current observation index (points to the most recent observation)
    pub observation_index: u16,
    
    /// Current number of initialized observations
    pub observation_cardinality: u16,
    
    /// Next cardinality to grow to (for gradual expansion)
    pub observation_cardinality_next: u16,
    
    /// Base timestamp for delta encoding (timestamp of first observation)
    pub base_timestamp: u32,
    
    /// Base sqrt price for delta encoding
    pub base_sqrt_price: u128,
    
    /// Base tick accumulator for delta encoding
    pub base_tick_cumulative: i64,
    
    /// Base seconds per liquidity accumulator for delta encoding
    pub base_seconds_per_liquidity_cumulative: u128,
    
    /// Array of compressed observations (used when storage optimization is enabled)
    pub compressed_observations: Vec<CompressedObservation>,
    
    /// Flag to indicate if compression is enabled
    pub compression_enabled: bool,
}

impl Oracle {
    /// Size of the oracle account with a given number of observations
    pub fn size(num_observations: usize) -> usize {
        8 +  // Anchor discriminator
        32 + // pool
        4 +  // observations vec length
        num_observations * std::mem::size_of::<Observation>() + 
        2 +  // observation_index
        2 +  // observation_cardinality
         2 +  // observation_cardinality_next
        4 +  // base_timestamp
        16 + // base_sqrt_price
        8 +  // base_tick_cumulative
        16 + // base_seconds_per_liquidity_cumulative
        4 +  // compressed_observations vec length
        num_observations * std::mem::size_of::<CompressedObservation>() +
        1    // compression_enabled
    }
    
    /// Initialize a new oracle with a single observation
    pub fn initialize(&mut self, pool: Pubkey, block_timestamp: u32, sqrt_price: u128) -> Result<()> {
        self.pool = pool;
        self.observation_index = 0;
        self.observation_cardinality = 1;
        self.observation_cardinality_next = 1;
        self.base_timestamp = block_timestamp;
        self.base_sqrt_price = sqrt_price;
        self.base_tick_cumulative = 0;
        self.base_seconds_per_liquidity_cumulative = 0;
        self.compression_enabled = false;
        
        // Initialize the first observation
        let observation = Observation {
            block_timestamp,
            sqrt_price,
            tick_cumulative: 0,
            seconds_per_liquidity_cumulative: 0,
            initialized: true,
        };
        
        // Ensure observations vec has capacity and push the first observation
        if self.observations.is_empty() {
            self.observations = vec![Observation::new(); 1];
        }
        self.observations[0] = observation;
        
        Ok(())
    }
    
    /// Write a new observation to the oracle
    pub fn write(&mut self, 
                block_timestamp: u32, 
                sqrt_price: u128, 
                tick: i32, 
                liquidity: u128) -> Result<()> {
        
        // Ensure we're only writing at increasing timestamps
        let last_observation = self.get_last_observation()?.clone();
        if block_timestamp <= last_observation.block_timestamp {
            return Err(ErrorCode::OracleInvalidTimestamp.into());
        }
        
        // Calculate seconds elapsed and tick delta
        let time_elapsed = block_timestamp.saturating_sub(last_observation.block_timestamp);
        let tick_cumulative = last_observation.tick_cumulative
            .checked_add((tick as i64).checked_mul(time_elapsed as i64).ok_or(ErrorCode::MathOverflow)?)
            .ok_or(ErrorCode::MathOverflow)?;
            
        // Calculate seconds per liquidity
        let seconds_per_liquidity_delta = if liquidity > 0 {
            // Scale time elapsed by 2^128 and divide by liquidity
            ((time_elapsed as u128).checked_shl(128).ok_or(ErrorCode::MathOverflow)?)
                .checked_div(liquidity).ok_or(ErrorCode::MathOverflow)?
        } else {
            0 // If no liquidity, increment is 0
        };
        
        let seconds_per_liquidity_cumulative = last_observation.seconds_per_liquidity_cumulative
            .checked_add(seconds_per_liquidity_delta)
            .ok_or(ErrorCode::MathOverflow)?;
            
        // Determine the index for the new observation
        let new_index = (self.observation_index + 1) % self.observation_cardinality;
        
        // Create the new observation
        let new_observation = Observation {
            block_timestamp,
            sqrt_price,
            tick_cumulative,
            seconds_per_liquidity_cumulative,
            initialized: true,
        };
        
        // Ensure we have enough capacity in the observations vec
        if new_index as usize >= self.observations.len() {
            // Grow observations if needed
            let mut new_observations = self.observations.clone();
            new_observations.resize(self.observation_cardinality as usize, Observation::new());
            self.observations = new_observations;
        }
        
        // Write the new observation
        self.observations[new_index as usize] = new_observation.clone();
        
        // If compression is enabled, also store a compressed version
        if self.compression_enabled {
            self.add_compressed_observation(&new_observation, &last_observation)?;
        }
        
        self.observation_index = new_index;
        
        Ok(())
    }
    
    /// Add a compressed observation
    fn add_compressed_observation(
        &mut self, 
        observation: &Observation,
        previous: &Observation
    ) -> Result<()> {
        // Ensure the compressed observations array is initialized
        if self.compressed_observations.is_empty() {
            self.compressed_observations = vec![CompressedObservation::default(); self.observation_cardinality as usize];
        }
        
        // Calculate time delta (seconds since last observation)
        let time_delta = observation.block_timestamp
            .checked_sub(previous.block_timestamp)
            .ok_or(ErrorCode::MathOverflow)?;
            
        if time_delta > u16::MAX as u32 {
            return Err(ErrorCode::ObservationTimeDeltaTooLarge.into());
        }
        
        // Calculate price delta from previous observation
        let sqrt_price_delta = if observation.sqrt_price >= previous.sqrt_price {
            observation
                .sqrt_price
                .checked_sub(previous.sqrt_price)
                .ok_or(ErrorCode::MathOverflow)? as i64
        } else {
            -(previous
                .sqrt_price
                .checked_sub(observation.sqrt_price)
                .ok_or(ErrorCode::MathOverflow)? as i64)
        };
        
        // Calculate tick accumulator delta
        let tick_acc_delta = if observation.tick_cumulative >= previous.tick_cumulative {
            observation
                .tick_cumulative
                .checked_sub(previous.tick_cumulative)
                .ok_or(ErrorCode::MathOverflow)?
        } else {
            -(previous
                .tick_cumulative
                .checked_sub(observation.tick_cumulative)
                .ok_or(ErrorCode::MathOverflow)?)
        };
        
        // Calculate seconds per liquidity delta
        let sec_per_liq_delta = observation
            .seconds_per_liquidity_cumulative
            .checked_sub(previous.seconds_per_liquidity_cumulative)
            .ok_or(ErrorCode::MathOverflow)? as u64;
        
        let compressed = CompressedObservation {
            time_delta: time_delta as u16,
            sqrt_price_delta,
            tick_accumulator_delta: tick_acc_delta,
            seconds_per_liquidity_delta: sec_per_liq_delta,
            flags: if observation.initialized { 1 } else { 0 },
        };
        
        // Store the compressed observation at the same index as the full observation
        self.compressed_observations[self.observation_index as usize] = compressed;
        
        Ok(())
    }
    
    /// Get the most recent observation
    pub fn get_last_observation(&self) -> Result<&Observation> {
        if self.observation_cardinality == 0 {
            return Err(ErrorCode::OracleNotInitialized.into());
        }
        
        let observation = &self.observations[self.observation_index as usize];
        if !observation.initialized {
            return Err(ErrorCode::OracleNotInitialized.into());
        }
        
        Ok(observation)
    }
    
    /// Calculate time-weighted average price (TWAP) over a specified time period
    pub fn calculate_twap(&self, 
                         target_timestamp: u32, 
                         seconds_ago: u32) -> Result<u128> {
        // Calculate the target timestamp to look back from
        let target = if seconds_ago == 0 {
            target_timestamp
        } else {
            target_timestamp.saturating_sub(seconds_ago)
        };
        
        // Find closest observations before and after target time
        let (observation_before, observation_after) = self.get_surrounding_observations(target)?;
        
        // If the timestamps are the same, return that observation's price
        if observation_before.block_timestamp == target {
            return Ok(observation_before.sqrt_price);
        }
        
        // Check if we have valid observations
        if !observation_before.initialized || !observation_after.initialized {
            return Err(ErrorCode::OracleInsufficientData.into());
        }
        
        // Calculate the weighted average using linear interpolation
        let _time_point = target.wrapping_sub(observation_before.block_timestamp);
        let time_delta = observation_after.block_timestamp.wrapping_sub(observation_before.block_timestamp);
        
        if time_delta == 0 {
            return Ok(observation_before.sqrt_price);
        }
        
        // Calculate tick cumulative delta
        let tick_cumulative_delta = observation_after.tick_cumulative.wrapping_sub(observation_before.tick_cumulative);
        
        // Calculate the average tick
        let avg_tick = ((tick_cumulative_delta as f64) / (time_delta as f64)).round() as i32;
        
        // Convert average tick to sqrt price
        tick_to_sqrt_price(avg_tick)
    }

    /// Enable compression for future observations
    pub fn enable_compression(&mut self) -> Result<()> {
        if self.compression_enabled {
            return Ok(());
        }
        
        self.compression_enabled = true;
        
        // Initialize compressed observations array
        if self.compressed_observations.is_empty() {
            self.compressed_observations = vec![CompressedObservation::default(); self.observation_cardinality as usize];
            
            // Compress existing observations
            for i in 0..self.observation_cardinality {
                if i == 0 {
                    // First observation is special since it uses base values directly
                    let observation = &self.observations[0];
                    if observation.initialized {
                        let compressed = CompressedObservation {
                            time_delta: 0,
                            sqrt_price_delta: 0,
                            tick_accumulator_delta: 0,
                            seconds_per_liquidity_delta: 0,
                            flags: 1,
                        };
                        self.compressed_observations[0] = compressed;
                    }
                } else {
                    let idx = i as usize;
                    let prev_idx = if idx == 0 { self.observation_cardinality as usize - 1 } else { idx - 1 };
                    
                    // Clone the observations to avoid borrowing self when we call add_compressed_observation
                    let observation_clone = self.observations[idx].clone();
                    let prev_observation_clone = self.observations[prev_idx].clone();
                    
                    if observation_clone.initialized && prev_observation_clone.initialized {
                        let _ = self.add_compressed_observation(&observation_clone, &prev_observation_clone);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Get observations that surround the target timestamp
    fn get_surrounding_observations(&self, target: u32) -> Result<(&Observation, &Observation)> {
        if self.observation_cardinality <= 1 {
            return Err(ErrorCode::OracleInsufficientData.into());
        }
        
        let mut oldest_index = self.observation_index;
        let mut oldest_timestamp = u32::MAX;
        let mut newest_index = self.observation_index;
        let mut newest_timestamp = 0;
        
        // Find oldest and newest observations
        for i in 0..self.observation_cardinality {
            let index = (self.observation_index + i) % self.observation_cardinality;
            let observation = &self.observations[index as usize];
            
            if observation.initialized {
                if observation.block_timestamp < oldest_timestamp {
                    oldest_timestamp = observation.block_timestamp;
                    oldest_index = index;
                }
                
                if observation.block_timestamp > newest_timestamp {
                    newest_timestamp = observation.block_timestamp;
                    newest_index = index;
                }
            }
        }
        
        // If target is older than our oldest observation, use the oldest two
        if target <= oldest_timestamp {
            // Find second oldest
            let mut second_oldest_index = oldest_index;
            let mut second_oldest_timestamp = u32::MAX;
            
            for i in 0..self.observation_cardinality {
                let index = (self.observation_index + i) % self.observation_cardinality;
                if index != oldest_index {
                    let observation = &self.observations[index as usize];
                    if observation.initialized && observation.block_timestamp < second_oldest_timestamp {
                        second_oldest_timestamp = observation.block_timestamp;
                        second_oldest_index = index;
                    }
                }
            }
            
            return Ok((&self.observations[oldest_index as usize], 
                       &self.observations[second_oldest_index as usize]));
        }
        
        // If target is newer than our newest observation, use the newest two
        if target >= newest_timestamp {
            // Find second newest
            let mut second_newest_index = newest_index;
            let mut second_newest_timestamp = 0;
            
            for i in 0..self.observation_cardinality {
                let index = (self.observation_index + i) % self.observation_cardinality;
                if index != newest_index {
                    let observation = &self.observations[index as usize];
                    if observation.initialized && observation.block_timestamp > second_newest_timestamp {
                        second_newest_timestamp = observation.block_timestamp;
                        second_newest_index = index;
                    }
                }
            }
            
            return Ok((&self.observations[second_newest_index as usize], 
                       &self.observations[newest_index as usize]));
        }
        
        // Target is between observations, find the observations that surround it
        let mut before_index = 0;
        let mut before_timestamp = 0;
        let mut after_index = 0;
        let mut after_timestamp = u32::MAX;
        
        for i in 0..self.observation_cardinality {
            let index = (self.observation_index + i) % self.observation_cardinality;
            let observation = &self.observations[index as usize];
            
            if observation.initialized {
                // If the observation is before target but newer than current "before"
                if observation.block_timestamp <= target && observation.block_timestamp > before_timestamp {
                    before_timestamp = observation.block_timestamp;
                    before_index = index;
                }
                
                // If the observation is after target but older than current "after"
                if observation.block_timestamp > target && observation.block_timestamp < after_timestamp {
                    after_timestamp = observation.block_timestamp;
                    after_index = index;
                }
            }
        }
        
        Ok((&self.observations[before_index as usize], 
            &self.observations[after_index as usize]))
    }
    
    /// Increase the cardinality of the oracle to support more observations
    pub fn increase_cardinality(&mut self, new_cardinality: u16) -> Result<()> {
        // Validate the new cardinality
        if new_cardinality <= self.observation_cardinality {
            return Ok(());  // No need to increase
        }
        
        if new_cardinality > MAX_ORACLE_OBSERVATIONS as u16 {
            return Err(ErrorCode::OracleCardinalityTooLarge.into());
        }
        
        // Update the target cardinality
        self.observation_cardinality_next = new_cardinality;
        
        Ok(())
    }
    
    /// Actually grow the observations array to the next cardinality
    pub fn grow_observations(&mut self) -> Result<()> {
        if self.observation_cardinality >= self.observation_cardinality_next {
            return Ok(());  // Already at target size
        }
        
        // Resize the observations array
        self.observations.resize(
            self.observation_cardinality_next as usize, 
            Observation::new()
        );
        
        // If compression is enabled, also resize the compressed observations array
        if self.compression_enabled {
            self.compressed_observations.resize(
                self.observation_cardinality_next as usize,
                CompressedObservation::default()
            );
        }
        
        // Update cardinality
        self.observation_cardinality = self.observation_cardinality_next;
        
        Ok(())
    }
    
    /// Calculate compression ratio (percentage of storage saved)
    pub fn calculate_compression_ratio(&self) -> f64 {
        if !self.compression_enabled || self.observation_cardinality == 0 {
            return 0.0;
        }
        
        let uncompressed_size = self.observation_cardinality as usize * std::mem::size_of::<Observation>();
        let compressed_size = self.observation_cardinality as usize * std::mem::size_of::<CompressedObservation>();
        
        if uncompressed_size == 0 {
            return 0.0;
        }
        
        let savings = uncompressed_size.saturating_sub(compressed_size) as f64;
        (savings / uncompressed_size as f64) * 100.0
    }
}

/// Helper function to convert tick index to sqrt price in Q64.96 format
fn tick_to_sqrt_price(tick: i32) -> Result<u128> {
    // Implementation should match the one in math.rs
    // For now, using a simplified version
    let tick_abs = tick.unsigned_abs();
    let mut sqrt_price = 1u128 << 96;  // Q64.96 representation of 1.0
    
    // Calculate 1.0001^tick
    for _i in 0..tick_abs {
        if tick < 0 {
            sqrt_price = sqrt_price * 9999 / 10000;
        } else {
            sqrt_price = sqrt_price * 10001 / 10000;
        }
    }
    
    Ok(sqrt_price)
}

/// Instruction context for initializing an oracle
#[derive(Accounts)]
pub struct InitializeOracle<'info> {
    /// The oracle account to initialize
    #[account(mut)]
    pub oracle: Account<'info, Oracle>,
    
    /// The pool this oracle belongs to
    pub pool: AccountInfo<'info>,
    
    /// The payer for rent
    #[account(mut)]
    pub payer: Signer<'info>,
    
    /// System program
    pub system_program: Program<'info, System>,
    
    /// Rent sysvar
    pub rent: Sysvar<'info, Rent>,
}

/// Instruction context for writing an observation
#[derive(Accounts)]
pub struct WriteObservation<'info> {
    /// The oracle account to update
    #[account(mut)]
    pub oracle: Account<'info, Oracle>,
    
    /// The pool this oracle belongs to
    pub pool: AccountInfo<'info>,
}

/// Instruction context for increasing oracle cardinality
#[derive(Accounts)]
pub struct IncreaseCardinality<'info> {
    /// The oracle account to update
    #[account(mut)]
    pub oracle: Account<'info, Oracle>,
    
    /// The payer for rent (if needed for increased space)
    #[account(mut)]
    pub payer: Signer<'info>,
    
    /// System program
    pub system_program: Program<'info, System>,
}

/// Instruction context for enabling compression
#[derive(Accounts)]
pub struct EnableCompression<'info> {
    /// The oracle account to update
    #[account(mut)]
    pub oracle: Account<'info, Oracle>,
    
    /// The payer for any additional rent needed
    #[account(mut)]
    pub payer: Signer<'info>,
    
    /// System program
    pub system_program: Program<'info, System>,
}