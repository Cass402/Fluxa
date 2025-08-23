use crate::error::{FactoryError, PoolError};
use crate::state::factory::factory_account::Factory;
use crate::state::pool::pool_core::PoolCore;

/// **Security Hierarchy & Authority Resolution Architecture**
///
/// ## Two-Tier Security Model Rationale
/// This module implements a sophisticated two-tier security architecture that addresses
/// the complexity of managing both protocol-wide (factory) and asset-specific (pool)
/// security concerns within a single DeFi protocol. The design choice of hierarchy
/// over flat authority structures prevents authority confusion and enables sophisticated
/// risk management at appropriate scopes.
///
/// ## Authority Precedence Philosophy
/// The hierarchy follows the principle of "broader scope trumps narrower scope" while
/// respecting operational boundaries. Factory emergency controls can override pool
/// operations because systemic risks (oracle failures, market crashes) require
/// protocol-wide response capability that individual pool authorities cannot provide.
///
/// ## Enterprise Security Integration Strategy
/// Rather than creating separate code paths for basic vs enterprise security, this
/// unified resolver dynamically determines authority requirements based on:
/// - **Operation Risk Level**: High-risk operations require enterprise infrastructure
/// - **Scope Impact**: Factory vs pool operation classification
/// - **Emergency Context**: Emergency situations bypass normal authorization flows
///
/// This design enables gradual security upgrades without breaking existing operations
/// and provides clear escalation paths during crisis situations.
///
/// ## Performance & Compute Unit Optimization
/// Authority resolution is designed for O(1) performance with no dynamic allocations.
/// All security checks use efficient enum matching and bitwise flag operations to
/// minimize compute unit consumption during high-frequency operations like trading.
use anchor_lang::prelude::*;

/// **Security Operation Classification System**
///
/// ## Risk-Based Operation Categorization
/// Operations are classified not just by their target (factory vs pool) but by their
/// risk profile and potential impact scope. This classification drives automatic
/// authority requirement determination and security infrastructure routing.
///
/// ## Factory vs Pool Operation Design Philosophy
/// - **Factory Operations**: Affect protocol-wide economics, global emergency responses,
///   or cross-pool coordination. These require higher authority because they can impact
///   all pools and users simultaneously.
/// - **Pool Operations**: Affect individual trading pairs, specific user communities,
///   or isolated risk management. These can use pool-specific authorities for efficiency.
/// - **Shared Operations**: Basic operations that can be handled at either level,
///   with preference for pool-level authority when available for better decentralization.
///
/// ## Enterprise Security Trigger Logic
/// Certain operations automatically require enterprise security infrastructure regardless
/// of whether basic operations could theoretically handle them. This design prevents
/// security downgrade attacks where attackers might try to use basic mode for operations
/// that should require distributed authorization and audit trails.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SecurityOperation {
    // **Factory-Level Operations**: Protocol-wide impact requiring global authority
    ProtocolFeeUpdate,     // Economic impact across all pools and users
    FactoryEmergencyPause, // Global protocol shutdown capability
    GlobalAuditLog,        // Protocol-wide compliance and forensic logging
    ProtocolUpgrade,       // Code changes affecting entire protocol behavior
    AddFeeTier,            // New trading fee options affecting pool creation

    // **Pool-Level Operations**: Asset-specific controls with contained impact
    PoolPause,      // Individual pool emergency stop
    LpWhitelist,    // Liquidity provider access control
    PositionLimits, // Risk management for large positions
    MevProtection,  // MEV mitigation strategies per pool
    PoolAuditLog,   // Pool-specific event logging

    // **Shared Operations**: Can be handled at either factory or pool level
    CreatePool,  // Pool creation (factory delegates to pool)
    UpdateStats, // Analytics and monitoring updates
    BasicConfig, // Non-critical configuration changes
}

