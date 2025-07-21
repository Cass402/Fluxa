//! PDA (Program Derived Address) management utilities for Fluxa protocol
//!
//! # Rationale
//! This module centralizes all PDA derivation logic to ensure deterministic, collision-resistant, and auditable address generation across the protocol.
//! By enforcing canonical token ordering, explicit domain separation, and type-safe interfaces, we prevent subtle bugs, replay attacks, and address collisions.
//! The design is optimized for both on-chain and client SDK usage, supporting robust authority isolation and minimizing the risk of mis-derivation or privilege escalation.

use crate::error::MathError::InvalidPriceRange;
use anchor_lang::prelude::*;

/// Represents a derived PDA and its bump seed.
///
/// # Why this struct?
/// Encapsulates both the address and bump, ensuring that all PDA operations are explicit and type-safe.
/// This prevents accidental loss of the bump (which is required for signing) and makes intent clear at call sites.
#[derive(Clone, Copy, Debug)]
pub struct PdaInfo {
    pub address: Pubkey,
    pub bump: u8,
}

/// Centralized PDA manager for all protocol address derivations.
///
/// # Design Intent
/// - All PDA derivations are funneled through this type to guarantee consistency and prevent duplication of seed logic.
/// - Each method encodes domain separation and canonicalization, reducing the risk of address collision or privilege confusion.
/// - By using explicit arguments and seed construction, we make all address derivations auditable and reproducible.
pub struct PdaManager;

impl PdaManager {
    /// Deterministically derives the core pool PDA for a token pair and fee tier.
    ///
    /// # Why this approach?
    /// - **Canonicalization:** Enforces a single, canonical order for token pairs, preventing duplicate pools and address collisions.
    /// - **Domain Separation:** Uses a unique seed prefix ("pool_core") and explicit fee tier bytes to ensure that each pool is uniquely identified by its parameters.
    /// - **Safety:** Returns an error if tokens are identical, preventing degenerate pools and price range ambiguity.
    pub fn pool_core(
        token_0: &Pubkey,
        token_1: &Pubkey,
        fee_tier: u32,
        program_id: &Pubkey,
    ) -> Result<PdaInfo> {
        // Canonical ordering is critical for preventing duplicate pools and ensuring address determinism.
        let (token_a, token_b) = Self::canonical_token_order(token_0, token_1)?;
        let fee_tier_bytes = fee_tier.to_le_bytes();
        let seeds = [
            b"pool_core",
            token_a.as_ref(),
            token_b.as_ref(),
            &fee_tier_bytes,
        ];
        let (address, bump) = Pubkey::find_program_address(&seeds, program_id);
        Ok(PdaInfo { address, bump })
    }

    /// Derives the core authority PDA for a pool core.
    ///
    /// # Why this approach?
    /// - **Authority Isolation:** Each pool core has a unique authority, preventing cross-pool privilege escalation.
    /// - **Domain Separation:** Uses a unique seed prefix to ensure no overlap with other PDAs.
    /// # Arguments
    /// * `pool_core` - The public key of the pool core.
    /// * `program_id` - The program ID that will be used to derive the PDA.
    /// # Returns
    /// * `Ok(PdaInfo)` - If the PDA is successfully derived and validated.
    pub fn core_authority(pool_core: &Pubkey, program_id: &Pubkey) -> Result<PdaInfo> {
        // Derive the PDA for the core authority using the pool core
        // This ensures that the core authority is uniquely identified by the pool core
        let seeds = [b"core_authority", pool_core.as_ref()];

        // Derive the PDA using the seeds and program ID
        let (address, bump) = Pubkey::find_program_address(&seeds, program_id);

        Ok(PdaInfo { address, bump })
    }

    /// Derives the multisig configuration PDA for a pool core.
    ///
    /// # Why this approach?
    /// - **Decentralized Control:** Each pool core can have its own multisig config, supporting flexible, pool-specific governance.
    /// - **Domain Separation:** Prevents accidental overlap with other authority or config PDAs.
    /// # Arguments
    /// * `pool_core` - The public key of the pool core.
    /// * `program_id` - The program ID that will be used to derive the PDA.
    /// # Returns
    /// * `Ok(PdaInfo)` - If the PDA is successfully derived and validated.
    pub fn multisig_config(pool_core: &Pubkey, program_id: &Pubkey) -> Result<PdaInfo> {
        // Derive the PDA for the multisig configuration using the pool core
        // This ensures that the multisig configuration is uniquely identified by the pool core
        let seeds = [b"multisig_config", pool_core.as_ref()];

        // Derive the PDA using the seeds and program ID
        let (address, bump) = Pubkey::find_program_address(&seeds, program_id);

        Ok(PdaInfo { address, bump })
    }

