use anchor_lang::prelude::*;

/// Global reputation for a creator — aggregated across all vaults.
/// Seeds: [b"reputation", creator.key]
///
/// Unlike SquadScore (per vault), ReputationAccount is a single PDA per creator
/// that accumulates a cross-campaign track record. Updated by oracle after each
/// campaign settles.
#[account]
#[derive(InitSpace)]
pub struct ReputationAccount {
    /// Creator wallet
    pub creator: Pubkey,
    /// Global composite score 0–100 (lifetime weighted average)
    pub global_score: u8,
    /// Total campaigns participated in
    pub total_campaigns: u16,
    /// Total yield earned across all campaigns (in USDC base units)
    pub total_yield_earned: u64,
    /// Unix timestamp of last oracle update
    pub last_updated: i64,
    pub bump: u8,
}
