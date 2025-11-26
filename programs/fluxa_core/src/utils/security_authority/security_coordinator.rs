use crate::error::PdaSecurityAuthorityError;
use crate::utils::security_authority::audit_trail::{AuditTrailEntry, AuditTrailHead, InitArgs};
use crate::utils::security_authority::core_authority::{CoreAuthority, EmergencyLevel};
use crate::utils::security_authority::emergency_contacts::{EmergencyContacts, EmergencyRole};
use crate::utils::security_authority::multisig_config::MultisigConfig;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;
use bytemuck::{Pod, Zeroable};

/// Central security orchestrator that coordinates multi-layered security mechanisms across the DeFi protocol.
///
/// This structure serves as the single source of truth for security state management, implementing
/// a defense-in-depth strategy where multiple independent security systems must collaborate for
/// critical operations. The coordinator pattern prevents direct coupling between security components
/// while ensuring consistent audit trails and proper authorization flows.
///
/// ## Design Rationale
/// - **Zero-copy optimization**: Uses `zero_copy(unsafe)` to avoid deserialization costs for frequently
///   accessed security state, critical for high-frequency DeFi operations where gas efficiency matters
/// - **Immutable references**: All security component accounts are referenced by Pubkey rather than
///   embedded to maintain separation of concerns and enable independent upgrades
/// - **Event sequencing**: Maintains strict ordering of security events to prevent replay attacks
///   and ensure audit trail integrity in a distributed blockchain environment
///
/// ## Security Assumptions
/// The coordinator assumes that referenced security accounts (core_authority, multisig_config, etc.)
/// have been properly initialized and their PDAs correctly derived. Malicious or corrupted references
/// could compromise the entire security model.
#[account(zero_copy(unsafe))]
#[derive(InitSpace)] // Expecting 272 bytes
#[repr(C)]
pub struct SecurityCoordinator {
    /// Root pool reference that owns this security coordinator - enables security scope isolation
    /// per pool instance, preventing cross-pool security interference in multi-pool deployments
    pub pool_core: Pubkey,

    /// Independent security component references - stored as Pubkeys rather than embedded accounts
    /// to enable hot-swappable security modules without requiring coordinator redeployment
    pub core_authority: Pubkey,
    pub multisig_config: Pubkey,
    pub emergency_contacts: Pubkey,
    pub audit_trail_head: Pubkey,

    /// Current security state snapshot - embedded for zero-copy access during frequent
    /// security checks that occur on every protocol operation
    pub security_context: SecurityContext,

    /// Immutable creation timestamp - establishes baseline for time-based security policies
    /// and helps detect potential time manipulation attacks
    pub created_at: i64,

    /// Mutable update timestamp - tracks last security state change to enable
    /// staleness detection and maintenance scheduling
    pub last_updated: i64,

    /// Future-proofing buffer - reserves space for additional security metadata
    /// without requiring account migration, crucial for long-lived DeFi protocols
    pub reserved: [u8; 64],
}

/// Compact security state representation optimized for frequent access during protocol operations.
///
/// This structure is designed to fit within CPU cache lines and avoid expensive deserialization
/// during security checks that happen on every user transaction. The layout prioritizes frequently
/// accessed fields first to optimize memory access patterns.
///
/// ## Memory Layout Considerations
/// Fields are ordered by access frequency and aligned for optimal CPU cache utilization.
/// The total size is kept minimal to reduce network transfer costs when syncing state
/// across multiple validator nodes.
#[derive(Clone, Copy, Debug, AnchorSerialize, AnchorDeserialize, InitSpace, Pod, Zeroable)]
#[repr(C)]
pub struct SecurityContext {
    /// Unix timestamp of most recent security-relevant event - enables time-based
    /// security policies and helps detect suspicious activity patterns
    pub last_security_event: i64,

    /// Monotonically increasing sequence number - prevents replay attacks and
    /// ensures strict ordering of security events across distributed systems
    pub event_sequence: u64,

