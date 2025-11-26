use crate::utils::security_authority::core_authority::CoreAuthority;
use anchor_lang::prelude::*;

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
    /// CHECK: initialize method where business logic can perform comprehensive checks.
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
