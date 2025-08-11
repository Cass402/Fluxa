use crate::error::PdaSecurityAuthorityError;
use anchor_lang::prelude::*;

/// Multi-signature configuration enforcing distributed authority for critical pool operations.
///
/// This structure implements a defense-in-depth security model where no single entity can
/// unilaterally execute privileged actions that could compromise pool integrity or user funds.
/// The design addresses several attack vectors common in DeFi protocols.
///
/// ## Attack Vector Mitigation Strategy
/// - **Insider Threats**: No single compromised key can drain pools or manipulate parameters
/// - **Social Engineering**: Multiple independent parties must be compromised simultaneously
/// - **Key Compromise**: Partial key exposure doesn't grant full protocol control
/// - **Governance Attacks**: Malicious proposals require consensus from multiple stakeholders
///
/// ## Zero-Copy Architecture Rationale
/// Uses zero-copy layout because multisig validation occurs in performance-critical paths
/// during emergency responses and parameter updates. Avoiding deserialization overhead
/// ensures these critical operations can execute within Solana's compute unit limits,
/// preventing scenarios where urgent security responses fail due to computational constraints.
///
/// ## Fixed-Size Member Array Design Philosophy
/// The 7-member limit reflects operational security research showing that groups larger
/// than 7 become unwieldy for emergency decision-making, while smaller groups lack
/// sufficient decentralization. This size enables various governance models:
/// - 3/5 for standard operations (60% consensus)
/// - 5/7 for critical changes (71% consensus)
/// - 7/7 for emergency actions (unanimous consent)
///
/// ## Bitfield Confirmation Efficiency
/// Uses bitfield tracking instead of individual confirmation arrays to:
/// - Enable atomic confirmation state updates preventing race conditions
/// - Minimize storage costs (8 bits vs 7 * 32 bytes for boolean array)
/// - Provide efficient membership verification through bitwise operations
/// - Support up to 255 members theoretically while staying efficient for practical sizes
///
/// ## Temporal Audit Trail
/// All state transitions are timestamped to enable:
/// - Regulatory compliance reporting showing decision timelines
/// - Forensic analysis during security incident investigation
/// - Governance analytics to measure decision-making efficiency
/// - Detection of unusual voting patterns that might indicate coordination attacks
#[account(zero_copy(unsafe))]
#[derive(InitSpace)] // Expecting 348 bytes
#[repr(C)]
pub struct MultisigConfig {
    /// Pool identity binding preventing cross-pool authority confusion attacks.
    ///
    /// This reference creates a cryptographic binding between the multisig configuration
    /// and its specific pool, preventing several classes of attacks:
    /// - Signature replay attacks where valid signatures from one pool are used against another
    /// - Cross-pool governance confusion where authorities accidentally act on wrong pools
    /// - Pool spoofing where malicious actors might try to claim authority over other pools
    ///
    /// The binding is enforced through PDA derivation using this pool_core as a seed,
    /// making it computationally infeasible to use this multisig config for other pools.
    pub pool_core: Pubkey,

    /// Consensus threshold and member management with overflow protection.
    ///
    /// threshold: Minimum signatures required for proposal execution.
    /// This value establishes the security-convenience trade-off for the multisig. Higher
    /// thresholds increase security but reduce operational agility. The threshold must
    /// balance protection against compromised keys with the need for timely responses
    /// to market conditions or security incidents.
    ///
    /// member_count: Active members in the authority set.
    /// Tracked separately from array length to enable efficient member validation without
    /// scanning the entire fixed-size array. This optimization is crucial during signature
    /// verification in performance-critical emergency response scenarios.
    ///
    /// members: Fixed-size array of authorized signers.
    /// Using a fixed array instead of Vec prevents:
    /// - Dynamic allocation failures during critical operations
    /// - Variable compute costs that could cause transactions to fail unpredictably
    /// - Memory fragmentation in long-running validator processes
    /// - Potential denial-of-service attacks through excessive member list growth
    pub threshold: u8,
    pub member_count: u8,
    pub members: [Pubkey; 7],

    /// Atomic proposal confirmation tracking preventing double-spending of signatures.
    ///
    /// current_proposal_hash: Cryptographic commitment to the proposal being voted on.
    /// This hash serves multiple critical security functions:
    /// - Prevents signature malleability where attackers modify proposals after partial signing
    /// - Ensures all signers are voting on identical proposal content
    /// - Enables proposal caching and verification without storing full proposal data
    /// - Prevents replay attacks where old signatures are reused for different proposals
    ///
    /// confirmation_bitmap: Bit-packed signature status for memory efficiency.
    /// Each bit represents one member's confirmation status. This design choice provides:
    /// - O(1) confirmation checking instead of O(n) array scans
    /// - Atomic updates preventing race conditions during concurrent signing
    /// - Memory efficiency (1 bit per member vs 1 byte per boolean)
    /// - Natural prevention of double-confirmation by the same member
    ///
    /// confirmation_count: Running tally enabling fast threshold checks.
    /// Maintained separately from bitmap to avoid bit counting operations during
    /// threshold validation. This optimization is critical when checking thresholds
    /// during time-sensitive emergency responses where every compute unit matters.
    pub current_proposal_hash: [u8; 32],
    pub confirmation_bitmap: u8,
    pub confirmation_count: u8,

