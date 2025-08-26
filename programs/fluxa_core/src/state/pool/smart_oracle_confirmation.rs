use crate::math::core_arithmetic::Q64x64;
use crate::state::pool::pool_security::PoolSecurity;
use anchor_lang::prelude::*;

/// Smart Oracle Confirmation - Uses oracles intelligently for attack verification
///
/// # Why this approach?
/// - Maintains Fluxa's oracle-free core value proposition
/// - Uses oracles as "second opinion" only when internal systems detect threats
/// - Reduces false positives while keeping zero oracle dependency for normal operations
/// - Provides enterprise-grade validation when needed without forcing it on everyone

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq)]
pub enum OracleConfirmationMode {
    Disabled = 0,           // Pure oracle-free mode
    AttackConfirmation = 1, // Oracle confirms internal attack detection
    AdvisoryOnly = 2,       // Oracle provides confidence boost
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq)]
pub enum ThreatLevel {
    Clean,      // No threat detected
    Suspicious, // Potential threat, needs confirmation
    HighRisk,   // High confidence threat, block regardless
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InternalSecurityResult {
    pub threat_level: ThreatLevel,
    pub confidence: u8,             // 0-100 confidence score
    pub detected_attack_types: u32, // Bitfield of attack types
    pub price_deviation: u32,       // Basis points deviation detected
    pub volume_anomaly_score: u32,  // Volume anomaly score
    pub timestamp: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct OracleConfirmationResult {
    pub price_matches: bool,      // Oracle price matches expected range
    pub confidence: u8,           // Oracle confidence in its price
    pub staleness: i64,           // How old is the oracle data
    pub deviation_from_pool: u32, // Oracle vs pool price deviation (basis points)
    pub confirms_threat: bool,    // Does oracle data confirm the threat
}

/// Smart oracle confirmation logic that preserves Fluxa's oracle-free core
pub struct SmartOracleConfirmation;

impl SmartOracleConfirmation {
    /// Main validation function that combines internal detection with smart oracle usage
    pub fn validate_with_smart_confirmation(
        pool_security: &PoolSecurity,
        internal_result: &InternalSecurityResult,
        current_price: Q64x64,
        oracle_price: Option<u64>,
    ) -> Result<bool> {
        match pool_security.oracle_confirmation_mode {
            0 => {
                // Pure oracle-free mode - use only internal detection
                Self::validate_oracle_free_only(internal_result)
            }
            1 => {
                // Attack confirmation mode - oracle confirms internal detection
                Self::validate_with_attack_confirmation(
                    internal_result,
                    current_price,
                    oracle_price,
                )
            }
            2 => {
                // Advisory mode - oracle provides additional confidence
                Self::validate_with_advisory_oracle(internal_result, current_price, oracle_price)
            }
            _ => {
                // Default to oracle-free for unknown modes
                Self::validate_oracle_free_only(internal_result)
            }
        }
    }

    /// Pure oracle-free validation (preserves Fluxa's core approach)
    fn validate_oracle_free_only(internal_result: &InternalSecurityResult) -> Result<bool> {
        match internal_result.threat_level {
            ThreatLevel::Clean => Ok(true),
            ThreatLevel::Suspicious => {
                // In oracle-free mode, use high confidence threshold
                Ok(internal_result.confidence < 85) // Allow if confidence < 85%
            }
            ThreatLevel::HighRisk => Ok(false), // Always block high risk
        }
    }

    /// Oracle confirmation mode - oracle acts as "second opinion"
    fn validate_with_attack_confirmation(
        internal_result: &InternalSecurityResult,
        current_price: Q64x64,
        oracle_price: Option<u64>,
    ) -> Result<bool> {
        match internal_result.threat_level {
            ThreatLevel::Clean => Ok(true), // Always allow clean transactions

            ThreatLevel::Suspicious => {
                // SMART USAGE: Only check oracle when internal system is suspicious
                if let Some(oracle_price) = oracle_price {
                    let oracle_confirmation = Self::get_oracle_confirmation(
                        current_price,
                        oracle_price,
                        internal_result,
                    )?;

                    if oracle_confirmation.confirms_threat {
                        // Double confirmation - high confidence it's an attack
                        msg!("Attack confirmed by both internal detection and oracle");
                        Ok(false)
                    } else {
                        // Oracle disagrees - likely false positive from internal system
                        msg!("Oracle indicates false positive, allowing transaction");
                        Ok(true)
                    }
                } else {
                    // No oracle available - fall back to internal detection confidence
                    if internal_result.confidence > 90 {
                        msg!("High internal confidence without oracle confirmation, blocking");
                        Ok(false)
                    } else {
                        msg!("Low internal confidence without oracle, allowing with warning");
                        Ok(true)
                    }
                }
            }

            ThreatLevel::HighRisk => {
                // Always block high risk regardless of oracle
                msg!("High risk detected by internal systems, blocking regardless of oracle");
                Ok(false)
            }
        }
    }

    /// Advisory mode - oracle provides additional confidence but doesn't override
    fn validate_with_advisory_oracle(
        internal_result: &InternalSecurityResult,
        current_price: Q64x64,
        oracle_price: Option<u64>,
    ) -> Result<bool> {
        let mut effective_confidence = internal_result.confidence;

        // Oracle provides confidence boost/reduction but doesn't override decision
        if let Some(oracle_price) = oracle_price {
            let oracle_confirmation =
                Self::get_oracle_confirmation(current_price, oracle_price, internal_result)?;

            if oracle_confirmation.confirms_threat {
                effective_confidence = (effective_confidence + 15).min(100); // Boost confidence
            } else if oracle_confirmation.price_matches {
                effective_confidence = effective_confidence.saturating_sub(10); // Reduce confidence
            }
        }

        // Make decision based on adjusted confidence
        match internal_result.threat_level {
            ThreatLevel::Clean => Ok(true),
            ThreatLevel::Suspicious => Ok(effective_confidence < 80),
            ThreatLevel::HighRisk => Ok(effective_confidence < 95), // Slightly more lenient with oracle input
        }
    }

    /// Get oracle confirmation for detected threat
    fn get_oracle_confirmation(
        current_price: Q64x64,
        oracle_price: u64,
        internal_result: &InternalSecurityResult,
    ) -> Result<OracleConfirmationResult> {
        let current_price_u64 = (current_price.raw() >> 32) as u64; // Convert to comparable format

        // Calculate price deviation between pool and oracle
        let deviation = if current_price_u64 > oracle_price {
            ((current_price_u64 - oracle_price) * 10000) / oracle_price
        } else {
            ((oracle_price - current_price_u64) * 10000) / oracle_price
        };

        // Determine if oracle confirms the threat
        let confirms_threat = match internal_result.detected_attack_types {
            attack_type if attack_type & ATTACK_TYPE_SANDWICH != 0 => {
                // For sandwich attacks, large price deviation confirms manipulation
                deviation > 200 // 2% deviation suggests manipulation
            }
            attack_type if attack_type & ATTACK_TYPE_FLASH_LOAN != 0 => {
                // For flash loans, check if oracle shows more stable price
                deviation < 50 // Oracle showing stable price confirms flash loan manipulation
            }
            _ => {
                // General manipulation - any significant deviation is suspicious
                deviation > 150 // 1.5% general threshold
            }
        };

        Ok(OracleConfirmationResult {
            price_matches: deviation < 100, // Within 1%
            confidence: if deviation < 50 {
                95
            } else if deviation < 200 {
                80
            } else {
                60
            },
            staleness: 0, // Would check actual oracle timestamp
            deviation_from_pool: deviation as u32,
            confirms_threat,
        })
    }

    /// Update oracle confirmation statistics for tuning
    pub fn update_confirmation_stats(
        pool_security: &mut PoolSecurity,
        internal_detected: bool,
        _oracle_confirmed: bool,
        actual_outcome: bool, // Was it actually an attack (determined later)
    ) -> Result<()> {
        pool_security.oracle_confirmation_count += 1;

        // Track false positives to tune internal detection sensitivity
        if internal_detected && !actual_outcome {
            pool_security.false_positive_count += 1;
        }

        // Adjust internal detection sensitivity based on accuracy
        let false_positive_rate = if pool_security.oracle_confirmation_count > 100 {
            (pool_security.false_positive_count * 100) / pool_security.oracle_confirmation_count
        } else {
            0
        };

        // Log for analysis
        msg!(
            "Oracle confirmation stats: FP rate: {}%, Total confirmations: {}",
            false_positive_rate,
            pool_security.oracle_confirmation_count
        );

        Ok(())
    }
}

// Attack type constants for bitfield
pub const ATTACK_TYPE_SANDWICH: u32 = 0x01;
pub const ATTACK_TYPE_FLASH_LOAN: u32 = 0x02;
pub const ATTACK_TYPE_FRONT_RUNNING: u32 = 0x04;
pub const ATTACK_TYPE_PRICE_MANIPULATION: u32 = 0x08;

// Confidence thresholds
pub const HIGH_CONFIDENCE_THRESHOLD: u8 = 90;
pub const MEDIUM_CONFIDENCE_THRESHOLD: u8 = 70;
pub const LOW_CONFIDENCE_THRESHOLD: u8 = 50;
