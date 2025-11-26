use crate::error::FactoryError;
use crate::state::factory::factory_account::{Factory, FactoryConfig};
use crate::utils::security_authority::core_authority::CoreAuthority;
use anchor_lang::prelude::*;

/// **Factory Initialization Context**: Atomic factory deployment with authority integration.
///
/// ## Deterministic Address Generation
/// Seed-based PDA creation ensures:
/// - **Predictable addresses**: Factory address derivable from protocol constants
/// - **Upgrade safety**: Consistent addressing across protocol versions
/// - **Collision avoidance**: Cryptographic uniqueness prevents address conflicts
///
/// ## Authority Validation Strategy
/// Core authority constraint ensures factory initialization only occurs with
/// properly established governance, preventing orphaned factories that could
/// bypass security controls.
///
/// ## Atomic Account Creation
/// All accounts created within single transaction context prevents partial
/// deployment states that could leave protocol in inconsistent condition.
#[derive(Accounts)]
#[instruction(config: FactoryConfig)]
pub struct InitializeFactory<'info> {
    /// **Factory Account**: Primary protocol state with deterministic addressing.
    ///
    /// Space calculation includes Anchor discriminator (8 bytes) plus Factory struct size
    /// for accurate rent calculation and prevents account size mismatches that could
    /// cause deployment failures.
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<Factory>(),
        seeds = [b"factory"],
        bump
    )]
    pub factory: AccountLoader<'info, Factory>,

    /// **Governance Authority Reference**: Validates legitimate factory initialization.
    ///
    /// Must exist before factory creation to ensure proper governance chain establishment.
    /// Seed-based validation prevents factory creation with invalid or malicious authorities.
    #[account(
        seeds = [b"core_authority", factory.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// **Transaction Fee Sponsor**: Account funding the factory deployment.
    ///
    /// Mutable access required for rent payment deduction during account creation.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// **Solana System Program**: Required for account creation operations.
    pub system_program: Program<'info, System>,
}

/// **Factory Bootstrap Handler**: Establishes protocol foundation with comprehensive validation.
///
/// ## Initialization Safety Strategy
/// All validation occurs before any state mutation to prevent partial initialization
/// that could leave the protocol in an inconsistent or vulnerable state. This
/// fail-fast approach ensures atomic success/failure behavior.
///
/// ## Authority Chain Validation
/// Verifies core authority binding to prevent factory initialization with malicious
/// or incorrectly configured governance, establishing legitimate authority lineage
/// from the start of protocol operation.
///
/// ## Configuration Integrity
/// Parameter validation prevents economically destructive configurations (like
/// confiscatory fee rates) that could damage protocol adoption or user trust.
pub fn initialize_factory(ctx: Context<InitializeFactory>, config: FactoryConfig) -> Result<()> {
    let factory = &mut ctx.accounts.factory.load_init()?;
    let clock = Clock::get()?;

    // Authority lineage validation: ensure legitimate governance establishment
    let core_authority = &ctx.accounts.core_authority.load()?;
    require!(
        core_authority.pool_core == ctx.accounts.factory.key(),
        FactoryError::InvalidAuthority
    );

    // Atomic initialization with validated parameters and authority binding
    factory.initialize(ctx.accounts.core_authority.key(), config, clock.slot)?;

    msg!(
        "Factory initialized with core authority: {}",
        ctx.accounts.core_authority.key()
    );
    Ok(())
}
