use crate::state::factory::factory_account::Factory;
use crate::state::pool::pool_config::PoolConfig;
use crate::state::pool::pool_core::PoolCore;
use crate::utils::security_authority::security_coordinator::SecurityCoordinator;
use crate::utils::security_hierarchy::{SecurityOperation, SecurityResolver};

/// **Two-Tier Security Architecture Implementation Examples**
///
/// ## Educational and Integration Purpose
/// This module serves as both documentation and reference implementation for integrating
/// the two-tier security architecture into actual instruction handlers. These examples
/// demonstrate the practical application of security design patterns that balance
/// decentralized governance with institutional-grade security requirements.
///
/// ## Real-World Security Patterns
/// Each example represents a common security pattern found in DeFi protocols,
/// from simple single-authority operations to complex multi-tier scenarios
/// involving emergency overrides and cross-scope authority validation.
///
/// ## Anchor Framework Integration
/// Examples show how to properly structure Anchor account contexts to support
/// the security hierarchy while maintaining the zero-copy performance benefits
/// and account validation patterns that Anchor provides.
///
/// ## Security-First Design Philosophy
/// These patterns prioritize security validation before any state changes,
/// demonstrating defense-in-depth approaches where multiple validation layers
/// protect against various attack vectors including privilege escalation,
/// authority spoofing, and cross-scope unauthorized operations.
use anchor_lang::prelude::*;

/// **Example 1: Factory Enterprise Protocol Fee Update**
///
/// ## Protocol-Wide Economic Security Validation
/// Protocol fee updates affect all trading pairs and can significantly impact protocol
/// economics, user experience, and competitive positioning. This example demonstrates
/// why such operations require the highest factory-level security validation rather
/// than allowing individual pools to set their own protocol fees.
///
/// ## Enterprise Security Requirement Rationale
/// Fee updates bypass basic authority validation because:
/// - Economic impact spans the entire protocol ecosystem
/// - Incorrect fees could destabilize trading dynamics across all pairs
/// - Regulatory compliance may require multi-party authorization for fee changes
/// - Revenue distribution affects all stakeholders, not just individual pools
///
/// ## Single-Point-of-Authority Design
/// Factory-level validation ensures protocol-wide consistency and prevents
/// conflicting fee policies that could arise from decentralized pool management.
/// This centralizes economic policy while keeping operational management decentralized.
pub fn example_update_protocol_fee(
    factory: &Factory,
    signer: &Pubkey,
    _new_fee: u32,
) -> Result<()> {
    // Validate enterprise authority for protocol fee updates
    SecurityResolver::validate_factory_enterprise_authority(
        factory,
        signer,
        SecurityOperation::ProtocolFeeUpdate,
    )?;

    msg!(
        "Protocol fee update authorized by: {} (enterprise: {})",
        signer,
        factory.enterprise_mode
    );

    Ok(())
}

/// **Example 2: Pool Enterprise Pause**
///
/// ## Granular Pool Control vs Protocol Isolation
/// Pool pausing demonstrates the two-tier architecture's core philosophy: pools should
/// manage their own operations when possible to enable decentralized governance, but
/// with enterprise-grade security when operations could affect user funds or market stability.
///
/// ## Account Separation Design Pattern
/// This example shows why pool authorities are stored in separate `pool_config` accounts
/// rather than the main `pool` account. This separation enables:
/// - Zero-copy access to pool state without loading authority data during every trade
/// - Independent security coordinator accounts for enterprise pools
/// - Clear separation between operational state and governance configuration
///
/// ## Optional Enterprise Infrastructure
/// The `Option<SecurityCoordinator>` pattern allows pools to operate in basic mode
/// (single authority) or enterprise mode (multi-signature coordinator) without
/// requiring all pools to have enterprise infrastructure, reducing operational overhead
/// for pools that don't need sophisticated security.
///
/// ## Security Coordinator Integration
/// When present, the security coordinator provides enterprise-grade authorization
/// with audit trails, time locks, and multi-signature requirements specifically
/// scoped to the individual pool to prevent cross-pool contamination.
pub fn example_pause_pool(
    pool: &PoolCore,
    pool_config: &PoolConfig,
    security_coordinator: Option<&SecurityCoordinator>,
    signer: &Pubkey,
) -> Result<()> {
    // Extract security coordinator pubkey if available
    let security_coordinator_pubkey = security_coordinator.map(|sc| sc.pool_core);

    // Validate pool authority with proper context
    SecurityResolver::validate_pool_authority_with_config(
        pool,
        pool_config.core_authority,
        security_coordinator_pubkey,
        signer,
        SecurityOperation::PoolPause,
    )?;

    msg!(
        "Pool pause authorized by: {} (pool enterprise: {})",
        signer,
        pool.is_enterprise()
    );

    Ok(())
}

