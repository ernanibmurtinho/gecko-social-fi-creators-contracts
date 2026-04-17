use anchor_lang::prelude::*;

use crate::{
    constants::{CONFIG_SEED, DEFAULT_FEE_BPS},
    state::GeckoConfig,
};

/// Initialize the singleton protocol config.
/// Called once by the deployer after program deployment.
pub(crate) fn process(
    ctx: Context<InitConfig>,
    treasury: Pubkey,
    automation_authority: Pubkey,
    oracle_authority: Pubkey,
    allowed_mints: Vec<Pubkey>,
) -> Result<()> {
    require!(!allowed_mints.is_empty(), crate::errors::GeckoError::UnsupportedMint);

    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.authority.key();
    config.treasury = treasury;
    config.automation_authority = automation_authority;
    config.oracle_authority = oracle_authority;
    config.fee_bps = DEFAULT_FEE_BPS;
    config.bump = ctx.bumps.config;
    config.allowed_mints = allowed_mints;
    Ok(())
}

#[derive(Accounts)]
pub struct InitConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + GeckoConfig::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump,
    )]
    pub config: Account<'info, GeckoConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}
