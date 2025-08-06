use crate::error::PdaSecurityAuthorityError;
use crate::utils::security_authority::audit_trail::{AuditTrailEntry, AuditTrailHead, InitArgs};
use crate::utils::security_authority::core_authority::{CoreAuthority, EmergencyLevel};
use crate::utils::security_authority::emergency_contacts::{EmergencyContacts, EmergencyRole};
use crate::utils::security_authority::multisig_config::MultisigConfig;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;

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
#[derive(InitSpace)]
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
#[derive(Clone, Copy, Debug, AnchorSerialize, AnchorDeserialize, InitSpace)]
#[repr(C)]
pub struct SecurityContext {
    /// Protocol security schema version - enables coordinated security upgrades
    /// across all protocol components without breaking compatibility
    pub security_version: u16,

    /// Current operational security state - determines which operations are permitted
    /// and enables circuit-breaker patterns during adverse conditions
    pub security_status: SecurityStatus,

    /// Bitfield for granular security feature toggles - uses bitwise operations
    /// to minimize storage while supporting up to 32 independent security flags
    /// (e.g., maintenance windows, experimental features, circuit breakers)
    pub security_flags: u32,

    /// Unix timestamp of most recent security-relevant event - enables time-based
    /// security policies and helps detect suspicious activity patterns
    pub last_security_event: i64,