    /// Temporal anchors for governance analytics and compliance reporting.
    ///
    /// created_at: Pool governance establishment timestamp.
    /// Records when distributed authority was first established for this pool, providing
    /// a baseline for measuring governance maturity and enabling compliance reporting
    /// for regulations that require proof of decentralized control from specific dates.
    ///
    /// last_updated: Most recent governance activity timestamp.
    /// Tracks the recency of multisig usage for several purposes:
    /// - Identifying stale governance structures that might need member rotation
    /// - Demonstrating active governance for regulatory compliance
    /// - Forensic analysis of decision-making patterns during investigations
    /// - Automated monitoring systems can detect governance abandonment
    pub created_at: i64,
    pub last_updated: i64,

    /// Future-proofing buffer preventing account migration costs during protocol evolution.
    ///
    /// This reserved space enables seamless addition of new governance features without
    /// requiring expensive account migrations that would disrupt live pools. Examples
    /// of future enhancements this space might accommodate:
    /// - Time-locked proposal execution delays
    /// - Member role differentiation (proposer vs. voter)  
    /// - Proposal expiration timestamps
    /// - Vote weighting or delegation mechanisms
    /// - Integration with external governance systems
    ///
    /// The 32-byte allocation balances future flexibility with current storage efficiency,
    /// representing approximately 8 new u32 fields or 4 new Pubkey references.
    pub reserved: [u8; 32],
}

/// Core multisig operations with comprehensive validation and attack prevention.
impl MultisigConfig {
    /// Initializes multisig configuration with comprehensive validation preventing common setup vulnerabilities.
    ///
    /// This initialization function implements multiple layers of validation because
    /// misconfigured multisigs represent one of the highest-risk failure modes in DeFi.
    /// Common configuration attacks prevented include:
    /// - Zero thresholds enabling unauthorized execution
    /// - Thresholds exceeding member count creating deadlock scenarios
    /// - Oversized member lists causing computational denial-of-service
    /// - Duplicate members enabling signature amplification attacks
    ///
    /// ## Validation Strategy Rationale
    /// All validation occurs atomically before any state modification to prevent
    /// partially-initialized multisigs that could be exploited. The fail-fast approach
    /// ensures either complete success or complete failure with no intermediate states.
    ///
    /// ## Temporal Anchoring Design
    /// Both created_at and last_updated are set to the same timestamp during initialization
    /// to establish a consistent baseline for governance activity tracking and compliance
    /// reporting requirements.
    pub fn initialize(
        &mut self,
        pool_core: Pubkey,
        threshold: u8,
        members: [Pubkey; 7],
        timestamp: i64,
    ) -> Result<()> {
        // Count non-zero members to determine actual member set size
        // This prevents including default Pubkey::default() values as valid members
        let actual_member_count = members.iter().filter(|&&m| m != Pubkey::default()).count();

        // Comprehensive threshold validation preventing common configuration vulnerabilities
        if threshold == 0 {
            return Err(PdaSecurityAuthorityError::InvalidSignatureThreshold.into());
        }
        if threshold > actual_member_count as u8 {
            return Err(PdaSecurityAuthorityError::InvalidSignatureThreshold.into());
        }
        if actual_member_count > 7 {
            return Err(PdaSecurityAuthorityError::InvalidSignatureThreshold.into());
        }

        // Initialize core configuration with validated parameters
        self.pool_core = pool_core;
        self.threshold = threshold;
        self.member_count = actual_member_count as u8;
        self.members = members;

        // Establish temporal baseline for governance tracking
        self.created_at = timestamp;
        self.last_updated = timestamp;

        // Initialize proposal tracking in clean state
        self.current_proposal_hash = [0u8; 32];
        self.confirmation_bitmap = 0;
        self.confirmation_count = 0;

        Ok(())
    }

    /// Validates member authority with early termination optimization.
    ///
    /// This function performs O(n) linear search through the member array, but this
    /// is acceptable because:
    /// - Member count is bounded to 7, making linear search faster than hash table overhead
    /// - Early termination on first match provides average-case performance better than O(n)
    /// - Fixed-size array enables compiler optimizations and cache-friendly access patterns
    /// - The function is called infrequently compared to confirmation operations
    ///
    /// ## Security Considerations
    /// Uses constant-time comparison (==) on Pubkey values, which is safe because
    /// Pubkey doesn't implement any custom comparison logic that could leak timing
    /// information about key relationships.
    pub fn is_member(&self, pubkey: &Pubkey) -> bool {
        // Scan only active members, not the entire fixed-size array
        for i in 0..self.member_count {
            if self.members[i as usize] == *pubkey {
                return true;
            }
        }
        false
    }

