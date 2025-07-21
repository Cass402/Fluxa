//! Utility functions supporting the Security Authority module.
//!
//! # Rationale
//! This module provides deterministic, stateless helpers for cryptographic audit trails within the Security Authority.
//! The design ensures that all audit operations are verifiable, tamper-evident, and do not rely on mutable or external state.
//! By using hash chaining and explicit parameterization, we guarantee that every audit entry is uniquely and reproducibly identified, supporting robust protocol auditability and forensic analysis.

/// Utility struct for stateless audit operations in the Security Authority module.
///
/// # Design Intent
/// This struct is a namespace for pure functions that facilitate cryptographic audit trails.
/// It is intentionally non-instantiable and stateless, enforcing that all audit logic is deterministic and side-effect free.
use crate::error::PdaSecurityAuthorityError;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;

pub struct AuditUtils;

impl AuditUtils {
    /// Computes a deterministic, collision-resistant hash for an audit entry.
    ///
    /// # Why this approach?
    /// - **Chain Integrity:** By including the previous hash, we create a cryptographically linked chain of audit entries, making tampering with history computationally infeasible.
    /// - **Explicit Provenance:** All relevant context (action, data, timestamp, index) is included, so the hash uniquely identifies the entry and its position in the audit trail.
    /// - **No Dynamic Allocation:** All inputs are fixed-size or slices, avoiding heap allocation and ensuring predictable, on-chain-safe behavior.
    ///
    /// # Arguments
    /// * `previous_hash` - The hash of the previous audit entry, enforcing chain continuity.
    /// * `action` - Encodes the semantic intent (e.g., "create", "update").
    /// * `data` - The payload being audited; must be deterministic and canonicalized by the caller.
    /// * `timestamp` - Protocol time of the action; must be monotonic to prevent replay attacks.
    /// * `audit_index` - Enforces strict ordering and guards against reordering attacks.
    ///
    /// # Returns
    /// A 32-byte hash uniquely representing this audit entry in the chain.
    pub fn create_audit_hash(
        previous_hash: &[u8; 32],
        action: &[u8],
        data: &[u8],
        timestamp: i64,
        audit_index: u64,
    ) -> [u8; 32] {
        // Hash all fields together to ensure that any change in the audit trail is detectable.
        // This design prevents undetectable insertion, deletion, or modification of audit entries.
        hashv(&[
            previous_hash,
            action,
            data,
            &timestamp.to_le_bytes(),
            &audit_index.to_le_bytes(),
        ])
        .to_bytes()
    }

    /// Verifies that an audit entry is valid and untampered by recomputing its expected hash.
    ///
    /// # Why this approach?
    /// - **Tamper Evidence:** By requiring all original parameters, this function ensures that the audit chain is cryptographically sound and that no entry has been altered or replaced.
    /// - **Protocol Safety:** Returns a custom error if verification fails, supporting robust error handling and on-chain auditability.
    /// - **No Side Effects:** Pure function, so it is safe to call in any context (including simulation and off-chain verification).
    ///
    /// # Returns
    /// Ok(()) if the audit entry is valid; otherwise, returns a protocol error for audit trail compromise.
    pub fn verify_audit_chain(
        current_hash: &[u8; 32],
        previous_hash: &[u8; 32],
        action: &[u8],
        data: &[u8],
        timestamp: i64,
        audit_index: u64,
    ) -> Result<()> {
        // Recompute the expected hash for this entry using the same deterministic logic as creation.
        let expected_hash =
            Self::create_audit_hash(previous_hash, action, data, timestamp, audit_index);
        // If the hashes do not match, the audit trail has been tampered with or corrupted.
        if *current_hash != expected_hash {
            return Err(PdaSecurityAuthorityError::AuditTrailVerificationFailed.into());
        }

        Ok(())
    }
}
