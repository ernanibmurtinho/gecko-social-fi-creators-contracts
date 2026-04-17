use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::{
    constants::{BETTOR_SEED, POOL_SEED, POOL_TOKEN_SEED, VAULT_SEED},
    errors::GeckoError,
    state::{BettorPda, ConfidencePool, PoolStatus, SponsorVault},
};

/// Claim winnings from a settled confidence pool.
///
/// Winners receive a proportional share of the losing side's total stake,
/// plus their original stake back. Formula:
///
///   winning_total = your_side_amount + losing_side_amount
///   your_share = (your_stake / your_side_amount) * winning_total
///
/// Losers cannot claim — they forfeited their stake to the winners.
///
/// Security:
///   - Pool must be Settled
///   - Bettor must be on the winning side
///   - Cannot claim twice (bettor_pda.claimed = true after first claim)
pub(crate) fn process(ctx: Context<ClaimWinnings>) -> Result<()> {
    require!(
        ctx.accounts.pool.status == PoolStatus::Settled,
        GeckoError::PoolNotSettled
    );
    require!(!ctx.accounts.bettor_pda.claimed, GeckoError::AlreadyClaimed);

    let outcome = ctx.accounts.pool.outcome.unwrap();
    require!(
        ctx.accounts.bettor_pda.side == outcome,
        GeckoError::LosingBet
    );

    let (winning_side_total, losing_side_total) = if outcome {
        (ctx.accounts.pool.yes_amount, ctx.accounts.pool.no_amount)
    } else {
        (ctx.accounts.pool.no_amount, ctx.accounts.pool.yes_amount)
    };

    // Proportional share of the full pot (winning side + losing side)
    let bettor_stake = ctx.accounts.bettor_pda.amount;
    let full_pool = winning_side_total
        .checked_add(losing_side_total)
        .ok_or(GeckoError::Overflow)?;
    let payout = bettor_stake
        .checked_mul(full_pool)
        .ok_or(GeckoError::Overflow)?
        .checked_div(winning_side_total)
        .ok_or(GeckoError::Overflow)?;

    // Pool PDA signer seeds
    let vault_key = ctx.accounts.vault.key();
    let pool_bump = ctx.accounts.pool.bump;
    let pool_seeds: &[&[u8]] = &[
        POOL_SEED,
        vault_key.as_ref(),
        &[pool_bump],
    ];
    let signer_seeds = &[pool_seeds];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.pool_token_account.to_account_info(),
                to: ctx.accounts.bettor_token_account.to_account_info(),
                authority: ctx.accounts.pool.to_account_info(),
            },
            signer_seeds,
        ),
        payout,
    )?;

    ctx.accounts.bettor_pda.claimed = true;

    msg!(
        "Winnings claimed: {} USDC to {} (stake: {} on {})",
        payout,
        ctx.accounts.bettor_pda.bettor,
        bettor_stake,
        if outcome { "YES" } else { "NO" },
    );

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimWinnings<'info> {
    #[account(
        seeds = [VAULT_SEED, vault.sponsor.as_ref(), vault.campaign_id.to_le_bytes().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, SponsorVault>,

    #[account(
        seeds = [POOL_SEED, vault.key().as_ref()],
        bump = pool.bump,
        has_one = vault,
        has_one = pool_token_account,
    )]
    pub pool: Account<'info, ConfidencePool>,

    #[account(
        mut,
        seeds = [POOL_TOKEN_SEED, pool.key().as_ref()],
        bump,
    )]
    pub pool_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [BETTOR_SEED, pool.key().as_ref(), bettor.key().as_ref()],
        bump = bettor_pda.bump,
        has_one = pool,
    )]
    pub bettor_pda: Account<'info, BettorPda>,

    /// Bettor's USDC token account — receives the payout
    #[account(
        mut,
        token::mint = mint,
        token::authority = bettor,
    )]
    pub bettor_token_account: Account<'info, TokenAccount>,

    pub mint: Account<'info, Mint>,

    pub bettor: Signer<'info>,

    pub token_program: Program<'info, Token>,
}
