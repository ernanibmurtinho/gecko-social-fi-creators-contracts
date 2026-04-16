use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_DENOMINATOR, CONFIG_SEED, MILESTONE_SEED, VAULT_SEED},
    errors::GeckoError,
    state::{GeckoConfig, MilestoneStatus, PerformanceMilestone, SponsorVault, VaultStatus},
};

/// Create a performance milestone on behalf of the vault, signed by the
/// Gecko automation authority instead of the sponsor.
///
/// Enables fully-automated advance payments: automation creates a
/// score_threshold=0 milestone (always-true) and immediately calls
/// release_milestone, crediting the creator without any sponsor signature.
///
/// Key differences from `create_milestone`:
/// - Signer is `automation` (config.automation_authority), not `sponsor`
/// - `score_threshold` may be 0 (sponsor-signed version requires >= 1)
/// - `automation` pays rent for the milestone PDA
pub(crate) fn process(
    ctx: Context<CreateMilestoneByAutomation>,
    description: String,
    score_threshold: u8,
    payout_bps: u16,
    target_creator: Pubkey,
    index: u8,
) -> Result<()> {
    require!(description.len() <= 200, GeckoError::InvalidAccountData);
    require!(
        ctx.accounts.vault.status == VaultStatus::Active,
        GeckoError::VaultNotActive
    );
    require!(
        score_threshold <= 100,
        GeckoError::InvalidScoreThreshold
    );
    require!(
        payout_bps >= 1 && payout_bps <= BPS_DENOMINATOR,
        GeckoError::InvalidMilestonePayoutBps
    );

    let milestone = &mut ctx.accounts.milestone;
    milestone.vault = ctx.accounts.vault.key();
    milestone.sponsor = ctx.accounts.vault.sponsor;
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
        "Automation created milestone {} for vault {}: threshold={} payout={}bps",
        index,
        ctx.accounts.vault.key(),
        score_threshold,
        payout_bps,
    );

    Ok(())
}

#[derive(Accounts)]
#[instruction(description: String, score_threshold: u8, payout_bps: u16, target_creator: Pubkey, index: u8)]
pub struct CreateMilestoneByAutomation<'info> {
    #[account(
        seeds = [VAULT_SEED, vault.sponsor.as_ref(), vault.campaign_id.to_le_bytes().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, SponsorVault>,

    #[account(
        init,
        payer = automation,
        space = 8 + PerformanceMilestone::INIT_SPACE,
        seeds = [MILESTONE_SEED, vault.key().as_ref(), &[index]],
        bump,
    )]
    pub milestone: Account<'info, PerformanceMilestone>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Account<'info, GeckoConfig>,

    /// The Gecko automation keypair — must equal config.automation_authority.
    #[account(
        mut,
        constraint = automation.key() == config.automation_authority @ GeckoError::Unauthorized
    )]
    pub automation: Signer<'info>,

    pub system_program: Program<'info, System>,
}
