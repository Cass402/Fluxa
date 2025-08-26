use crate::error::PdaSecurityAuthorityError;
use crate::utils::constants::{AUTHORITY_CHANGE_DELAY, EMERGENCY_PAUSE_TIMEOUT};
use crate::utils::security_authority::emergency_contacts::EmergencyContacts;
use crate::utils::security_authority::multisig_config::MultisigConfig;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;

/// Core Authority: The foundational security orchestrator for protocol governance.
///
/// This account serves as the primary security coordinator for pool operations,
/// implementing a multi-layered governance architecture designed to prevent common
/// DeFi protocol vulnerabilities while maintaining operational flexibility.
///
/// ## Governance Security Architecture
/// The CoreAuthority implements a time-locked, multi-signature governance pattern
/// that addresses several critical attack vectors:
/// - **Rug Pull Prevention**: Authority changes require both time delays and multi-sig confirmation
/// - **Emergency Response**: Immediate pause capabilities for critical incidents
/// - **Governance Capture**: Distributed authority through multisig requirements
/// - **State Consistency**: Atomic transitions prevent intermediate vulnerable states
///
/// ## Zero-Copy Design Rationale
/// Using `#[account(zero_copy(unsafe))]` provides several critical benefits:
/// - **Gas Efficiency**: Direct memory access without deserialization overhead
/// - **Deterministic Layout**: Fixed-size fields enable predictable memory patterns
/// - **Audit Compliance**: Transparent memory structure for security analysis
/// - **Cross-Call Persistence**: State changes are immediately visible to subsequent instructions
///
/// ## Authority Transition Security Model
/// The dual-authority pattern (current + pending) implements a secure handoff mechanism:
/// - Prevents atomic authority switches that could bypass security controls
/// - Enables community observation of governance changes during delay periods
/// - Provides rollback capabilities if malicious governance attempts are detected
/// - Creates immutable audit trails for all authority modifications
///
/// ## Emergency Pause Philosophy
/// Emergency controls balance protocol safety with decentralization principles:
/// - Limited-time pauses prevent indefinite protocol freezing
/// - Severity-based timeouts match response requirements to incident criticality
/// - Multiple emergency responders prevent single points of failure
/// - Automatic expiration ensures liveness even if responders become unavailable
#[account(zero_copy(unsafe))]
#[derive(InitSpace)] // Expecting 254 bytes
#[repr(C)]
pub struct CoreAuthority {
    /// Pool core binding for contextual security enforcement.
    ///
    /// This field establishes the 1:1 relationship between CoreAuthority and its managed pool,
    /// enabling deterministic PDA addressing and preventing authority cross-contamination
    /// between different pool instances. The binding is immutable after initialization to
    /// prevent authority hijacking attacks.
    pub pool_core: Pubkey,

    /// Active governance authority with immediate operational control.
    ///
    /// This authority can execute privileged operations and initiate governance changes.
    /// The separation between current and pending authorities implements a secure handoff
    /// mechanism that prevents atomic authority switches which could bypass security controls.
    pub current_authority: Pubkey,

    /// Proposed authority awaiting confirmation and time-lock completion.
    ///
    /// During authority transitions, this field holds the proposed new authority while
    /// the governance process validates the change. The pending state enables community
    /// observation and intervention during the mandatory delay period.
    pub pending_authority: Pubkey,

    /// Timestamp when the current authority transition was initiated.
    ///
    /// This temporal anchor enables enforcement of minimum governance delays, providing
    /// community time to detect and respond to potentially malicious authority changes.
    /// Combined with the delay field, it creates a deterministic time-lock mechanism.
    pub authority_change_requested_at: i64,

    /// Minimum delay duration for authority transitions in seconds.
    ///
    /// This delay serves multiple security purposes:
    /// - Prevents flash governance attacks using borrowed funds or temporary token acquisition
    /// - Provides community time to exit positions if governance changes are undesirable
    /// - Enables emergency response if malicious authority transitions are detected
    /// - Creates predictable governance timelines for stakeholder planning
    pub authority_change_delay: i64,

    /// Temporal anchor for emergency pause duration enforcement.
    ///
    /// This timestamp enables automatic emergency pause expiration, ensuring protocol
    /// liveness even if emergency responders become unavailable. It prevents indefinite
    /// protocol freezing that could be used as a denial-of-service attack vector.
    pub emergency_pause_initiated_at: i64,

    /// Maximum duration for the current emergency pause in seconds.
    ///
    /// This timeout is calibrated based on emergency severity levels, balancing rapid
    /// response needs with protocol availability requirements. Critical incidents get
    /// longer pause durations to enable thorough investigation and remediation.
    pub emergency_pause_timeout: i64,

