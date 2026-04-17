use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::{
    constants::{BETTOR_SEED, POOL_SEED, POOL_TOKEN_SEED, VAULT_SEED},
    errors::GeckoError,
    state::{BettorPda, ConfidencePool, PoolStatus, SponsorVault},
};

/// Place a bet in a confidence pool.
///
/// - `side`: true = YES (campaign will succeed), false = NO (campaign will fail)
/// - `amount`: USDC to stake (in base units, 6 decimals)
///
/// Creates a BettorPda to track the position. Each wallet can only bet once per pool.
/// Transfers USDC from the bettor's token account to the pool token account.
pub(crate) fn process(ctx: Context<Stake>, side: bool, amount: u64) -> Result<()> {
    require!(amount > 0, GeckoError::ZeroAmount);
    require!(
        ctx.accounts.pool.status == PoolStatus::Open,
        GeckoError::PoolNotOpen
    );

    // Transfer USDC from bettor to pool
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.bettor_token_account.to_account_info(),
                to: ctx.accounts.pool_token_account.to_account_info(),
                authority: ctx.accounts.bettor.to_account_info(),
            },
        ),
        amount,
    )?;

    // Update pool totals
    let pool = &mut ctx.accounts.pool;
    if side {
        pool.yes_amount = pool.yes_amount.checked_add(amount).ok_or(GeckoError::Overflow)?;
    } else {
        pool.no_amount = pool.no_amount.checked_add(amount).ok_or(GeckoError::Overflow)?;
    }
    pool.bettor_count = pool.bettor_count.checked_add(1).ok_or(GeckoError::Overflow)?;

    // Initialize bettor position
    let bettor_pda = &mut ctx.accounts.bettor_pda;
    bettor_pda.pool = ctx.accounts.pool.key();
    bettor_pda.bettor = ctx.accounts.bettor.key();
    bettor_pda.side = side;
    bettor_pda.amount = amount;
    bettor_pda.claimed = false;
    bettor_pda.bump = ctx.bumps.bettor_pda;

    msg!(
        "Bet placed: {} {} {} USDC on vault {}",
        ctx.accounts.bettor.key(),
        if side { "YES" } else { "NO" },
        amount,
        ctx.accounts.vault.key(),
    );

    Ok(())
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(
        seeds = [VAULT_SEED, vault.sponsor.as_ref(), vault.campaign_id.to_le_bytes().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, SponsorVault>,

    #[account(
        mut,
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

    /// Bettor's USDC token account — must be owned by bettor
    #[account(
        mut,
        token::mint = mint,
        token::authority = bettor,
    )]
    pub bettor_token_account: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = bettor,
        space = 8 + BettorPda::INIT_SPACE,
        seeds = [BETTOR_SEED, pool.key().as_ref(), bettor.key().as_ref()],
        bump,
    )]
    pub bettor_pda: Account<'info, BettorPda>,

    pub mint: Account<'info, Mint>,

    #[account(mut)]
    pub bettor: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
