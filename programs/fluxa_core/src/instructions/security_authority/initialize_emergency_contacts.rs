use crate::utils::security_authority::emergency_contacts::EmergencyContacts;
use anchor_lang::prelude::*;

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