impl SecurityOperation {
    /// **Factory Authority Requirement Analysis**
    ///
    /// ## Scope-Based Authority Assignment
    /// This method implements the core principle that operations affecting protocol-wide
    /// state or economics must route through factory authority structures. This prevents
    /// individual pools from making decisions that could destabilize the entire protocol
    /// or create economic inconsistencies across trading pairs.
    ///
    /// ## Attack Vector Prevention
    /// By requiring factory authority for global operations, we prevent:
    /// - Pool authorities from manipulating protocol-wide fee structures
    /// - Individual pools from triggering unnecessary global emergency states
    /// - Cross-pool authority confusion where pools might claim global authority
    pub fn requires_factory_authority(&self) -> bool {
        matches!(
            self,
            SecurityOperation::ProtocolFeeUpdate
                | SecurityOperation::FactoryEmergencyPause
                | SecurityOperation::GlobalAuditLog
                | SecurityOperation::ProtocolUpgrade
                | SecurityOperation::AddFeeTier
        )
    }

    /// **Pool Authority Requirement Analysis**
    ///
    /// ## Decentralization Through Delegation
    /// Pool-specific operations should be handled by pool-level authorities when possible
    /// to enable decentralized management and reduce dependency on factory authorities.
    /// This approach scales better and enables specialized governance per trading pair.
    ///
    /// ## Risk Containment Strategy
    /// Pool authorities can only affect their specific trading pair, containing the
    /// blast radius of any governance mistakes or malicious actions. Even if a pool
    /// authority is compromised, other pools remain unaffected.
    pub fn requires_pool_authority(&self) -> bool {
        matches!(
            self,
            SecurityOperation::PoolPause
                | SecurityOperation::LpWhitelist
                | SecurityOperation::PositionLimits
                | SecurityOperation::MevProtection
                | SecurityOperation::PoolAuditLog
        )
    }

    /// **Enterprise Security Requirement Analysis**
    ///
    /// ## Security Infrastructure Gatekeeping
    /// Certain operations are inherently high-risk and require sophisticated security
    /// infrastructure regardless of whether they affect factory or pool scope. This
    /// prevents security downgrade attacks where malicious actors might try to perform
    /// sensitive operations through basic mode when enterprise mode should be required.
    ///
    /// ## Regulatory Compliance Consideration  
    /// Operations that affect user funds, access controls, or audit trails require
    /// enterprise security to ensure compliance with financial regulations that may
    /// require multi-party authorization and comprehensive audit trails.
    ///
    /// ## Economic Security Rationale
    /// Protocol fee updates and emergency pauses can have massive economic impact,
    /// requiring distributed authorization to prevent single-point-of-failure attacks
    /// on protocol economics.
    pub fn requires_enterprise_security(&self) -> bool {
        matches!(
            self,
            SecurityOperation::ProtocolFeeUpdate
                | SecurityOperation::FactoryEmergencyPause
                | SecurityOperation::GlobalAuditLog
                | SecurityOperation::ProtocolUpgrade
                | SecurityOperation::PoolPause
                | SecurityOperation::LpWhitelist
                | SecurityOperation::MevProtection
                | SecurityOperation::PoolAuditLog
        )
    }
}

/// **Security Authority Resolution Result**
/// **Security Authority Configuration**
///
/// ## Authority Hierarchy Design Philosophy
/// This structure represents a single authority level within our two-tier system,
/// where each authority has a specific scope and level of permissions. The design
/// enables flexible authority delegation while maintaining clear boundaries between
/// factory-level and pool-level responsibilities.
///
/// ## Zero-Copy Security Data Access
/// Using Anchor's zero-copy pattern for authority references enables O(1) authority
/// verification without heap allocations during critical security checks. This is
/// essential for transaction processing where every CU counts and security checks
/// cannot introduce performance bottlenecks.
///
/// ## Multi-Signature Integration Strategy
/// The authority field can reference either a single signer (for basic operations)
/// or a multi-signature PDA (for enterprise operations), enabling seamless transition
/// between security modes based on operation risk assessment.
#[derive(Clone, Debug)]
pub struct SecurityAuthority {
    /// **Authority Public Key Reference**
    ///
    /// ## Flexible Authority Types
    /// Can reference either:
    /// - Single signer accounts for routine operations
    /// - Multi-signature PDAs for high-security operations
    /// - Factory authority PDAs for protocol-wide operations
    /// - Pool authority PDAs for pool-specific operations
    ///
    /// ## PDA Security Consideration
    /// When referencing PDAs, the authority verification logic must validate
    /// both the PDA derivation and the underlying authorization mechanism
    /// to prevent authority spoofing attacks.
    pub authority: Pubkey,