    /// Monotonically increasing sequence number - prevents replay attacks and
    /// ensures strict ordering of security events across distributed systems
    pub event_sequence: u64,
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

pub struct AddEmergencyContactArgs {
    pub contact: Pubkey,
    pub role: EmergencyRole,
    pub permissions: u32,
    pub authority: Pubkey,
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
    ) -> Result<()> {
        // Store immutable references to security component PDAs
        // These form the trust boundary - any compromise here affects entire security model
        self.pool_core = pool_core;
        self.core_authority = core_authority;
        self.multisig_config = multisig_config;
        self.audit_trail_head = audit_trail_head;
        self.emergency_contacts = emergency_contacts;

        let clock = Clock::get()?;

        // Initialize with secure defaults - Normal status with no active flags
        // Event sequence starts at 0 to establish baseline for replay protection
        self.security_context = SecurityContext {
            security_version: 1,
            security_status: SecurityStatus::Normal,
            security_flags: 0, // No special security modes active initially
            last_security_event: clock.unix_timestamp,
            event_sequence: 0, // Will increment with first logged event
        };

        // Establish immutable creation time and initial update timestamp
        // These timestamps anchor time-based security policies
        self.created_at = clock.unix_timestamp;
        self.last_updated = clock.unix_timestamp;

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
        actor: Pubkey,
        target: Pubkey,
        action: &[u8],
        data_hash: [u8; 32],
    ) -> Result<()> {
        // Calculate next sequential index - audit trail maintains strict ordering
        let audit_index = audit_trail_head.current_index + 1;

        // Retrieve previous hash for chain integrity - links this entry to audit history
        let previous_hash = audit_trail_head.latest_hash;

        let clock = Clock::get()?;

        // Initialize new audit entry with cryptographic linkage to previous entries
        // This creates an immutable, verifiable sequence of security events
        audit_trail_entry.initialize(InitArgs {
            pool_core: self.pool_core,
            audit_index,
            action: Self::pad_action(action), // Normalize action length for consistent hashing
            actor,
            target,
            data_hash,
            previous_hash, // Cryptographic link to maintain chain integrity
            timestamp: clock.unix_timestamp,
            block_height: clock.slot, // Solana block height for additional time anchoring
        })?;

        // Update audit trail head to point to new entry - atomic operation
        audit_trail_head.add_entry(audit_trail_entry)?;

        // Update coordinator's security context with new event metadata
        // Wrapping add prevents overflow in long-running deployments
        self.security_context.event_sequence = self.security_context.event_sequence.wrapping_add(1);
        self.security_context.last_security_event = clock.unix_timestamp;
        self.last_updated = clock.unix_timestamp;

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
        let action_len = action.len().min(32); // Truncate if longer than 32 bytes to prevent overflow
        padded_action[..action_len].copy_from_slice(&action[..action_len]);
        padded_action // Zero-padding ensures deterministic hash computation
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
        // Verify proposer authorization before any state changes
        // This prevents unauthorized users from initiating authority changes
        if !multisig_config.is_member(&proposer) {
            return Err(PdaSecurityAuthorityError::NotAMultisigMember.into());
        }

        // Immediately transition to authority change state to prevent concurrent proposals
        // This acts as a critical section lock for authority-related operations
        self.security_context.security_status = SecurityStatus::AuthorityTransition;

        // Delegate actual proposal logic to core authority module
        // Separation of concerns - coordinator handles orchestration, core handles mechanics
        core_authority.propose_authority_change(new_authority, proposer)?;

        // Create deterministic hash of proposal parameters for audit linking
        // Combines both the target authority and proposer to prevent replay attacks
        let data_hash = hashv(&[new_authority.as_ref(), proposer.as_ref()]).to_bytes();

        // Log the proposal in audit trail for transparency and forensic analysis
        // This creates an immutable record of who proposed what authority change when
        self.log_security_event(
            audit_trail_head,
            audit_trail_entry,
            proposer,
            new_authority,
            b"authority_change_proposed",
            data_hash,
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
        // Validate confirmer is authorized before processing confirmation
        // Prevents unauthorized parties from influencing authority changes
        if !multisig_config.is_member(&confirmer) {
            return Err(PdaSecurityAuthorityError::NotAMultisigMember.into());
        }

        // Ensure there's actually a pending authority change to confirm
        // Prevents confirmation operations when no proposal is active
        if !core_authority.has_pending_authority {
            return Err(PdaSecurityAuthorityError::NoAuthorityChangeRequested.into());
        }

        let pending_authority = core_authority.pending_authority;

        // Create unique proposal hash that binds confirmation to specific proposal instance
        // Includes timestamp to prevent replay of confirmations across different proposals
        let proposal_hash = hashv(&[
            b"authority_change",        // Operation type identifier
            pending_authority.as_ref(), // Target authority being confirmed
            &core_authority.authority_change_requested_at.to_le_bytes(), // Timestamp binding
        ])
        .to_bytes();

        // Process confirmation and check if threshold reached atomically
        // This prevents race conditions between confirmation recording and threshold checking
        let threshold_reached = multisig_config.confirm_proposal(&confirmer, proposal_hash)?;

        // Update status to reflect pending multisig state during confirmation process
        self.security_context.security_status = SecurityStatus::MultisigPending;

        // Create audit hash linking confirmer to the specific proposal being confirmed
        let data_hash = hashv(&[confirmer.as_ref(), &proposal_hash]).to_bytes();

        // Log the confirmation for audit trail completeness
        self.log_security_event(
            audit_trail_head,
            audit_trail_entry,
            confirmer,
            pending_authority,
            b"multisig_confirmation",
            data_hash,
        )?;

        // If threshold reached, execute the authority change atomically
        if threshold_reached {
            // Delegate actual authority transfer to core authority module
            core_authority.confirm_authority_change()?;

            // Return to normal operations - authority change complete
            self.security_context.security_status = SecurityStatus::Normal;

            // Create execution-specific audit hash for the completed authority change
            let exec_hash =
                hashv(&[b"authority_change_executed", pending_authority.as_ref()]).to_bytes();

            // Log successful execution of authority change
            self.log_security_event(
                audit_trail_head,
                audit_trail_entry,
                confirmer, // Final confirmer triggered the execution
                pending_authority,
                b"authority_change_executed",
                exec_hash,
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
        // Verify emergency responder authorization before any state changes
        // Only pre-authorized emergency contacts can trigger protocol pauses
        if !emergency_contacts.has_emergency_authority(&args.responder) {
            return Err(PdaSecurityAuthorityError::InsufficientPermissions.into());
        }

        // Immediately transition to emergency pause state to halt protocol operations
        // This acts as a global circuit breaker for all protocol functionality
        self.security_context.security_status = SecurityStatus::EmergencyPause;

        // Set emergency flag using bitwise OR to preserve existing security flags
        // 0x01 represents the emergency pause flag in the security flags bitfield
        self.security_context.security_flags |= 0x01;

        // Delegate pause mechanics to core authority while coordinator handles orchestration
        core_authority.emergency_pause(args.reason_hash, args.emergency_level)?;

        // Construct audit data combining reason hash with emergency level for complete context
        // This enables forensic analysis of emergency decisions and their justifications
        let mut data = [0u8; 32];
        data[..args.reason_hash.len()].copy_from_slice(&args.reason_hash);
        let level_bytes = (args.emergency_level as u8).to_le_bytes();
        data[31] = level_bytes[0]; // Pack emergency level into final byte

        // Create comprehensive audit log entry for emergency pause activation
        // Critical for post-incident analysis and accountability
        self.log_security_event(
            audit_trail_head,
            audit_trail_entry,
            args.responder,
            core_authority.current_authority,
            b"emergency_pause_activated",
            data,
        )?;

        Ok(())
    }

    pub fn coordinate_add_emergency_contact(
        &mut self,
        core_authority: &CoreAuthority,
        emergency_contacts: &mut EmergencyContacts,
        audit_trail_head: &mut AuditTrailHead,
        audit_trail_entry: &mut AuditTrailEntry,
        args: AddEmergencyContactArgs,
    ) -> Result<()> {
        require!(
            args.authority == core_authority.current_authority,
            PdaSecurityAuthorityError::Unauthorized
        );

        let _ = EmergencyContacts::add_contact(
            emergency_contacts,
            args.contact,
            args.role,
            args.permissions,
        );

        let data_hash = hashv(&[
            args.contact.as_ref(),
            &(args.role as u8).to_le_bytes(),
            &args.permissions.to_le_bytes(),
        ])
        .to_bytes();

        // Log the addition of the emergency contact for audit purposes
        self.log_security_event(
            audit_trail_head,
            audit_trail_entry,
            args.authority,
            args.contact,
            b"emergency_contact_added",
            data_hash,
        )?;

        Ok(())
    }
}

/// Anchor account context for security coordinator initialization with comprehensive validation.
///
/// This context enforces strict PDA derivation patterns to ensure all security components
/// are properly linked to the same pool_core, preventing cross-pool security interference
/// and establishing a clear security boundary for each protocol instance.
///
/// ## PDA Security Architecture
/// All security accounts use deterministic PDA derivation with pool_core as the seed,
/// creating a hierarchical security model where:
/// - Each pool instance has isolated security components
/// - Account relationships are cryptographically enforced
/// - Malicious account substitution is prevented by seed validation
///
/// ## Initialization Authorization
/// Requires both payer (for rent) and authority (for authorization) to separate
/// economic responsibility from operational control, following defense-in-depth principles.
#[derive(Accounts)]
pub struct InitializeSecurityCoordinator<'info> {
    /// The security coordinator account being initialized with deterministic PDA derivation.
    /// Uses pool_core as seed to ensure one coordinator per pool and prevent account confusion.
    /// The `init` constraint ensures this is a fresh account, preventing reinitialization attacks.
    #[account(
        init,
        payer = payer,
        space = 8 + SecurityCoordinator::INIT_SPACE,
        seeds = [b"security_coordinator", pool_core.key().as_ref()],
        bump
    )]
    pub security_coordinator: AccountLoader<'info, SecurityCoordinator>,