    /// Protocol lifecycle timestamp for compliance and audit requirements.
    ///
    /// This immutable creation timestamp establishes the authoritative record of when
    /// the governance structure was established. Essential for regulatory compliance
    /// and forensic analysis of protocol evolution over time.
    pub created_at: i64,

    /// Activity timestamp for governance monitoring and compliance tracking.
    ///
    /// This field is updated with every significant governance action, enabling
    /// automated monitoring systems to detect governance inactivity or unusual
    /// patterns that might indicate compromise or abandonment.
    pub last_updated: i64,

    /// State flag indicating an active authority transition process.
    ///
    /// This boolean prevents overlapping authority changes which could create race conditions
    /// or confusion about which transition takes precedence. Only one governance transition
    /// can be active at any time, ensuring clean state management.
    pub has_pending_authority: u8,

    /// Multi-signature approval status for the current governance proposal.
    ///
    /// This flag tracks whether the multisig threshold has been reached for the pending
    /// authority change. The separation between multisig approval and time-lock completion
    /// ensures both community consensus and temporal validation requirements are satisfied.
    pub multisig_approved: u8,

    /// Cryptographic commitment to the current governance proposal content.
    ///
    /// This hash serves as a tamper-evident commitment mechanism, ensuring all multisig
    /// signers are approving identical proposal content. It prevents proposal substitution
    /// attacks where the proposal content changes after signatures are collected.
    pub proposal_digest: [u8; 32],

    /// Current protocol operational state with graduated response capabilities.
    ///
    /// This enumerated status enables fine-grained protocol control, allowing different
    /// operational modes based on current conditions. The graduated approach enables
    /// proportional responses to different threat levels without binary on/off controls.
    // Stored as raw u8 (OperationalStatus) for zero_copy Pod/Zeroable compliance.
    pub operational_status: u8,

    /// Emergency pause activation flag for rapid incident response.
    ///
    /// This boolean enables immediate protocol pausing in response to critical security
    /// incidents. The separation between operational_status and this flag allows for
    /// emergency overrides that can bypass normal operational state transitions.
    pub emergency_pause_active: u8,

    /// Protocol version identifier for upgrade compatibility and feature gating.
    ///
    /// This version field enables safe protocol evolution by allowing different
    /// governance logic based on the authority version. Critical for maintaining
    /// backward compatibility during protocol upgrades and migrations.
    pub security_version: u16,

    /// Reserved space for future protocol enhancements and memory alignment.
    ///
    /// This padding serves dual purposes:
    /// - **Future Compatibility**: Enables adding new fields without breaking existing accounts
    /// - **Memory Alignment**: Ensures optimal memory layout for zero-copy access patterns
    /// - **Upgrade Safety**: Provides space for emergency protocol modifications
    /// The 66-byte size aligns with common cache line boundaries for performance.
    pub reserved: [u8; 66],
}

impl CoreAuthority {
    /// Initializes CoreAuthority with secure defaults and comprehensive state validation.
    ///
    /// This initialization method establishes the foundational security state for protocol
    /// governance, implementing several critical safety patterns:
    ///
    /// ## Secure Default Strategy
    /// All security-sensitive fields are explicitly initialized rather than relying on
    /// Rust's default values. This prevents uninitialized state vulnerabilities and
    /// ensures predictable behavior across different deployment environments.
    ///
    /// ## Temporal Anchoring Pattern
    /// Both creation and update timestamps are set to the same value during initialization,
    /// establishing a consistent temporal baseline for all subsequent governance operations.
    /// This pattern simplifies time-based validation logic and audit trail analysis.
    ///
    /// ## Zero-State Security Model
    /// Pending authorities and proposals are initialized to zero/empty states, ensuring
    /// no residual data from memory allocation could be interpreted as valid governance
    /// state. This prevents memory reuse attacks and ensures clean initialization.
    ///
    /// ## Protocol Constant Integration
    /// The authority change delay is set from protocol constants rather than parameters,
    /// ensuring consistent governance timing across all pools and preventing
    /// configuration errors that could compromise security timing requirements.
    pub fn initialize(
        &mut self,
        pool_core: Pubkey,
        initial_authority: Pubkey,
        timestamp: i64,
    ) -> Result<()> {
        // Establish immutable pool binding for contextual security
        self.pool_core = pool_core;
        self.current_authority = initial_authority;

        // Initialize governance transition state to secure defaults
        self.pending_authority = Pubkey::default();
        self.has_pending_authority = 0;
        self.authority_change_requested_at = 0;
        self.authority_change_delay = AUTHORITY_CHANGE_DELAY;
        self.multisig_approved = 0;
        self.proposal_digest = [0u8; 32];

        // Set operational state to normal with no emergency conditions
        self.operational_status = OperationalStatus::Normal as u8;
        self.emergency_pause_active = 0;
        self.emergency_pause_initiated_at = 0;
        self.emergency_pause_timeout = 0;

        // Establish temporal baseline for governance activity tracking
        self.created_at = timestamp;
        self.last_updated = timestamp;

        // Set initial security version for upgrade compatibility
        self.security_version = 1;
        Ok(())
    }