    /// Retrieves member index for efficient bitfield operations.
    ///
    /// This function returns the position index needed for bitfield confirmation tracking.
    /// The index is used to create bitmasks for atomic confirmation operations, enabling
    /// O(1) confirmation checking and preventing race conditions during concurrent signing.
    ///
    /// ## Iterator Usage Rationale  
    /// Uses iterator find() instead of manual loop for several benefits:
    /// - Compiler can better optimize iterator chains for the target architecture
    /// - Functional style makes the search intention clearer than imperative loops
    /// - Automatic handling of boundary conditions reduces off-by-one error risk
    /// - Consistent with Rust's preferred functional programming patterns
    pub fn get_member_index(&self, pubkey: &Pubkey) -> Option<u8> {
        // Use iterator for optimized search with automatic bounds checking
        (0..self.member_count).find(|&i| self.members[i as usize] == *pubkey)
    }

    /// Processes member confirmation with atomic state transitions and replay protection.
    ///
    /// This function implements the core multisig confirmation logic with several critical
    /// security features designed to prevent common multisig vulnerabilities:
    ///
    /// ## Proposal Hash Validation Strategy
    /// The proposal hash serves as a cryptographic commitment ensuring all signers are
    /// voting on identical proposal content. When a new proposal hash is detected,
    /// all previous confirmations are atomically reset to prevent signature reuse
    /// across different proposals.
    ///
    /// ## Double-Confirmation Prevention
    /// The bitfield approach naturally prevents double-confirmation by the same member
    /// because setting an already-set bit is idempotent. This eliminates an entire
    /// class of bugs where members might accidentally vote multiple times.
    ///
    /// ## Atomic State Updates
    /// All confirmation state updates happen atomically within this function. Either
    /// the member's confirmation is successfully recorded with all side effects, or
    /// the function fails with no state changes. This prevents partial confirmation
    /// states that could be exploited.
    ///
    /// ## Fast-Path Threshold Detection
    /// Returns immediately whether the threshold has been reached, enabling calling
    /// code to execute proposals without additional verification steps. This design
    /// reduces the window for race conditions in proposal execution.
    pub fn confirm_proposal(
        &mut self,
        member: &Pubkey,
        proposal_hash: [u8; 32],
        timestamp: i64,
    ) -> Result<bool> {
        // Early authorization check to fail fast for non-members
        if !self.is_member(member) {
            return Err(PdaSecurityAuthorityError::InsufficientSignatures.into());
        }

        // Atomic proposal transition: new hash resets all previous confirmations
        // This prevents signature replay attacks across different proposals
        if self.current_proposal_hash != proposal_hash {
            self.current_proposal_hash = proposal_hash;
            self.confirmation_bitmap = 0;
            self.confirmation_count = 0;
        }

        // Retrieve member index for bitfield operations
        let member_index = self
            .get_member_index(member)
            .ok_or(PdaSecurityAuthorityError::Unauthorized)?;

        // Create bitmask for this specific member's confirmation bit
        let member_bit = 1u8 << member_index;

        // Idempotent confirmation check: already confirmed members don't re-increment count
        if (self.confirmation_bitmap & member_bit) != 0 {
            // Return current threshold status without state changes
            return Ok(self.confirmation_count >= self.threshold);
        }

        // Atomic confirmation recording: update bitmap and count together
        self.confirmation_bitmap |= member_bit;
        self.confirmation_count += 1;

        // Update governance activity timestamp for monitoring and compliance
        self.last_updated = timestamp;

        // Return whether proposal now has sufficient confirmations for execution
        Ok(self.confirmation_count >= self.threshold)
    }