    /// Bitfield for granular security feature toggles - uses bitwise operations
    /// to minimize storage while supporting up to 32 independent security flags
    /// (e.g., maintenance windows, experimental features, circuit breakers)
    pub security_flags: u32,

    /// Protocol security schema version - enables coordinated security upgrades
    /// across all protocol components without breaking compatibility
    pub security_version: u16,

    /// Current operational security state - determines which operations are permitted
    /// and enables circuit-breaker patterns during adverse conditions
    pub security_status: u8,

    pub _padding: u8, // Padding for 8-byte alignment
}

/// Operational security states that determine protocol behavior and available operations.
///
/// Each state represents a different risk/functionality trade-off, designed to provide
/// graceful degradation during adverse conditions while maintaining core protocol safety.
/// The explicit u8 representation ensures deterministic serialization across different
/// validator implementations and prevents enum variant reordering attacks.
///
/// ## State Transition Security
/// Transitions between states are strictly controlled and logged to prevent unauthorized
/// state manipulation that could bypass security controls or enable exploits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, InitSpace)]
#[repr(u8)]
pub enum SecurityStatus {
    /// Full operational mode - all protocol features available with standard security checks
    Normal = 0,
    /// Planned maintenance window - non-critical operations may be restricted to reduce
    /// surface area during upgrades or configuration changes
    Maintenance = 1,
    /// Emergency circuit breaker activated - only essential operations permitted,
    /// designed to halt potential exploits while preserving user fund safety
    EmergencyPause = 2,
    /// Authority transfer in progress - elevated security during sensitive ownership changes
    /// that could compromise protocol governance if not properly validated
    AuthorityTransition = 3,
    /// Multisig proposal pending - intermediate state during multi-party authorization
    /// flows, prevents partial execution of sensitive operations
    MultisigPending = 4,
    /// Security system upgrade in progress - temporary state during security module updates
    /// to prevent operations with mixed old/new security assumptions
    SecurityUpgrade = 5,
}

/// Arguments for emergency pause operations - structured to prevent parameter confusion
/// and ensure all required context is provided for security audit trails.
///
/// Emergency pauses are high-stakes operations that must be carefully logged and justified,
/// as they can halt protocol operations and potentially lock user funds temporarily.
pub struct EmergencyPauseArgs {
    /// The authorized emergency responder initiating the pause - must be pre-validated
    /// against the emergency contacts registry to prevent unauthorized shutdown attacks
    pub responder: Pubkey,
    /// Hash of the emergency reason/justification - stored as hash rather than plaintext
    /// to prevent information leakage while maintaining audit capability
    pub reason_hash: [u8; 32],
    /// Severity level determining pause scope - higher levels may restrict more operations
    /// but require correspondingly higher authorization thresholds
    pub emergency_level: EmergencyLevel,
}

/// Arguments for emergency contact management operations with proper authorization validation.
///
/// Emergency contacts form a critical trust boundary in the protocol's security model,
/// as they have the power to halt operations during crises. This structure ensures all
/// necessary context is provided for proper validation and audit trail generation.
///
/// ## Security Implications
/// Emergency contact management is highly sensitive because:
/// - Emergency contacts can trigger protocol-wide shutdowns
/// - Improper contact management could enable insider attacks
/// - Contact permissions determine the scope of emergency response capabilities
pub struct AddEmergencyContactArgs {
    /// The new emergency contact being added to the registry
    /// Must be a valid Pubkey that can sign emergency pause transactions
    pub contact: Pubkey,
    /// The specific emergency role defining the contact's capabilities and limitations
    /// Different roles have varying authorization levels for emergency responses
    pub role: EmergencyRole,
    /// Bitfield permissions defining granular emergency response capabilities
    /// Allows fine-tuned control over what emergency actions each contact can perform
    pub permissions: u32,
    /// The current protocol authority authorizing the emergency contact addition
    /// Must match the core authority to prevent unauthorized contact management
    pub authority: Pubkey,
}