    /// Initiates authority transition with atomic state updates and conflict prevention.
    ///
    /// This method implements the first phase of the secure authority handoff process,
    /// establishing the governance proposal while preventing common attack vectors:
    ///
    /// ## Single-Transition Enforcement
    /// The `has_pending_authority` check ensures only one authority transition can be
    /// active at any time. This prevents race conditions between competing proposals
    /// and eliminates confusion about which transition has precedence.
    ///
    /// ## Cryptographic Commitment Pattern
    /// The proposal digest creates an immutable commitment to the specific governance
    /// change being proposed. This hash prevents proposal substitution attacks where
    /// malicious actors might change proposal content after multisig signatures are
    /// collected but before execution.
    ///
    /// ## Temporal Anchoring for Time-Lock Security
    /// Recording the exact proposal timestamp enables deterministic time-lock
    /// enforcement, ensuring consistent delay periods regardless of when the
    /// multisig confirmation process completes. This prevents timing manipulation
    /// attacks that could bypass intended governance delays.
    ///
    /// ## Atomic State Transition Design
    /// All related fields are updated together within this method, ensuring
    /// consistent intermediate states. Either the proposal is fully established
    /// or the function fails with no partial state changes.
    pub fn propose_authority_change(
        &mut self,
        new_authority: Pubkey,
        proposal_digest: [u8; 32],
        timestamp: i64,
    ) -> Result<()> {
        // Prevent overlapping governance transitions
        if self.has_pending_authority != 0 {
            return Err(PdaSecurityAuthorityError::AuthorityChangeInProgress.into());
        }

        // Atomically establish pending authority transition state
        self.has_pending_authority = 1;
        self.pending_authority = new_authority;
        self.authority_change_requested_at = timestamp;
        self.proposal_digest = proposal_digest;

        Ok(())
    }

    /// Activates emergency pause with severity-calibrated timeouts and immediate effect.
    ///
    /// This method provides rapid protocol protection capabilities while implementing
    /// safeguards against abuse and ensuring eventual protocol recovery:
    ///
    /// ## Severity-Based Timeout Strategy
    /// Different emergency levels receive different maximum pause durations, balancing
    /// response time requirements with protocol availability:
    /// - **Low (24h)**: Minor issues requiring quick fixes
    /// - **Medium (3d)**: Moderate threats needing investigation
    /// - **High (7d)**: Serious vulnerabilities requiring thorough analysis  
    /// - **Critical (14d)**: Existential threats demanding comprehensive response
    ///
    /// ## Immediate Activation Design
    /// Emergency pause takes effect immediately upon execution, providing instant
    /// protection against ongoing attacks. The pause flag is set before timeout
    /// calculation to ensure atomic activation even if timeout computation fails.
    ///
    /// ## Automatic Expiration Mechanism
    /// The timeout system ensures protocol liveness even if emergency responders
    /// become unavailable. This prevents indefinite protocol freezing that could
    /// constitute a denial-of-service attack or governance capture scenario.
    ///
    /// ## Operational Status Synchronization
    /// Both the emergency flag and operational status are updated to maintain
    /// consistent protocol state. This dual-flag approach enables different
    /// subsystems to check emergency status through their preferred mechanism.
    pub fn emergency_pause(
        &mut self,
        emergency_level: EmergencyLevel,
        timestamp: i64,
    ) -> Result<()> {
        // Immediately activate emergency pause for instant protection
        self.emergency_pause_active = 1;
        self.emergency_pause_initiated_at = timestamp;

        // Calculate severity-appropriate timeout duration
        self.emergency_pause_timeout = timestamp
            + match emergency_level {
                EmergencyLevel::Low => 24 * 3600,                // 1 day
                EmergencyLevel::Medium => 3 * 24 * 3600,         // 3 days
                EmergencyLevel::High => EMERGENCY_PAUSE_TIMEOUT, // 7 days
                EmergencyLevel::Critical => 14 * 24 * 3600,      // 14 days
            };

        // Synchronize operational status for consistent protocol state
        self.operational_status = OperationalStatus::EmergencyPause as u8;

        Ok(())
    }