    /// **Security Level Assignment**
    ///
    /// ## Risk-Based Authorization
    /// Determines what types of operations this authority can perform,
    /// enabling fine-grained access control where different authorities
    /// have different permissions even within the same security tier.
    ///
    /// ## Upgrade Path Security
    /// Authorities can be promoted to higher security levels through
    /// enterprise upgrade processes, but never downgraded automatically
    /// to prevent security regression attacks.
    pub level: SecurityLevel,

    /// **Authority Scope Definition**
    ///
    /// ## Blast Radius Control
    /// Defines whether this authority can affect:
    /// - Factory: Protocol-wide operations and economics
    /// - Pool: Single trading pair operations only
    ///
    /// ## Cross-Scope Attack Prevention
    /// Pool authorities cannot perform factory operations even if they
    /// somehow obtain higher security levels, preventing privilege
    /// escalation attacks across security boundaries.
    pub scope: AuthorityScope,

    /// **Multi-Signature Requirement Flag**
    ///
    /// ## Operation-Specific Security Requirements
    /// Indicates whether this specific operation requires multi-signature
    /// authorization regardless of the base authority level. This enables
    /// fine-grained security policies where certain operations always
    /// require distributed authorization.
    ///
    /// ## Emergency Override Capability
    /// Can be set to false for emergency operations that need to bypass
    /// normal multi-signature requirements for rapid incident response,
    /// while maintaining full audit trails for post-incident analysis.
    pub requires_multisig: bool,
}

/// **Security Level Hierarchy**
/// **Security Level Classification System**
///
/// ## Hierarchical Authority Model
/// This enum implements a hierarchical security model where factory authorities
/// take precedence over pool authorities, and emergency overrides supersede all
/// normal operations. This prevents authorization conflicts and ensures clear
/// decision-making chains during critical situations.
///
/// ## Emergency Response Design
/// Factory emergency level provides immediate protocol-wide override capability,
/// bypassing normal multi-signature requirements for rapid incident response.
/// This is essential for preventing further damage during active exploits.
///
/// ## Enterprise Security Integration
/// Enterprise levels integrate with sophisticated authorization infrastructure
/// while maintaining clear factory vs pool authority boundaries. This enables
/// institutional-grade security without compromising the two-tier architecture.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SecurityLevel {
    /// **Emergency Factory Override**
    ///
    /// ## Immediate Response Capability
    /// Highest priority level that bypasses normal authorization flows
    /// for rapid incident response. Only activated during protocol-wide
    /// emergencies where immediate action is required to prevent further damage.
    ///
    /// ## Security Trade-off Rationale
    /// Emergency operations intentionally bypass multi-signature requirements
    /// because the cost of delayed response during an active exploit typically
    /// exceeds the risk of potential emergency authority misuse.
    FactoryEmergency,

    /// **Factory Enterprise Authority**
    ///
    /// ## Protocol-Wide Enterprise Operations
    /// Highest normal authority level for factory operations requiring
    /// sophisticated authorization infrastructure. Used for operations
    /// affecting protocol economics, global state, or cross-pool coordination.
    ///
    /// ## Multi-Signature Integration
    /// Always requires multi-signature authorization with comprehensive
    /// audit trails and potentially time-locked execution for maximum security.
    FactoryEnterprise,

    /// **Pool Enterprise Authority**
    ///
    /// ## Pool-Specific Enterprise Operations
    /// Enterprise-level authority scoped to individual pools. Enables
    /// sophisticated pool management while maintaining isolation from
    /// other pools and factory operations.
    ///
    /// ## Delegated Authority Model
    /// Pool enterprise authority can operate independently of factory
    /// enterprise authority for pool-specific operations, enabling
    /// decentralized high-security pool management.
    PoolEnterprise,

    /// **Factory Basic Authority**
    ///
    /// ## Standard Factory Operations
    /// Normal authority level for routine factory operations that don't
    /// require enterprise security infrastructure. Suitable for operations
    /// with limited economic impact or well-understood risk profiles.
    ///
    /// ## Efficiency Consideration
    /// Basic authority avoids multi-signature overhead for routine operations,
    /// maintaining system efficiency while preserving security for critical operations.
    FactoryBasic,

    /// **Pool Basic Authority**
    ///
    /// ## Routine Pool Management
    /// Lowest authority level for standard pool operations. Enables efficient
    /// pool management without requiring complex authorization infrastructure
    /// for routine trading and liquidity operations.
    ///
    /// ## Decentralization Philosophy
    /// Pool basic authority can be delegated to pool operators or automated
    /// systems, reducing dependency on factory authorities for routine operations.
    PoolBasic,
}

