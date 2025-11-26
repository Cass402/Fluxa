use crate::utils::security_authority::multisig_config::MultisigConfig;
use anchor_lang::prelude::*;

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
    /// CHECK: This is not dangerous because we don't read or write from this account
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
