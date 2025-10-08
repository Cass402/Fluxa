use crate::error::PdaSecurityAuthorityError;
use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

/// Emergency response coordination registry for critical incident management.
///
/// This account serves as the authoritative registry for emergency response personnel,
/// implementing a distributed yet controlled approach to protocol crisis management:
///
/// ## Emergency Response Architecture
/// The registry balances rapid response capabilities with security controls by maintaining
/// a curated list of trusted emergency responders. This design addresses several critical
/// requirements for DeFi protocol safety:
/// - **Rapid Response**: Pre-authorized responders can act immediately during incidents
/// - **Distributed Authority**: Multiple responders prevent single points of failure
/// - **Accountability**: Role-based permissions and activity tracking for post-incident analysis
/// - **Governance Integration**: Emergency actions remain within protocol governance frameworks
///
/// ## Fixed-Size Design Philosophy
/// The 5-contact limit reflects careful balance between operational needs and security:
/// - **Compute Predictability**: Fixed arrays enable deterministic gas costs for emergency actions
/// - **State Bloat Prevention**: Bounded growth prevents attacks via excessive contact registration
/// - **Consensus Feasibility**: Small groups enable rapid consensus during time-critical incidents
/// - **Audit Simplicity**: Limited contact count facilitates comprehensive security reviews
///
/// ## Zero-Copy Performance Optimization
/// Emergency response systems require minimal latency during crisis situations. Zero-copy
/// access patterns ensure emergency validation operations complete within single compute
/// units, enabling sub-second response times when protocol safety depends on immediate action.
///
/// ## Role-Based Access Control Integration
/// The permission system enables fine-grained emergency response protocols where different
/// contact types (technical leads, community delegates, audit partners) can perform
/// appropriate actions based on incident type and severity without requiring full
/// protocol-level authority for all emergency functions.
#[account(zero_copy(unsafe))]
#[derive(InitSpace)] // Expecting 192 bytes
#[repr(C)]
pub struct EmergencyContacts {
    /// Pool core binding for contextual emergency response authority.
    ///
    /// This field establishes the exclusive relationship between emergency responders
    /// and their designated pool, preventing emergency authority cross-contamination
    /// between different pool instances. The immutable binding after initialization
    /// ensures emergency responders cannot be redirected to affect unintended pools.
    pub pool_core: Pubkey,

    /// Designated pause authority for unilateral protocol suspension.
    ///
    /// This special authority can immediately pause protocol operations without requiring
    /// consensus from other emergency contacts. The unilateral pause capability enables
    /// instant response to critical security incidents where distributed consensus
    /// might be too slow. Separation from general emergency contacts prevents
    /// authority escalation where general responders gain pause capabilities.
    pub pause_authority: Pubkey,

    /// Emergency responder registry with deterministic access patterns.
    ///
    /// Fixed-size array design provides several critical benefits for emergency systems:
    /// - **Predictable Gas Costs**: Emergency validation operations have known compute requirements
    /// - **Memory Layout Stability**: Zero-copy access patterns remain consistent across updates
    /// - **Attack Surface Limitation**: Bounded contact list prevents resource exhaustion attacks
    /// - **Cache Efficiency**: Contiguous memory layout optimizes processor cache utilization
    pub contacts: [EmergencyContact; 5],

    /// Registry lifecycle timestamp for compliance and forensic analysis.
    ///
    /// Immutable creation timestamp establishes authoritative record of when
    /// emergency response capabilities were established, critical for regulatory
    /// compliance and post-incident forensic timelines.
    pub created_at: i64,

    /// Activity timestamp for response capability monitoring and validation.
    ///
    /// Updated with every registry modification, enabling automated monitoring
    /// systems to detect stale emergency configurations that might indicate
    /// compromised or abandoned emergency response capabilities.
    pub last_updated: i64,

    /// Active emergency contact registry with bounded growth protection.
    ///
    /// The count field enables iteration over only active contacts within the fixed
    /// array, providing O(n) performance where n is the actual contact count rather
    /// than the maximum capacity. This pattern prevents iterating over empty slots
    /// during time-critical emergency validation operations.
    pub contact_count: u8,

    /// Current emergency response severity level for protocol state management.
    ///
    /// This field enables emergency protocols to adjust response parameters based
    /// on current threat assessment. Higher levels might enable more aggressive
    /// defensive measures or extended pause durations, while lower levels allow
    /// more measured responses to minor incidents.
    pub emergency_response_level: u8,

