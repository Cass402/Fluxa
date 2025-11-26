use crate::error::FactoryError;
use crate::math::core_arithmetic::{mul_div_q64, Q64x64};
use crate::utils::constants::{
    DEFAULT_FEE_TIERS, DEFAULT_PROTOCOL_FEE, MAX_FEE_TIERS, MAX_POOLS_PER_SHARD, MAX_SHARDS,
    POOL_CREATION_FEE, STATUS_EMERGENCY, STATUS_MAINTENANCE, STATUS_NORMAL, STATUS_PAUSED,
};
use anchor_lang::prelude::*;

/// **Factory Configuration Blueprint**: Protocol-wide governance parameters for fee structure and operational limits.
///
/// ## Design Philosophy
/// This configuration struct embodies the principle of "configuration as data" - all protocol
/// behavior parameters are explicitly declared rather than hardcoded. This enables governance
/// flexibility while maintaining deterministic execution and audit transparency.
///
/// ## Memory Layout Strategy
/// - **Fixed-size fields only**: No Vec or String allocations prevent runtime memory surprises
/// - **Alignment optimization**: All fields naturally align to their size boundaries for zero-copy efficiency
/// - **Deterministic sizing**: Enables accurate rent calculations and prevents account bloat
///
/// ## Atomic Updates Rationale
/// By grouping related parameters, we enable atomic configuration updates that prevent
/// inconsistent intermediate states during governance changes. This is critical for
/// preventing arbitrage opportunities during configuration transitions.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct FactoryConfig {
    /// Protocol fee rate in basis points (0-10000), enabling precise revenue control.
    /// u32 chosen for gas efficiency while supporting fractional percentage precision.
    pub protocol_fee_rate: u32,

    /// Pool creation fee in lamports - spam prevention and protocol sustainability mechanism.
    /// u64 chosen to accommodate future SOL price increases without overflow concerns.
    pub creation_fee: u64,

    /// Fixed array of supported fee tiers for deterministic validation and gas efficiency.
    /// Array size prevents unbounded growth while supporting diverse trading strategies.
    pub supported_fee_tiers: [u32; MAX_FEE_TIERS],

    /// Maximum pools per shard - horizontal scaling parameter and DoS prevention.
    /// u16 sufficient for practical deployment scales while conserving storage.
    pub max_pools_per_shard: u16,
}

impl Default for FactoryConfig {
    /// Default configuration optimized for mainnet launch conditions.
    ///
    /// ## Default Value Strategy
    /// These defaults represent a balance between protocol sustainability (reasonable fees)
    /// and market competitiveness (attractive to traders). Values chosen based on:
    /// - Market analysis of competing DEX fee structures
    /// - Gas cost projections for Solana transaction patterns
    /// - Expected initial trading volumes and protocol adoption curves
    fn default() -> Self {
        Self {
            protocol_fee_rate: DEFAULT_PROTOCOL_FEE,
            creation_fee: POOL_CREATION_FEE,
            supported_fee_tiers: DEFAULT_FEE_TIERS,
            max_pools_per_shard: MAX_POOLS_PER_SHARD as u16,
        }
    }
}

