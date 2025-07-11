//! PDA (Program Derived Address) management utilities
//!
//! This module provides a clean, type-safe interface for PDA derivation.
//! This file is primarily for the client SDK so that it can derive PDAs

use crate::error::MathError::InvalidPriceRange;
use anchor_lang::prelude::*;

/// Information about a derived PDA.
#[derive(Clone, Copy, Debug)]
pub struct PdaInfo {
    pub address: Pubkey,
    pub bump: u8,
}

/// Centralized PDA manager that provides a clean interface for PDA operations and provides consistent patterns for PDA derivation and validation.
pub struct PdaManager;

impl PdaManager {
    /// Derives the core pool PDA for a given pair of tokens and fee tier.
    /// This function ensures that the tokens are in canonical order and that the fee tier is correctly applied.
    /// # Arguments
    /// * `token_0` - The first token's public key.
    /// * `token_1` - The second token's public key.
    /// * `fee_tier` - The fee tier for the pool, represented as a u32.
    /// * `program_id` - The program ID that will be used to derive the PDA.
    /// # Returns
    /// * `Ok(PdaInfo)` - If the PDA is successfully derived and validated.
    /// * `Err(MathError)` - If the tokens are the same, indicating an invalid price range.
    pub fn pool_core(
        token_0: &Pubkey,
        token_1: &Pubkey,
        fee_tier: u32,
        program_id: &Pubkey,
    ) -> Result<PdaInfo> {
        // Ensure that the tokens are in canonical order
        let (token_a, token_b) = Self::canonical_token_order(token_0, token_1)?;

        // Convert the fee tier to bytes and derive the PDA
        // This ensures that the same pair of tokens and fee tier always results in the same PDA
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

    /// Derives the core authority PDA for a given pool core.
    /// This function provides a way to uniquely identify the core authority for a specific pool core,
    /// ensuring that authority operations are securely managed.
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

    /// Derives the multisig configuration PDA for a given pool core.
    /// This function provides a way to uniquely identify the multisig configuration for a specific pool core,
    /// ensuring that multisig operations are securely managed. Multisig configurations are used to manage permissions and authority in a decentralized manner.
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

    /// Derives the emergency contacts PDA for a given pool core which is list of emergency contacts for the pool.
    /// This function provides a way to uniquely identify the emergency contacts for a specific pool core,
    /// ensuring that emergency contacts are securely managed. Emergency contacts are used to handle critical situations in the protocol.
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

    /// Derives the individual emergency contact PDA for a given pool core and emergency contact public key.
    /// This function provides a way to uniquely identify an emergency contact within the pool core,
    /// ensuring that each emergency contact is securely managed. Emergency contacts are used to handle critical situations
    /// in the protocol.
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

    /// Derives the timelock operation PDA for a given pool core and operation ID.
    /// This function provides a way to uniquely identify a timelock operation within the pool core,
    /// ensuring that each operation is securely managed. Timelock operations are used to delay the
    /// execution of certain actions in the protocol, providing a safety mechanism against immediate changes.
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

    /// Derives the audit trail entry PDA for a given pool core and audit index.
    /// This function provides a way to uniquely identify an audit trail entry within the pool core,
    /// ensuring that each entry is securely managed. Audit trail entries are used to track changes and
    /// operations performed on the pool, providing a transparent history of actions.
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

    /// Derives the audit trail head PDA for a given pool core.
    /// This function provides a way to uniquely identify the head of the audit trail for a specific pool core,
    /// ensuring that the audit trail is securely managed. The audit trail head is used to track the latest entry in the audit trail,
    /// providing a point of reference for all audit trail entries.
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

    /// Derives the position PDA for a given pool core, owner, and position ID.
    /// This function provides a way to uniquely identify positions within a pool,
    /// ensuring that each position is associated with a specific owner and ID.
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

    /// Derives the optimized batch - position batch PDA for a given pool core, owner, and batch ID.
    /// This function provides a way to uniquely identify position batches within a pool,
    /// ensuring that each batch is associated with a specific owner and ID.
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

    /// Derives the governance authority PDA for a given pool core and governance realm.
    /// This function provides a way to uniquely identify the governance authority for a specific pool core
    /// and governance realm, ensuring that governance operations are securely managed.
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

    /// Returns the canonical order of two tokens.
    /// This function ensures that the tokens are always returned in a consistent order,
    /// regardless of the order they are provided in. This is important for ensuring that
    /// the same pair of tokens always results in the same PDA, which is crucial for
    /// security and consistency in the protocol.
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
        // Ensure that the tokens are not the same
        if token_0 == token_1 {
            return Err(InvalidPriceRange.into());
        }

        // Return the tokens in a canonical order (lexicographically)
        if token_0 < token_1 {
            Ok((token_0, token_1))
        } else {
            Ok((token_1, token_0))
        }
    }
}

/// Security domain enumeration for authority isolation.
/// This enum defines the different security domains that can be used to isolate authority
/// and permissions within the Fluxa protocol. Each domain represents a specific area of
/// responsibility and control, allowing for fine-grained access control and security management.
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
    /// Converts the SecurityDomain to a byte representation.
    /// This method provides a way to serialize the security domain into a single byte,
    /// which can be useful for storage or transmission purposes.
    /// # Returns
    /// * `[u8; 1]` - A byte array representing the security domain.
    /// Each security domain is mapped to a unique byte value, allowing for efficient storage and comparison
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
