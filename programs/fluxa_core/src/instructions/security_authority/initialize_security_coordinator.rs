use crate::utils::security_authority::audit_trail::AuditTrailHead;
use crate::utils::security_authority::core_authority::CoreAuthority;
use crate::utils::security_authority::emergency_contacts::EmergencyContacts;
use crate::utils::security_authority::multisig_config::MultisigConfig;
use crate::utils::security_authority::security_coordinator::SecurityCoordinator;
use anchor_lang::prelude::*;

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
    /// CHECK: This is not dangerous because we don't read or write from this account
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

    let clock = Clock::get()?;
    // Initialize with validated account references - all constraints checked by Anchor
    // These Pubkeys form the immutable security architecture for this pool instance
    security_coordinator.initialize(
        ctx.accounts.pool_core.key(),
        ctx.accounts.core_authority.key(),
        ctx.accounts.multisig_config.key(),
        ctx.accounts.audit_trail_head.key(),
        ctx.accounts.emergency_contacts.key(),
        clock.unix_timestamp,
    )?;

    Ok(())
}
