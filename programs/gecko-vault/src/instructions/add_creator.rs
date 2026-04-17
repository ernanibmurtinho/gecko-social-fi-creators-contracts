use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_DENOMINATOR, MEMBER_SEED, SCORE_SEED, VAULT_SEED},
    errors::GeckoError,
    state::{SquadMember, SquadScore, SponsorVault, VaultStatus},
};

/// Add a creator to the vault's squad and assign their yield allocation.
///
/// - `allocation_bps`: this creator's share of yield (e.g. 5000 = 50%)
///
/// Also initializes the creator's SquadScore PDA for this vault (score starts at 0,
/// updated by the oracle service after first scoring epoch).
///
/// The sum of all member allocation_bps must reach exactly 10_000 before
/// route_yield can be called. Sponsors build the squad incrementally.
pub(crate) fn process(ctx: Context<AddCreator>, allocation_bps: u16) -> Result<()> {
    require!(
        ctx.accounts.vault.status == VaultStatus::Active,
        GeckoError::VaultNotActive
    );
    require!(
        allocation_bps > 0 && allocation_bps <= BPS_DENOMINATOR,
        GeckoError::InvalidAllocationBps
    );

    let new_total = ctx
        .accounts
        .vault
        .total_allocation_bps
        .checked_add(allocation_bps)
        .ok_or(GeckoError::Overflow)?;

    require!(new_total <= BPS_DENOMINATOR, GeckoError::TotalAllocationExceeded);

    // Initialize SquadMember
    let member = &mut ctx.accounts.member;
    member.vault = ctx.accounts.vault.key();
    member.creator = ctx.accounts.creator.key();
    member.allocation_bps = allocation_bps;
    member.stream_id = Pubkey::default(); // set when Streamflow stream is created
    member.total_received = 0;
    member.bump = ctx.bumps.member;

    // Initialize SquadScore (starts at zero — oracle will update after first epoch)
    let score = &mut ctx.accounts.score;
    score.vault = ctx.accounts.vault.key();
    score.creator = ctx.accounts.creator.key();
    score.score = 0;
    score.campaigns_completed = 0;
    score.approval_rate = 0;
    score.on_time_delivery = 0;
    score.updated_at = Clock::get()?.unix_timestamp;
    score.bump = ctx.bumps.score;

    let vault = &mut ctx.accounts.vault;
    vault.member_count = vault.member_count.checked_add(1).ok_or(GeckoError::Overflow)?;
    vault.total_allocation_bps = new_total;

    msg!(
        "Creator {} added to vault {} with {}bps allocation (total: {}bps)",
        ctx.accounts.creator.key(),
        vault.key(),
        allocation_bps,
        new_total,
    );

    Ok(())
}

#[derive(Accounts)]
pub struct AddCreator<'info> {
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
        space = 8 + SquadMember::INIT_SPACE,
        seeds = [MEMBER_SEED, vault.key().as_ref(), creator.key().as_ref()],
        bump,
    )]
    pub member: Account<'info, SquadMember>,

    /// SquadScore PDA initialized alongside the member — oracle updates this after
    /// each scoring epoch.
    #[account(
        init,
        payer = sponsor,
        space = 8 + SquadScore::INIT_SPACE,
        seeds = [SCORE_SEED, vault.key().as_ref(), creator.key().as_ref()],
        bump,
    )]
    pub score: Account<'info, SquadScore>,

    /// CHECK: Creator wallet — validated by PDA seed uniqueness.
    /// Using UncheckedAccount since creators may not have signed yet
    /// (sponsor sets up the squad on their behalf).
    pub creator: UncheckedAccount<'info>,

    #[account(mut)]
    pub sponsor: Signer<'info>,

    pub system_program: Program<'info, System>,
}