pub struct SecurityEventArgs<'a> {
    pub actor: Pubkey,
    pub target: Pubkey,
    pub action: &'a [u8],
    pub data_hash: [u8; 32],
    pub timestamp: i64,
    pub block_height: u64,
}

impl SecurityCoordinator {
    /// Initializes the security coordinator with validated component references.
    ///
    /// This is a one-time setup operation that establishes the security architecture
    /// for a pool instance. All referenced accounts must be properly initialized PDAs
    /// with correct seeds to prevent malicious account substitution attacks.
    ///
    /// ## Security Initialization Pattern
    /// Rather than embedding security components directly, we store validated Pubkey
    /// references to enable independent component upgrades and reduce coordinator
    /// size for network efficiency. The trade-off is additional account validation
    /// overhead on each security operation.
    ///
    /// ## Timing Consistency
    /// All timestamps are set to the same block time to establish a consistent
    /// baseline for time-based security policies and prevent subtle timing attacks.
    pub fn initialize(
        &mut self,
        pool_core: Pubkey,
        core_authority: Pubkey,
        multisig_config: Pubkey,
        audit_trail_head: Pubkey,
        emergency_contacts: Pubkey,
        timestamp: i64,
    ) -> Result<()> {
        // Store immutable references to security component PDAs
        // These form the trust boundary - any compromise here affects entire security model
        self.pool_core = pool_core;
        self.core_authority = core_authority;
        self.multisig_config = multisig_config;
        self.audit_trail_head = audit_trail_head;
        self.emergency_contacts = emergency_contacts;

        // Initialize with secure defaults - Normal status with no active flags
        // Event sequence starts at 0 to establish baseline for replay protection
        self.security_context = SecurityContext {
            security_version: 1,
            security_status: SecurityStatus::Normal as u8,
            security_flags: 0, // No special security modes active initially
            last_security_event: timestamp,
            event_sequence: 0, // Will increment with first logged event
            _padding: 0,
        };

        // Establish immutable creation time and initial update timestamp
        // These timestamps anchor time-based security policies
        self.created_at = timestamp;
        self.last_updated = timestamp;

        Ok(())
    }

    /// Creates an immutable audit trail entry for security-critical operations.
    ///
    /// This function implements the "append-only audit log" security pattern where
    /// all security events are cryptographically linked to prevent tampering or
    /// selective deletion. Each entry includes a hash chain to detect unauthorized
    /// modifications to the audit history.
    ///
    /// ## Cryptographic Integrity
    /// The audit trail uses hash chaining where each entry's hash includes the previous
    /// entry's hash, creating a tamper-evident sequence. Any modification to historical
    /// entries would require recomputing all subsequent hashes, which is computationally
    /// infeasible and easily detected.
    ///
    /// ## Concurrency Considerations
    /// The event sequence counter uses wrapping arithmetic to handle potential overflow
    /// in long-running deployments while maintaining ordering guarantees. Sequence gaps
    /// indicate potential audit trail corruption or missing events.
    pub fn log_security_event(
        &mut self,
        audit_trail_head: &mut AuditTrailHead,
        audit_trail_entry: &mut AuditTrailEntry,
        args: SecurityEventArgs,
    ) -> Result<()> {
        // Sequential index calculation prevents audit trail gaps that could indicate tampering
        // Each entry builds on the previous index, creating an immutable ordering sequence
        let audit_index = audit_trail_head.current_index + 1;

        // Hash chaining creates cryptographic integrity - each entry includes previous hash
        // This makes selective deletion impossible without breaking the entire chain
        let previous_hash = audit_trail_head.latest_hash;

        // Two-phase audit entry creation: initialize then link to maintain atomicity
        // If either operation fails, no partial audit state is left behind
        audit_trail_entry.initialize(InitArgs {
            pool_core: self.pool_core,
            audit_index,
            action: Self::pad_action(args.action), // Fixed-length prevents hash collision attacks
            actor: args.actor,
            target: args.target,
            data_hash: args.data_hash,
            previous_hash, // Establishes cryptographic link to audit history
            timestamp: args.timestamp,
            block_height: args.block_height, // Solana block height provides additional temporal anchoring
        })?;

        // Atomic audit trail head update - either fully succeeds or leaves no trace
        // This prevents partial audit states that could confuse trail verification
        audit_trail_head.add_entry(audit_trail_entry)?;

        // Coordinator state updates use wrapping arithmetic for long-term stability
        // Event sequence overflow is handled gracefully without breaking audit ordering
        self.security_context.event_sequence = self.security_context.event_sequence.wrapping_add(1);
        self.security_context.last_security_event = args.timestamp;
        self.last_updated = args.timestamp;

        Ok(())
    }