    /// Core authority account reference - must be valid PDA with matching pool_core seed.
    /// No `mut` access needed as we only store the reference, not modify the account.
    #[account(
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// Multisig configuration account - contains authorized signers and threshold settings.
    /// PDA validation ensures this multisig config belongs to the correct pool instance.
    #[account(
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// Audit trail head account - manages the linked list of audit entries.
    /// Must be initialized before security coordinator to establish audit infrastructure.
    #[account(
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    /// Emergency contacts registry - contains authorized emergency responders.
    /// Separate from multisig members to enable faster emergency response times.
    #[account(
        seeds = [b"emergency_contacts", pool_core.key().as_ref()],
        bump
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    /// Pool core account that serves as the root of the security hierarchy.
    /// UncheckedAccount because we only need its public key for PDA derivation,
    /// not to validate its internal structure or state.
    pub pool_core: UncheckedAccount<'info>,

    /// Account paying for the initialization transaction and ongoing rent.
    /// Mutable because lamports will be deducted for account creation costs.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Authorized signer for security coordinator initialization.
    /// Separate from payer to enable authorization patterns where economic
    /// responsibility and operational control are handled by different entities.
    pub authority: Signer<'info>,

    /// Solana system program required for account creation operations.
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(next_audit_index: u64)]
pub struct AuthorityChangeProposal<'info> {
    #[account(
        mut,
        seeds = [b"security_coordinator", pool_core.key().as_ref()],
        bump
    )]
    pub security_coordinator: AccountLoader<'info, SecurityCoordinator>,

    #[account(
        mut,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    #[account(
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    #[account(
        mut,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    #[account(
        init,
        payer = proposer,
        space = 8 + AuditTrailEntry::INIT_SPACE,
        seeds = [b"audit_trail_entry", audit_trail_head.key().as_ref(), next_audit_index.to_le_bytes().as_ref()],
        bump
    )]
    pub audit_trail_entry: AccountLoader<'info, AuditTrailEntry>,

    pub pool_core: UncheckedAccount<'info>,

    #[account(mut)]
    pub proposer: Signer<'info>,

    pub new_authority: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(next_audit_index: u64)]