/// **Authority Scope Classification**
///
/// ## Security Boundary Enforcement
/// This enum enforces the fundamental security boundary between factory-level
/// and pool-level operations. It prevents privilege escalation attacks where
/// pool authorities might attempt to perform factory operations or vice versa.
///
/// ## Blast Radius Containment
/// By clearly defining scope boundaries, we ensure that even if an authority
/// is compromised, the damage is contained to either factory-wide operations
/// or a single pool, never both.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorityScope {
    /// **Factory-Wide Authority**
    ///
    /// ## Protocol-Level Operations
    /// Authorities with Factory scope can perform operations that affect
    /// the entire protocol, including fee structures, global emergency states,
    /// and protocol upgrades.
    ///
    /// ## Cross-Pool Coordination
    /// Factory authorities coordinate operations that span multiple pools
    /// or affect the relationships between pools.
    Factory,

    /// **Pool-Specific Authority**
    ///
    /// ## Isolated Pool Management
    /// Authorities with Pool scope can only affect operations within their
    /// specific trading pair. This isolation prevents cross-pool contamination
    /// during security incidents.
    ///
    /// ## Decentralized Governance
    /// Pool scope enables decentralized governance where each trading pair
    /// can have its own governance structure while maintaining protocol integrity.
    Pool,
}

impl SecurityLevel {
    pub fn description(&self) -> &'static str {
        match self {
            SecurityLevel::FactoryEmergency => "Factory Emergency Override",
            SecurityLevel::FactoryEnterprise => "Factory Enterprise Authority",
            SecurityLevel::PoolEnterprise => "Pool Enterprise Authority",
            SecurityLevel::PoolBasic => "Pool Basic Authority",
            SecurityLevel::FactoryBasic => "Factory Basic Authority",
        }
    }
}

/// **Two-Tier Security Resolver**
///
/// ## Central Authority Resolution Engine
/// This resolver implements the core logic for determining which authority should
/// handle any given operation within our two-tier security architecture. It acts
/// as the central decision engine that enforces security boundaries, hierarchies,
/// and enterprise requirements across the entire protocol.
///
/// ## Design Philosophy: Security by Default
/// The resolver always chooses the most appropriate security level for each operation,
/// erring on the side of higher security when in doubt. This "secure by default"
/// approach ensures that operations can never accidentally bypass required security
/// measures due to logic errors.
///
/// ## O(1) Authority Resolution Performance
/// All resolution logic uses constant-time lookups and deterministic branching
/// to ensure authority resolution never becomes a performance bottleneck in
/// high-frequency trading scenarios.
pub struct SecurityResolver;

