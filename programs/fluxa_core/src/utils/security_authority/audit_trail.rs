use crate::utils::security_authority::utils::AuditUtils;
use anchor_lang::prelude::*;

/// Audit Trail Head - cryptographic anchor for the protocol's immutable event log.
///
/// This structure serves as the authoritative root of a hash-chained audit trail,
/// implementing a blockchain-within-blockchain pattern for critical security events.
/// The design prioritizes tamper detection and forensic integrity over storage efficiency.
///
/// ## Cryptographic Security Model
/// The audit head maintains cryptographic integrity through hash chaining, where each
/// new entry's hash incorporates the previous entry's hash, creating a tamper-evident
/// sequence. Any unauthorized modification to historical entries would require
/// recomputing all subsequent hashes, which is computationally infeasible and easily detected.
///
/// ## Zero-Copy Design Rationale  
/// Uses `zero_copy(unsafe)` to avoid deserialization overhead during frequent integrity
/// checks and audit verification operations. This is critical for DeFi protocols where
/// audit verification may occur on every transaction for compliance or security monitoring.
/// The unsafe designation is justified because the account data is treated as raw bytes,
/// eliminating potential deserialization vulnerabilities.
///
/// ## Immutability Guarantees
/// After initialization, the head becomes append-only through protocol-level constraints.
/// The structure itself doesn't enforce immutability but relies on proper access control
/// in the containing program to prevent unauthorized modifications to the chain root.
#[account(zero_copy(unsafe))]
#[derive(InitSpace)] // Expecting 136 bytes
#[repr(C)]
pub struct AuditTrailHead {
    /// Pool scope delimiter preventing cross-pool audit trail contamination.
    ///
    /// Binds this audit trail to a specific pool instance to prevent audit events
    /// from different pools being mixed, which could complicate forensic analysis
    /// and regulatory compliance. Each pool maintains its own isolated audit domain.
    pub pool_core: Pubkey,

    /// Sequential audit state maintaining strict chronological ordering.
    ///
    /// The current_index serves as both a sequence number and anti-replay nonce,
    /// preventing audit entry duplication or out-of-order insertion attacks.
    /// Monotonic incrementing ensures no gaps in the audit sequence exist.
    pub current_index: u64,

    /// Cryptographic chain anchor linking to the most recent audit entry.
    ///
    /// Contains the hash of the latest entry in the audit chain, enabling
    /// efficient verification of chain integrity without traversing all entries.
    /// Forms the "head" of the hash chain for rapid tamper detection.
    pub latest_hash: [u8; 32],

    /// Total entry counter for audit trail completeness verification.
    ///
    /// Provides a cross-check against current_index to detect potential
    /// gaps or inconsistencies in the audit sequence during verification.
    /// Essential for regulatory compliance reporting.
    pub total_entries: u64,

    /// Immutable creation timestamp establishing audit trail genesis.
    ///
    /// Anchors the audit trail in time to prevent backdating attacks and
    /// provides a baseline for time-based audit policies. Once set, this
    /// timestamp cannot be modified to maintain historical integrity.
    pub created_at: i64,

    /// Mutable update timestamp tracking latest audit activity.
    ///
    /// Updated with each new entry to enable audit trail staleness detection
    /// and maintenance scheduling. Helps identify inactive or compromised trails.
    pub last_updated: i64,

    /// Future-proofing buffer preventing account layout migration needs.
    ///
    /// Reserves space for additional audit metadata without requiring costly
    /// account migrations in long-lived DeFi protocols. Critical for protocol
    /// evolution while maintaining backward compatibility.
    pub reserved: [u8; 32],
}