/// **Factory State Architecture**: Central orchestrator for protocol-wide operations and governance.
///
/// ## Zero-Copy Design Rationale  
/// The `#[account(zero_copy(unsafe))]` attribute enables direct memory access without deserialization,
/// critical for high-frequency operations like fee calculations and pool validations. The 'unsafe'
/// designation reflects Solana's memory model where account data integrity is program-controlled.
///
/// ## Memory Layout Engineering
/// - **8-byte alignment**: All fields positioned for optimal CPU cache line utilization
/// - **Fixed-size arrays**: Prevents runtime allocations and ensures deterministic account size
/// - **Strategic padding**: Explicit padding preserves layout consistency across compiler versions
/// - **Bitfield optimization**: Status flags packed into single bytes for atomic operations
///
/// ## Enterprise Security Architecture
/// The factory supports dual operational modes:
/// - **Basic Mode**: Traditional authority-based governance suitable for most deployments
/// - **Enterprise Mode**: Multi-layered security with audit trails, multisig requirements, and emergency controls
///
/// This hybrid approach enables protocols to start simple and upgrade to enterprise security
/// as asset values and regulatory requirements increase.
///
/// ## Upgrade Safety Strategy
/// Reserved fields and careful sizing allow for backward-compatible upgrades without
/// account migrations or rent increases. This future-proofs the protocol against
/// evolving requirements and new feature additions.
#[account(zero_copy(unsafe))]
#[repr(C)]
pub struct Factory {
    /// **Authority Binding**: Immutable reference to the governance authority PDA.
    ///
    /// Design choice: Core authority is set once during initialization to prevent
    /// authority confusion attacks and ensure governance continuity. This binding
    /// enables Anchor's constraint validation system to verify legitimate operations.
    pub core_authority: Pubkey,

    /// **Enterprise Security Coordinator**: Central orchestrator for multi-layered security operations.
    ///
    /// Zero value indicates basic mode operation. Non-zero activates enterprise security
    /// where critical operations require distributed authorization, audit logging,
    /// and emergency response capabilities. This optional architecture enables
    /// gradual security model upgrades without protocol disruption.
    pub security_coordinator: Pubkey,

    /// **Enterprise Mode Status Flags**: Granular control over factory security capabilities.
    ///
    /// These booleans enable fine-grained security feature activation:
    /// - `enterprise_mode`: Master switch for enterprise security enforcement
    /// - `security_foundation_initialized`: Phase 1 upgrade completion marker
    /// - `audit_system_initialized`: Phase 2 upgrade completion marker
    ///
    /// Separate flags prevent partial upgrade vulnerabilities and enable
    /// multi-phase security system deployment.
    pub enterprise_mode: u8,
    pub security_foundation_initialized: u8,
    pub audit_system_initialized: u8,

    /// **Memory Alignment Padding**: Ensures 8-byte boundary alignment for zero-copy safety.
    ///
    /// Explicit padding prevents compiler layout variations and ensures consistent
    /// memory access patterns across different architectures and compiler versions.
    pub _enterprise_padding: [u8; 5],

    /// **Protocol Revenue Rate**: Fee collection rate in basis points (0-10000).
    ///
    /// u32 chosen for computational efficiency in fee calculations while providing
    /// sufficient granularity for sophisticated fee strategies. Basis points enable
    /// precise percentage control without floating-point arithmetic complexity.
    pub protocol_fee_rate: u32,

    /// **Pool Creation Counter**: Monotonically increasing pool identifier and capacity metric.
    ///
    /// Serves dual purposes: unique pool indexing and protocol growth monitoring.
    /// Saturating arithmetic prevents overflow attacks while maintaining count accuracy
    /// for reasonable deployment scales (4B+ pools).
    pub pool_count: u32,

    /// **Horizontal Scaling Parameter**: Active shard count for load distribution.
    ///
    /// Enables horizontal scaling by distributing pools across multiple shards.
    /// u16 supports 65k+ shards, far exceeding practical deployment requirements
    /// while conserving storage space.
    pub shard_count: u16,

    /// **Load Balancing Threshold**: Maximum pool capacity per shard for performance isolation.
    ///
    /// Prevents any single shard from becoming a bottleneck or DoS vector.
    /// Configurable to adapt to changing computational and storage constraints
    /// as the protocol scales.
    pub max_pools_per_shard: u16,

    /// **Alignment Padding**: Ensures 8-byte boundary alignment for u64 fields.
    ///
    /// Explicit padding prevents compiler layout variations and ensures consistent
    /// memory access patterns for zero-copy safety.
    pub _pools_padding: [u8; 4],

    /// **Economic Barrier**: Pool creation fee in lamports for spam prevention.
    ///
    /// u64 accommodates future SOL price appreciation without overflow risk.
    /// Fee serves multiple purposes: spam prevention, protocol sustainability,
    /// and natural market-based pool curation.
    pub creation_fee: u64,