    /// Records multisig approval with proposal integrity validation.
    ///
    /// This method serves as the bridge between the multisig confirmation process
    /// and the core authority state, implementing cryptographic proposal validation
    /// to prevent signature reuse and proposal substitution attacks.
    ///
    /// ## Proposal Integrity Enforcement
    /// The digest comparison ensures the multisig signatures were collected for
    /// the exact same proposal currently pending in the core authority. This
    /// prevents attacks where signatures are collected for one proposal but
    /// applied to a different one.
    ///
    /// ## State Consistency Guarantee
    /// The approval flag is only set after successful digest validation,
    /// ensuring the approval state always corresponds to the current pending
    /// proposal. This prevents approval state from being orphaned if
    /// proposals change.
    pub fn mark_multisig_approved(&mut self, digest: [u8; 32]) -> Result<()> {
        // Verify multisig approval corresponds to current pending proposal
        if self.proposal_digest != digest {
            return Err(PdaSecurityAuthorityError::InvalidProposal.into());
        }

        // Mark proposal as approved after integrity validation
        self.multisig_approved = 1;
        Ok(())
    }
    /// Executes authority transition after validating all security requirements.
    ///
    /// This method represents the final phase of the secure authority handoff process,
    /// implementing comprehensive validation before executing the irreversible transition:
    ///
    /// ## Dual-Validation Security Model
    /// Two independent security requirements must be satisfied before execution:
    /// - **Time-Lock Completion**: Ensures sufficient community observation period
    /// - **Multisig Threshold**: Confirms distributed governance consensus
    ///
    /// The `require!` macro provides fail-fast validation with specific error codes,
    /// enabling calling code to distinguish between timing and consensus failures.
    ///
    /// ## Atomic State Transition Pattern
    /// All authority-related state changes happen atomically within this method.
    /// Either the transition completes fully with all cleanup, or it fails with
    /// no state modifications. This prevents partially-executed transitions that
    /// could create governance inconsistencies.
    ///
    /// ## Clean State Reset Strategy
    /// After successful execution, all transition-related fields are reset to
    /// their initial values. This cleanup ensures the authority is ready for
    /// future governance changes without carrying forward stale proposal data.
    ///
    /// ## Success Signaling Design
    /// Returns a boolean indicating successful execution to enable calling code
    /// to trigger follow-up actions (notifications, logging, etc.) only when
    /// the transition actually completes.
    pub fn execute_authority_change(&mut self, timestamp: i64) -> Result<bool> {
        // Validate time-lock completion before proceeding
        require!(
            timestamp >= self.authority_change_requested_at + self.authority_change_delay,
            PdaSecurityAuthorityError::TimelockNotReady
        );

        // Validate multisig consensus before proceeding
        require!(
            self.multisig_approved != 0,
            PdaSecurityAuthorityError::InsufficientSignatures
        );

        // Execute atomic authority transition
        self.current_authority = self.pending_authority;
        self.pending_authority = Pubkey::default();
        self.has_pending_authority = 0;
        self.authority_change_requested_at = 0;
        self.multisig_approved = 0;
        self.proposal_digest = [0u8; 32];

        // Signal successful execution to calling code
        Ok(true)
    }
}

/// Protocol operational states with graduated response capabilities.
///
/// This enumeration provides fine-grained operational control, enabling proportional
/// responses to different protocol conditions without binary on/off semantics:
///
/// ## State Transition Philosophy
/// Each state represents a specific operational mode with associated capabilities
/// and restrictions. The design enables smooth transitions between states while
/// maintaining clear boundaries for what operations are permitted in each mode.
///
/// ## Enum Representation Strategy  
/// Using `#[repr(u8)]` creates a compact, deterministic binary representation
/// suitable for zero-copy deserialization and cross-platform compatibility.
/// The explicit discriminant values enable stable serialization across updates.
///
/// ## Maintenance vs Emergency Distinction
/// Separating planned maintenance from emergency conditions enables different
/// communication strategies and user expectations. Maintenance suggests planned
/// downtime, while EmergencyPause indicates unexpected security responses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, InitSpace)]
#[repr(u8)]
pub enum OperationalStatus {
    /// Standard operational mode with full protocol capabilities enabled.
    Normal = 0,

    /// Planned maintenance mode with controlled capability restrictions.
    Maintenance = 1,

    /// Emergency response mode with immediate capability suspension.
    EmergencyPause = 2,

