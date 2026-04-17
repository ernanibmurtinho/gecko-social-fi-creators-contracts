use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::{
    constants::{BPS_DENOMINATOR, CONFIG_SEED, MEMBER_SEED, VAULT_SEED, VAULT_TOKEN_SEED},
    errors::GeckoError,
    state::{GeckoConfig, SquadMember, SponsorVault, VaultStatus},
};

/// Distribute one member's yield share for the current epoch.
///
/// Called by the automation authority (Helius webhook relayer) once per member
/// per yield epoch.
///
/// `yield_amount` is this specific member's pre-calculated yield allocation:
///   yield_amount = total_epoch_yield × (member.allocation_bps / 10_000)
///
/// The program then:
///   1. Deducts the Gecko protocol fee from yield_amount → treasury
///   2. Transfers the remainder to the creator's token account
///
/// Security:
///   - vault_token_account.amount must cover principal + yield_amount
///   - Only automation_authority can call this instruction
///   - Token account authorities are enforced by constraints
///
/// Phase 2 (TODO): Replace direct transfer with Streamflow CPI stream top-up.
pub(crate) fn process(ctx: Context<RouteYield>, yield_amount: u64) -> Result<()> {
    require!(yield_amount > 0, GeckoError::ZeroAmount);
    require!(
        ctx.accounts.vault.status == VaultStatus::Active,
        GeckoError::VaultNotActive
    );
    require!(
        ctx.accounts.vault.total_allocation_bps == BPS_DENOMINATOR,
        GeckoError::AllocationNotFull
    );

    // Security: yield_amount must not dip into the locked principal
    let vault_balance = ctx.accounts.vault_token_account.amount;
    let principal = ctx.accounts.vault.principal;
    let available_yield = vault_balance
        .checked_sub(principal)
        .ok_or(GeckoError::InsufficientBalance)?;
    require!(yield_amount <= available_yield, GeckoError::InsufficientBalance);

    let vault_bump = ctx.accounts.vault.bump;
    let sponsor_key = ctx.accounts.vault.sponsor;
    let campaign_id = ctx.accounts.vault.campaign_id;

    // Fee calculation
    let fee_bps = ctx.accounts.config.fee_bps as u64;
    let gecko_fee = yield_amount
        .checked_mul(fee_bps)
        .ok_or(GeckoError::Overflow)?
        .checked_div(BPS_DENOMINATOR as u64)
        .ok_or(GeckoError::Overflow)?;

    let creator_share = yield_amount
        .checked_sub(gecko_fee)
        .ok_or(GeckoError::Overflow)?;

    // Vault PDA signer seeds
    let campaign_id_bytes = campaign_id.to_le_bytes();
    let vault_seeds: &[&[u8]] = &[
        VAULT_SEED,
        sponsor_key.as_ref(),
        campaign_id_bytes.as_ref(),
        &[vault_bump],
    ];
    let signer_seeds = &[vault_seeds];

    // Transfer Gecko fee to treasury
    if gecko_fee > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    to: ctx.accounts.treasury_token_account.to_account_info(),
                    authority: ctx.accounts.vault.to_account_info(),
                },
                signer_seeds,
            ),
            gecko_fee,
        )?;
    }

    // Transfer creator share
    // Phase 2 (TODO): CPI to Streamflow to top-up the creator's real-time stream
    if creator_share > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    to: ctx.accounts.creator_token_account.to_account_info(),
                    authority: ctx.accounts.vault.to_account_info(),
                },
                signer_seeds,
            ),
            creator_share,
        )?;
    }

    // Update state
    let member = &mut ctx.accounts.member;
    member.total_received = member
        .total_received
        .checked_add(creator_share)
        .ok_or(GeckoError::Overflow)?;

    let vault = &mut ctx.accounts.vault;
    vault.total_yield_routed = vault
        .total_yield_routed
        .checked_add(yield_amount)
        .ok_or(GeckoError::Overflow)?;

    msg!(
        "Yield routed: {} total | {} fee | {} to creator {}",
        yield_amount,
        gecko_fee,
        creator_share,
        member.creator,
    );

    Ok(())
}

#[derive(Accounts)]
pub struct RouteYield<'info> {
    #[account(
        mut,
        seeds = [VAULT_SEED, vault.sponsor.as_ref(), vault.campaign_id.to_le_bytes().as_ref()],
        bump = vault.bump,
        has_one = vault_token_account,
    )]
    pub vault: Account<'info, SponsorVault>,

    #[account(
        mut,
        seeds = [VAULT_TOKEN_SEED, vault.key().as_ref()],
        bump,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [MEMBER_SEED, vault.key().as_ref(), member.creator.as_ref()],
        bump = member.bump,
        has_one = vault,
    )]
    pub member: Account<'info, SquadMember>,

    /// Creator's token account — must be owned by the creator recorded in member PDA.
    /// Boxed to keep RouteYield::try_accounts within the 4096-byte stack limit.
    #[account(
        mut,
        token::mint = mint,
        token::authority = member.creator,
    )]
    pub creator_token_account: Box<Account<'info, TokenAccount>>,

    /// Gecko treasury token account — must be owned by config.treasury.
    /// Boxed to keep RouteYield::try_accounts within the 4096-byte stack limit.
    #[account(
        mut,
        token::mint = mint,
        token::authority = config.treasury,
    )]
    pub treasury_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        constraint = config.automation_authority == authority.key() @ GeckoError::Unauthorized,
    )]
    pub config: Account<'info, GeckoConfig>,

    pub mint: Account<'info, Mint>,

    /// Must match config.automation_authority
    pub authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
}
