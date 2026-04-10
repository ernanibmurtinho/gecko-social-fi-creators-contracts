use anchor_lang::prelude::*;
use anchor_spl::token::{self, CloseAccount, Mint, Token, TokenAccount, Transfer};

use crate::{
    constants::{VAULT_SEED, VAULT_TOKEN_SEED},
    errors::GeckoError,
    state::{SponsorVault, VaultStatus},
};

/// Sponsor reclaims their locked principal after the cliff period.
///
/// Transfers remaining vault balance to the sponsor's token account,
/// closes the vault token account (returns rent), and marks the vault Closed.
///
/// All SquadMember PDAs should be closed by the sponsor before calling this
/// (via remove_creator) to recover rent. Any remaining members are left open
/// — their lamports stay locked until manually closed.
pub(crate) fn process(ctx: Context<CloseVault>) -> Result<()> {
    require!(
        ctx.accounts.vault.status != VaultStatus::Closed,
        GeckoError::VaultAlreadyClosed
    );

    let now = Clock::get()?.unix_timestamp;
    require!(now >= ctx.accounts.vault.cliff_ts, GeckoError::CliffNotElapsed);

    let remaining_balance = ctx.accounts.vault_token_account.amount;
    let vault_bump = ctx.accounts.vault.bump;
    let sponsor_key = ctx.accounts.vault.sponsor;
    let campaign_id = ctx.accounts.vault.campaign_id;
    let campaign_id_bytes = campaign_id.to_le_bytes();

    let vault_seeds: &[&[u8]] = &[
        VAULT_SEED,
        sponsor_key.as_ref(),
        campaign_id_bytes.as_ref(),
        &[vault_bump],
    ];
    let signer_seeds = &[vault_seeds];

    // Transfer remaining principal back to sponsor
    if remaining_balance > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    to: ctx.accounts.sponsor_token_account.to_account_info(),
                    authority: ctx.accounts.vault.to_account_info(),
                },
                signer_seeds,
            ),
            remaining_balance,
        )?;
    }

    // Close the vault token account, rent lamports go to sponsor
    token::close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        CloseAccount {
            account: ctx.accounts.vault_token_account.to_account_info(),
            destination: ctx.accounts.sponsor.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        },
        signer_seeds,
    ))?;

    ctx.accounts.vault.status = VaultStatus::Closed;
    ctx.accounts.vault.principal = 0;

    msg!(
        "Vault {} closed — {} tokens returned to sponsor",
        ctx.accounts.vault.key(),
        remaining_balance,
    );

    Ok(())
}

#[derive(Accounts)]
pub struct CloseVault<'info> {
    #[account(
        mut,
        seeds = [VAULT_SEED, sponsor.key().as_ref(), vault.campaign_id.to_le_bytes().as_ref()],
        bump = vault.bump,
        has_one = sponsor @ GeckoError::Unauthorized,
        has_one = vault_token_account,
        has_one = mint,
    )]
    pub vault: Account<'info, SponsorVault>,

    #[account(
        mut,
        seeds = [VAULT_TOKEN_SEED, vault.key().as_ref()],
        bump,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    /// Sponsor receives the returned principal
    #[account(
        mut,
        token::mint = mint,
        token::authority = sponsor,
    )]
    pub sponsor_token_account: Account<'info, TokenAccount>,

    pub mint: Account<'info, Mint>,

    #[account(mut)]
    pub sponsor: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