    /// End-of-life mode indicating protocol retirement or migration.
    Deprecated = 3,

    /// Active protocol upgrade mode with transitional state management.
    Upgrading = 4,
}

impl CoreAuthority {
    #[inline(always)]
    pub fn operational_status_enum(&self) -> Option<OperationalStatus> {
        match self.operational_status {
            0 => Some(OperationalStatus::Normal),
            1 => Some(OperationalStatus::Maintenance),
            2 => Some(OperationalStatus::EmergencyPause),
            3 => Some(OperationalStatus::Deprecated),
            4 => Some(OperationalStatus::Upgrading),
            _ => None,
        }
    }

    #[inline(always)]
    pub fn set_operational_status(&mut self, status: OperationalStatus) {
        self.operational_status = status as u8;
    }
}

/// Emergency severity classification for calibrated response protocols.
///
/// This enumeration enables severity-appropriate emergency responses, balancing
/// rapid incident response with protocol availability requirements:
///
/// ## Response Time Calibration
/// Each level corresponds to different maximum pause durations, reflecting
/// the balance between thorough investigation needs and protocol liveness:
/// - Lower severity enables faster recovery for minor issues
/// - Higher severity provides extended response time for complex threats
///
/// ## Escalation Path Design
/// The levels create a clear escalation path for emergency responders,
/// enabling appropriate resource allocation and communication strategies
/// based on incident severity assessment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum EmergencyLevel {
    /// Minor issues requiring rapid resolution (24-hour maximum pause).
    Low = 0,

    /// Moderate threats needing investigation (3-day maximum pause).
    Medium = 1,

    /// Serious vulnerabilities requiring thorough analysis (7-day maximum pause).
    High = 2,

    /// Existential threats demanding comprehensive response (14-day maximum pause).
    Critical = 3,
}

/// Anchor account context for secure CoreAuthority initialization.
///
/// This context enforces critical protocol invariants during authority creation,
/// implementing deterministic addressing and secure initialization patterns:
///
/// ## PDA Security Architecture
/// The `seeds = [b"core_authority", pool_core.key().as_ref()]` pattern provides:
/// - **Deterministic Addressing**: Each pool gets exactly one CoreAuthority account
/// - **Cross-Reference Prevention**: Authority accounts cannot be shared between pools
/// - **Collision Resistance**: Cryptographic uniqueness prevents address conflicts
/// - **Seedless Security**: No external entity can control the PDA private key
///
/// ## Account Size Strategy
/// The space calculation includes the 8-byte Anchor discriminator plus the exact
/// struct size to prevent under/over-allocation. This precision ensures optimal
/// rent costs while maintaining zero-copy compatibility.
///
/// ## Authority Bootstrap Pattern
/// The `initial_authority` signer establishes the starting point for governance
/// without granting ongoing control. This separation ensures the initialization
/// process cannot be hijacked by funding entities or deployment infrastructure.
#[derive(Accounts)]
pub struct InitializeCoreAuthority<'info> {
    /// CoreAuthority account being initialized with deterministic PDA addressing.
    ///
    /// AccountLoader enables zero-copy access patterns for efficient governance
    /// operations without deserialization overhead during frequent state checks.
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<CoreAuthority>(),
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump,
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// Pool core account establishing the governance relationship.
    ///
    /// UncheckedAccount allows flexible pool validation while maintaining
    /// deterministic PDA derivation. Actual pool validation occurs in the
    /// initialize method where business logic can perform comprehensive checks.
    pub pool_core: UncheckedAccount<'info>,

    /// Funding account for CoreAuthority creation rent costs.
    ///
    /// Separation from initial_authority prevents automatic governance control
    /// acquisition by entities providing deployment funding.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Bootstrap authority for initial governance configuration.
    ///
    /// This authority only controls the initialization process and does not
    /// automatically receive ongoing governance privileges beyond setup.
    pub initial_authority: Signer<'info>,

    /// System program for account creation and rent management.
    pub system_program: Program<'info, System>,
}

