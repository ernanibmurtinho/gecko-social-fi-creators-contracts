use anchor_lang::prelude::*;

use crate::{
    constants::{CONFIG_SEED, REPUTATION_SEED},
    errors::GeckoError,
    state::{GeckoConfig, ReputationAccount},
};

/// Update a creator's global reputation account.
///
/// Called by the gecko-oracle service after each campaign settles. Aggregates
/// the creator's track record across all vaults into a single global score.
///
/// If the ReputationAccount PDA does not yet exist, it must be initialized by
/// the oracle in the same transaction (via `init` constraint).
///
/// Only `config.oracle_authority` can call this instruction.
pub(crate) fn process(
    ctx: Context<UpdateReputation>,
    global_score: u8,
    total_campaigns: u16,
    total_yield_earned: u64,
) -> Result<()> {
    require!(global_score <= 100, GeckoError::InvalidScore);

    let reputation = &mut ctx.accounts.reputation;
    reputation.creator = ctx.accounts.creator.key();
    reputation.global_score = global_score;
    reputation.total_campaigns = total_campaigns;
    reputation.total_yield_earned = total_yield_earned;
    reputation.last_updated = Clock::get()?.unix_timestamp;

    msg!(
        "Reputation updated for creator {}: global_score={} campaigns={}",
        reputation.creator,
        global_score,
        total_campaigns,
    );

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateReputation<'info> {
    #[account(
        init_if_needed,
        payer = oracle,
        space = 8 + ReputationAccount::INIT_SPACE,
        seeds = [REPUTATION_SEED, creator.key().as_ref()],
        bump,
    )]
    pub reputation: Account<'info, ReputationAccount>,

    /// CHECK: Creator wallet — identity only, validated by PDA seed
    pub creator: UncheckedAccount<'info>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        constraint = config.oracle_authority == oracle.key() @ GeckoError::Unauthorized,
    )]
    pub config: Account<'info, GeckoConfig>,

    /// Must match config.oracle_authority — pays for PDA init if first time
    #[account(mut)]
    pub oracle: Signer<'info>,

    pub system_program: Program<'info, System>,
}
