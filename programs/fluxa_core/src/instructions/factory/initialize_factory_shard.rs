use crate::error::FactoryError;
use crate::state::factory::factory_account::Factory;
use crate::state::factory::factory_shard::FactoryShard;
use crate::utils::security_authority::core_authority::CoreAuthority;
use anchor_lang::prelude::*;

/// Anchor context for initializing a new shard within the factory.
///
/// # Why this context?
/// - All accounts are validated and initialized atomically, minimizing risk of partial state.
/// - Seeds and bumps are used for deterministic address derivation and upgrade safety.
#[derive(Accounts)]
#[instruction(shard_index: u16)]
pub struct InitializeShard<'info> {
    /// Shard account to be initialized
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<FactoryShard>(),
        seeds = [b"shard", factory.key().as_ref(), &shard_index.to_le_bytes()],
        bump
    )]
    pub shard: AccountLoader<'info, FactoryShard>,

    /// Factory account to which this shard belongs
    #[account(
        mut,
        seeds = [b"factory"],
        bump
    )]
    pub factory: AccountLoader<'info, Factory>,

    /// Core authority validation
    #[account(
        seeds = [b"core_authority", factory.key().as_ref()],
        bump
    )]
    pub core_authority: AccountLoader<'info, CoreAuthority>,

    /// Authority that is initializing the shard
    pub authority: Signer<'info>,

    /// Payer for the transaction
    #[account(mut)]
    pub payer: Signer<'info>,

    /// System program for account creation
    pub system_program: Program<'info, System>,
}

/// Anchor instruction for initializing a new shard within the factory, with authority validation.
///
/// # Why this function?
/// - Ensures only authorized and operational factories can create new shards, protecting protocol safety.
/// - All updates are atomic and validated for safety and auditability.
/// - Shard index and state are set deterministically for upgrade and monitoring safety.
///
/// # Arguments
/// * `ctx` - The context containing the accounts required for initialization.
/// * `shard_index` - The index of the shard to be initialized.
/// # Returns
/// A `Result<()>` indicating success or failure of the initialization.
pub fn initialize_shard(ctx: Context<InitializeShard>, shard_index: u16) -> Result<()> {
    // Load accounts
    let factory = &mut ctx.accounts.factory.load_mut()?;
    let shard = &mut ctx.accounts.shard.load_init()?;
    let clock = Clock::get()?;

    // Validate authority
    let core_authority = &ctx.accounts.core_authority.load()?;
    require!(
        // Ensure the authority is the current core authority
        core_authority.current_authority == ctx.accounts.authority.key(),
        FactoryError::InvalidAuthority
    );

    // Validate factory is operational
    require!(factory.is_operational(), FactoryError::FactoryPaused);

    shard.initialize(ctx.accounts.factory.key(), shard_index, clock.slot)?;

    // Update factory shard count and register shard
    let shard_index = factory.add_shard(clock.slot)?;
    factory.register_shard(ctx.accounts.shard.key(), clock.slot)?;

    msg!("Shard {} initialized for factory", shard_index);
    Ok(())
}