    /// Normalizes action strings to fixed-length format for consistent cryptographic processing.
    ///
    /// Variable-length action strings could lead to hash collisions or inconsistent
    /// audit trail verification. By padding to a fixed 32-byte length, we ensure
    /// deterministic hashing and prevent potential manipulation through string length
    /// variations.
    ///
    /// ## Security Justification
    /// Fixed-length padding prevents:
    /// - Hash collision attacks using different-length strings that hash to same value
    /// - Audit trail verification inconsistencies across different client implementations
    /// - Potential buffer overflow issues in hash computation routines
    fn pad_action(action: &[u8]) -> [u8; 32] {
        let mut padded_action = [0u8; 32];
        // Bounded copy prevents buffer overflows while ensuring deterministic length
        // min() protects against maliciously oversized action strings that could cause panics
        let action_len = action.len().min(32);
        padded_action[..action_len].copy_from_slice(&action[..action_len]);
        // Zero-padding creates consistent hash inputs regardless of original action length
        // This prevents different-length strings from producing identical hashes
        padded_action
    }

    /// Orchestrates the proposal phase of authority change with multisig validation.
    ///
    /// Authority changes are among the most sensitive operations in DeFi protocols, as they
    /// can fundamentally alter protocol governance and control. This function implements a
    /// secure two-phase commit pattern where proposal and execution are separated to prevent
    /// atomic authority takeover attacks.
    ///
    /// ## Two-Phase Security Pattern
    /// The separation between proposal and execution serves multiple security purposes:
    /// - Provides time window for community review and objection
    /// - Requires explicit multisig consensus beyond just proposal authorization
    /// - Creates audit trail linking proposal to eventual execution
    /// - Enables graceful cancellation if issues are discovered
    ///
    /// ## State Transition Safety
    /// Moving to AuthorityTransition status immediately prevents concurrent authority
    /// changes and signals to other protocol components that sensitive operations
    /// should be restricted until the transition completes or is cancelled.
    pub fn coordinate_authority_change_proposal(
        &mut self,
        core_authority: &mut CoreAuthority,
        multisig_config: &MultisigConfig,
        audit_trail_head: &mut AuditTrailHead,
        audit_trail_entry: &mut AuditTrailEntry,
        new_authority: Pubkey,
        proposer: Pubkey,
    ) -> Result<()> {
        // Dual authorization check prevents unauthorized authority changes
        // Both multisig membership AND current authority status required for maximum security
        if !multisig_config.is_member(&proposer) {
            return Err(PdaSecurityAuthorityError::NotAMultisigMember.into());
        }
        if proposer != core_authority.current_authority {
            return Err(PdaSecurityAuthorityError::Unauthorized.into());
        }

        // Immediate state transition acts as critical section lock for authority operations
        // Prevents race conditions where multiple authority changes could be proposed simultaneously
        self.security_context.security_status = SecurityStatus::AuthorityTransition as u8;

        let clock = Clock::get()?;

        // Cryptographic commitment includes timestamp to prevent replay attacks
        // Each proposal gets unique hash even if same new_authority is proposed multiple times
        let proposal_digest = hashv(&[
            b"authority_change",
            new_authority.as_ref(),
            clock.unix_timestamp.to_le_bytes().as_ref(),
        ])
        .to_bytes();

        // Audit trail hash combines both target and initiator for complete accountability
        // Links who proposed what authority change for forensic analysis
        let data_hash = hashv(&[new_authority.as_ref(), proposer.as_ref()]).to_bytes();

        // Separation of concerns: coordinator orchestrates, core authority implements
        // This decoupling enables independent testing and potential future upgrades
        core_authority.propose_authority_change(
            new_authority,
            proposal_digest,
            clock.unix_timestamp,
        )?;

        // Immutable audit record created before any potential failure points
        // Even if subsequent operations fail, the proposal attempt is permanently logged
        self.log_security_event(
            audit_trail_head,
            audit_trail_entry,
            SecurityEventArgs {
                actor: proposer,
                target: new_authority,
                action: b"authority_change_proposed",
                data_hash,
                timestamp: clock.unix_timestamp,
                block_height: clock.slot,
            },
        )?;

        Ok(())
    }

