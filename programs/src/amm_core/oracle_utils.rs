/// Oracle Utilities Module
///
/// This module implements compression techniques for oracle observations to optimize
/// on-chain storage, as specified in Section 9.3 of the technical design document.
/// It uses delta encoding and other compression techniques to store price and time data efficiently.
use crate::errors::ErrorCode;
use anchor_lang::prelude::*;

/// Maximum number of observations that can be stored in a single oracle account
pub const MAX_OBSERVATIONS: usize = 64;

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

/// Full uncompressed observation data
#[derive(Debug, Clone)]
pub struct Observation {
    /// Absolute timestamp of the observation
    pub timestamp: i64,

    /// Full sqrt price in Q64.96 format
    pub sqrt_price: u128,

    /// Tick index accumulator
    pub tick_accumulator: i128,

    /// Seconds per liquidity accumulator
    pub seconds_per_liquidity: u128,

    /// Whether this observation is initialized
    pub initialized: bool,
}

/// Observation storage with compression
pub struct ObservationStorage {
    /// Base timestamp for delta encoding (timestamp of first observation)
    pub base_timestamp: i64,

    /// Base sqrt price for delta encoding
    pub base_sqrt_price: u128,

    /// Base tick accumulator for delta encoding
    pub base_tick_accumulator: i128,

    /// Base seconds per liquidity accumulator for delta encoding
    pub base_seconds_per_liquidity: u128,

    /// Array of compressed observations
    pub observations: [CompressedObservation; MAX_OBSERVATIONS],

    /// Number of observations currently stored
    pub observation_count: u8,

    /// Index for the most recent observation
    pub current_observation_index: u8,

    /// Cardinality (maximum number of observations to use)
    pub cardinality: u8,
}

impl Default for ObservationStorage {
    fn default() -> Self {
        Self {
            base_timestamp: 0,
            base_sqrt_price: 0,
            base_tick_accumulator: 0,
            base_seconds_per_liquidity: 0,
            observations: [CompressedObservation::default(); MAX_OBSERVATIONS],
            observation_count: 0,
            current_observation_index: 0,
            cardinality: 1,
        }
    }
}

impl ObservationStorage {
    /// Initialize the observation storage
    pub fn initialize(&mut self, cardinality: u8) -> Result<()> {
        if cardinality == 0 || cardinality > MAX_OBSERVATIONS as u8 {
            return Err(ErrorCode::InvalidObservationCardinality.into());
        }

        self.cardinality = cardinality;
        self.observation_count = 0;
        self.current_observation_index = 0;

        Ok(())
    }

    /// Write a new observation
    pub fn write(&mut self, observation: &Observation) -> Result<()> {
        // For the first observation, set the base values
        if self.observation_count == 0 {
            self.base_timestamp = observation.timestamp;
            self.base_sqrt_price = observation.sqrt_price;
            self.base_tick_accumulator = observation.tick_accumulator;
            self.base_seconds_per_liquidity = observation.seconds_per_liquidity;

            let compressed = self.compress_observation(observation, true)?;
            self.observations[0] = compressed;
            self.observation_count = 1;
            return Ok(());
        }

        // Calculate the next index
        let next_index = (self.current_observation_index as usize + 1) % self.cardinality as usize;
        let last_observation = self.get_observation(self.current_observation_index as usize)?;

        // Compress the observation relative to the previous one
        let compressed = self.compress_observation_from_previous(observation, &last_observation)?;

        // Store the compressed observation
        self.observations[next_index] = compressed;

        // Update indexes
        self.current_observation_index = next_index as u8;
        if self.observation_count < self.cardinality {
            self.observation_count += 1;
        }

        Ok(())
    }

    /// Get an observation by index
    pub fn get_observation(&self, index: usize) -> Result<Observation> {
        if index >= self.observation_count as usize {
            return Err(ErrorCode::ObservationIndexOutOfBounds.into());
        }

        // For the first observation, use the base values
        if index == 0 {
            let compressed = self.observations[0];
            return self.decompress_observation(&compressed, true);
        }

        // For subsequent observations, decompress using deltas
        let mut current = self.decompress_observation(&self.observations[0], true)?;

        // Apply deltas sequentially to reconstruct the observation at the given index
        for i in 1..=index {
            let compressed = self.observations[i];
            let next = self.decompress_observation_from_previous(&compressed, &current)?;
            current = next;
        }

        Ok(current)
    }

    /// Get the most recent observation
    pub fn get_latest_observation(&self) -> Result<Observation> {
        if self.observation_count == 0 {
            return Err(ErrorCode::NoObservations.into());
        }

        self.get_observation(self.current_observation_index as usize)
    }