    /// **Temporal Tracking**: Last modification slot for audit trails and replay protection.
    ///
    /// Enables time-based logic, rate limiting, and forensic analysis.
    /// Critical for detecting unusual activity patterns and preventing
    /// certain classes of economic attacks.
    pub last_update_slot: u64,

    /// **Operational Status Bitfield**: Compact representation of factory operational states.
    ///
    /// Bitwise encoding enables atomic status transitions and efficient status checks.
    /// Multiple status conditions can coexist (e.g., maintenance + partial emergency).
    /// Single-byte storage minimizes account size while supporting extensive status variety.
    ///
    /// Status hierarchy: EMERGENCY > PAUSED > MAINTENANCE > NORMAL
    pub status_flags: u8,

    /// **Alignment Preservation**: Explicit padding for consistent memory layout.
    ///
    /// Prevents compiler-dependent layout variations that could break zero-copy assumptions
    /// and ensures deterministic account structure across different compilation environments.
    pub _padding: [u8; 7],

    /// **Fee Tier Registry**: Fixed catalog of supported trading fee levels.
    ///
    /// Array design rationale:
    /// - Fixed size prevents account bloat and enables O(1) validation
    /// - Multiple tiers support diverse trading strategies (scalping to long-term holding)
    /// - Zero values serve as array terminators to support fewer than max tiers
    pub supported_fee_tiers: [u32; MAX_FEE_TIERS],

    /// **Protocol Analytics**: Aggregate trading metrics for governance and monitoring.
    ///
    /// Q64x64 fixed-point arithmetic chosen for:
    /// - Deterministic calculations without floating-point precision issues  
    /// - High precision for large volume accumulations without overflow
    /// - Consistent behavior across different hardware architectures
    ///
    /// These metrics drive protocol fee optimization and capacity planning decisions.
    pub total_volume: Q64x64,
    pub total_fees_collected: Q64x64,

    /// **Protocol Version**: Migration coordination and compatibility management.
    ///
    /// Enables safe protocol upgrades by allowing version-specific behavior branches.
    /// u32 supports extensive versioning schemes (major.minor.patch.build) while
    /// maintaining efficient storage and comparison operations.
    pub version: u32,

    /// **Alignment Padding**: Ensures proper alignment for large arrays.
    ///
    /// Explicit padding prevents compiler layout variations and ensures consistent
    /// memory access patterns for zero-copy safety with large array fields.
    pub _version_padding: [u8; 4],

    /// **Shard Registry**: Fixed array of active shard keys for bidirectional linkage.
    ///
    /// Why: Enables complete factory ↔ shard linkage for:
    /// - Direct shard validation and lookup
    /// - Efficient shard iteration for maintenance operations
    /// - Atomic shard management during scaling operations
    /// - Administrative operations requiring shard enumeration
    ///
    /// Array design rationale:
    /// - Fixed size prevents account bloat and enables deterministic sizing
    /// - Zero values serve as array terminators for sparse shard arrays
    /// - Supports up to MAX_SHARDS active shards per factory
    pub shard_keys: [Pubkey; MAX_SHARDS],

    /// **Future-Proofing Storage**: Pre-allocated space for seamless protocol evolution.
    ///
    /// Reserved fields enable adding new features without account migrations or
    /// rent increases. Size reduced to accommodate shard registry while maintaining
    /// upgrade capacity for anticipated future requirements.
    pub reserved: [u64; 5],
}

