use anchor_lang::prelude::*;

use crate::{
    constants::{CONFIG_SEED, POOL_SEED, VAULT_SEED},
    errors::GeckoError,
    state::{ConfidencePool, GeckoConfig, PoolStatus, SponsorVault},
};

/// Settle a confidence pool with the final outcome.
///
/// Called by the oracle authority after the campaign's end_ts has passed and
/// the oracle has determined whether the campaign succeeded or failed.
///
/// `outcome`: true = campaign succeeded (YES wins), false = campaign failed (NO wins)
///
/// After settlement, winners can call claim_winnings to receive their proportional share.
pub(crate) fn process(ctx: Context<SettlePool>, outcome: bool) -> Result<()> {
    require!(
        ctx.accounts.pool.status == PoolStatus::Open,
        GeckoError::PoolNotOpen
    );

    let pool = &mut ctx.accounts.pool;
    pool.status = PoolStatus::Settled;
    pool.outcome = Some(outcome);
    pool.settled_at = Clock::get()?.unix_timestamp;

    msg!(
        "Pool settled for vault {}: outcome={}",
        ctx.accounts.vault.key(),
        if outcome { "YES" } else { "NO" },
    );

    Ok(())
}

#[derive(Accounts)]
pub struct SettlePool<'info> {
    #[account(
        seeds = [VAULT_SEED, vault.sponsor.as_ref(), vault.campaign_id.to_le_bytes().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, SponsorVault>,

    #[account(
        mut,
        seeds = [POOL_SEED, vault.key().as_ref()],
        bump = pool.bump,
        has_one = vault,
    )]
    pub pool: Account<'info, ConfidencePool>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        constraint = config.oracle_authority == oracle.key() @ GeckoError::Unauthorized,
    )]
    pub config: Account<'info, GeckoConfig>,

    /// Must match config.oracle_authority
    pub oracle: Signer<'info>,
}