    /// Derives the emergency contacts PDA for a pool core.
    ///
    /// # Why this approach?
    /// - **Crisis Response:** Each pool core can have a dedicated set of emergency contacts, supporting rapid, pool-specific incident response.
    /// - **Domain Separation:** Ensures emergency contacts are isolated from other authority/config PDAs.
    /// # Arguments
    /// * `pool_core` - The public key of the pool core.
    /// * `program_id` - The program ID that will be used to derive the PDA.
    /// # Returns
    /// * `Ok(PdaInfo)` - If the PDA is successfully derived and validated.
    pub fn emergency_contacts(pool_core: &Pubkey, program_id: &Pubkey) -> Result<PdaInfo> {
        // Derive the PDA for emergency contacts using the pool core
        // This ensures that the emergency contacts are uniquely identified by the pool core
        let seeds = [b"emergency_contacts", pool_core.as_ref()];

        // Derive the PDA using the seeds and program ID
        let (address, bump) = Pubkey::find_program_address(&seeds, program_id);

        Ok(PdaInfo { address, bump })
    }

    /// Derives the PDA for an individual emergency contact within a pool core.
    ///
    /// # Why this approach?
    /// - **Fine-Grained Control:** Allows for per-contact management and revocation, rather than a monolithic list.
    /// - **Domain Separation:** Prevents address collision with other pool or contact PDAs.
    /// # Arguments
    /// * `pool_core` - The public key of the pool core.
    /// * `emergency_contact` - The public key of the emergency contact.
    /// * `program_id` - The program ID that will be used to derive the PDA.
    /// # Returns
    /// * `Ok(PdaInfo)` - If the PDA is successfully derived and validated.
    pub fn emergency_contact(
        pool_core: &Pubkey,
        emergency_contact: &Pubkey,
        program_id: &Pubkey,
    ) -> Result<PdaInfo> {
        // Derive the PDA for an individual emergency contact using the pool core and emergency contact public key
        // This ensures that each emergency contact is uniquely identified by the pool core and their public key
        let seeds = [
            b"emergency_contact",
            pool_core.as_ref(),
            emergency_contact.as_ref(),
        ];

        // Derive the PDA using the seeds and program ID
        let (address, bump) = Pubkey::find_program_address(&seeds, program_id);

        Ok(PdaInfo { address, bump })
    }

    /// Derives the timelock operation PDA for a pool core and operation ID.
    ///
    /// # Why this approach?
    /// - **Change Management:** Each operation is uniquely identified, supporting granular timelock enforcement and auditability.
    /// - **Replay Protection:** Operation ID is included to prevent replay or overwrite of timelock actions.
    /// # Arguments
    /// * `pool_core` - The public key of the pool core.
    /// * `operation_id` - The unique identifier for the timelock operation, represented as a u64.
    /// * `program_id` - The program ID that will be used to derive the PDA.
    /// # Returns
    /// * `Ok(PdaInfo)` - If the PDA is successfully derived and validated.
    pub fn timelock_operation(
        pool_core: &Pubkey,
        operation_id: u64,
        program_id: &Pubkey,
    ) -> Result<PdaInfo> {
        // Derive the PDA for a timelock operation using the pool core and operation ID
        // This ensures that each timelock operation is uniquely identified by the pool core and operation ID
        let operation_id_bytes = operation_id.to_le_bytes();
        let seeds = [
            b"timelock_operation",
            pool_core.as_ref(),
            &operation_id_bytes,
        ];

        // Derive the PDA using the seeds and program ID
        let (address, bump) = Pubkey::find_program_address(&seeds, program_id);

        Ok(PdaInfo { address, bump })
    }