/// **Example 3: Factory Emergency Override**
///
/// ## Hierarchical Emergency Response Design
/// This example demonstrates the critical security principle that factory-level emergencies
/// can override pool-level operations. This hierarchy is essential during protocol-wide
/// incidents where individual pools might not recognize the severity of the situation
/// or where pool authorities might be compromised.
///
/// ## Emergency vs Normal Authority Trade-off
/// Emergency override bypasses normal pool governance because:
/// - Speed of response during active exploits outweighs governance process delays
/// - Protocol-wide incidents require centralized coordination to prevent further damage
/// - Pool authorities might be compromised during sophisticated attacks
/// - Emergency state should be rare enough that temporary centralization is acceptable
///
/// ## Security Coordinator Authority Requirement
/// Only the factory security coordinator can perform emergency overrides, not the
/// basic factory authority. This ensures emergency powers are only available to
/// accounts configured with enterprise security infrastructure and audit capabilities.
///
/// ## Fail-Safe Default Behavior
/// If emergency override conditions aren't met, the function explicitly fails rather
/// than falling back to normal validation, preventing accidental bypass of proper
/// authorization chains during non-emergency situations.
pub fn example_factory_emergency_override(
    factory: &Factory,
    pool: &PoolCore,
    signer: &Pubkey,
) -> Result<()> {
    // Check if factory is in emergency mode and signer is factory security coordinator
    if SecurityResolver::can_override_emergency(factory, signer) {
        msg!(
            "Factory emergency override active - pausing pool {}",
            pool.token_0
        );
        // Factory can override pool operations during emergency
        return Ok(());
    }

    // Otherwise validate normal pool authority
    msg!("Normal pool authority validation required");
    err!(crate::error::FactoryError::InsufficientPermissions)
}

/// **Example 4: Mixed Authority Operations**
///
/// ## Flexible Authority Resolution Pattern
/// Some operations can legitimately be authorized by either factory or pool authorities,
/// depending on the context and governance preferences. This pattern enables protocol
/// operators to choose between centralized policy enforcement (factory) and decentralized
/// pool management while maintaining security standards.
///
/// ## Authority Precedence Logic
/// Factory authority is checked first for mixed operations because:
/// - Factory policies should take precedence over individual pool policies
/// - Enterprise factory authority represents higher security infrastructure
/// - Protocol-wide consistency benefits from factory-level coordination
/// - Fallback to pool authority maintains decentralization when factory doesn't intervene
///
/// ## Enterprise vs Basic Operation Distinction
/// The example distinguishes between factory enterprise authority (security coordinator)
/// and basic factory authority. Mixed operations often benefit from enterprise-level
/// authorization due to their cross-cutting nature and potential for wider impact.
///
/// ## Graceful Authority Delegation
/// If factory authority doesn't apply or isn't available, the operation gracefully
/// falls back to pool-specific authority validation, enabling flexible governance
/// models where pools can self-manage when factory policies don't override.
pub fn example_mixed_authority_operation(
    factory: &Factory,
    pool: &PoolCore,
    pool_config: &PoolConfig,
    security_coordinator: Option<&SecurityCoordinator>,
    signer: &Pubkey,
) -> Result<()> {
    // Try factory authority first (for protocol-wide policies)
    if factory.enterprise_mode == 1 && signer == &factory.security_coordinator {
        msg!("Operation authorized by factory enterprise security");
        return Ok(());
    }

    // Fall back to pool-specific authority
    let security_coordinator_pubkey = security_coordinator.map(|sc| sc.pool_core);

    SecurityResolver::validate_pool_authority_with_config(
        pool,
        pool_config.core_authority,
        security_coordinator_pubkey,
        signer,
        SecurityOperation::BasicConfig,
    )?;

    msg!("Operation authorized by pool authority");
    Ok(())
}

