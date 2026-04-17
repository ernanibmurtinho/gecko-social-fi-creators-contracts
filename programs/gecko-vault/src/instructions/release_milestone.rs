use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::{
    constants::{CONFIG_SEED, MILESTONE_SEED, SCORE_SEED, VAULT_SEED, VAULT_TOKEN_SEED},
    errors::GeckoError,
    state::{GeckoConfig, MilestoneStatus, PerformanceMilestone, SquadScore, SponsorVault, VaultStatus},
};

/// Release a performance milestone payout to the target creator.
///
/// Called by the automation authority after the oracle has updated the creator's
/// SquadScore to meet or exceed the milestone threshold. Transfers `vault.principal
/// * milestone.payout_bps / 10_000` from the vault token account to the creator.
///
/// Security:
///   - Only automation_authority can call this
///   - Milestone must be Pending
///   - Creator's SquadScore.score >= milestone.score_threshold
///   - Vault must be Active (cannot release after close)
pub(crate) fn process(ctx: Context<ReleaseMilestone>) -> Result<()> {
    require!(
        ctx.accounts.vault.status == VaultStatus::Active,
        GeckoError::VaultNotActive
    );
    require!(
        ctx.accounts.milestone.status == MilestoneStatus::Pending,
        GeckoError::MilestoneNotPending
    );
    require!(
        ctx.accounts.score.score >= ctx.accounts.milestone.score_threshold,
        GeckoError::ScoreThresholdNotMet
    );

    let vault_bump = ctx.accounts.vault.bump;
    let sponsor_key = ctx.accounts.vault.sponsor;
    let campaign_id = ctx.accounts.vault.campaign_id;

    // Calculate payout: principal * payout_bps / 10_000
    let payout = ctx.accounts.vault.principal
        .checked_mul(ctx.accounts.milestone.payout_bps as u64)
        .ok_or(GeckoError::Overflow)?
        .checked_div(10_000u64)
        .ok_or(GeckoError::Overflow)?;

    require!(payout > 0, GeckoError::ZeroAmount);

    // Verify vault has enough balance (above principal floor after payout)
    let vault_balance = ctx.accounts.vault_token_account.amount;
    require!(vault_balance >= payout, GeckoError::InsufficientBalance);

    // Vault PDA signer seeds
    let campaign_id_bytes = campaign_id.to_le_bytes();
    let vault_seeds: &[&[u8]] = &[
        VAULT_SEED,
        sponsor_key.as_ref(),
        campaign_id_bytes.as_ref(),
        &[vault_bump],
    ];
    let signer_seeds = &[vault_seeds];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault_token_account.to_account_info(),
                to: ctx.accounts.creator_token_account.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            signer_seeds,
        ),
        payout,
    )?;

    // Reduce principal to reflect the milestone payout
    let vault = &mut ctx.accounts.vault;
    vault.principal = vault.principal.checked_sub(payout).ok_or(GeckoError::Overflow)?;

    // Mark milestone released
    let milestone = &mut ctx.accounts.milestone;
    milestone.status = MilestoneStatus::Released;
    milestone.released_at = Clock::get()?.unix_timestamp;

    msg!(
        "Milestone {} released: {} USDC to creator {} (score {}/{})",
        milestone.index,
        payout,
        ctx.accounts.score.creator,
        ctx.accounts.score.score,
        milestone.score_threshold,
    );

    Ok(())
}

#[derive(Accounts)]
pub struct ReleaseMilestone<'info> {
    #[account(
        mut,
        seeds = [VAULT_SEED, vault.sponsor.as_ref(), vault.campaign_id.to_le_bytes().as_ref()],
        bump = vault.bump,
        has_one = vault_token_account,
    )]
    pub vault: Account<'info, SponsorVault>,

    #[account(
        mut,
        seeds = [VAULT_TOKEN_SEED, vault.key().as_ref()],
        bump,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [MILESTONE_SEED, vault.key().as_ref(), &[milestone.index]],
        bump = milestone.bump,
        has_one = vault,
    )]
    pub milestone: Account<'info, PerformanceMilestone>,

    /// Creator's SquadScore — must meet threshold
    #[account(
        seeds = [SCORE_SEED, vault.key().as_ref(), score.creator.as_ref()],
        bump = score.bump,
        has_one = vault,
    )]
    pub score: Account<'info, SquadScore>,

    /// Creator's USDC token account — receives the milestone payout
    #[account(
        mut,
        token::mint = mint,
        token::authority = score.creator,
    )]
    pub creator_token_account: Account<'info, TokenAccount>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        constraint = config.automation_authority == authority.key() @ GeckoError::Unauthorized,
    )]
    pub config: Account<'info, GeckoConfig>,

    pub mint: Account<'info, Mint>,

    /// Must match config.automation_authority
    pub authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
}