    /// Derives the audit trail entry PDA for a pool core and audit index.
    ///
    /// # Why this approach?
    /// - **Forensic Traceability:** Each audit entry is uniquely addressable, supporting tamper-evident, append-only audit trails.
    /// - **Indexing:** Audit index ensures strict ordering and prevents accidental overwrites.
    /// # Arguments
    /// * `pool_core` - The public key of the pool core.
    /// * `audit_index` - The unique identifier for the audit trail entry, represented as a u64.
    /// * `program_id` - The program ID that will be used to derive the PDA.
    /// # Returns
    /// * `Ok(PdaInfo)` - If the PDA is successfully derived and validated.
    pub fn audit_trail_entry(
        pool_core: &Pubkey,
        audit_index: u64,
        program_id: &Pubkey,
    ) -> Result<PdaInfo> {
        // Derive the PDA for an audit trail entry using the pool core and audit index
        // This ensures that each audit trail entry is uniquely identified by the pool core and audit index
        let audit_index_bytes = audit_index.to_le_bytes();
        let seeds = [b"audit_trail_entry", pool_core.as_ref(), &audit_index_bytes];

        // Derive the PDA using the seeds and program ID
        let (address, bump) = Pubkey::find_program_address(&seeds, program_id);

        Ok(PdaInfo { address, bump })
    }

    /// Derives the audit trail head PDA for a pool core.
    ///
    /// # Why this approach?
    /// - **Efficient Lookups:** Provides a single, canonical reference to the latest audit entry, supporting efficient append and verification.
    /// - **Domain Separation:** Prevents collision with audit entry PDAs.
    /// # Arguments
    /// * `pool_core` - The public key of the pool core.
    /// * `program_id` - The program ID that will be used to derive the PDA.
    /// # Returns
    /// * `Ok(PdaInfo)` - If the PDA is successfully derived and validated.
    pub fn audit_trail_head(pool_core: &Pubkey, program_id: &Pubkey) -> Result<PdaInfo> {
        // Derive the PDA for the audit trail head using the pool core
        // This ensures that the audit trail head is uniquely identified by the pool core
        let seeds = [b"audit_trail_head", pool_core.as_ref()];

        // Derive the PDA using the seeds and program ID
        let (address, bump) = Pubkey::find_program_address(&seeds, program_id);

        Ok(PdaInfo { address, bump })
    }

    /// Derives the position PDA for a pool core, owner, and position ID.
    ///
    /// # Why this approach?
    /// - **User Isolation:** Each position is uniquely tied to both the pool and the owner, preventing cross-user or cross-pool confusion.
    /// - **Indexing:** Position ID ensures that multiple positions per user are uniquely addressable.
    /// # Arguments
    /// * `pool_core` - The public key of the pool core.
    /// * `owner` - The public key of the owner of the position.
    /// * `position_id` - The unique identifier for the position, represented as a u64.
    /// * `program_id` - The program ID that will be used to derive the PDA.
    /// # Returns
    /// * `Ok(PdaInfo)` - If the PDA is successfully derived and validated.
    pub fn position(
        pool_core: &Pubkey,
        owner: &Pubkey,
        position_id: u64,
        program_id: &Pubkey,
    ) -> Result<PdaInfo> {
        // Derive the PDA for a position using the pool core, owner, and position ID
        // This ensures that each position is uniquely identified by its owner and ID
        let position_id_bytes = position_id.to_le_bytes();
        let seeds = [
            b"position",
            pool_core.as_ref(),
            owner.as_ref(),
            &position_id_bytes,
        ];

        // Derive the PDA using the seeds and program ID
        let (address, bump) = Pubkey::find_program_address(&seeds, program_id);

        Ok(PdaInfo { address, bump })
    }

    /// Derives the position batch PDA for a pool core, owner, and batch ID.
    ///
    /// # Why this approach?
    /// - **Batch Operations:** Enables efficient, atomic operations on groups of positions, supporting advanced DeFi use cases.
    /// - **User Isolation:** Batch is tied to both pool and owner, preventing privilege confusion.
    /// # Arguments
    /// * `pool_core` - The public key of the pool core.
    /// * `owner` - The public key of the owner of the position batch.
    /// * `batch_id` - The unique identifier for the position batch, represented as a u64.
    /// * `program_id` - The program ID that will be used to derive the PDA.
    /// # Returns
    /// * `Ok(PdaInfo)` - If the PDA is successfully derived and validated.
    pub fn position_batch(
        pool_core: &Pubkey,
        owner: &Pubkey,
        batch_id: u64,
        program_id: &Pubkey,
    ) -> Result<PdaInfo> {
        // Derive the PDA for a position batch using the pool core, owner, and batch ID
        // This ensures that each position batch is uniquely identified by its owner and ID
        let batch_id_bytes = batch_id.to_le_bytes();
        let seeds = [
            b"position_batch",
            pool_core.as_ref(),
            owner.as_ref(),
            &batch_id_bytes,
        ];

        // Derive the PDA using the seeds and program ID
        let (address, bump) = Pubkey::find_program_address(&seeds, program_id);

        Ok(PdaInfo { address, bump })
    }