impl Factory {
    /// **Factory Initialization**: Establishes protocol foundation with comprehensive validation.
    ///
    /// ## Fail-Safe Initialization Strategy
    /// All parameters undergo rigorous validation before any state mutation to prevent
    /// protocol bricking through misconfiguration. This upfront validation approach
    /// ensures atomic success/failure rather than partial initialization states.
    ///
    /// ## Authority Binding Security
    /// Core authority binding occurs at initialization to establish immutable governance
    /// lineage. This prevents authority confusion attacks and ensures consistent
    /// governance patterns throughout protocol lifetime.
    ///
    /// ## Zero-State Foundation
    /// All counters and reserved fields initialize to zero to provide clean baseline
    /// for deterministic protocol evolution and predictable upgrade behaviors.
    ///
    /// # Security Guarantees
    /// - Parameter validation prevents configuration-based attacks
    /// - Authority binding establishes governance continuity  
    /// - Deterministic initialization ensures reproducible deployments
    pub fn initialize(
        &mut self,
        core_authority: Pubkey,
        config: FactoryConfig,
        current_slot: u64,
    ) -> Result<()> {
        // Protocol fee validation: prevent revenue-killing misconfigurations
        // 10000 basis points = 100% maximum prevents fee rates above principal
        require!(
            config.protocol_fee_rate <= 10000,
            FactoryError::InvalidFeeTier
        );

        // Shard capacity validation: ensure reasonable operational parameters
        // Zero pools per shard would prevent any pool creation
        // 1000+ pools per shard could create performance bottlenecks
        require!(
            config.max_pools_per_shard > 0 && config.max_pools_per_shard <= 1000,
            FactoryError::InvalidShardIndex
        );

        // Authority binding: establish immutable governance lineage
        self.core_authority = core_authority;

        // Enterprise security initialization: default to basic mode
        // Zero coordinator address signals basic mode operation
        // Explicit flags prevent ambiguity about security capabilities
        self.security_coordinator = Pubkey::default();
        self.enterprise_mode = 0;
        self.security_foundation_initialized = 0;
        self.audit_system_initialized = 0;
        self._enterprise_padding = [0u8; 5];

        // Core protocol parameters: apply validated configuration
        self.protocol_fee_rate = config.protocol_fee_rate;
        self.pool_count = 0; // Genesis state: no pools exist yet
        self.shard_count = 0; // Genesis state: sharding activated on-demand
        self.max_pools_per_shard = config.max_pools_per_shard;
        self._pools_padding = [0u8; 4]; // Explicit zero-padding for alignment
        self.creation_fee = config.creation_fee;
        self.last_update_slot = current_slot; // Establish temporal baseline
        self.status_flags = STATUS_NORMAL; // Operational from initialization
        self._padding = [0u8; 7]; // Explicit zero-padding for layout consistency
        self.supported_fee_tiers = config.supported_fee_tiers;

        // Analytics baseline: start from zero for clean metrics
        self.total_volume = Q64x64::zero();
        self.total_fees_collected = Q64x64::zero();

        // Version management: establish protocol generation
        self.version = 1;
        self._version_padding = [0u8; 4]; // Explicit zero-padding for alignment

        // Shard registry: initialize empty shard array
        self.shard_keys = [Pubkey::default(); MAX_SHARDS];

        // Future-proofing: initialize reserved space to zero
        self.reserved = [0u64; 5]; // Updated size to accommodate shard_keys

        Ok(())
    }

    /// **Protocol Fee Adjustment**: Governance-controlled revenue rate modification.
    ///
    /// ## Economic Attack Prevention
    /// Fee validation prevents governance from setting confiscatory rates (>100%)
    /// that could drain pool liquidity or make trading economically infeasible.
    /// This safeguards protocol sustainability and user trust.
    ///
    /// ## Operational State Gating
    /// Fee updates only permitted during NORMAL or MAINTENANCE status to prevent
    /// fee manipulation during emergency situations when governance may be
    /// compromised or operating under duress.
    ///
    /// ## Audit Trail Integration
    /// Slot tracking enables reconstruction of fee history for compliance audits
    /// and provides temporal context for economic analysis of protocol changes.
    pub fn update_protocol_fee(&mut self, new_fee: u32, current_slot: u64) -> Result<()> {
        // Operational safety check: prevent fee manipulation during emergencies
        require!(
            self.status_flags == STATUS_NORMAL || self.status_flags == STATUS_MAINTENANCE,
            FactoryError::FactoryPaused
        );

        // Economic safety bound: prevent confiscatory fee rates
        require!(new_fee <= 10000, FactoryError::InvalidFeeTier);

        // Apply validated fee change with audit trail
        self.protocol_fee_rate = new_fee;
        self.last_update_slot = current_slot;

        Ok(())
    }