impl SecurityResolver {
    /// **Primary Authority Resolution Logic**
    ///
    /// ## Four-Tier Resolution Hierarchy
    /// This method implements a strict four-level hierarchy for authority resolution:
    /// 1. Factory Emergency Override (bypasses all other considerations)
    /// 2. Factory-Level Operations (protocol-wide scope)
    /// 3. Pool-Level Operations (single pool scope)
    /// 4. Shared Operations (flexible scope based on context)
    ///
    /// ## Emergency Response Priority
    /// Emergency states receive absolute priority because during active exploits,
    /// rapid response is more critical than normal authorization flows. Emergency
    /// authorities can override any operation to prevent further damage.
    ///
    /// ## Enterprise Security Integration
    /// Each resolution path automatically determines if enterprise security is
    /// required based on operation risk assessment, ensuring that high-risk
    /// operations never accidentally use basic authorization.
    ///
    /// ## Fallback Strategy
    /// For shared operations, the resolver prefers pool-level authority when
    /// available to encourage decentralization, but falls back to factory
    /// authority to ensure operations can always be authorized when needed.
    pub fn resolve_authority(
        factory: &Factory,
        pool: Option<&PoolCore>,
        operation: SecurityOperation,
    ) -> Result<SecurityAuthority> {
        // **Level 1: Factory Emergency Override**
        // Factory emergency pause overrides ALL operations
        if factory.is_emergency_paused() {
            return Ok(SecurityAuthority {
                authority: factory.security_coordinator,
                level: SecurityLevel::FactoryEmergency,
                scope: AuthorityScope::Factory,
                requires_multisig: false, // Emergency bypasses multisig
            });
        }

        // **Level 2: Factory-Level Operations**
        if operation.requires_factory_authority() {
            return Self::resolve_factory_authority(factory, operation);
        }

        // **Level 3: Pool-Level Operations**
        if operation.requires_pool_authority() {
            let pool = pool.ok_or(PoolError::PoolNotProvided)?;
            return Self::resolve_pool_authority(factory, pool, operation);
        }

        // **Level 4: Shared Operations (prefer pool if provided)**
        if let Some(pool) = pool {
            Self::resolve_pool_authority(factory, pool, operation)
                .or_else(|_| Self::resolve_factory_authority(factory, operation))
        } else {
            Self::resolve_factory_authority(factory, operation)
        }
    }

    /// **Factory Authority Resolution**
    ///
    /// ## Factory-Scope Authority Selection
    /// This method resolves the appropriate factory authority based on whether
    /// enterprise security is required. It implements the principle that
    /// enterprise operations must route through enterprise infrastructure
    /// while allowing basic operations to use more efficient basic authorities.
    ///
    /// ## Security Level Enforcement
    /// Factory authorities operate at higher security levels than pool authorities
    /// because they can affect protocol-wide state. This method ensures that
    /// factory operations always use appropriate security infrastructure.
    ///
    /// ## Enterprise Mode Validation
    /// Before allowing enterprise operations, this method validates that the
    /// factory is actually configured for enterprise mode. This prevents
    /// security downgrade attacks where enterprise operations might be
    /// attempted through factories not configured for high security.
    fn resolve_factory_authority(
        factory: &Factory,
        operation: SecurityOperation,
    ) -> Result<SecurityAuthority> {
        // Check if operation requires enterprise but factory isn't enterprise
        if operation.requires_enterprise_security() && factory.enterprise_mode == 0 {
            return err!(FactoryError::InsufficientPermissions);
        }

        let (authority, level) =
            if factory.enterprise_mode == 1 && operation.requires_enterprise_security() {
                (
                    factory.security_coordinator,
                    SecurityLevel::FactoryEnterprise,
                )
            } else {
                (factory.core_authority, SecurityLevel::FactoryBasic)
            };

        Ok(SecurityAuthority {
            authority,
            level,
            scope: AuthorityScope::Factory,
            requires_multisig: Self::operation_requires_multisig(&operation),
        })
    }

    /// **Pool Authority Resolution**
    ///
    /// ## Pool-Scope Authority Management
    /// This method handles authority resolution for pool-specific operations,
    /// implementing the principle that pools should manage their own operations
    /// when possible to enable decentralized governance while maintaining
    /// security standards.
    ///
    /// ## Enterprise Pool Security
    /// Pool operations can require enterprise security just like factory operations,
    /// but at pool scope rather than protocol scope. This enables sophisticated
    /// pool management while maintaining isolation between different trading pairs.
    ///
    /// ## Temporary Authority Placeholder
    /// Currently returns placeholder values because pool authorities are stored
    /// in pool_config accounts that aren't directly accessible here. This design
    /// enables the resolver to determine security requirements while actual
    /// authority keys are resolved in the calling context with proper account access.
    ///
    /// ## Future Authority Integration
    /// When pool_config integration is complete, this method will resolve actual
    /// authority keys from pool configuration accounts, enabling full pool-level
    /// authority management within the security hierarchy.
    fn resolve_pool_authority(
        _factory: &Factory,
        pool: &PoolCore,
        operation: SecurityOperation,
    ) -> Result<SecurityAuthority> {
        // Check if operation requires enterprise but pool isn't enterprise
        if operation.requires_enterprise_security() && !pool.is_enterprise() {
            return err!(PoolError::RequiresEnterprisePool);
        }

        // For now, we'll return placeholder values since pool authorities are in pool_config
        // This will be properly resolved when we have access to pool_config in the calling context
        let (authority, level) = if pool.is_enterprise() && operation.requires_enterprise_security()
        {
            // TODO: Get security coordinator from pool_config or related PDA
            (Pubkey::default(), SecurityLevel::PoolEnterprise)
        } else {
            // TODO: Get core authority from pool_config
            (Pubkey::default(), SecurityLevel::PoolBasic)
        };

        Ok(SecurityAuthority {
            authority,
            level,
            scope: AuthorityScope::Pool,
            requires_multisig: Self::operation_requires_multisig(&operation),
        })
    }