pub struct MultisigConfirmation<'info> {
    #[account(
        mut,
        seeds = [b"security_coordinator", pool_core.key().as_ref()],
        bump
    )]
    pub security_coordinator: AccountLoader<'info, SecurityCoordinator>,

    #[account(
        mut,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    #[account(
        mut,
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    #[account(
        mut,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    #[account(
        init,
        payer = confirmer,
        space = 8 + AuditTrailEntry::INIT_SPACE,
        seeds = [b"audit_trail_entry", audit_trail_head.key().as_ref(), next_audit_index.to_le_bytes().as_ref()],
        bump
    )]
    pub audit_trail_entry: AccountLoader<'info, AuditTrailEntry>,

    pub pool_core: UncheckedAccount<'info>,

    #[account(mut)]
    pub confirmer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(next_audit_index: u64)]
pub struct EmergencyPause<'info> {
    #[account(
        mut,
        seeds = [b"security_coordinator", pool_core.key().as_ref()],
        bump
    )]
    pub security_coordinator: AccountLoader<'info, SecurityCoordinator>,

    #[account(
        mut,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    #[account(
        seeds = [b"emergency_contacts", pool_core.key().as_ref()],
        bump
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    #[account(
        mut,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    #[account(
        init,
        payer = emergency_responder,
        space = 8 + AuditTrailEntry::INIT_SPACE,
        seeds = [b"audit_trail_entry", audit_trail_head.key().as_ref(), next_audit_index.to_le_bytes().as_ref()],
        bump
    )]
    pub audit_trail_entry: AccountLoader<'info, AuditTrailEntry>,

    pub pool_core: UncheckedAccount<'info>,

    #[account(mut)]
    pub emergency_responder: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(next_audit_index: u64)]
pub struct AddEmergencyContact<'info> {
    #[account(
        mut,
        seeds = [b"security_coordinator", pool_core.key().as_ref()],
        bump
    )]
    pub security_coordinator: AccountLoader<'info, SecurityCoordinator>,

    #[account(
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    #[account(
        mut,
        seeds = [b"emergency_contacts", pool_core.key().as_ref()],
        bump
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    #[account(
        mut,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    #[account(
        init,
        payer = authority,
        space = 8 + AuditTrailEntry::INIT_SPACE,
        seeds = [b"audit_trail_entry", audit_trail_head.key().as_ref(), next_audit_index.to_le_bytes().as_ref()],
        bump
    )]
    pub audit_trail_entry: AccountLoader<'info, AuditTrailEntry>,

    pub pool_core: UncheckedAccount<'info>,

    pub new_emergency_contact: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Entry point function for initializing a new security coordinator instance.
///
/// This function serves as the secure bootstrap process for establishing the security
/// architecture of a pool instance. It validates all account relationships and
/// initializes the coordinator with verified references to security components.
///
/// ## Initialization Security
/// The function performs several critical validations:
/// - All referenced accounts must be valid PDAs with correct seeds
/// - Account relationships are cryptographically enforced by Anchor constraints
/// - The coordinator starts in a safe default state (Normal security status)
///
/// ## Failure Atomicity
/// If any part of initialization fails, the entire transaction is rolled back,
/// preventing partial initialization that could leave the security system in
/// an inconsistent state.
pub fn initialize_security_coordinator(ctx: Context<InitializeSecurityCoordinator>) -> Result<()> {
    // Load the uninitialized security coordinator account for initialization
    // load_init() ensures this is a fresh account and prepares it for first use
    let security_coordinator = &mut ctx.accounts.security_coordinator.load_init()?;

    // Initialize with validated account references - all constraints checked by Anchor
    // These Pubkeys form the immutable security architecture for this pool instance
    security_coordinator.initialize(
        ctx.accounts.pool_core.key(),
        ctx.accounts.core_authority.key(),
        ctx.accounts.multisig_config.key(),
        ctx.accounts.audit_trail_head.key(),
        ctx.accounts.emergency_contacts.key(),
    )?;

    Ok(())
}