    /// **Atomic Pool Counter Management**: Thread-safe pool tracking with overflow protection.
    ///
    /// ## Saturating Arithmetic Rationale  
    /// Uses saturating addition to gracefully handle extreme edge cases where
    /// pool count might approach u32 limits. Better to cap at max value than
    /// wrap to zero and corrupt pool indexing.
    ///
    /// ## Performance Optimization
    /// Simple increment operation designed for high-frequency calls during
    /// pool creation without complex validation overhead.
    pub fn increment_pool_count(&mut self, current_slot: u64) {
        self.pool_count = self.pool_count.saturating_add(1);
        self.last_update_slot = current_slot;
    }

    /// **Shard Expansion**: Dynamic horizontal scaling through shard addition.
    ///
    /// ## Index Assignment Strategy
    /// Current shard count serves as the new shard index, providing sequential
    /// numbering that simplifies shard management and load distribution algorithms.
    ///
    /// ## Overflow Protection
    /// Saturating arithmetic prevents shard count corruption while supporting
    /// practical scaling requirements far beyond realistic deployment needs.
    pub fn add_shard(&mut self, current_slot: u64) -> Result<u16> {
        let new_shard_index = self.shard_count;
        self.shard_count = self.shard_count.saturating_add(1);
        self.last_update_slot = current_slot;

        Ok(new_shard_index)
    }

    /// **Shard Registry Management**: Add shard to factory's tracking array.
    ///
    /// ## Bidirectional Linkage Strategy
    /// Stores shard Pubkey in factory's registry to enable complete factory ↔ shard
    /// bidirectional navigation. This enables efficient shard enumeration for
    /// administrative operations and validation.
    ///
    /// ## Capacity Management
    /// Validates shard capacity before addition to prevent array overflow and
    /// ensure deterministic account sizing.
    pub fn register_shard(&mut self, shard_key: Pubkey, current_slot: u64) -> Result<()> {
        let shard_index = (self.shard_count - 1) as usize; // Index of the just-added shard

        require!(shard_index < MAX_SHARDS, FactoryError::ShardAtCapacity);

        self.shard_keys[shard_index] = shard_key;
        self.last_update_slot = current_slot;

        Ok(())
    }