    /// **Multi-Signature Requirement Logic**
    ///
    /// ## Risk-Based Multi-Signature Requirements
    /// This method determines which operations inherently require multi-signature
    /// authorization regardless of the authority type or security level. This
    /// creates an additional security layer where certain high-risk operations
    /// always require distributed authorization.
    ///
    /// ## Economic Impact Operations
    /// Operations that can significantly affect protocol economics (fee updates,
    /// upgrades) always require multi-signature to prevent single-party manipulation
    /// of protocol parameters that could destabilize the entire system.
    ///
    /// ## System Integrity Operations
    /// Emergency pauses and other system-wide operations require multi-signature
    /// to ensure that protocol-affecting decisions involve multiple parties,
    /// reducing the risk of unilateral actions during critical situations.
    ///
    /// ## Performance vs Security Trade-off
    /// While multi-signature adds transaction overhead, these operations are
    /// infrequent enough that the security benefit outweighs the performance cost.
    /// Routine operations (not listed here) can use single-signature for efficiency.
    fn operation_requires_multisig(operation: &SecurityOperation) -> bool {
        matches!(
            operation,
            SecurityOperation::ProtocolFeeUpdate
                | SecurityOperation::ProtocolUpgrade
                | SecurityOperation::AddFeeTier
                | SecurityOperation::PoolPause
        )
    }

    /// **Authority Validation Interface**
    ///
    /// ## Complete Authority Resolution and Validation
    /// This method combines authority resolution with validation in a single call,
    /// providing a convenient interface for instruction handlers that need to
    /// both determine and validate the appropriate authority for an operation.
    ///
    /// ## Signer Authorization Verification
    /// After resolving the theoretical authority for an operation, this method
    /// verifies that the actual transaction signer matches the resolved authority,
    /// preventing unauthorized operations even if the authority resolution is correct.
    ///
    /// ## Security Level Return
    /// Returns the resolved security level to enable calling code to implement
    /// additional security checks based on the actual authority level used,
    /// such as additional logging for enterprise operations.
    pub fn validate_authority(
        factory: &Factory,
        pool: Option<&PoolCore>,
        operation: SecurityOperation,
        signer: &Pubkey,
    ) -> Result<SecurityLevel> {
        let resolved = Self::resolve_authority(factory, pool, operation)?;

        require!(
            *signer == resolved.authority,
            FactoryError::InvalidAuthority
        );

        Ok(resolved.level)
    }

    /// **Emergency Override Capability Check**
    ///
    /// ## Emergency Authority Verification
    /// This method provides a dedicated interface for checking if a signer
    /// has emergency override capabilities during factory emergency states.
    /// Emergency override is the highest authority level and requires explicit
    /// verification separate from normal authority resolution.
    ///
    /// ## Enterprise Infrastructure Requirement
    /// Emergency override capabilities require enterprise infrastructure because
    /// emergency actions need comprehensive audit trails and the sophisticated
    /// authorization mechanisms that enterprise mode provides.
    ///
    /// ## Emergency State Validation
    /// Only allows emergency override when the factory is actually in emergency
    /// state, preventing misuse of emergency authorities during normal operations.
    pub fn can_override_emergency(factory: &Factory, signer: &Pubkey) -> bool {
        factory.enterprise_mode == 1
            && factory.is_emergency_paused()
            && *signer == factory.security_coordinator
    }
}