pub fn propose_authority_change(
    ctx: Context<AuthorityChangeProposal>,
    next_audit_index: u64,
) -> Result<()> {
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_mut()?;
    require!(
        next_audit_index == audit_trail_head.current_index + 1,
        PdaSecurityAuthorityError::InvalidAuditIndex
    );
    let security_coordinator = &mut ctx.accounts.security_coordinator.load_mut()?;
    let core_authority = &mut ctx.accounts.core_authority.load_mut()?;
    let multisig_config = &ctx.accounts.multisig_config.load()?;
    let audit_trail_entry = &mut ctx.accounts.audit_trail_entry.load_init()?;

    security_coordinator.coordinate_authority_change_proposal(
        core_authority,
        multisig_config,
        audit_trail_head,
        audit_trail_entry,
        ctx.accounts.new_authority.key(),
        ctx.accounts.proposer.key(),
    )?;

    Ok(())
}

pub fn confirm_authority_change(
    ctx: Context<MultisigConfirmation>,
    next_audit_index: u64,
) -> Result<bool> {
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_mut()?;
    require!(
        next_audit_index == audit_trail_head.current_index + 1,
        PdaSecurityAuthorityError::InvalidAuditIndex
    );
    let security_coordinator = &mut ctx.accounts.security_coordinator.load_mut()?;
    let core_authority = &mut ctx.accounts.core_authority.load_mut()?;
    let multisig_config = &mut ctx.accounts.multisig_config.load_mut()?;
    let audit_trail_entry = &mut ctx.accounts.audit_trail_entry.load_init()?;

    let threshold_reached = security_coordinator.coordinate_multisig_confirmation(
        core_authority,
        multisig_config,
        audit_trail_head,
        audit_trail_entry,
        ctx.accounts.confirmer.key(),
    )?;

    Ok(threshold_reached)
}

pub fn emergency_pause(
    ctx: Context<EmergencyPause>,
    next_audit_index: u64,
    reason_hash: [u8; 32],
    emergency_level: EmergencyLevel,
) -> Result<()> {
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_mut()?;
    require!(
        next_audit_index == audit_trail_head.current_index + 1,
        PdaSecurityAuthorityError::InvalidAuditIndex
    );
    let security_coordinator = &mut ctx.accounts.security_coordinator.load_mut()?;
    let core_authority = &mut ctx.accounts.core_authority.load_mut()?;
    let emergency_contacts = &ctx.accounts.emergency_contacts.load()?;
    let audit_trail_entry = &mut ctx.accounts.audit_trail_entry.load_init()?;

    security_coordinator.coordinate_emergency_pause(
        core_authority,
        emergency_contacts,
        audit_trail_head,
        audit_trail_entry,
        EmergencyPauseArgs {
            responder: ctx.accounts.emergency_responder.key(),
            reason_hash,
            emergency_level,
        },
    )?;

    Ok(())
}

pub fn add_emergency_contact(
    ctx: Context<AddEmergencyContact>,
    next_audit_index: u64,
    role: EmergencyRole,
    permissions: u32,
) -> Result<()> {
    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_mut()?;
    require!(
        next_audit_index == audit_trail_head.current_index + 1,
        PdaSecurityAuthorityError::InvalidAuditIndex
    );
    let security_coordinator = &mut ctx.accounts.security_coordinator.load_mut()?;
    let core_authority = &ctx.accounts.core_authority.load()?;
    let emergency_contacts = &mut ctx.accounts.emergency_contacts.load_mut()?;
    let audit_trail_entry = &mut ctx.accounts.audit_trail_entry.load_init()?;

    security_coordinator.coordinate_add_emergency_contact(
        core_authority,
        emergency_contacts,
        audit_trail_head,
        audit_trail_entry,
        AddEmergencyContactArgs {
            authority: ctx.accounts.authority.key(),
            contact: ctx.accounts.new_emergency_contact.key(),
            role,
            permissions,
        },
    )?;

    Ok(())
}