    /// Compress an observation (first observation in the series)
    fn compress_observation(
        &self,
        observation: &Observation,
        is_first: bool,
    ) -> Result<CompressedObservation> {
        // For the first observation, store minimal information
        if is_first {
            let compressed = CompressedObservation {
                time_delta: 0,                  // First observation has no delta
                sqrt_price_delta: 0,            // Using base value
                tick_accumulator_delta: 0,      // Using base value
                seconds_per_liquidity_delta: 0, // Using base value
                flags: if observation.initialized { 1 } else { 0 },
            };
            return Ok(compressed);
        }

        // For non-first observations, calculate deltas from base values
        let time_delta = observation
            .timestamp
            .checked_sub(self.base_timestamp)
            .ok_or(ErrorCode::MathOverflow)?;

        if time_delta < 0 || time_delta > u16::MAX as i64 {
            return Err(ErrorCode::ObservationTimeDeltaTooLarge.into());
        }

        let sqrt_price_delta = observation
            .sqrt_price
            .checked_sub(self.base_sqrt_price)
            .ok_or(ErrorCode::MathOverflow)?;

        let tick_acc_delta = observation
            .tick_accumulator
            .checked_sub(self.base_tick_accumulator)
            .ok_or(ErrorCode::MathOverflow)?;

        let sec_per_liq_delta = observation
            .seconds_per_liquidity
            .checked_sub(self.base_seconds_per_liquidity)
            .ok_or(ErrorCode::MathOverflow)?;

        // Compress into smaller representation
        let compressed = CompressedObservation {
            time_delta: time_delta as u16,
            sqrt_price_delta: sqrt_price_delta as i64, // Reduced precision
            tick_accumulator_delta: tick_acc_delta as i64, // Reduced precision
            seconds_per_liquidity_delta: sec_per_liq_delta as u64, // Reduced precision
            flags: if observation.initialized { 1 } else { 0 },
        };

        Ok(compressed)
    }