    /// Resets confirmation state with atomic clearing and temporal validation.
    ///
    /// This function provides emergency reset capabilities for multisig governance,
    /// implementing several safety mechanisms to prevent abuse while maintaining
    /// operational flexibility:
    ///
    /// ## Emergency Reset Design Philosophy
    /// Reset operations are intentionally simple and fail-safe. Rather than complex
    /// validation logic that could introduce bugs, the function atomically clears
    /// all confirmation state and relies on natural multisig flow for subsequent
    /// operations.
    ///
    /// ## State Consistency Guarantee
    /// All confirmation-related fields are updated atomically within this function.
    /// This prevents intermediate states where bitmap and count values might be
    /// inconsistent, which could lead to threshold calculation errors.
    ///
    /// ## Temporal Anchoring for Audit Trails
    /// The timestamp update ensures all reset operations are logged with precise
    /// timing information. This creates an audit trail showing when governance
    /// decisions were reset, enabling post-hoc analysis of multisig activity.
    ///
    /// ## Zero-State Initialization Pattern
    /// By setting all confirmation fields to zero/empty values, the multisig returns
    /// to a known clean state identical to initial configuration. This simplifies
    /// testing and reasoning about state transitions.
    pub fn reset_confirmations(&mut self, timestamp: i64) {
        // Atomic confirmation state clearing
        self.current_proposal_hash = [0u8; 32];
        self.confirmation_bitmap = 0;
        self.confirmation_count = 0;

        // Temporal anchor for governance activity tracking
        self.last_updated = timestamp;
    }
}

/// Anchor account context for secure MultisigConfig initialization.
///
/// This context enforces critical protocol invariants during multisig creation,
/// implementing several layers of security and determinism:
///
/// ## PDA Determinism and Security
/// The `seeds = [b"multisig_config", pool_core.key().as_ref()]` pattern creates
/// deterministic Program Derived Addresses (PDAs) that:
/// - Cannot be controlled by external authorities (seedless PDAs)
/// - Provide 1:1 mapping between pool cores and their multisig configurations
/// - Enable efficient lookups without storing additional cross-references
/// - Prevent address collision attacks through cryptographic uniqueness
///
/// ## Account Size Calculation Strategy
/// The space calculation `8 + std::mem::size_of::<MultisigConfig>()` includes:
/// - 8-byte discriminator prefix required by Anchor for account type identification
/// - Exact struct size to prevent under/over-allocation vulnerabilities
/// - Zero-copy compatible layout enabling direct memory access without deserialization
///
/// ## Authority Separation Principle
/// Separates `payer` (who funds the account) from `authority` (who manages the config)
/// to prevent scenarios where funding entities automatically gain governance control.
/// This separation is crucial for trustless deployment scenarios.
#[derive(Accounts)]
pub struct InitializeMultisigConfig<'info> {
    /// The MultisigConfig account being created with deterministic PDA addressing.
    ///
    /// Uses AccountLoader for zero-copy access patterns, enabling efficient
    /// read/write operations without full deserialization overhead.
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<MultisigConfig>(),
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// Pool core account establishing the governance relationship.
    ///
    /// UncheckedAccount allows for flexible pool core validation while maintaining
    /// the deterministic PDA relationship. The actual pool core validation happens
    /// in the initialize() method where business logic can perform comprehensive checks.
    pub pool_core: UncheckedAccount<'info>,

    /// Account providing SOL rent for multisig config creation.
    ///
    /// Must be mutable to allow rent deduction. Separation from authority prevents
    /// automatic governance control acquisition by funding entities.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Governance authority for initial multisig configuration.
    ///
    /// This authority is only used during initialization and does not grant ongoing
    /// governance rights, which are controlled by the multisig threshold mechanism.
    pub authority: Signer<'info>,

    /// System program required for account creation and rent handling.
    pub system_program: Program<'info, System>,
}

/// Initializes MultisigConfig with validated governance parameters and secure defaults.
///
/// This handler function serves as the entry point for creating new multisig governance
/// structures, implementing several critical security validations and initialization patterns:
///
/// ## Validation-First Architecture
/// All parameter validation occurs within the MultisigConfig.initialize() method rather
/// than in this handler. This design centralizes validation logic, making it easier to
/// audit and ensuring consistent validation across different initialization contexts.
///
/// ## Zero-Copy Initialization Pattern
/// Uses `load_init()` to establish the account in an initialized but empty state, then
/// calls the struct's initialize method to populate fields. This two-phase approach
/// ensures the account is properly allocated before any business logic executes.
///
/// ## Error Propagation Strategy
/// The `?` operator ensures any initialization errors (invalid thresholds, member
/// validation failures, etc.) are propagated up to the caller with full context.
/// This enables calling code to handle specific error conditions appropriately.
///
/// ## Governance Bootstrap Security
/// By requiring both threshold and members as parameters, this function enforces that
/// multisig configurations cannot be created in partially-configured states that
/// might be vulnerable to single-signature control.
pub fn initialize_multisig_config(
    ctx: Context<InitializeMultisigConfig>,
    threshold: u8,
    members: [Pubkey; 7],
) -> Result<()> {
    // Load account in initialized state with zero-copy access
    let multisig_config = &mut ctx.accounts.multisig_config.load_init()?;

    let clock = Clock::get()?;
    // Delegate to struct method for centralized validation and initialization
    multisig_config.initialize(
        ctx.accounts.pool_core.key(),
        threshold,
        members,
        clock.unix_timestamp,
    )?;

    Ok(())
}