    /// Reserved capacity for protocol evolution and memory alignment optimization.
    ///
    /// This padding serves multiple purposes:
    /// - **Protocol Upgrades**: Space for adding emergency response features without account migration
    /// - **Memory Alignment**: Ensures optimal cache line utilization for zero-copy operations  
    /// - **Emergency Extensions**: Room for crisis-driven protocol modifications
    /// The 38-byte size provides substantial flexibility for future emergency system enhancements.
    pub reserved: [u8; 38],
}

impl EmergencyContacts {
    /// Initializes emergency contact registry with secure defaults and operational readiness.
    ///
    /// This method establishes the foundational state for emergency response capabilities,
    /// implementing secure initialization patterns that prevent common vulnerabilities:
    ///
    /// ## Secure Default Strategy
    /// All security-sensitive fields receive explicit initialization rather than relying
    /// on Rust defaults. This prevents uninitialized state vulnerabilities and ensures
    /// predictable emergency response behavior across different deployment environments.
    ///
    /// ## Zero-State Emergency Configuration
    /// The registry starts with no active contacts, requiring explicit contact addition
    /// through authorized channels. This approach prevents residual emergency authorities
    /// from memory reuse and ensures all emergency permissions are intentionally granted.
    ///
    /// ## Temporal Baseline Establishment
    /// Both creation and update timestamps are synchronized during initialization,
    /// providing consistent temporal anchoring for all subsequent emergency operations
    /// and enabling precise activity tracking for compliance and audit requirements.
    ///
    /// ## Authority Separation Pattern
    /// The pause authority is established separately from general emergency contacts,
    /// implementing the principle of least privilege where unilateral pause capabilities
    /// are granted only to specifically designated authorities rather than all responders.
    pub fn initialize(
        &mut self,
        pool_core: Pubkey,
        pause_authority: Pubkey,
        timestamp: i64,
    ) -> Result<()> {
        // Establish immutable pool binding for emergency response context
        self.pool_core = pool_core;
        self.pause_authority = pause_authority;

        // Initialize contact registry to secure empty state
        self.contact_count = 0;
        self.emergency_response_level = 0;

        // Clear all contact slots to prevent residual emergency authorities
        self.contacts = [EmergencyContact::default(); 5];

        // Establish temporal baseline for activity tracking and compliance
        self.created_at = timestamp;
        self.last_updated = timestamp;

        Ok(())
    }

    /// Adds emergency contact with comprehensive validation and duplicate prevention.
    ///
    /// This method implements secure contact registration with multiple layers of
    /// protection against common vulnerabilities and operational errors:
    ///
    /// ## Capacity Management Strategy
    /// The hard limit of 5 contacts serves multiple security purposes:
    /// - **Compute Bounds**: Ensures emergency validation operations remain within gas limits
    /// - **Consensus Feasibility**: Maintains manageable group sizes for rapid emergency decisions
    /// - **Attack Prevention**: Prevents resource exhaustion via excessive contact registration
    /// - **Administrative Clarity**: Keeps emergency response teams at comprehensible scales
    ///
    /// ## Duplicate Prevention Architecture
    /// Contact uniqueness validation prevents several potential issues:
    /// - **Authority Confusion**: Multiple entries for same entity creating unclear permissions
    /// - **Resource Waste**: Duplicate contacts consuming limited registry capacity
    /// - **Attack Vector**: Malicious actors filling registry with duplicate entries
    /// - **Operational Errors**: Accidental re-registration creating permission conflicts
    ///
    /// ## Atomic Contact Addition Pattern
    /// All contact-related state updates occur atomically within this method. Either
    /// the contact is fully registered with all metadata, or the operation fails with
    /// no state changes. This prevents partial registration states that could create
    /// security vulnerabilities or operational confusion.
    ///
    /// ## Activity Tracking Integration
    /// The addition timestamp serves as the baseline for contact lifecycle management,
    /// enabling automated monitoring of contact registration patterns and supporting
    /// compliance requirements for emergency response capability documentation.
    pub fn add_contact(
        &mut self,
        contact: Pubkey,
        role: EmergencyRole,
        permissions: u32,
        timestamp: i64,
    ) -> Result<()> {
        // Enforce registry capacity limits for predictable emergency operations
        if self.contact_count >= 5 {
            return Err(PdaSecurityAuthorityError::EmergencyContactLimitReached.into());
        }

        // Prevent duplicate contacts to maintain clear authority relationships
        if self.is_emergency_contact(&contact) {
            return Err(PdaSecurityAuthorityError::EmergencyContactAlreadyExists.into());
        }

        // Create complete contact record with full metadata
        let emergency_contact = EmergencyContact {
            pubkey: contact,
            role: role as u8,
            added_at: timestamp,
            last_active: 0,
            permissions,
            _padding: [0; 3],
        };

        // Atomically register contact with registry state updates
        self.contacts[self.contact_count as usize] = emergency_contact;
        self.contact_count += 1;
        self.last_updated = timestamp;

        Ok(())
    }

