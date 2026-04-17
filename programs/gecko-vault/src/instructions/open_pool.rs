use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{
    constants::{POOL_SEED, POOL_TOKEN_SEED, VAULT_SEED},
    errors::GeckoError,
    state::{ConfidencePool, PoolStatus, SponsorVault, VaultStatus},
};

/// Open a community confidence pool for a vault campaign.
///
/// Called by the sponsor after vault is active. Only one pool per vault
/// (enforced by PDA seeds). Participants can then call `stake` to place bets.
pub(crate) fn process(ctx: Context<OpenPool>) -> Result<()> {
    require!(
        ctx.accounts.vault.status == VaultStatus::Active,
        GeckoError::VaultNotActive
    );

    let pool = &mut ctx.accounts.pool;
    pool.vault = ctx.accounts.vault.key();
    pool.pool_token_account = ctx.accounts.pool_token_account.key();
    pool.yes_amount = 0;
    pool.no_amount = 0;
    pool.bettor_count = 0;
    pool.status = PoolStatus::Open;
    pool.outcome = None;
    pool.opened_at = Clock::get()?.unix_timestamp;
    pool.settled_at = 0;
    pool.bump = ctx.bumps.pool;

    msg!(
        "Confidence pool opened for vault {}",
        ctx.accounts.vault.key()
    );

    Ok(())
}

#[derive(Accounts)]
pub struct OpenPool<'info> {
    #[account(
        seeds = [VAULT_SEED, sponsor.key().as_ref(), vault.campaign_id.to_le_bytes().as_ref()],
        bump = vault.bump,
        has_one = sponsor @ GeckoError::Unauthorized,
    )]
    pub vault: Account<'info, SponsorVault>,

    #[account(
        init,
        payer = sponsor,
        space = 8 + ConfidencePool::INIT_SPACE,
        seeds = [POOL_SEED, vault.key().as_ref()],
        bump,
    )]
    pub pool: Account<'info, ConfidencePool>,

    /// PDA token account that holds staked USDC for this pool
    #[account(
        init,
        payer = sponsor,
        token::mint = mint,
        token::authority = pool,
        seeds = [POOL_TOKEN_SEED, pool.key().as_ref()],
        bump,
    )]
    pub pool_token_account: Account<'info, TokenAccount>,

    pub mint: Account<'info, Mint>,

    #[account(mut)]
    pub sponsor: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