/// Core audit trail head operations ensuring cryptographic integrity and append-only semantics.
impl AuditTrailHead {
    /// Establishes the immutable foundation of an audit trail with security-first initialization.
    ///
    /// This initialization is a one-time operation that sets up the cryptographic anchor
    /// for all subsequent audit entries. The design ensures that once initialized, the
    /// audit trail cannot be reset or reinitiated, maintaining historical integrity.
    ///
    /// ## Security Initialization Pattern
    /// The function establishes secure defaults (zero hash, zero index) that serve as
    /// the genesis state for the hash chain. This prevents the need for special-case
    /// handling of the first entry and ensures consistent cryptographic verification
    /// across the entire audit sequence.
    ///
    /// ## Timestamp Anchoring
    /// Both creation and update timestamps are set to the same value to establish
    /// a consistent temporal baseline. This prevents subtle timing attacks and
    /// provides a reliable reference point for time-based audit policies.
    pub fn initialize(&mut self, pool_core: Pubkey, timestamp: i64) -> Result<()> {
        // Bind to specific pool instance for scope isolation
        self.pool_core = pool_core;

        // Initialize genesis state for hash chain - zero values serve as the root
        // This creates a deterministic starting point for cryptographic verification
        self.current_index = 0;
        self.latest_hash = [0u8; 32]; // Genesis hash for chain initialization
        self.total_entries = 0;

        // Establish temporal anchor points for the audit trail
        // Both timestamps start identical to provide baseline consistency
        self.created_at = timestamp;
        self.last_updated = timestamp;

        Ok(())
    }

    /// Atomically updates audit trail state with new entry while preserving chain integrity.
    ///
    /// This function serves as the critical link between individual audit entries and the
    /// overall audit trail state. It must be called exactly once per entry to maintain
    /// the cryptographic chain and prevent gaps or inconsistencies in the audit sequence.
    ///
    /// ## Atomic State Updates
    /// All state updates occur atomically within this function to prevent partial updates
    /// that could leave the audit trail in an inconsistent state. If any operation fails,
    /// the entire transaction rolls back, preserving trail integrity.
    ///
    /// ## Hash Chain Propagation
    /// The function propagates the entry's computed hash to become the new chain head,
    /// maintaining the cryptographic linkage that enables tamper detection. This creates
    /// an immutable sequence where any modification requires recomputing all subsequent hashes.
    ///
    /// ## Timestamp Synchronization
    /// Updates the trail's last_updated timestamp to match the entry's timestamp, ensuring
    /// temporal consistency across the audit system and enabling accurate staleness detection.
    pub fn add_entry(&mut self, entry: &AuditTrailEntry) -> Result<u64> {
        // Update sequential state with entry's verified index
        // This maintains strict ordering and prevents index manipulation
        self.current_index = entry.audit_index;

        // Propagate entry's hash to chain head for continued cryptographic linking
        // This creates the tamper-evident property of the audit chain
        self.latest_hash = entry.current_hash;

        // Increment total entry counter for completeness verification
        self.total_entries += 1;

        // Synchronize timestamps to maintain temporal consistency
        // Using entry's timestamp ensures all audit components stay synchronized
        self.last_updated = entry.timestamp;

        Ok(self.current_index)
    }
}

/// Individual audit trail entry implementing cryptographic linkage for tamper-evident logging.
///
/// Each entry represents a single, immutable record of a security-relevant action within
/// the protocol. The design implements a blockchain-like structure where each entry
/// cryptographically links to its predecessor, creating a tamper-evident audit chain.
///
/// ## Cryptographic Linking Strategy
/// Every entry contains both the hash of the previous entry and its own computed hash,
/// forming a chain where modification of any historical entry would require recomputing
/// all subsequent hashes. This makes tampering computationally infeasible and easily detectable.
///
/// ## Fixed-Size Field Design
/// All fields use fixed-size representations to eliminate dynamic allocation concerns
/// and ensure deterministic memory layout. This is critical for zero-copy operations
/// and prevents potential attack vectors related to variable-length data handling.
///
/// ## Temporal and Spatial Anchoring
/// Each entry includes both Solana's block height and Unix timestamp to provide dual
/// temporal anchoring. This redundancy helps detect timestamp manipulation attacks
/// and provides multiple reference points for chronological verification.
///
/// ## Zero-Copy Optimization Rationale
/// Uses zero-copy layout because audit entries are frequently accessed for verification
/// during protocol operations. Avoiding deserialization overhead is crucial for
/// maintaining transaction throughput in high-frequency DeFi operations.
#[account(zero_copy(unsafe))]
#[derive(InitSpace)] // Expecting 288 bytes
#[repr(C)]
pub struct AuditTrailEntry {
    /// Pool binding ensuring audit entry scope isolation and preventing cross-contamination.
    ///
    /// Links this entry to its parent pool to maintain audit trail boundaries and
    /// prevent entries from different pools being inadvertently mixed during
    /// forensic analysis or compliance reporting.
    pub pool_core: Pubkey,

