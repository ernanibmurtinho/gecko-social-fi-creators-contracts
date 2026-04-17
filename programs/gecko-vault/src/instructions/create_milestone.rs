use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_DENOMINATOR, MILESTONE_SEED, VAULT_SEED},
    errors::GeckoError,
    state::{MilestoneStatus, PerformanceMilestone, SponsorVault, VaultStatus},
};

/// Create a performance milestone for a vault.
///
/// Called by the sponsor to define a bonus payout trigger. When a creator's
/// SquadScore reaches `score_threshold`, the automation authority can call
/// release_milestone to transfer the payout.
///
/// `index`: monotonic counter per vault (0, 1, 2...) — sponsor tracks this off-chain.
/// `target_creator`: specific creator to reward, or Pubkey::default() for all members.
pub(crate) fn process(
    ctx: Context<CreateMilestone>,
    description: String,
    score_threshold: u8,
    payout_bps: u16,
    target_creator: Pubkey,
    index: u8,
) -> Result<()> {
    require!(
        ctx.accounts.vault.status == VaultStatus::Active,
        GeckoError::VaultNotActive
    );
    require!(
        score_threshold >= 1 && score_threshold <= 100,
        GeckoError::InvalidScoreThreshold
    );
    require!(
        payout_bps >= 1 && payout_bps <= BPS_DENOMINATOR,
        GeckoError::InvalidMilestonePayoutBps
    );

    let milestone = &mut ctx.accounts.milestone;
    milestone.vault = ctx.accounts.vault.key();
    milestone.sponsor = ctx.accounts.sponsor.key();
    milestone.description = description;
    milestone.score_threshold = score_threshold;
    milestone.payout_bps = payout_bps;
    milestone.target_creator = target_creator;
    milestone.index = index;
    milestone.status = MilestoneStatus::Pending;
    milestone.created_at = Clock::get()?.unix_timestamp;
    milestone.released_at = 0;
    milestone.bump = ctx.bumps.milestone;

    msg!(
        "Milestone {} created for vault {}: threshold={} payout={}bps",
        index,
        ctx.accounts.vault.key(),
        score_threshold,
        payout_bps,
    );

    Ok(())
}

#[derive(Accounts)]
#[instruction(description: String, score_threshold: u8, payout_bps: u16, target_creator: Pubkey, index: u8)]
pub struct CreateMilestone<'info> {
    #[account(
        mut,
        seeds = [VAULT_SEED, sponsor.key().as_ref(), vault.campaign_id.to_le_bytes().as_ref()],
        bump = vault.bump,
        has_one = sponsor @ GeckoError::Unauthorized,
    )]
    pub vault: Account<'info, SponsorVault>,

    #[account(
        init,
        payer = sponsor,
        space = 8 + PerformanceMilestone::INIT_SPACE,
        seeds = [MILESTONE_SEED, vault.key().as_ref(), &[index]],
        bump,
    )]
    pub milestone: Account<'info, PerformanceMilestone>,

    #[account(mut)]
    pub sponsor: Signer<'info>,

    pub system_program: Program<'info, System>,
}