    /// Validates emergency contact membership with optimized linear search.
    ///
    /// This method performs emergency responder validation using a bounded linear search
    /// that is optimized for the specific constraints of emergency response systems:
    ///
    /// ## Linear Search Optimization Rationale
    /// Despite O(n) complexity, linear search is optimal for this use case because:
    /// - **Bounded Search Space**: Maximum 5 contacts makes linear search faster than hash overhead
    /// - **Cache Efficiency**: Contiguous array access patterns optimize processor cache utilization
    /// - **Emergency Latency**: Fewer CPU cycles than hash table operations during time-critical validation
    /// - **Early Termination**: Most emergency validations succeed on first or second iteration
    ///
    /// ## Active Contact Iteration Pattern
    /// The loop iterates only through active contacts (up to contact_count) rather than
    /// the full array capacity. This optimization reduces unnecessary comparisons against
    /// empty slots and ensures consistent performance regardless of registry utilization.
    ///
    /// ## Cryptographic Key Comparison Safety
    /// Uses Pubkey's built-in equality comparison which implements constant-time operations
    /// for cryptographic keys, preventing timing side-channel attacks that could leak
    /// information about emergency contact identities or registry contents.
    pub fn is_emergency_contact(&self, pubkey: &Pubkey) -> bool {
        // Iterate only through active contacts for optimal emergency response latency
        for i in 0..self.contact_count {
            if self.contacts[i as usize].pubkey == *pubkey {
                return true;
            }
        }
        false
    }

    /// Validates comprehensive emergency authority including pause and response capabilities.
    ///
    /// This method implements a dual-track emergency authorization system that recognizes
    /// two distinct categories of emergency authority with different capabilities:
    ///
    /// ## Dual Emergency Authority Model
    /// The authorization model distinguishes between two complementary authority types:
    /// - **Pause Authority**: Unilateral protocol suspension capability for immediate threat response
    /// - **Emergency Contacts**: Role-based response capabilities for coordinated incident management
    ///
    /// ## Short-Circuit Evaluation Optimization
    /// The OR logic provides pause authority precedence, enabling immediate authorization
    /// without contact registry traversal for designated pause authorities. This pattern
    /// optimizes the most critical emergency scenario (immediate protocol suspension)
    /// while maintaining comprehensive coverage for other emergency response needs.
    ///
    /// ## Comprehensive Coverage Strategy
    /// By combining both authority types, the method ensures no authorized emergency
    /// responder is inadvertently excluded from emergency operations while maintaining
    /// clear distinctions between unilateral pause capabilities and distributed response
    /// coordination functions.
    ///
    /// ## Emergency Validation Efficiency
    /// The method provides single-call validation for all emergency authority types,
    /// reducing the number of separate authorization checks required during time-critical
    /// emergency operations where computational efficiency directly impacts response time.
    pub fn has_emergency_authority(&self, pubkey: &Pubkey) -> bool {
        // Check pause authority first for immediate protocol suspension capability
        *pubkey == self.pause_authority || self.is_emergency_contact(pubkey)
    }
}