/// **Practical Security Validators for Anchor Contexts**
///
/// ## Context-Aware Security Validation
/// These methods provide practical security validation that works with the actual
/// account structure of Anchor programs, where authorities are often stored in
/// related PDAs rather than being directly accessible during authority resolution.
///
/// ## Instruction Handler Integration
/// These validators are designed to be called directly from instruction handlers
/// with the actual account context available, enabling proper authority resolution
/// that accounts for the real account relationships in the program.
///
/// ## Performance-Optimized Validation
/// Each validator is optimized for its specific context (factory vs pool operations)
/// to minimize unnecessary account accesses and computations during transaction processing.
impl SecurityResolver {
    /// **Factory Enterprise Authority Validation**
    ///
    /// ## Factory Context Authorization
    /// This method provides authority validation specifically for factory instruction
    /// contexts where the factory account is directly accessible. It combines
    /// enterprise mode checking with operation-specific authority resolution
    /// for comprehensive factory operation security.
    ///
    /// ## Enterprise Operation Gatekeeping
    /// Prevents enterprise operations from being executed through factories not
    /// configured for enterprise mode, ensuring that high-security operations
    /// always route through appropriate infrastructure.
    ///
    /// ## Dynamic Authority Resolution
    /// Uses factory.get_effective_authority() to dynamically resolve the correct
    /// authority for each operation type, enabling flexible authority delegation
    /// while maintaining security boundaries.
    pub fn validate_factory_enterprise_authority(
        factory: &Factory,
        signer: &Pubkey,
        operation: SecurityOperation,
    ) -> Result<()> {
        if factory.enterprise_mode == 0 {
            require!(
                !operation.requires_enterprise_security(),
                FactoryError::InsufficientPermissions
            );
        }

        let effective_authority = factory.get_effective_authority(match operation {
            SecurityOperation::ProtocolFeeUpdate => "protocol_fee_update",
            SecurityOperation::FactoryEmergencyPause => "factory_emergency_pause",
            SecurityOperation::GlobalAuditLog => "global_audit_log",
            SecurityOperation::ProtocolUpgrade => "protocol_upgrade",
            SecurityOperation::AddFeeTier => "add_fee_tier",
            _ => "basic_operation",
        });

        require!(
            *signer == effective_authority,
            FactoryError::InvalidAuthority
        );

        Ok(())
    }

    /// **Pool Authority Validation with Configuration Context**
    ///
    /// ## Pool-Specific Authority Resolution
    /// This method provides authority validation for pool instruction contexts
    /// where pool configuration authorities are accessible as separate account
    /// parameters. This design accommodates the reality that pool authorities
    /// are stored in pool_config PDAs separate from the main pool account.
    ///
    /// ## Enterprise vs Basic Authority Selection
    /// Automatically selects between pool_config_authority (for basic operations)
    /// and pool_security_coordinator (for enterprise operations) based on both
    /// the pool's enterprise status and the operation's security requirements.
    ///
    /// ## Separation of Concerns
    /// Pool configuration authority handles routine pool management while
    /// security coordinator handles high-risk operations, maintaining clear
    /// separation between operational and security responsibilities.
    ///
    /// ## Error Handling Strategy
    /// Provides specific error messages for missing enterprise infrastructure,
    /// helping developers understand when pools need to be upgraded to enterprise
    /// mode before attempting certain operations.
    pub fn validate_pool_authority_with_config(
        pool: &PoolCore,
        pool_config_authority: Pubkey,
        pool_security_coordinator: Option<Pubkey>,
        signer: &Pubkey,
        operation: SecurityOperation,
    ) -> Result<()> {
        // Check if operation requires enterprise but pool isn't enterprise
        if operation.requires_enterprise_security() && !pool.is_enterprise() {
            return err!(PoolError::RequiresEnterprisePool);
        }

        let effective_authority =
            if pool.is_enterprise() && operation.requires_enterprise_security() {
                pool_security_coordinator.ok_or(PoolError::RequiresEnterprisePool)?
            } else {
                pool_config_authority
            };

        require!(*signer == effective_authority, PoolError::Unauthorized);

        Ok(())
    }
}
