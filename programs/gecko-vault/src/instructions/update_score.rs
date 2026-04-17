use anchor_lang::prelude::*;

use crate::{
    constants::{CONFIG_SEED, SCORE_SEED, VAULT_SEED},
    errors::GeckoError,
    state::{GeckoConfig, SquadScore, SponsorVault},
};

/// Update a creator's performance score for a specific vault.
///
/// Called by the gecko-oracle service after each scoring epoch. The oracle
/// aggregates off-chain metrics (Twitter/Instagram engagement, submission
/// approval rates, on-time delivery) and writes the result on-chain.
///
/// Only `config.oracle_authority` can call this instruction.
pub(crate) fn process(
    ctx: Context<UpdateScore>,
    score: u8,
    campaigns_completed: u16,
    approval_rate: u8,
    on_time_delivery: u8,
) -> Result<()> {
    require!(score <= 100, GeckoError::InvalidScore);
    require!(approval_rate <= 100, GeckoError::InvalidScore);
    require!(on_time_delivery <= 100, GeckoError::InvalidScore);

    let squad_score = &mut ctx.accounts.score;
    squad_score.score = score;
    squad_score.campaigns_completed = campaigns_completed;
    squad_score.approval_rate = approval_rate;
    squad_score.on_time_delivery = on_time_delivery;
    squad_score.updated_at = Clock::get()?.unix_timestamp;

    msg!(
        "Score updated for creator {} on vault {}: score={} approval={}% on_time={}%",
        squad_score.creator,
        squad_score.vault,
        score,
        approval_rate,
        on_time_delivery,
    );

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateScore<'info> {
    #[account(
        seeds = [VAULT_SEED, vault.sponsor.as_ref(), vault.campaign_id.to_le_bytes().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, SponsorVault>,

    #[account(
        mut,
        seeds = [SCORE_SEED, vault.key().as_ref(), score.creator.as_ref()],
        bump = score.bump,
        has_one = vault,
    )]
    pub score: Account<'info, SquadScore>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        constraint = config.oracle_authority == oracle.key() @ GeckoError::Unauthorized,
    )]
    pub config: Account<'info, GeckoConfig>,

    /// Must match config.oracle_authority
    pub oracle: Signer<'info>,
}