/// **Example 5: Authority Hierarchy Demonstration**
///
/// ## Complete Security Hierarchy Implementation
/// This function demonstrates the full authority hierarchy in a single resolution,
/// showing how the security system prioritizes different authority types based on
/// their scope, security level, and emergency status. This pattern is useful for
/// debugging, auditing, and understanding authority precedence.
///
/// ## Five-Tier Authority Resolution
/// The hierarchy implements a clear priority order that reflects security principles:
/// 1. Emergency override trumps all other authorities (incident response priority)
/// 2. Factory enterprise for protocol-wide coordination (scope precedence)
/// 3. Pool enterprise for sophisticated pool management (specialized governance)
/// 4. Pool basic for routine pool operations (decentralized efficiency)
/// 5. Factory basic as fallback (protocol stability backup)
///
/// ## String Return Pattern for Integration
/// Returning authority type strings enables integration with logging, monitoring,
/// and audit systems that need to track which authority type was used for operations.
/// This is particularly valuable for compliance and post-incident analysis.
///
/// ## Early Return Optimization
/// Each authority check uses early return to avoid unnecessary computation once
/// a valid authority is found. This optimization is important because authority
/// validation happens on every secured operation and should minimize CPU usage.
///
/// ## Explicit Authority Failure
/// Rather than defaulting to some arbitrary authority, the function explicitly
/// fails if no valid authority is found, preventing unauthorized operations and
/// ensuring all operations have clear authorization trails.
pub fn example_authority_hierarchy(
    factory: &Factory,
    pool: &PoolCore,
    pool_config: &PoolConfig,
    security_coordinator: Option<&SecurityCoordinator>,
    signer: &Pubkey,
) -> Result<String> {
    // 1. Factory Emergency Override (Highest Priority)
    if factory.is_emergency_paused()
        && factory.enterprise_mode == 1
        && signer == &factory.security_coordinator
    {
        return Ok("FACTORY_EMERGENCY_OVERRIDE".to_string());
    }

    // 2. Factory Enterprise Authority (Protocol Operations)
    if factory.enterprise_mode == 1 && signer == &factory.security_coordinator {
        return Ok("FACTORY_ENTERPRISE".to_string());
    }

    // 3. Pool Enterprise Authority (Pool-Specific Operations)
    if pool.is_enterprise() {
        if let Some(sc) = security_coordinator {
            if signer == &sc.pool_core {
                return Ok("POOL_ENTERPRISE".to_string());
            }
        }
    }

    // 4. Pool Basic Authority
    if signer == &pool_config.core_authority {
        return Ok("POOL_BASIC".to_string());
    }

    // 5. Factory Basic Authority (Fallback)
    if signer == &factory.core_authority {
        return Ok("FACTORY_BASIC".to_string());
    }

    err!(crate::error::FactoryError::InvalidAuthority)
}

/// **Example Anchor Context for Two-Tier Security**
///
/// ## Account Structure Design for Security Hierarchy
/// This Anchor context demonstrates how to structure accounts to support the two-tier
/// security architecture while maintaining Anchor's account validation and zero-copy
/// benefits. The design separates operational accounts from security accounts to
/// enable flexible security policies without impacting trading performance.
///
/// ## Optional Account Pattern for Enterprise Features
/// Using `Option<AccountLoader>` for security coordinators enables the same instruction
/// to handle both basic and enterprise mode operations without requiring separate
/// instruction handlers. This reduces code duplication while maintaining clear
/// security boundaries between operating modes.
///
/// ## Account Relationship Design
/// The context shows how accounts relate in the security hierarchy:
/// - Factory provides protocol-wide security policies and emergency controls
/// - Pool contains operational state optimized for zero-copy trading operations  
/// - PoolConfig contains governance configuration separate from trading state
/// - SecurityCoordinators provide enterprise infrastructure when needed
///
/// ## Performance vs Security Trade-off
/// Separating security accounts from operational accounts enables:
/// - Fast zero-copy access to trading state without loading security data
/// - Independent security infrastructure that can be upgraded without affecting trades
/// - Optional enterprise features that don't impact basic operation performance
#[derive(Accounts)]
pub struct TwoTierSecurityExample<'info> {
    /// **Factory Account - Protocol-Wide Security Authority**
    ///
    /// ## Single Source of Protocol Truth
    /// Factory account serves as the authoritative source for protocol-wide policies,
    /// emergency states, and enterprise configuration. Using seeds-based derivation
    /// ensures there's exactly one factory per program deployment, preventing
    /// multiple competing protocol authorities.
    #[account(
        seeds = [b"factory"],
        bump,
    )]
    pub factory: AccountLoader<'info, Factory>,

    /// **Pool Account - Operational Trading State**
    ///
    /// ## Zero-Copy Trading Optimization
    /// Pool account contains frequently accessed trading state optimized for
    /// zero-copy access patterns. Marked as mutable because trading operations
    /// modify pool state, but authority validation happens before any modifications.
    #[account(mut)]
    pub pool: AccountLoader<'info, PoolCore>,

    /// **Pool Configuration - Governance and Authority Data**
    ///
    /// ## Separation of Operational and Governance State
    /// Pool configuration is separate from pool trading state to enable
    /// authority updates without affecting trading performance. Seeds derivation
    /// ensures each pool has exactly one configuration account.
    #[account(
        seeds = [b"pool_config", pool.key().as_ref()],
        bump,
    )]
    pub pool_config: AccountLoader<'info, PoolConfig>,

    /// **Factory Security Coordinator - Enterprise Infrastructure**
    ///
    /// ## Optional Enterprise Security Infrastructure
    /// Only present when factory operates in enterprise mode. This pattern
    /// enables basic factories to operate without enterprise overhead while
    /// providing sophisticated security for institutions that need it.
    /// Use Option to handle both basic and enterprise factories
    pub factory_security_coordinator: Option<AccountLoader<'info, SecurityCoordinator>>,

    /// **Pool Security Coordinator - Pool-Specific Enterprise Security**
    ///
    /// ## Independent Pool Enterprise Infrastructure
    /// Enables individual pools to have enterprise security independent of
    /// factory enterprise status. This supports mixed environments where
    /// some pools need institutional-grade security while others operate
    /// with simpler governance models.
    pub pool_security_coordinator: Option<AccountLoader<'info, SecurityCoordinator>>,

    /// **Authority Signer - Operation Authorization**
    ///
    /// ## Transaction Authorization Source
    /// The account attempting to authorize the operation. Must match one of
    /// the authority accounts in the security hierarchy based on operation
    /// type and scope. Anchor's Signer constraint ensures the account
    /// actually signed the transaction.
    pub authority: Signer<'info>,
}

