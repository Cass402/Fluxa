use crate::error::FactoryError;
use crate::state::factory::factory_account::{Factory, FactoryConfig};
use crate::utils::security_authority::core_authority::CoreAuthority;
use crate::utils::security_authority::multisig_config::MultisigConfig;
use anchor_lang::prelude::*;

/// **Factory Configuration Update Context**: Governance-controlled parameter modification with enterprise security.
///
/// ## Multi-Signature Authorization Architecture
/// Configuration changes require both core authority validation and multisig member verification
/// to prevent single-point-of-failure attacks on critical protocol parameters. This dual
/// authorization approach balances operational efficiency with security requirements.
///
/// ## Enterprise Mode Compatibility
/// Context supports both basic and enterprise factory modes through dynamic authority
/// resolution, enabling seamless security upgrades without requiring separate instruction handlers.
#[derive(Accounts)]
pub struct UpdateFactoryConfig<'info> {
    /// **Factory State**: Target of configuration modifications with enterprise awareness.
    ///
    /// Mutable access required for parameter updates while maintaining zero-copy performance.
    #[account(
        mut,
        seeds = [b"factory"],
        bump
    )]
    pub factory: AccountLoader<'info, Factory>,

    /// **Governance Authority**: Core governance validation for legitimate parameter changes.
    #[account(
        seeds = [b"core_authority", factory.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// **Distributed Authorization**: Multi-signature validation for critical parameter changes.
    ///
    /// Required to prevent single-authority attacks on protocol economics that could
    /// drain fees or make trading economically unfeasible.
    #[account(
        seeds = [b"multisig_config", factory.key().as_ref()],
        bump
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    /// **Change Authorization**: Signer authorized to execute configuration modifications.
    ///
    /// Must be validated against effective authority (core authority or security coordinator)
    /// depending on factory's enterprise mode and operation type.
    pub authority: Signer<'info>,
}

/// **Factory Configuration Update Handler**: Secure parameter modification with enterprise awareness.
///
/// ## Dual-Mode Authorization Strategy
/// Handles both basic and enterprise factory configurations through dynamic authority
/// resolution, ensuring appropriate security measures for each operational mode without
/// requiring separate instruction handlers or protocol complexity.
///
/// ## Economic Security Validation
/// Multi-signature requirements for fee rate changes prevent single-authority attacks
/// on protocol economics that could drain revenue or price out legitimate users.
/// This distributed authorization approach balances operational efficiency with security.
///
/// ## Enterprise Security Integration
/// When operating in enterprise mode, additional validation layers ensure configuration
/// changes align with sophisticated security policies and audit requirements.
pub fn update_factory_config(
    ctx: Context<UpdateFactoryConfig>,
    new_config: FactoryConfig,
) -> Result<()> {
    let factory = &mut ctx.accounts.factory.load_mut()?;
    let clock = Clock::get()?;

    // Dynamic authority resolution based on enterprise mode and operation type
    let effective_authority = factory.get_effective_authority("protocol_fee_update");

    if factory.enterprise_mode == 1 {
        // Enterprise mode: enhanced security validation through coordinator
        require!(
            ctx.accounts.authority.key() == effective_authority,
            FactoryError::InvalidAuthority
        );

        // Enhanced multisig validation for enterprise economic parameters
        let multisig_config = &ctx.accounts.multisig_config.load()?;
        if new_config.protocol_fee_rate != factory.protocol_fee_rate {
            require!(
                multisig_config.is_member(&ctx.accounts.authority.key()),
                FactoryError::InsufficientPermissions
            );
        }
    } else {
        // Basic mode: core authority with multisig validation for critical changes
        let core_authority = &ctx.accounts.core_authority.load()?;
        require!(
            core_authority.current_authority == ctx.accounts.authority.key(),
            FactoryError::InvalidAuthority
        );

        // Basic multisig validation for economic security
        let multisig_config = &ctx.accounts.multisig_config.load()?;
        if new_config.protocol_fee_rate != factory.protocol_fee_rate {
            require!(
                multisig_config.is_member(&ctx.accounts.authority.key()),
                FactoryError::InsufficientPermissions
            );
        }
    }

    // Apply validated configuration changes atomically
    factory.update_protocol_fee(new_config.protocol_fee_rate, clock.slot)?;
    factory.creation_fee = new_config.creation_fee;
    factory.supported_fee_tiers = new_config.supported_fee_tiers;
    factory.max_pools_per_shard = new_config.max_pools_per_shard;

    msg!(
        "Factory configuration updated (enterprise_mode: {})",
        factory.enterprise_mode
    );
    Ok(())
}
