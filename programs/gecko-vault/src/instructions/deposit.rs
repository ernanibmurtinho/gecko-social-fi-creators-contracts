use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::{
    constants::{VAULT_SEED, VAULT_TOKEN_SEED},
    errors::GeckoError,
    state::{SponsorVault, VaultStatus},
};

/// Sponsor transfers stablecoins into the vault.
///
/// Phase 1: Tokens are held in the vault PDA token account.
/// Phase 2 (post-hackathon): CPI to Kamino to deposit into yield-bearing position.
pub(crate) fn process(ctx: Context<Deposit>, amount: u64) -> Result<()> {
    require!(amount > 0, GeckoError::ZeroAmount);
    require!(
        ctx.accounts.vault.status == VaultStatus::Active,
        GeckoError::VaultNotActive
    );

    // Transfer from sponsor's token account into vault's token account
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.sponsor_token_account.to_account_info(),
                to: ctx.accounts.vault_token_account.to_account_info(),
                authority: ctx.accounts.sponsor.to_account_info(),
            },
        ),
        amount,
    )?;

    let vault = &mut ctx.accounts.vault;
    vault.principal = vault.principal.checked_add(amount).ok_or(GeckoError::Overflow)?;

    // TODO (Phase 2): CPI to Kamino lending program to deposit principal
    // and receive kTokens representing the yield-bearing position.
    // The vault_token_account will hold kTokens after this point.

    msg!("Deposited {} tokens into vault {}", amount, vault.key());
    Ok(())
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(
        mut,
        seeds = [VAULT_SEED, sponsor.key().as_ref(), vault.campaign_id.to_le_bytes().as_ref()],
        bump = vault.bump,
        has_one = sponsor @ GeckoError::Unauthorized,
        has_one = mint,
        has_one = vault_token_account,
    )]
    pub vault: Account<'info, SponsorVault>,

    #[account(
        mut,
        seeds = [VAULT_TOKEN_SEED, vault.key().as_ref()],
        bump,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    /// Sponsor's source token account
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
}