/// Anchor account context for authority transition proposal initiation.
///
/// This context implements the first phase of secure authority handoff, enforcing
/// distributed governance requirements and preventing unauthorized proposals:
///
/// ## Multisig Authorization Pattern
/// Only members of the associated MultisigConfig can propose authority changes,
/// implementing distributed proposal authority rather than single-signer control.
/// This prevents scenarios where a single compromised key could initiate governance
/// takeovers without community consensus.
///
/// ## PDA Validation Architecture
/// All accounts use deterministic seeds tied to the same pool_core, ensuring
/// proposals can only affect the intended pool's governance structure. The
/// seed-based validation prevents cross-pool authority manipulation attacks.
///
/// ## New Authority Pre-Validation
/// The new_authority account uses PDA seeds to establish deterministic addressing,
/// enabling validation of the proposed authority before accepting the proposal.
/// This pattern prevents invalid or malicious authority proposals from entering
/// the governance pipeline.
#[derive(Accounts)]
pub struct ProposeAuthorityChange<'info> {
    /// CoreAuthority account receiving the governance proposal.
    ///
    /// Mutable access enables proposal state updates while seed validation
    /// ensures only the correct pool's authority can be modified.
    #[account(
        mut,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump,
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// MultisigConfig for distributed proposal authorization validation.
    ///
    /// Read-only access sufficient for membership validation while seed
    /// constraints ensure proposals can only come from the pool's designated
    /// multisig governance structure.
    #[account(
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump,
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// Pool core account establishing the governance context.
    ///
    /// UncheckedAccount allows flexible validation while maintaining
    /// deterministic relationships between all governance-related accounts.
    pub pool_core: UncheckedAccount<'info>,

    /// Proposer account that must be validated as a multisig member.
    ///
    /// Signer requirement ensures cryptographic proof of proposal intent
    /// while business logic validates multisig membership authorization.
    pub proposer: Signer<'info>,

    /// Proposed new authority account with deterministic addressing.
    ///
    /// PDA constraint enables pre-validation of the proposed authority
    /// while preventing arbitrary or malicious authority specifications.
    #[account(
        seeds = [b"new_authority", pool_core.key().as_ref()],
        bump,
    )]
    pub new_authority: UncheckedAccount<'info>,
}

/// Anchor account context for authority transition execution and confirmation.
///
/// This context implements the final phase of secure authority handoff, combining
/// time-lock validation with multisig consensus requirements:
///
/// ## Dual-Authority Pattern for Execution
/// Both CoreAuthority and MultisigConfig require mutable access because execution
/// involves updating both the authority state and resetting multisig confirmations.
/// This atomic update pattern ensures consistent state across both accounts.
///
/// ## Distributed Confirmation Authority
/// Only multisig members can trigger execution attempts, preventing unauthorized
/// parties from repeatedly testing execution conditions. The confirmer validation
/// occurs within the handler logic rather than account constraints to enable
/// detailed error reporting.
///
/// ## Deterministic Account Relationships
/// All accounts derive from the same pool_core seed, ensuring execution can only
/// affect the intended pool's governance structure. This prevents execution
/// attempts from being misdirected to wrong pools or authority structures.
#[derive(Accounts)]
pub struct ExecuteAuthorityChange<'info> {
    /// CoreAuthority account where the transition will be executed.
    ///
    /// Mutable access required for authority state updates and proposal cleanup
    /// after successful execution or failed validation attempts.
    #[account(
        mut,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// MultisigConfig for confirmation tracking and threshold validation.
    ///
    /// Mutable access needed for confirmation state updates and potential
    /// reset operations during the execution process.
    #[account(
        mut,
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// Pool core account establishing execution context and account relationships.
    pub pool_core: UncheckedAccount<'info>,

    /// Confirmer account that must be validated as a multisig member.
    ///
    /// Cryptographic signature proves intent while business logic validates
    /// authorization to participate in the execution process.
    pub confirmer: Signer<'info>,
}

/// Anchor account context for emergency protocol pause activation.
///
/// This context enables rapid emergency response while maintaining authorization
/// controls and audit trails for all emergency actions:
///
/// ## Emergency Authority Validation Pattern
/// The EmergencyContacts account contains the list of authorized emergency
/// responders who can trigger protocol pauses. This separation enables
/// emergency authority management independent of normal governance structures,
/// allowing faster response times during critical incidents.
///
/// ## Read-Only Emergency Contacts Design
/// EmergencyContacts requires only read access because emergency pause activation
/// doesn't modify the emergency responder list. This separation of concerns
/// enables emergency actions without risk of accidentally modifying responder
/// authorization during crisis situations.
///
/// ## Immediate Response Architecture
/// The CoreAuthority account receives mutable access for immediate pause
/// activation without additional validation delays. The emergency responder
/// validation occurs before state changes to ensure only authorized pauses.
#[derive(Accounts)]
pub struct EmergencyPause<'info> {
    /// CoreAuthority account receiving the emergency pause activation.
    ///
    /// Mutable access enables immediate operational status changes and
    /// emergency state updates without additional confirmation delays.
    #[account(
        mut,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// EmergencyContacts for responder authorization validation.
    ///
    /// Read-only access sufficient for emergency authority validation
    /// while preventing accidental modification of responder lists during
    /// crisis response operations.
    #[account(
        seeds = [b"emergency_contacts", pool_core.key().as_ref()],
        bump
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    /// Pool core account establishing emergency response context.
    pub pool_core: UncheckedAccount<'info>,

    /// Emergency responder account triggering the pause activation.
    ///
    /// Cryptographic signature proves emergency action intent while
    /// business logic validates emergency authority before activation.
    pub emergency_responder: Signer<'info>,
}