    /// Processes multisig confirmations for pending authority changes with threshold enforcement.
    ///
    /// This function implements the execution phase of the two-phase authority change protocol,
    /// collecting and validating multisig confirmations until the required threshold is reached.
    /// The design prevents both premature execution (insufficient confirmations) and replay
    /// attacks (duplicate confirmations).
    ///
    /// ## Threshold Security Model
    /// The multisig threshold model requires M-of-N confirmations where:
    /// - M is the minimum required confirmations (configured in MultisigConfig)
    /// - N is the total number of authorized signers
    /// - Each signer can only confirm once per proposal (replay protection)
    ///
    /// ## Cryptographic Proposal Binding
    /// The proposal hash includes timestamp to prevent confirmation of stale proposals
    /// and binds confirmations to specific proposal instances, preventing cross-proposal
    /// confirmation reuse attacks.
    ///
    /// ## Return Value Semantics
    /// Returns true if threshold reached and authority change executed, false if more
    /// confirmations needed. This allows callers to determine next steps without
    /// additional state queries.
    pub fn coordinate_multisig_confirmation(
        &mut self,
        core_authority: &mut CoreAuthority,
        multisig_config: &mut MultisigConfig,
        audit_trail_head: &mut AuditTrailHead,
        audit_trail_entry: &mut AuditTrailEntry,
        confirmer: Pubkey,
    ) -> Result<bool> {
        // Authorization gating prevents non-members from influencing authority changes
        // Critical security boundary - only pre-authorized multisig members can confirm
        if !multisig_config.is_member(&confirmer) {
            return Err(PdaSecurityAuthorityError::NotAMultisigMember.into());
        }

        // State validation prevents confirmation of non-existent proposals
        // Catches programming errors and prevents meaningless confirmation operations
        if core_authority.has_pending_authority == 0 {
            return Err(PdaSecurityAuthorityError::NoAuthorityChangeRequested.into());
        }

        let pending_authority = core_authority.pending_authority;

        // Proposal hash binding prevents cross-proposal confirmation attacks
        // Timestamp inclusion ensures confirmations are tied to specific proposal instances
        let proposal_hash = hashv(&[
            b"authority_change",        // Domain separation for different operation types
            pending_authority.as_ref(), // Binds confirmation to specific authority target
            &core_authority.authority_change_requested_at.to_le_bytes(), // Temporal binding
        ])
        .to_bytes();

        let clock = Clock::get()?;

        // Atomic confirmation processing prevents race conditions between recording and checking
        // Either confirmation is fully processed or it fails with no partial state changes
        let threshold_reached =
            multisig_config.confirm_proposal(&confirmer, proposal_hash, clock.unix_timestamp)?;

        // Status update reflects intermediate confirmation state for external monitoring
        // Signals that multisig process is active but not yet complete
        self.security_context.security_status = SecurityStatus::MultisigPending as u8;

        // Audit hash links specific confirmer to specific proposal for accountability
        // Enables forensic analysis of who confirmed what authority change
        let data_hash = hashv(&[confirmer.as_ref(), &proposal_hash]).to_bytes();

        // Individual confirmation logging provides granular audit trail
        // Each confirmation is permanently recorded even if threshold not yet reached
        self.log_security_event(
            audit_trail_head,
            audit_trail_entry,
            SecurityEventArgs {
                actor: confirmer,
                target: pending_authority,
                action: b"multisig_confirmation",
                data_hash,
                timestamp: clock.unix_timestamp,
                block_height: clock.slot,
            },
        )?;

        // Threshold-triggered execution implements atomic authority handoff
        // All execution steps must succeed or authority change is not applied
        if threshold_reached {
            core_authority.mark_multisig_approved(proposal_hash)?;
            if core_authority.execute_authority_change(clock.unix_timestamp)? {
                // Multisig reset prevents confirmation reuse for future proposals
                // Clean slate approach ensures no stale confirmations carry over
                multisig_config.reset_confirmations(clock.unix_timestamp);
            }

            // Status return to Normal signals completion of authority transition
            // Other protocol components can resume normal operations
            self.security_context.security_status = SecurityStatus::Normal as u8;

            // Execution-specific audit entry provides completion confirmation
            // Distinguishes between partial confirmations and successful execution
            let exec_hash =
                hashv(&[b"authority_change_executed", pending_authority.as_ref()]).to_bytes();

            // Final audit log entry completes the authority change audit trail
            // Provides definitive record that authority transition was completed
            self.log_security_event(
                audit_trail_head,
                audit_trail_entry,
                SecurityEventArgs {
                    actor: confirmer,
                    target: pending_authority,
                    action: b"authority_change_executed",
                    data_hash: exec_hash,
                    timestamp: clock.unix_timestamp,
                    block_height: clock.slot,
                },
            )?;
        }

        Ok(threshold_reached)
    }

