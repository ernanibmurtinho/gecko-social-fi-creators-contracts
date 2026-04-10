use anchor_lang::prelude::*;

use crate::{
    constants::{MEMBER_SEED, VAULT_SEED},
    errors::GeckoError,
    state::{SquadMember, SponsorVault, VaultStatus},
};

/// Remove a creator from the squad and reclaim their allocation.
///
/// The freed allocation_bps are subtracted from vault.total_allocation_bps,
/// allowing the sponsor to re-assign them via add_creator.
///
/// Cannot remove the last member — close_vault handles that case.
pub(crate) fn process(ctx: Context<RemoveCreator>) -> Result<()> {
    require!(
        ctx.accounts.vault.status == VaultStatus::Active,
        GeckoError::VaultNotActive
    );
    require!(
        ctx.accounts.vault.member_count > 1,
        GeckoError::CannotRemoveLastMember
    );

    let freed_bps = ctx.accounts.member.allocation_bps;

    let vault = &mut ctx.accounts.vault;
    vault.member_count = vault.member_count.checked_sub(1).ok_or(GeckoError::Overflow)?;
    vault.total_allocation_bps = vault
        .total_allocation_bps
        .checked_sub(freed_bps)
        .ok_or(GeckoError::Overflow)?;

    msg!(
        "Creator {} removed from vault {} (freed {}bps, total now {}bps)",
        ctx.accounts.member.creator,
        vault.key(),
        freed_bps,
        vault.total_allocation_bps,
    );

    // Closing the member account returns lamports to the sponsor
    Ok(())
}

#[derive(Accounts)]
pub struct RemoveCreator<'info> {
    #[account(
        mut,
        seeds = [VAULT_SEED, sponsor.key().as_ref(), vault.campaign_id.to_le_bytes().as_ref()],
        bump = vault.bump,
        has_one = sponsor @ GeckoError::Unauthorized,
    )]
    pub vault: Account<'info, SponsorVault>,

    #[account(
        mut,
        close = sponsor,
        seeds = [MEMBER_SEED, vault.key().as_ref(), member.creator.as_ref()],
        bump = member.bump,
        has_one = vault,
    )]
    pub member: Account<'info, SquadMember>,

    #[account(mut)]
    pub sponsor: Signer<'info>,

    pub system_program: Program<'info, System>,
}