    /// Derives the governance authority PDA for a pool core and governance realm.
    ///
    /// # Why this approach?
    /// - **Governance Isolation:** Each pool core and governance realm pair has a unique authority, supporting flexible, multi-realm governance.
    /// - **Domain Separation:** Prevents overlap with other authority PDAs.
    /// # Arguments
    /// * `pool_core` - The public key of the pool core.
    /// * `governance_realm` - The public key of the governance realm.
    /// * `program_id` - The program ID that will be used to derive the PDA.
    /// # Returns
    /// * `Ok(PdaInfo)` - If the PDA is successfully derived and validated.
    pub fn governance_authority(
        pool_core: &Pubkey,
        governance_realm: &Pubkey,
        program_id: &Pubkey,
    ) -> Result<PdaInfo> {
        // Derive the PDA for governance authority using the pool core and governance realm
        // This ensures that the governance authority is uniquely identified by the pool core and governance realm
        let seeds = [
            b"governance_authority",
            pool_core.as_ref(),
            governance_realm.as_ref(),
        ];

        // Derive the PDA using the seeds and program ID
        let (address, bump) = Pubkey::find_program_address(&seeds, program_id);

        Ok(PdaInfo { address, bump })
    }

    /// Returns the canonical (lexicographic) order of two tokens.
    ///
    /// # Why this approach?
    /// - **Determinism:** Ensures that all address derivations for a token pair are order-independent, preventing duplicate pools and address collisions.
    /// - **Safety:** Returns an error if tokens are identical, preventing degenerate pools and ambiguous price ranges.
    /// # Arguments
    /// * `token_0` - The first token's public key.
    /// * `token_1` - The second token's public key.
    /// # Returns
    /// * `Ok((Pubkey, Pubkey))` - A tuple containing the two tokens in canonical order.
    /// * `Err(MathError)` - If the tokens are the same, indicating an invalid price range.
    fn canonical_token_order<'a>(
        token_0: &'a Pubkey,
        token_1: &'a Pubkey,
    ) -> Result<(&'a Pubkey, &'a Pubkey)> {
        // Reject identical tokens to prevent degenerate pools and ambiguous price ranges.
        if token_0 == token_1 {
            return Err(InvalidPriceRange.into());
        }
        // Lexicographic ordering is used for determinism and to prevent duplicate pools.
        if token_0 < token_1 {
            Ok((token_0, token_1))
        } else {
            Ok((token_1, token_0))
        }
    }
}

/// Enumerates protocol security domains for authority isolation.
///
/// # Why this enum?
/// - **Fine-Grained Access Control:** Each domain represents a distinct area of protocol responsibility, supporting least-privilege and separation of duties.
/// - **Auditability:** Explicit domain tagging makes privilege boundaries clear for future maintainers and auditors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub enum SecurityDomain {
    ProtocolAdmin,
    EmergencyResponse,
    GovernanceExecution,
    MultisigOperation,
    TreasuryManagement,
}

/// Implementation of the SecurityDomain enum
impl SecurityDomain {
    /// Serializes the security domain to a single byte.
    ///
    /// # Why this approach?
    /// - **Storage Efficiency:** Compact representation for on-chain storage and cross-program communication.
    /// - **Protocol Safety:** Each domain is mapped to a unique byte, preventing ambiguity in privilege checks.
    pub fn to_bytes(&self) -> [u8; 1] {
        match self {
            SecurityDomain::ProtocolAdmin => [0],
            SecurityDomain::EmergencyResponse => [1],
            SecurityDomain::GovernanceExecution => [2],
            SecurityDomain::MultisigOperation => [3],
            SecurityDomain::TreasuryManagement => [4],
        }
    }
}