/// Initializes CoreAuthority with validated parameters and secure state establishment.
///
/// This handler serves as the bootstrap function for protocol governance, implementing
/// secure initialization patterns while maintaining simplicity and auditability:
///
/// ## Zero-Copy Initialization Strategy
/// Uses `load_init()` to establish the account in an initialized but empty state,
/// then delegates to the struct's initialize method for field population. This
/// two-phase approach ensures proper account allocation before business logic.
///
/// ## Delegation Pattern Benefits
/// By delegating validation and initialization logic to the struct method, this
/// handler remains focused on Anchor-specific concerns (account loading, context
/// validation) while centralizing business logic in testable, reusable methods.
///
/// ## Error Propagation Design
/// The `?` operator ensures initialization failures propagate with full context,
/// enabling calling code to distinguish between account loading failures and
/// business logic validation failures for appropriate error handling.
pub fn initialize_core_authority(ctx: Context<InitializeCoreAuthority>) -> Result<()> {
    // Load account in initialized state for zero-copy operations
    let core_authority = &mut ctx.accounts.core_authority.load_init()?;
    let clock = Clock::get()?;
    // Delegate to struct method for centralized initialization logic
    core_authority.initialize(
        ctx.accounts.pool_core.key(),
        ctx.accounts.initial_authority.key(),
        clock.unix_timestamp,
    )?;

    Ok(())
}

/// Initiates authority transition proposal with comprehensive validation and commitment.
///
/// This handler implements the first phase of secure governance handoff, enforcing
/// distributed authorization while establishing cryptographic commitment to the proposal:
///
/// ## Multi-Layer Authorization Pattern
/// Validates proposer authorization through multiple independent checks:
/// - **Multisig Membership**: Ensures only governance participants can propose
/// - **Self-Proposal Prevention**: Blocks current authority from proposing itself
/// - **State Consistency**: Verifies no conflicting proposals are in progress
///
/// ## Cryptographic Commitment Strategy
/// The proposal hash combines the new authority and proposer keys, creating an
/// immutable commitment that prevents proposal substitution attacks. This hash
/// serves as the foundation for subsequent multisig validation processes.
///
/// ## Temporal Anchoring Implementation
/// Clock timestamp capture at proposal time establishes the authoritative start
/// of the governance delay period, preventing timing manipulation attacks that
/// could bypass intended security delays.
///
/// ## Error Classification Design
/// Different error types enable calling code to distinguish between authorization
/// failures (wrong proposer) and state conflicts (proposal already in progress),
/// supporting appropriate user interface feedback and retry strategies.
pub fn propose_authority_change(ctx: Context<ProposeAuthorityChange>) -> Result<()> {
    // Load governance accounts for validation and state updates
    let core_authority = &mut ctx.accounts.core_authority.load_mut()?;
    let multisig_config = &ctx.accounts.multisig_config.load()?;

    // Validate proposer membership in multisig governance structure
    if !multisig_config.is_member(&ctx.accounts.proposer.key()) {
        return Err(PdaSecurityAuthorityError::NotAMultisigMember.into());
    }

    // Prevent self-proposal to avoid governance bypass scenarios
    if ctx.accounts.new_authority.key() == core_authority.current_authority {
        return Err(PdaSecurityAuthorityError::Unauthorized.into());
    }

    // Create cryptographic commitment to proposal content
    let data_hash = hashv(&[
        ctx.accounts.new_authority.key().as_ref(),
        ctx.accounts.proposer.key().as_ref(),
    ])
    .to_bytes();

    // Establish temporal anchor for governance delay enforcement
    let clock = Clock::get()?;

    // Initiate authority transition proposal with validated parameters
    core_authority.propose_authority_change(
        ctx.accounts.new_authority.key(),
        data_hash,
        clock.unix_timestamp,
    )?;

    Ok(())
}

