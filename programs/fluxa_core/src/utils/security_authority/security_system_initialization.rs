use crate::utils::security_authority::{
    audit_trail::{AuditTrailEntry, AuditTrailHead},
    core_authority::CoreAuthority,
    emergency_contacts::EmergencyContacts,
    multisig_config::MultisigConfig,
    security_coordinator::SecurityCoordinator,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;

#[derive(Accounts)]
pub struct SecuritySystemInitialization<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + SecurityCoordinator::INIT_SPACE,
        seeds = [b"security_coordinator", pool_core.key().as_ref()],
        bump,
    )]
    pub security_coordinator: AccountLoader<'info, SecurityCoordinator>,

    #[account(
        init,
        payer = payer,
        space = 8 + CoreAuthority::INIT_SPACE,
        seeds = [b"core_authority", pool_core.key().as_ref()],
        bump,
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    #[account(
        init,
        payer = payer,
        space = 8 + EmergencyContacts::INIT_SPACE,
        seeds = [b"emergency_contacts", pool_core.key().as_ref()],
        bump,
    )]
    pub emergency_contacts: AccountLoader<'info, EmergencyContacts>,

    #[account(
        init,
        payer = payer,
        space = 8 + MultisigConfig::INIT_SPACE,
        seeds = [b"multisig_config", pool_core.key().as_ref()],
        bump,
    )]
    pub multisig_config: AccountLoader<'info, MultisigConfig>,

    #[account(
        init,
        payer = payer,
        space = 8 + AuditTrailHead::INIT_SPACE,
        seeds = [b"audit_trail_head", pool_core.key().as_ref()],
        bump,
    )]
    pub audit_trail_head: AccountLoader<'info, AuditTrailHead>,

    #[account(
        init,
        payer = payer,
        space = 8 + AuditTrailEntry::INIT_SPACE,
        seeds = [b"audit_trail_entry", audit_trail_head.key().as_ref(), 1u8.to_le_bytes().as_ref()],
        bump,
    )]
    pub audit_trail_entry: AccountLoader<'info, AuditTrailEntry>,

    pub pool_core: UncheckedAccount<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub initial_authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_security_system(
    ctx: Context<SecuritySystemInitialization>,
    required_confirmations: u8,
    multisig_threshold: u8,
    multisig_members: [Pubkey; 7],
    pause_authority: Pubkey,
) -> Result<()> {
    let core_authority = &mut ctx.accounts.core_authority.load_init()?;
    core_authority.initialize(
        ctx.accounts.pool_core.key(),
        ctx.accounts.initial_authority.key(),
        required_confirmations,
    )?;

    let multisig_config = &mut ctx.accounts.multisig_config.load_init()?;
    multisig_config.initialize(
        ctx.accounts.pool_core.key(),
        multisig_threshold,
        multisig_members,
    )?;

    let audit_trail_head = &mut ctx.accounts.audit_trail_head.load_init()?;
    audit_trail_head.initialize(ctx.accounts.pool_core.key())?;

    let emergency_contacts = &mut ctx.accounts.emergency_contacts.load_init()?;
    emergency_contacts.initialize(ctx.accounts.pool_core.key(), pause_authority)?;

    let security_coordinator = &mut ctx.accounts.security_coordinator.load_init()?;
    security_coordinator.initialize(
        ctx.accounts.pool_core.key(),
        ctx.accounts.core_authority.key(),
        ctx.accounts.multisig_config.key(),
        ctx.accounts.audit_trail_head.key(),
        ctx.accounts.emergency_contacts.key(),
    )?;

    let audit_trail_entry = &mut ctx.accounts.audit_trail_entry.load_init()?;

    let data_hash = hashv(&[
        ctx.accounts.pool_core.key().as_ref(),
        ctx.accounts.initial_authority.key().as_ref(),
        ctx.accounts.core_authority.key().as_ref(),
        ctx.accounts.multisig_config.key().as_ref(),
        ctx.accounts.emergency_contacts.key().as_ref(),
        ctx.accounts.audit_trail_head.key().as_ref(),
        ctx.accounts.security_coordinator.key().as_ref(),
    ])
    .to_bytes();

    security_coordinator.log_security_event(
        audit_trail_head,
        audit_trail_entry,
        ctx.accounts.payer.key(),
        ctx.accounts.security_coordinator.key(),
        b"Security system initialized",
        data_hash,
    )?;

    Ok(())
}
