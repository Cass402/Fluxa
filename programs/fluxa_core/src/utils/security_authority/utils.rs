//! Utility functions for the Core Authority module.
//! This module provides helper functions for managing the Security Authority

/// Utility struct for the Audit functionality in the Security Authority module.
use crate::error::PdaSecurityAuthorityError;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;

pub struct AuditUtils;

/// Implementation of utility functions for the Audit functionality in the Security Authority module.
impl AuditUtils {
    /// Creates a unique audit hash based on the previous hash, action, data, timestamp, and audit index.
    /// This hash serves as a unique identifier for each audit entry, ensuring integrity and traceability.
    /// # Arguments
    /// * `previous_hash` - A reference to the previous audit hash (32 bytes).
    /// * `action` - A byte slice representing the action taken (e.g., "create", "update", "delete").
    /// * `data` - A byte slice containing the data associated with the action.
    /// * `timestamp` - The timestamp of the action in seconds since the Unix epoch.
    /// * `audit_index` - The index of the audit entry, used to maintain the order of audit entries.
    ///
    /// # Returns
    /// A 32-byte array representing the unique audit hash.
    pub fn create_audit_hash(
        previous_hash: &[u8; 32],
        action: &[u8],
        data: &[u8],
        timestamp: i64,
        audit_index: u64,
    ) -> [u8; 32] {
        hashv(&[
            previous_hash,
            action,
            data,
            &timestamp.to_le_bytes(),
            &audit_index.to_le_bytes(),
        ])
        .to_bytes()
    }

    /// Verifies the integrity of an audit chain by comparing the current hash with the expected hash.
    /// This function ensures that the audit trail has not been tampered with by checking if the
    /// current hash matches the expected hash derived from the previous hash, action, data, timestamp,
    /// and audit index.
    /// # Arguments
    /// * `current_hash` - A reference to the current audit hash (32 bytes).
    /// * `previous_hash` - A reference to the previous audit hash (32 bytes).
    /// * `action` - A byte slice representing the action taken (e.g., "create", "update", "delete").
    /// * `data` - A byte slice containing the  data associated with the action.
    /// * `timestamp` - The timestamp of the action in seconds since the Unix epoch.
    /// * `audit_index` - The index of the audit entry, used to maintain the order of audit entries.
    /// # Returns
    /// A `Result` indicating success or failure of the verification.
    pub fn verify_audit_chain(
        current_hash: &[u8; 32],
        previous_hash: &[u8; 32],
        action: &[u8],
        data: &[u8],
        timestamp: i64,
        audit_index: u64,
    ) -> Result<()> {
        // Calculate the expected hash based on the provided parameters
        let expected_hash =
            Self::create_audit_hash(previous_hash, action, data, timestamp, audit_index);
        // Compare the expected hash with the current hash
        // This ensures that the audit trail has not been tampered with.
        if *current_hash != expected_hash {
            return Err(PdaSecurityAuthorityError::AuditTrailVerificationFailed.into());
        }

        Ok(())
    }
}