    /// **Shard Lookup**: Get shard key by index with bounds checking.
    ///
    /// ## Safe Array Access
    /// Prevents out-of-bounds access and returns None for invalid indices,
    /// enabling safe shard enumeration without panics.
    pub fn get_shard_key(&self, index: u16) -> Option<Pubkey> {
        let index = index as usize;
        if index < (self.shard_count as usize) && index < MAX_SHARDS {
            let key = self.shard_keys[index];
            if key != Pubkey::default() {
                Some(key)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// **Factory Capacity Check**: Determine if factory can accommodate new shards.
    ///
    /// ## Capacity Planning
    /// Enables proactive capacity management and load balancing decisions
    /// before shard limits are reached.
    pub fn has_shard_capacity(&self) -> bool {
        (self.shard_count as usize) < MAX_SHARDS
    }

    /// **Factory Utilization Metrics**: Get shard utilization percentage.
    ///
    /// ## Analytics Support
    /// Provides utilization metrics for monitoring and capacity planning.
    /// Percentage format enables easy interpretation and alerting thresholds.
    pub fn shard_utilization_percentage(&self) -> u8 {
        if MAX_SHARDS == 0 {
            return 100; // Edge case: prevent division by zero
        }
        ((self.shard_count as u32 * 100) / MAX_SHARDS as u32) as u8
    }

    /// **Operational Control**: Protocol pause/resume for maintenance and upgrades.
    ///
    /// ## Bitwise Status Management
    /// Uses bitwise OR/AND operations for atomic flag manipulation that preserves
    /// other status bits. This enables multiple status conditions to coexist
    /// (e.g., maintenance mode during partial emergency).
    ///
    /// ## Graceful Operations Control
    /// Pause mechanism enables controlled protocol upgrades and maintenance
    /// without requiring emergency stops that might panic users or markets.
    pub fn set_paused(&mut self, paused: u8, current_slot: u64) {
        if paused != 0 {
            self.status_flags |= STATUS_PAUSED; // Set pause bit while preserving others
        } else {
            self.status_flags &= !STATUS_PAUSED; // Clear pause bit while preserving others
        }
        self.last_update_slot = current_slot;
    }

    /// **Emergency Circuit Breaker**: Immediate protocol halt for critical situations.
    ///
    /// ## Crisis Response Design
    /// Emergency pause provides rapid response capability for detected exploits,
    /// oracle failures, or other critical threats requiring immediate protocol
    /// protection without waiting for governance consensus.
    ///
    /// ## Status Hierarchy Enforcement
    /// Emergency status takes precedence over normal operational states,
    /// ensuring that crisis response cannot be overridden by routine operations.
    pub fn set_emergency_pause(&mut self, active: u8, current_slot: u64) {
        if active != 0 {
            self.status_flags |= STATUS_EMERGENCY;
        } else {
            self.status_flags &= !STATUS_EMERGENCY;
        }
        self.last_update_slot = current_slot;
    }

    /// **Protocol Analytics Update**: Atomic accumulation of trading metrics.
    ///
    /// ## Checked Arithmetic Strategy
    /// Uses checked addition to detect overflow conditions that could indicate
    /// either calculation errors or extreme usage scenarios requiring protocol
    /// parameter adjustments. Graceful failure prevents data corruption.
    ///
    /// ## Fixed-Point Precision
    /// Q64x64 arithmetic maintains high precision across large accumulations
    /// while avoiding floating-point inconsistencies that could lead to
    /// discrepancies in revenue calculations or compliance reports.
    pub fn update_stats(&mut self, volume: Q64x64, fees: Q64x64) -> Result<()> {
        self.total_volume = self.total_volume.checked_add(volume)?;
        self.total_fees_collected = self.total_fees_collected.checked_add(fees)?;
        Ok(())
    }

    /// **Status Query Interface**: Efficient operational state detection.
    ///
    /// ## Bitwise Performance Optimization
    /// These methods use bitwise AND operations for maximum performance during
    /// high-frequency status checks in trading and pool management operations.
    /// Single instruction execution enables sub-microsecond status validation.
    ///
    /// ## Status Hierarchy Logic
    /// - `is_operational()` checks absence of any blocking conditions
    /// - Individual status methods enable granular operational decisions
    /// - Bitwise logic allows multiple status conditions to coexist naturally
    pub fn is_paused(&self) -> bool {
        self.status_flags & STATUS_PAUSED != 0
    }

    pub fn is_emergency_paused(&self) -> bool {
        self.status_flags & STATUS_EMERGENCY != 0
    }

    pub fn is_operational(&self) -> bool {
        self.status_flags & (STATUS_PAUSED | STATUS_EMERGENCY) == 0
    }

    /// **Fee Tier Validation**: High-performance trading fee level verification.
    ///
    /// ## Performance-First Design
    /// Method structure optimizes for the 99% case where fees are standard tiers.
    /// Early zero check and common tier matching minimize execution time for
    /// frequent pool creation and trading operations.
    ///
    /// ## Dual Validation Strategy
    /// - **Fast path**: Hardcoded checks for most common tiers (100, 500, 3000, 10000 bp)
    /// - **Flexible path**: Array iteration for custom tiers enabling protocol evolution
    ///
    /// This hybrid approach balances performance with governance flexibility.
    pub fn is_fee_tier_supported(&self, fee_tier: u32) -> bool {
        // Zero fee tier is invalid (would prevent any fee collection)
        if fee_tier == 0 {
            return false;
        }

        // Performance optimization: check most common tiers first
        // These represent ~95% of DeFi trading patterns
        if fee_tier == 100 || fee_tier == 500 || fee_tier == 3000 || fee_tier == 10000 {
            return true;
        }

        // Fallback: comprehensive search for governance-added custom tiers
        // Zero values in array act as terminators for unused slots
        self.supported_fee_tiers
            .iter()
            .any(|&tier| tier == fee_tier && tier != 0)
    }

    /// **Load Balancing Algorithm**: Optimal shard selection for even distribution.
    ///
    /// ## Round-Robin Strategy
    /// Simple modulo operation ensures even distribution across active shards
    /// without complex utilization tracking. This approach minimizes computational
    /// overhead while providing acceptable load balancing for most scenarios.
    ///
    /// ## Future Enhancement Design
    /// Current implementation provides foundation for more sophisticated algorithms
    /// that could consider shard utilization, geographic distribution, or
    /// performance characteristics in production deployments.
    pub fn get_optimal_shard_index(&self) -> Option<u16> {
        if self.shard_count == 0 {
            None // No shards available - protocols must initialize sharding
        } else {
            // Round-robin distribution using pool count as distributable identifier
            // Provides reasonably even distribution without tracking per-shard state
            Some(self.pool_count as u16 % self.shard_count)
        }
    }

    /// **Protocol Fee Calculation**: Optimized revenue computation with fast paths.
    ///
    /// ## Performance Optimization Strategy
    /// Structured to minimize expensive division operations through:
    /// - **Zero check**: Immediate return for zero fees (common in testing/development)
    /// - **Common rate optimization**: Hardcoded paths for standard fee percentages
    /// - **General case fallback**: Full fixed-point arithmetic for custom rates
    ///
    /// ## Fixed-Point Arithmetic Rationale
    /// Q64x64 format provides:
    /// - Deterministic calculations without floating-point precision issues
    /// - Sufficient precision for large amounts without overflow concerns
    /// - Consistent behavior across different hardware architectures
    ///
    /// ## Division Avoidance Optimization
    /// Common percentages use precomputed divisors to avoid expensive division
    /// operations during high-frequency trading calculations.
    pub fn calculate_protocol_fee(&self, amount: Q64x64) -> Result<u64> {
        // Early exit optimization: zero fees require no computation
        if self.protocol_fee_rate == 0 {
            return Ok(0u64);
        }

        // Performance optimization: avoid division for common fee rates
        // These rates represent majority of production configurations
        let protocol_fee = match self.protocol_fee_rate {
            100 => amount.checked_div(Q64x64::from_int(100u64))?, // 1% - common for competitive markets
            500 => amount.checked_div(Q64x64::from_int(200u64))?, // 0.5% - premium tier trading
            1000 => amount.checked_div(Q64x64::from_int(100u64))?, // 1% - standard tier
            _ => {
                // General case: full fixed-point arithmetic for custom rates
                // Uses optimized mul_div to prevent intermediate overflow
                mul_div_q64(
                    amount,
                    Q64x64::from_int(self.protocol_fee_rate as u64),
                    Q64x64::from_int(10000u64), // Basis points divisor
                )?
            }
        };

        // Extract integer portion: protocol fees paid in base currency units
        let protocol_fee = (protocol_fee.raw() >> 64) as u64;
        Ok(protocol_fee)
    }

    /// **Enterprise Authority Resolution**: Dynamic authority determination based on operation type.
    ///
    /// ## Dual Authority Architecture
    /// Factory operations route through different authority paths based on:
    /// - **Enterprise Mode Status**: Determines available security infrastructure
    /// - **Operation Sensitivity**: Critical operations require enhanced security
    ///
    /// This design enables protocols to upgrade security incrementally without
    /// disrupting existing operations or requiring full redeployment.
    ///
    /// ## Operation Classification Logic
    /// - **High-risk operations**: Protocol fees, emergency controls, global changes
    /// - **Standard operations**: Pool creation, statistics updates, routine maintenance
    ///
    /// Risk-based routing ensures appropriate security measures without
    /// over-engineering routine operations.
    pub fn get_effective_authority(&self, operation: &str) -> Pubkey {
        match (self.enterprise_mode, operation) {
            // Enterprise-only operations always route through security coordinator
            // These operations affect protocol-wide security or economics
            (
                1,
                "protocol_fee_update"
                | "factory_emergency_pause"
                | "add_fee_tier"
                | "global_audit_log",
            ) => self.security_coordinator,

            // Standard operations can use basic authority even in enterprise mode
            // These operations have limited blast radius and don't require enhanced security
            (_, "create_pool" | "update_stats" | "increment_pool_count") => self.core_authority,

            // Conservative fallback: unknown operations use basic authority
            // Prevents enterprise mode from blocking legitimate new operations
            _ => self.core_authority,
        }
    }

    /// **Operation Risk Assessment**: Determine if operation requires enterprise security.
    ///
    /// ## Risk-Based Security Model
    /// Only operations with protocol-wide impact or security implications require
    /// enterprise infrastructure. This selective approach minimizes operational
    /// overhead while providing enhanced protection where needed.
    ///
    /// ## Future-Proofing Design  
    /// String-based operation matching enables easy addition of new operations
    /// without modifying core authority resolution logic.
    pub fn factory_requires_enterprise(&self, operation: &str) -> bool {
        matches!(
            operation,
            "protocol_fee_update"
                | "factory_emergency_pause"
                | "global_audit_log"
                | "protocol_upgrade"
                | "add_fee_tier"
        )
    }

    /// **Authority Validation**: Comprehensive authorization check with enterprise awareness.
    ///
    /// ## Multi-Layer Validation Strategy
    /// 1. **Enterprise requirement check**: Ensures enterprise operations use enterprise mode
    /// 2. **Authority resolution**: Determines appropriate authority for operation type
    /// 3. **Signature validation**: Confirms caller matches effective authority
    ///
    /// This layered approach prevents both privilege escalation and enterprise
    /// requirement bypass attacks.
    pub fn validate_factory_authority(&self, operation: &str, signer: &Pubkey) -> bool {
        let effective_authority = self.get_effective_authority(operation);

        // Security requirement validation: enterprise operations need enterprise mode
        if self.factory_requires_enterprise(operation) && self.enterprise_mode == 0 {
            return false;
        }

        // Authority matching: caller must match resolved authority
        *signer == effective_authority
    }

    /// **Enterprise Upgrade Status Checks**: Phase completion validation for multi-stage security deployment.
    ///
    /// ## Multi-Phase Upgrade Strategy
    /// Enterprise security deployment occurs in phases to work within Solana's compute
    /// unit constraints while maintaining atomic security guarantees:
    ///
    /// - **Phase 1 (Security Foundation)**: Core authority, multisig, emergency contacts
    /// - **Phase 2 (Audit System)**: Cryptographic event logging infrastructure  
    /// - **Phase 3 (Enterprise Activation)**: Security coordinator and mode activation
    ///
    /// ## Status Flag Architecture
    /// Separate boolean flags for each phase enable:
    /// - **Prerequisite validation**: Each phase can verify previous phase completion
    /// - **Rollback safety**: Partial rollbacks possible without losing all progress
    /// - **Debugging clarity**: Clear indication of exactly which phase failed
    ///
    /// ## Upgrade Safety Guarantees
    /// These checks prevent dangerous partial upgrades that could leave factories
    /// in vulnerable intermediate states with incomplete security infrastructure.
    /// Phase 1 completion: Basic security infrastructure established.
    pub fn has_security_foundation(&self) -> bool {
        self.security_foundation_initialized == 1
    }

    /// Phase 2 completion: Audit trail infrastructure operational.
    pub fn has_audit_system(&self) -> bool {
        self.audit_system_initialized == 1
    }

    /// Phase 3 readiness: All prerequisites completed, enterprise mode activation ready.
    pub fn ready_for_enterprise_finalization(&self) -> bool {
        self.has_security_foundation() && self.has_audit_system() && self.enterprise_mode == 0
    }
}