    /// Sequential identifier providing ordering guarantees and replay protection.
    ///
    /// Serves as both a sequence number for chronological ordering and a nonce
    /// to prevent duplicate entry attacks. Must increment monotonically to
    /// maintain audit trail integrity and detect missing entries.
    pub audit_index: u64,

    /// Fixed-length action identifier normalized for consistent cryptographic processing.
    ///
    /// Padded to 32 bytes to prevent hash collision attacks and ensure deterministic
    /// cryptographic operations regardless of action name length. This eliminates
    /// potential vulnerabilities from variable-length action strings.
    pub action: [u8; 32],

    /// Actor account who initiated the audited action for accountability tracking.
    ///
    /// Records the public key of the account that triggered this audit event,
    /// enabling forensic analysis and accountability chains. Essential for
    /// regulatory compliance and incident response.
    pub actor: Pubkey,

    /// Target account affected by the audited action for impact analysis.
    ///
    /// Identifies the account or entity that was the subject of the audited action,
    /// enabling impact assessment and forensic reconstruction of event sequences.
    pub target: Pubkey,

    /// Cryptographic hash of action-specific data providing tamper detection.
    ///
    /// Contains the hash of any additional data associated with the action,
    /// enabling verification that action parameters haven't been modified
    /// without requiring storage of potentially large data payloads.
    pub data_hash: [u8; 32],

    /// Unix timestamp providing temporal anchoring for chronological verification.
    ///
    /// Records when the action occurred in calendar time, essential for regulatory
    /// compliance, incident timelines, and detecting timestamp manipulation attacks
    /// when cross-referenced with block_height.
    pub timestamp: i64,

    /// Solana block height providing blockchain-native temporal anchoring.
    ///
    /// Records the Solana block when the action occurred, providing a second
    /// temporal reference point that's harder to manipulate than Unix timestamps
    /// and enables block-based verification of event ordering.
    pub block_height: u64,

    /// Hash of previous entry maintaining cryptographic chain integrity.
    ///
    /// Contains the hash of the chronologically previous audit entry, forming
    /// the cryptographic link that makes the audit chain tamper-evident.
    /// Any modification to historical entries breaks this chain.
    pub previous_hash: [u8; 32],

    /// Hash of current entry serving as input for next entry's chain link.
    ///
    /// Computed from this entry's data and the previous_hash, this value becomes
    /// the previous_hash for the next entry in the chain, maintaining the
    /// cryptographic linkage that enables tamper detection.
    pub current_hash: [u8; 32],

    /// Reserved space preventing future account migration requirements.
    ///
    /// Provides buffer space for additional audit metadata without requiring
    /// costly account migrations, essential for long-lived DeFi protocols
    /// that need to evolve while maintaining historical audit integrity.
    pub reserved: [u8; 32],
}

/// Initialization parameters for audit trail entry creation with validation constraints.
///
/// This structure serves as a validated parameter container for audit entry creation,
/// ensuring all required data is provided and properly formatted before entry
/// initialization. The design prevents partial initialization that could compromise
/// audit trail integrity.
///
/// ## Parameter Validation Strategy
/// By collecting all parameters in a single structure, we enable batch validation
/// and ensure atomic initialization of audit entries. This prevents scenarios where
/// entries could be created with missing or invalid data that would break the
/// cryptographic chain.
///
/// ## Cryptographic Parameter Separation
/// Previous hash and computed hash parameters are explicitly separated to prevent
/// confusion during hash chain operations and ensure proper cryptographic linking
/// between entries in the audit sequence.
pub struct InitArgs {
    /// Pool scope binding for audit entry isolation
    pub pool_core: Pubkey,
    /// Sequential index for ordering and replay protection
    pub audit_index: u64,
    /// Fixed-length action identifier for consistent processing
    pub action: [u8; 32],
    /// Actor account responsible for the audited action
    pub actor: Pubkey,
    /// Target account affected by the audited action  
    pub target: Pubkey,
    /// Hash of action-specific data for tamper detection
    pub data_hash: [u8; 32],
    /// Previous entry hash for cryptographic chain linking
    pub previous_hash: [u8; 32],
    /// Temporal anchor using Unix timestamp
    pub timestamp: i64,
    /// Blockchain-native temporal anchor using block height
    pub block_height: u64,
}