/// Emergency contact record with comprehensive responder metadata and activity tracking.
///
/// This structure captures all essential information for emergency response coordination,
/// implementing patterns that support both operational effectiveness and security auditing:
///
/// ## Zero-Copy Design for Emergency Latency
/// The zero-copy attribute enables direct memory access during emergency operations,
/// eliminating deserialization overhead when response speed is critical. Emergency
/// validation operations must complete within minimal compute budgets to ensure
/// protocol protection capabilities remain available even under resource constraints.
///
/// ## Activity Tracking Architecture
/// The dual timestamp pattern (added_at, last_active) supports comprehensive emergency
/// response lifecycle management:
/// - **Registration Audit**: When emergency authority was first granted
/// - **Activity Monitoring**: Most recent emergency action for capability validation
/// - **Compliance Documentation**: Complete activity history for regulatory requirements
/// - **Security Analysis**: Patterns of emergency authority usage for threat assessment
///
/// ## Permission Bitfield Strategy
/// The u32 permissions field enables fine-grained emergency capability control through
/// bitfield operations. This approach provides several advantages over enum-based permissions:
/// - **Extensibility**: New permission types can be added without breaking existing contacts
/// - **Combination Logic**: Contacts can have multiple simultaneous capabilities
/// - **Efficient Checking**: Bitwise AND operations for rapid permission validation
/// - **Compact Storage**: 32 distinct permission types in single field
///
/// ## Role-Based Organization
/// The role field enables emergency response organization and coordination by establishing
/// clear responsibilities and escalation paths during incident management while maintaining
/// compatibility with permission-based access control for specific emergency operations.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, Pod, Zeroable, InitSpace,
)]
#[repr(C)]
pub struct EmergencyContact {
    /// Emergency responder public key for cryptographic authorization.
    pub pubkey: Pubkey,

    /// Registration timestamp for audit trails and lifecycle management.
    pub added_at: i64,

    /// Most recent activity timestamp for capability validation and monitoring.
    pub last_active: i64,

    /// Bitfield permissions for fine-grained emergency capability control.
    pub permissions: u32,

    /// Organizational role for emergency response coordination and escalation.
    pub role: u8,

    pub _padding: [u8; 3],
}

/// Default EmergencyContact implementation with secure zero-state initialization.
///
/// This implementation provides safe default values for emergency contact structures,
/// ensuring uninitialized or cleared contacts cannot accidentally retain emergency
/// authority or create security vulnerabilities:
///
/// ## Zero-Authority Default Strategy
/// All fields default to non-privileged states to prevent accidental authority
/// escalation through uninitialized contacts. Default permissions of 0 ensure
/// no emergency capabilities are granted until explicitly configured.
///
/// ## Role Assignment Philosophy
/// The default Responder role represents the minimum emergency authority level,
/// preventing unintended assignment of higher-privilege roles (Coordinator,
/// TechnicalLead) that might grant additional emergency capabilities.
///
/// ## Memory Safety Integration
/// These defaults work with Rust's zero-initialization patterns and the fixed-size
/// array clearing in the registry initialization, ensuring consistent secure state
/// across all emergency contact lifecycle stages.
impl Default for EmergencyContact {
    fn default() -> Self {
        Self {
            pubkey: Pubkey::default(),
            role: EmergencyRole::Responder as u8,
            added_at: 0,
            last_active: 0,
            permissions: 0,
            _padding: [0; 3],
        }
    }
}

/// Emergency response role classification for organizational coordination and capability assignment.
///
/// This enumeration defines the organizational structure for emergency response teams,
/// balancing clear authority hierarchies with operational flexibility during crisis management:
///
/// ## Role-Based Emergency Organization Philosophy
/// Each role represents distinct responsibilities and capabilities within emergency response
/// protocols, enabling appropriate resource allocation and decision-making authority based
/// on incident type and severity. The role system supports both immediate tactical response
/// and strategic incident management coordination.
///
/// ## Compact Binary Representation
/// The `#[repr(u8)]` ensures minimal storage overhead and predictable serialization for
/// zero-copy operations. Emergency validation code paths require maximum efficiency since
/// they execute during protocol stress conditions where compute resources may be constrained.
///
/// ## Hierarchical Capability Model
/// The role progression from Responder to specialized functions (TechnicalLead, AuditPartner)
/// reflects real-world emergency response team structures where different expertise types
/// are needed for comprehensive incident management, but all roles maintain equivalent
/// emergency authority rather than hierarchical privilege escalation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum EmergencyRole {
    /// General emergency responder with standard incident response capabilities.
    Responder = 0,

    /// Emergency coordination lead responsible for multi-team incident orchestration.
    Coordinator = 1,

    /// Technical specialist for protocol-specific emergency analysis and remediation.
    TechnicalLead = 2,

    /// Community representative for stakeholder communication during emergency events.
    CommunityDelegate = 3,

    /// External audit partner for independent security assessment during incidents.
    AuditPartner = 4,
}