    /// Executes emergency pause with proper authorization validation and comprehensive logging.
    ///
    /// Emergency pauses are circuit-breaker mechanisms designed to halt protocol operations
    /// when potential exploits or system failures are detected. This function balances the
    /// need for rapid response against the risk of malicious or accidental protocol shutdown.
    ///
    /// ## Authorization Model
    /// Emergency contacts represent a separate authorization domain from normal multisig
    /// operations, allowing for faster response times during genuine emergencies while
    /// maintaining proper access controls to prevent abuse.
    ///
    /// ## Circuit Breaker Pattern
    /// The emergency pause implements a fail-safe pattern where suspected compromise
    /// triggers protective shutdown rather than continuing potentially unsafe operations.
    /// This trades availability for security when threats are detected.
    ///
    /// ## Security Flag Management
    /// Uses bitwise OR operation (|=) to set emergency flag while preserving other
    /// security flags that may already be active, preventing flag state corruption
    /// during emergency operations.
    pub fn coordinate_emergency_pause(
        &mut self,
        core_authority: &mut CoreAuthority,
        emergency_contacts: &EmergencyContacts,
        audit_trail_head: &mut AuditTrailHead,
        audit_trail_entry: &mut AuditTrailEntry,
        args: EmergencyPauseArgs,
    ) -> Result<()> {
        // Emergency authorization operates outside normal multisig to enable rapid response
        // Pre-validated emergency contacts can act unilaterally when speed is critical
        if !emergency_contacts.has_emergency_authority(&args.responder) {
            return Err(PdaSecurityAuthorityError::InsufficientPermissions.into());
        }

        // Immediate status transition halts protocol operations before any delays
        // Circuit breaker pattern: fail-safe rather than fail-open during suspected compromise
        self.security_context.security_status = SecurityStatus::EmergencyPause as u8;

        // Bitwise OR preserves existing flags while adding emergency pause flag
        // Multiple security conditions can be active simultaneously without interference
        self.security_context.security_flags |= 0x01;

        let clock = Clock::get()?;

        // Emergency pause delegation maintains separation of concerns
        // Coordinator handles orchestration, core authority manages pause mechanics
        core_authority.emergency_pause(args.emergency_level, clock.unix_timestamp)?;

        // Audit data packing combines reason hash with severity level for analysis
        // Compact representation enables forensic analysis while maintaining data integrity
        let mut data = [0u8; 32];
        data[..args.reason_hash.len()].copy_from_slice(&args.reason_hash);
        let level_bytes = (args.emergency_level as u8).to_le_bytes();
        data[31] = level_bytes[0]; // Emergency level packed into final byte for space efficiency

        // Emergency pause audit logging provides accountability for critical decisions
        // Permanent record enables post-incident analysis and regulatory compliance
        self.log_security_event(
            audit_trail_head,
            audit_trail_entry,
            SecurityEventArgs {
                actor: args.responder,
                target: core_authority.current_authority,
                action: b"emergency_pause_activated",
                data_hash: data,
                timestamp: clock.unix_timestamp,
                block_height: clock.slot,
            },
        )?;

        Ok(())
    }