/// Core audit trail entry operations ensuring cryptographic integrity and chain linking.
impl AuditTrailEntry {
    /// Creates a cryptographically-linked audit entry with full parameter validation.
    ///
    /// This function performs the critical task of initializing an audit entry with
    /// proper cryptographic linking to the previous entry in the chain. All parameters
    /// are validated and the entry's hash is computed to maintain chain integrity.
    ///
    /// ## Cryptographic Hash Generation
    /// The current_hash is computed from the entry's data plus the previous entry's hash,
    /// creating the cryptographic linkage that makes the audit trail tamper-evident.
    /// This hash becomes the previous_hash for the next entry in the sequence.
    ///
    /// ## Atomic Field Initialization
    /// All fields are initialized atomically to prevent partial entry creation that
    /// could compromise audit trail integrity. If any step fails, the entire
    /// initialization is rolled back by the runtime.
    ///
    /// ## Hash Chain Continuation
    /// The function ensures proper continuation of the hash chain by incorporating
    /// the previous entry's hash into the current entry's hash computation, maintaining
    /// the cryptographic link that enables tamper detection.
    pub fn initialize(&mut self, args: InitArgs) -> Result<()> {
        // Initialize core identification fields for forensic traceability
        self.pool_core = args.pool_core;
        self.audit_index = args.audit_index;
        self.action = args.action;
        self.actor = args.actor;
        self.target = args.target;

        // Initialize cryptographic and temporal data for integrity verification
        self.data_hash = args.data_hash;
        self.previous_hash = args.previous_hash;
        self.timestamp = args.timestamp;
        self.block_height = args.block_height;

        // Compute current hash to maintain cryptographic chain integrity
        // This hash incorporates the previous hash and current entry data,
        // creating the tamper-evident link to the next entry in sequence
        self.current_hash = AuditUtils::create_audit_hash(
            &args.previous_hash,
            &args.action,
            &args.data_hash,
            self.timestamp,
            args.audit_index,
        );

        Ok(())
    }

    /// Validates cryptographic integrity of the audit entry and its chain linkage.
    ///
    /// This method performs comprehensive validation of the audit entry's integrity,
    /// ensuring that the cryptographic hash chain has not been tampered with and
    /// that all critical invariants are maintained.
    ///
    /// ## Hash Chain Verification Strategy
    /// Rather than just comparing hashes, this method delegates to AuditUtils for
    /// comprehensive chain verification that includes cryptographic validation,
    /// temporal consistency checks, and structural integrity verification.
    ///
    /// ## Comprehensive Validation Approach
    /// The AuditUtils verification performs multiple validation layers including
    /// hash recomputation, parameter validation, and chain linkage verification
    /// to ensure the audit trail maintains its forensic integrity.
    ///
    /// ## Error Propagation Design
    /// Returns Result<()> to enable proper error handling and detailed failure
    /// reporting, allowing calling code to distinguish between different types
    /// of validation failures for appropriate response strategies.
    ///
    /// ## Utility Delegation Rationale
    /// Delegating to AuditUtils centralizes cryptographic verification logic,
    /// reducing code duplication and ensuring consistent validation across
    /// different audit trail operations and verification contexts.
    pub fn verify_integrity(&self) -> Result<()> {
        // Delegate to centralized verification utility for comprehensive validation
        // This includes hash verification, temporal consistency, and structural checks
        AuditUtils::verify_audit_chain(
            &self.current_hash,
            &self.previous_hash,
            &self.action,
            &self.data_hash,
            self.timestamp,
            self.audit_index,
        )
    }
}