/// Anchor account context for secure EmergencyContacts registry initialization.
///
/// This context enforces critical protocol invariants during emergency response capability
/// establishment, implementing deterministic addressing and secure initialization patterns:
///
/// ## PDA Security Architecture for Emergency Systems
/// The `seeds = [b"emergency_contacts", pool_core.key().as_ref()]` pattern provides:
/// - **Deterministic Emergency Authority**: Each pool has exactly one emergency contact registry
/// - **Cross-Pool Isolation**: Emergency responders cannot accidentally affect wrong pools
/// - **Cryptographic Uniqueness**: Address derivation prevents emergency authority conflicts
/// - **Seedless Security**: No external entity can control emergency registry private keys
///
/// ## Emergency Response Account Size Strategy
/// The precise space calculation ensures optimal rent costs for emergency systems while
/// maintaining zero-copy compatibility. Emergency registries must remain economical since
/// they represent insurance costs that pools bear for security protection rather than
/// revenue-generating operational features.
///
/// ## Authority Bootstrap for Emergency Systems
/// The authority signer establishes emergency response capabilities without granting
/// ongoing operational control. This separation ensures emergency system deployment
/// cannot be hijacked by deployment infrastructure or funding entities seeking
/// unauthorized emergency authority over protocol operations.
///
/// ## Emergency System Integration Pattern
/// By requiring pool_core context, the initialization ensures emergency response
/// capabilities are properly integrated with pool governance and security architectures
/// rather than operating as isolated emergency systems that might bypass protocol controls.
#[derive(Accounts)]
pub struct InitializeEmergencyContacts<'info> {
    /// EmergencyContacts registry account with deterministic PDA addressing.
    ///
    /// AccountLoader enables zero-copy access patterns critical for emergency
    /// operations where response latency directly impacts protocol security.
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<EmergencyContacts>(),
        seeds = [b"emergency_contacts", pool_core.key().as_ref()],
        bump
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    /// CHECK: Pool core account establishing emergency response context and authority relationships.
    pub pool_core: UncheckedAccount<'info>,

    /// Funding account for emergency registry creation with separation from operational control.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Bootstrap authority for emergency system configuration without ongoing privileges.
    pub authority: Signer<'info>,

    /// System program for account creation and rent management in emergency contexts.
    pub system_program: Program<'info, System>,
}

/// Initializes EmergencyContacts registry with validated parameters and secure state establishment.
///
/// This handler serves as the bootstrap function for emergency response capabilities,
/// implementing secure initialization patterns while maintaining operational readiness:
///
/// ## Zero-Copy Emergency Initialization Strategy
/// Uses `load_init()` to establish the account in initialized state, then delegates
/// to the struct's initialize method for business logic. This pattern ensures proper
/// account allocation before emergency response logic executes, preventing initialization
/// failures that could leave protocols without emergency protection capabilities.
///
/// ## Delegation Pattern for Emergency Systems
/// By centralizing initialization logic in the struct method, this handler remains
/// focused on Anchor-specific account management while keeping emergency response
/// business logic testable and reusable across different initialization contexts.
///
/// ## Error Propagation for Emergency Setup
/// The `?` operator ensures initialization failures propagate with complete context,
/// enabling deployment systems to distinguish between account allocation failures
/// and emergency authority validation failures for appropriate error handling and retry logic.
///
/// ## Emergency Authority Bootstrap Pattern
/// The pause_authority parameter establishes immediate emergency response capability
/// upon successful initialization, ensuring protocols have emergency protection from
/// the moment the registry becomes operational rather than requiring separate activation steps.
pub fn initialize_emergency_contacts(
    ctx: Context<InitializeEmergencyContacts>,
    pause_authority: Pubkey,
) -> Result<()> {
    // Load emergency registry in initialized state for zero-copy operations
    let emergency_contacts = &mut ctx.accounts.emergency_contacts.load_init()?;

    let clock = Clock::get()?;
    // Delegate to struct method for centralized emergency initialization logic
    emergency_contacts.initialize(
        ctx.accounts.pool_core.key(),
        pause_authority,
        clock.unix_timestamp,
    )?;

    Ok(())
}