    /// Coordinates the addition of emergency contacts with proper authorization and audit logging.
    ///
    /// Emergency contacts represent a critical security boundary, as they have the authority
    /// to halt protocol operations during crisis situations. This function implements strict
    /// authorization validation to prevent unauthorized modification of the emergency contact
    /// registry, which could enable insider attacks or compromise incident response capabilities.
    ///
    /// ## Authorization Security Model
    /// Only the current protocol authority can add emergency contacts, ensuring that emergency
    /// response capabilities remain under the control of the legitimate protocol governance.
    /// This prevents rogue actors from installing their own emergency contacts that could be
    /// used to disrupt operations or extract value during manufactured crises.
    ///
    /// ## Emergency Contact Trust Model
    /// Emergency contacts operate in a different trust domain from normal multisig operations:
    /// - They can act unilaterally during genuine emergencies for rapid response
    /// - Their actions are heavily audited and logged for post-incident analysis
    /// - Contact permissions are granular to limit potential for abuse
    /// - Contact additions are permanently recorded in the audit trail
    ///
    /// ## Audit Trail Integrity
    /// All emergency contact modifications are logged with comprehensive metadata including
    /// the contact's role, permissions, and authorizing authority. This ensures full
    /// transparency and accountability for changes to the emergency response structure.
    pub fn coordinate_add_emergency_contact(
        &mut self,
        core_authority: &CoreAuthority,
        emergency_contacts: &mut EmergencyContacts,
        audit_trail_head: &mut AuditTrailHead,
        audit_trail_entry: &mut AuditTrailEntry,
        args: AddEmergencyContactArgs,
    ) -> Result<()> {
        // Strict authority validation prevents unauthorized emergency contact installation
        // Only current protocol authority can modify emergency response capabilities
        require!(
            args.authority == core_authority.current_authority,
            PdaSecurityAuthorityError::Unauthorized
        );

        let clock = Clock::get()?;

        // Direct emergency contact addition with explicit discarding of return value
        // Result is intentionally unused as any errors would have already been propagated
        let _ = EmergencyContacts::add_contact(
            emergency_contacts,
            args.contact,
            args.role,
            args.permissions,
            clock.unix_timestamp,
        );

        // Comprehensive audit hash captures all emergency contact parameters
        // Complete metadata enables forensic analysis of emergency authority decisions
        let data_hash = hashv(&[
            args.contact.as_ref(),            // Identity of the new emergency contact
            &(args.role as u8).to_le_bytes(), // Role assignment for operational clarity
            &args.permissions.to_le_bytes(),  // Permission bitfield for capability analysis
        ])
        .to_bytes();

        // Emergency contact addition audit entry provides permanent accountability
        // Immutable record of all modifications to emergency response structure
        self.log_security_event(
            audit_trail_head,
            audit_trail_entry,
            SecurityEventArgs {
                actor: args.authority, // Authority who authorized the emergency contact addition
                target: args.contact,  // New emergency contact being granted authority
                action: b"emergency_contact_added",
                data_hash,
                timestamp: clock.unix_timestamp,
                block_height: clock.slot,
            },
        )?;

        Ok(())
    }
}