/// **Complete Two-Tier Security Validation Handler**
///
/// ## Real-World Instruction Handler Pattern
/// This function demonstrates how to implement comprehensive security validation
/// in actual Anchor instruction handlers. It shows the complete flow from operation
/// classification through authority resolution to validation, representing the
/// security-first approach that should be used in production DeFi protocols.
///
/// ## Operation Type Dispatch Strategy
/// Using string-based operation type dispatch enables flexible instruction design
/// where a single instruction can handle multiple operation types with appropriate
/// security validation for each. This reduces the number of instruction handlers
/// while maintaining security granularity.
///
/// ## Authority Resolution Before State Modification
/// The function performs all security validation before any state modifications,
/// implementing the principle that security checks should fail fast and prevent
/// unauthorized state changes rather than attempting to revert them.
///
/// ## Error Propagation Design
/// Security validation errors are propagated directly to the transaction level,
/// causing the entire transaction to fail rather than attempting recovery.
/// This fail-safe approach ensures that security failures are immediately
/// visible and cannot be masked by subsequent operations.
///
/// ## Mixed Authority Operation Pattern
/// For operations that can be authorized by multiple authority types, the function
/// attempts factory authority first, then falls back to pool authority. This
/// pattern enables flexible governance while maintaining clear precedence rules.
pub fn handle_two_tier_security_example(
    ctx: Context<TwoTierSecurityExample>,
    operation_type: String,
) -> Result<()> {
    let factory = ctx.accounts.factory.load()?;
    let pool = ctx.accounts.pool.load()?;
    let pool_config = ctx.accounts.pool_config.load()?;

    // Determine security operation type
    let operation = match operation_type.as_str() {
        "protocol_fee" => SecurityOperation::ProtocolFeeUpdate,
        "pool_pause" => SecurityOperation::PoolPause,
        "emergency" => SecurityOperation::FactoryEmergencyPause,
        _ => SecurityOperation::BasicConfig,
    };

    // Validate according to operation type
    match operation {
        SecurityOperation::ProtocolFeeUpdate => {
            // Factory-level operation
            SecurityResolver::validate_factory_enterprise_authority(
                &factory,
                &ctx.accounts.authority.key(),
                operation,
            )?;
        }
        SecurityOperation::PoolPause => {
            // Pool-level operation
            let security_coordinator = ctx
                .accounts
                .pool_security_coordinator
                .as_ref()
                .map(|sc| sc.load().unwrap().pool_core);

            SecurityResolver::validate_pool_authority_with_config(
                &pool,
                pool_config.core_authority,
                security_coordinator,
                &ctx.accounts.authority.key(),
                operation,
            )?;
        }
        _ => {
            // Mixed authority operation - try both
            let factory_valid = SecurityResolver::validate_factory_enterprise_authority(
                &factory,
                &ctx.accounts.authority.key(),
                operation,
            )
            .is_ok();

            if !factory_valid {
                let security_coordinator = ctx
                    .accounts
                    .pool_security_coordinator
                    .as_ref()
                    .map(|sc| sc.load().unwrap().pool_core);

                SecurityResolver::validate_pool_authority_with_config(
                    &pool,
                    pool_config.core_authority,
                    security_coordinator,
                    &ctx.accounts.authority.key(),
                    operation,
                )?;
            }
        }
    }

    msg!(
        "Two-tier security validation successful for operation: {}",
        operation_type
    );
    Ok(())
}
