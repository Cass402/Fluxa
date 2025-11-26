use crate::error::FactoryError;
use crate::state::factory::factory_account::Factory;
use crate::utils::security_authority::core_authority::CoreAuthority;
use crate::utils::security_authority::emergency_contacts::EmergencyContacts;
use anchor_lang::prelude::*;

/// **Emergency Protocol Control Context**: Rapid response capability for critical situations.
///
/// ## Crisis Response Architecture
/// Emergency pause provides immediate protocol protection without requiring full governance
/// consensus, critical for responding to detected exploits, oracle failures, or market
/// manipulation attacks where minutes determine containment success.
///
/// ## Multi-Modal Emergency Authority
/// Supports both basic emergency contacts and enterprise security coordinator paths,
/// ensuring emergency response capability regardless of factory security mode.
/// This redundancy prevents security upgrades from accidentally disabling crisis response.
#[derive(Accounts)]
pub struct EmergencyPauseFactory<'info> {
    /// **Factory State**: Target of emergency control actions.
    ///
    /// Mutable access required for immediate status flag modification during crisis response.
    #[account(
        mut,
        seeds = [b"factory"],
        bump
    )]
    pub factory: AccountLoader<'info, Factory>,

    /// **Core Governance**: Primary authority validation for emergency response coordination.
    #[account(
        seeds = [b"core_authority", factory.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// **Emergency Response Registry**: Rapid response contact validation for crisis situations.
    ///
    /// Separate from multisig to enable faster emergency responses when governance
    /// consensus would be too slow for effective threat containment.
    #[account(
        seeds = [b"emergency_contacts", factory.key().as_ref()],
        bump
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    /// **Crisis Response Authority**: Individual authorized for immediate protocol protection.
    ///
    /// Must be pre-authorized through emergency contacts or security coordinator
    /// to prevent unauthorized protocol disruption.
    pub emergency_responder: Signer<'info>,
}

/// **Emergency Protocol Control Handler**: Immediate crisis response with dual-mode authority validation.
///
/// ## Crisis Response Philosophy
/// Provides immediate protocol protection capability without governance consensus delays
/// that could prove fatal during exploit attempts or market manipulation attacks.
/// Emergency response speed often determines containment success in DeFi protocols.
///
/// ## Multi-Modal Authority Design
/// Supports both basic emergency contacts and enterprise security coordinator authorities,
/// ensuring crisis response remains functional across all factory security configurations.
/// This redundant approach prevents security upgrades from accidentally breaking emergency controls.
///
/// ## Abuse Prevention Strategy
/// Pre-authorization requirements through emergency contacts or security infrastructure
/// prevent malicious actors from disrupting protocol operations through false emergencies.
pub fn emergency_pause_factory(
    ctx: Context<EmergencyPauseFactory>,
    pause_active: u8,
) -> Result<()> {
    let factory = &mut ctx.accounts.factory.load_mut()?;
    let clock = Clock::get()?;

    // Dynamic emergency authority validation based on factory security mode
    if factory.enterprise_mode == 1 {
        // Enterprise mode: security coordinator manages emergency response
        let effective_authority = factory.get_effective_authority("factory_emergency_pause");
        require!(
            ctx.accounts.emergency_responder.key() == effective_authority,
            FactoryError::InsufficientPermissions
        );
    } else {
        // Basic mode: emergency contacts provide rapid response capability
        let emergency_contacts = &ctx.accounts.emergency_contacts.load()?;
        require!(
            emergency_contacts.has_emergency_authority(&ctx.accounts.emergency_responder.key()),
            FactoryError::InsufficientPermissions
        );
    }

    // Execute immediate protocol protection action
    factory.set_emergency_pause(pause_active, clock.slot);

    msg!(
        "Factory emergency pause: {} (enterprise_mode: {})",
        pause_active,
        factory.enterprise_mode
    );
    Ok(())
}