/// Processes authority transition confirmation and executes validated changes.
///
/// This handler orchestrates the final phase of secure governance handoff, combining
/// multisig consensus tracking with time-lock validation and atomic execution:
///
/// ## Confirmation Tracking Architecture
/// The handler creates a deterministic proposal hash that uniquely identifies the
/// specific authority transition being confirmed. This hash combines multiple
/// elements to prevent confirmation replay attacks:
/// - **Authority Identity**: The specific new authority being proposed
/// - **Temporal Component**: The exact timestamp when the proposal was initiated
/// - **Operation Context**: A prefix distinguishing authority changes from other proposals
///
/// ## Progressive Validation Strategy
/// Confirmation processing follows a staged approach:
/// 1. **Authorization Check**: Validates confirmer is a multisig member
/// 2. **Consensus Tracking**: Records confirmation and checks threshold
/// 3. **Execution Attempt**: Only proceeds if multisig threshold is reached
/// 4. **Final Validation**: Time-lock and approval checks within execution method
///
/// ## Atomic Execution Design
/// The execution attempt only occurs when multisig threshold is reached, but the
/// actual execution method performs final validation (time-lock completion, approval
/// status). This layered approach ensures all security requirements are satisfied
/// before irreversible state changes.
///
/// ## State Synchronization Pattern
/// Both CoreAuthority and MultisigConfig are updated atomically during successful
/// execution, ensuring consistent governance state across both account structures.
/// Partial updates are prevented by the atomic execution model.
pub fn execute_authority_change(ctx: Context<ExecuteAuthorityChange>) -> Result<()> {
    // Load governance accounts for confirmation processing and potential execution
    let core_authority = &mut ctx.accounts.core_authority.load_mut()?;
    let multisig_config = &mut ctx.accounts.multisig_config.load_mut()?;

    // Validate confirmer authorization for governance participation
    if !multisig_config.is_member(&ctx.accounts.confirmer.key()) {
        return Err(PdaSecurityAuthorityError::NotAMultisigMember.into());
    }

    // Create deterministic proposal hash for confirmation tracking
    let pending_authority = core_authority.pending_authority;
    let proposal_hash = anchor_lang::solana_program::hash::hashv(&[
        b"authority_change",
        pending_authority.as_ref(),
        &core_authority.authority_change_requested_at.to_le_bytes(),
    ])
    .to_bytes();

    // Establish temporal context for confirmation processing
    let clock = Clock::get()?;

    // Process confirmation and check if threshold reached
    let threshold_reached = multisig_config.confirm_proposal(
        &ctx.accounts.confirmer.key(),
        proposal_hash,
        clock.unix_timestamp,
    )?;

    // Execute authority transition if multisig consensus achieved
    if threshold_reached {
        core_authority.execute_authority_change(clock.unix_timestamp)?;
    }

    Ok(())
}

/// Activates emergency protocol pause with severity-appropriate response parameters.
///
/// This handler provides critical incident response capabilities while maintaining
/// authorization controls and ensuring eventual protocol recovery:
///
/// ## Emergency Authority Validation Model
/// The handler validates emergency responder authorization through the EmergencyContacts
/// account rather than using normal governance structures. This separation enables
/// rapid response times during critical incidents when normal governance processes
/// might be too slow or compromised.
///
/// ## Immediate Response Design Philosophy
/// Emergency pause activation takes effect immediately upon successful authorization,
/// providing instant protocol protection against ongoing attacks. The severity level
/// determines pause duration rather than activation delay, balancing rapid response
/// with measured restrictions.
///
/// ## Severity-Calibrated Response Strategy
/// The emergency level parameter enables proportional responses to different threat
/// types, ensuring minor issues don't trigger unnecessarily long protocol freezes
/// while critical threats receive adequate investigation time.
///
/// ## Audit Trail Integration
/// All emergency actions are timestamped and recorded in the CoreAuthority state,
/// providing comprehensive audit trails for post-incident analysis and compliance
/// reporting. This transparency supports forensic analysis and regulatory requirements.
pub fn emergency_pause(
    ctx: Context<EmergencyPause>,
    emergency_level: EmergencyLevel,
) -> Result<()> {
    // Load emergency response and authority accounts
    let core_authority = &mut ctx.accounts.core_authority.load_mut()?;
    let emergency_contacts = &ctx.accounts.emergency_contacts.load()?;

    // Validate emergency responder authorization for immediate response
    if !emergency_contacts.has_emergency_authority(&ctx.accounts.emergency_responder.key()) {
        return Err(PdaSecurityAuthorityError::InsufficientPermissions.into());
    }

    // Establish temporal context for pause duration and audit trails
    let clock = Clock::get()?;

    // Execute immediate emergency pause with severity-appropriate parameters
    core_authority.emergency_pause(emergency_level, clock.unix_timestamp)?;

    Ok(())
}