    /// Compress an observation relative to the previous one
    fn compress_observation_from_previous(
        &self,
        observation: &Observation,
        previous: &Observation,
    ) -> Result<CompressedObservation> {
        // Calculate time delta (seconds since last observation)
        let time_delta = observation
            .timestamp
            .checked_sub(previous.timestamp)
            .ok_or(ErrorCode::MathOverflow)?;

        if time_delta < 0 || time_delta > u16::MAX as i64 {
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
        let tick_acc_delta = if observation.tick_accumulator >= previous.tick_accumulator {
            observation
                .tick_accumulator
                .checked_sub(previous.tick_accumulator)
                .ok_or(ErrorCode::MathOverflow)? as i64
        } else {
            -(previous
                .tick_accumulator
                .checked_sub(observation.tick_accumulator)
                .ok_or(ErrorCode::MathOverflow)? as i64)
        };

        // Calculate seconds per liquidity delta
        let sec_per_liq_delta = observation
            .seconds_per_liquidity
            .checked_sub(previous.seconds_per_liquidity)
            .ok_or(ErrorCode::MathOverflow)? as u64;

        let compressed = CompressedObservation {
            time_delta: time_delta as u16,
            sqrt_price_delta,
            tick_accumulator_delta: tick_acc_delta,
            seconds_per_liquidity_delta: sec_per_liq_delta,
            flags: if observation.initialized { 1 } else { 0 },
        };

        Ok(compressed)
    }

    /// Decompress an observation
    fn decompress_observation(
        &self,
        compressed: &CompressedObservation,
        is_first: bool,
    ) -> Result<Observation> {
        if is_first {
            let observation = Observation {
                timestamp: self.base_timestamp,
                sqrt_price: self.base_sqrt_price,
                tick_accumulator: self.base_tick_accumulator,
                seconds_per_liquidity: self.base_seconds_per_liquidity,
                initialized: compressed.flags & 1 != 0,
            };
            return Ok(observation);
        }

        // Calculate absolute values from deltas
        let timestamp = self
            .base_timestamp
            .checked_add(compressed.time_delta as i64)
            .ok_or(ErrorCode::MathOverflow)?;

        let sqrt_price = if compressed.sqrt_price_delta >= 0 {
            self.base_sqrt_price
                .checked_add(compressed.sqrt_price_delta as u128)
                .ok_or(ErrorCode::MathOverflow)?
        } else {
            self.base_sqrt_price
                .checked_sub((-compressed.sqrt_price_delta) as u128)
                .ok_or(ErrorCode::MathOverflow)?
        };

        let tick_accumulator = if compressed.tick_accumulator_delta >= 0 {
            self.base_tick_accumulator
                .checked_add(compressed.tick_accumulator_delta as i128)
                .ok_or(ErrorCode::MathOverflow)?
        } else {
            self.base_tick_accumulator
                .checked_sub((-compressed.tick_accumulator_delta) as i128)
                .ok_or(ErrorCode::MathOverflow)?
        };

        let seconds_per_liquidity = self
            .base_seconds_per_liquidity
            .checked_add(compressed.seconds_per_liquidity_delta as u128)
            .ok_or(ErrorCode::MathOverflow)?;

        let observation = Observation {
            timestamp,
            sqrt_price,
            tick_accumulator,
            seconds_per_liquidity,
            initialized: compressed.flags & 1 != 0,
        };

        Ok(observation)
    }

    /// Decompress an observation from the previous one
    fn decompress_observation_from_previous(
        &self,
        compressed: &CompressedObservation,
        previous: &Observation,
    ) -> Result<Observation> {
        // Calculate absolute timestamp
        let timestamp = previous
            .timestamp
            .checked_add(compressed.time_delta as i64)
            .ok_or(ErrorCode::MathOverflow)?;

        // Calculate absolute sqrt price
        let sqrt_price = if compressed.sqrt_price_delta >= 0 {
            previous
                .sqrt_price
                .checked_add(compressed.sqrt_price_delta as u128)
                .ok_or(ErrorCode::MathOverflow)?
        } else {
            previous
                .sqrt_price
                .checked_sub((-compressed.sqrt_price_delta) as u128)
                .ok_or(ErrorCode::MathOverflow)?
        };

        // Calculate absolute tick accumulator
        let tick_accumulator = if compressed.tick_accumulator_delta >= 0 {
            previous
                .tick_accumulator
                .checked_add(compressed.tick_accumulator_delta as i128)
                .ok_or(ErrorCode::MathOverflow)?
        } else {
            previous
                .tick_accumulator
                .checked_sub((-compressed.tick_accumulator_delta) as i128)
                .ok_or(ErrorCode::MathOverflow)?
        };

        // Calculate absolute seconds per liquidity
        let seconds_per_liquidity = previous
            .seconds_per_liquidity
            .checked_add(compressed.seconds_per_liquidity_delta as u128)
            .ok_or(ErrorCode::MathOverflow)?;

        let observation = Observation {
            timestamp,
            sqrt_price,
            tick_accumulator,
            seconds_per_liquidity,
            initialized: compressed.flags & 1 != 0,
        };

        Ok(observation)
    }

    /// Increase the observation cardinality (max number of observations)
    pub fn increase_cardinality(&mut self, new_cardinality: u8) -> Result<()> {
        if new_cardinality <= self.cardinality || new_cardinality > MAX_OBSERVATIONS as u8 {
            return Err(ErrorCode::InvalidObservationCardinality.into());
        }

        // Just update the cardinality - the array already has space allocated
        self.cardinality = new_cardinality;

        Ok(())
    }

    /// Get TWAP (Time Weighted Average Price) between two timestamps
    pub fn get_twap(&self, start_timestamp: i64, end_timestamp: i64) -> Result<u128> {
        if self.observation_count < 2 {
            return Err(ErrorCode::OracleInsufficientData.into());
        }

        if end_timestamp <= start_timestamp {
            return Err(ErrorCode::InvalidInput.into());
        }

        // Find the nearest observations
        let start_observation = self.get_observation_at_or_before_timestamp(start_timestamp)?;
        let end_observation = self.get_observation_at_or_before_timestamp(end_timestamp)?;

        // Calculate time-weighted average price
        let time_delta = end_timestamp
            .checked_sub(start_timestamp)
            .ok_or(ErrorCode::MathOverflow)?;

        if time_delta <= 0 {
            return Err(ErrorCode::InvalidInput.into());
        }

        let tick_delta = end_observation
            .tick_accumulator
            .checked_sub(start_observation.tick_accumulator)
            .ok_or(ErrorCode::MathOverflow)?;

        // Calculate average tick
        let avg_tick = (tick_delta as f64) / (time_delta as f64);

        // Convert average tick to sqrt price (this is a simplified conversion)
        // In a real implementation, we would use the proper tick -> sqrt_price formula
        let avg_sqrt_price = (1.0001f64.powf(avg_tick / 2.0) * (1u128 << 96) as f64) as u128;

        Ok(avg_sqrt_price)
    }

    /// Find the observation nearest to (but not after) the given timestamp
    fn get_observation_at_or_before_timestamp(&self, timestamp: i64) -> Result<Observation> {
        if self.observation_count == 0 {
            return Err(ErrorCode::NoObservations.into());
        }

        // Get the latest observation
        let latest = self.get_latest_observation()?;

        // If the requested timestamp is after our latest observation, return the latest
        if timestamp >= latest.timestamp {
            return Ok(latest);
        }

        // If we only have one observation and it's after the requested timestamp, we can't provide data
        if self.observation_count == 1 {
            return Err(ErrorCode::ObservationBoundaryError.into());
        }

        // Binary search through observations to find the closest one before or at timestamp
        let mut low = 0;
        let mut high = self.observation_count as usize - 1;

        while low <= high {
            let mid = (low + high) / 2;
            let mid_observation = self.get_observation(mid)?;

            match mid_observation.timestamp.cmp(&timestamp) {
                std::cmp::Ordering::Equal => {
                    return Ok(mid_observation);
                }
                std::cmp::Ordering::Less => {
                    // Check if next observation exceeds timestamp
                    if mid < self.observation_count as usize - 1 {
                        let next_observation = self.get_observation(mid + 1)?;
                        if next_observation.timestamp > timestamp {
                            return Ok(mid_observation);
                        }
                    }
                    low = mid + 1;
                }
                std::cmp::Ordering::Greater => {
                    // Since high is unsigned, make sure we don't underflow
                    if mid == 0 {
                        // If we're already at the first observation and it's too late,
                        // return an error as we have no valid observation before timestamp
                        return Err(ErrorCode::ObservationBoundaryError.into());
                    }
                    high = mid - 1;
                }
            }
        }

        // If the binary search completed without finding an exact match,
        // 'high' will be the index of the closest observation before timestamp
        self.get_observation(high)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialization() {
        let mut storage = ObservationStorage::default();
        assert!(storage.initialize(10).is_ok());
        assert_eq!(storage.cardinality, 10);
        assert_eq!(storage.observation_count, 0);
    }

    #[test]
    fn test_invalid_cardinality() {
        let mut storage = ObservationStorage::default();
        let result = storage.initialize(0);
        assert!(result.is_err());

        let result = storage.initialize(MAX_OBSERVATIONS as u8 + 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_write_and_read_observation() {
        let mut storage = ObservationStorage::default();
        storage.initialize(5).unwrap();

        let observation1 = Observation {
            timestamp: 1000,
            sqrt_price: 1 << 96, // 1.0 in Q64.96
            tick_accumulator: 0,
            seconds_per_liquidity: 0,
            initialized: true,
        };

        // Write first observation
        storage.write(&observation1).unwrap();
        assert_eq!(storage.observation_count, 1);

        // Read it back
        let read1 = storage.get_latest_observation().unwrap();
        assert_eq!(read1.timestamp, 1000);
        assert_eq!(read1.sqrt_price, 1 << 96);
        assert!(read1.initialized);

        // Write a second observation
        let observation2 = Observation {
            timestamp: 1060,                   // 60 seconds later
            sqrt_price: (1 << 96) + (1 << 94), // 1.25 in Q64.96
            tick_accumulator: 600,             // Some accumulation
            seconds_per_liquidity: 1 << 64,    // Some accumulation
            initialized: true,
        };

        storage.write(&observation2).unwrap();
        assert_eq!(storage.observation_count, 2);

        // Read latest (should be observation2)
        let read2 = storage.get_latest_observation().unwrap();
        assert_eq!(read2.timestamp, 1060);
        assert_eq!(read2.sqrt_price, (1 << 96) + (1 << 94));
        assert_eq!(read2.tick_accumulator, 600);

        // We should also be able to read the first observation
        let read1_again = storage.get_observation(0).unwrap();
        assert_eq!(read1_again.timestamp, 1000);
    }

    #[test]
    fn test_increase_cardinality() {
        let mut storage = ObservationStorage::default();
        storage.initialize(5).unwrap();

        // Should be able to increase cardinality
        assert!(storage.increase_cardinality(10).is_ok());
        assert_eq!(storage.cardinality, 10);

        // Can't decrease cardinality
        let result = storage.increase_cardinality(9);
        assert!(result.is_err());

        // Can't exceed max
        let result = storage.increase_cardinality(MAX_OBSERVATIONS as u8 + 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_twap_calculation() {
        let mut storage = ObservationStorage::default();
        storage.initialize(5).unwrap();

        // Need at least 2 observations for TWAP
        let result = storage.get_twap(1000, 2000);
        assert!(result.is_err());

        // Add the observations
        let observation1 = Observation {
            timestamp: 1000,
            sqrt_price: 1 << 96, // 1.0 in Q64.96
            tick_accumulator: 0,
            seconds_per_liquidity: 0,
            initialized: true,
        };

        let observation2 = Observation {
            timestamp: 2000,
            sqrt_price: (1 << 96) + (1 << 94), // 1.25 in Q64.96
            tick_accumulator: 10000,           // Some accumulation
            seconds_per_liquidity: 1 << 64,    // Some accumulation
            initialized: true,
        };

        storage.write(&observation1).unwrap();
        storage.write(&observation2).unwrap();

        // Now TWAP should work
        let twap = storage.get_twap(1000, 2000);
        assert!(twap.is_ok());
    }
}
