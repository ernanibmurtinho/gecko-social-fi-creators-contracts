use anchor_lang::prelude::*;

/// Community confidence pool for a vault campaign.
/// Seeds: [b"pool", vault.key]
///
/// One pool per vault. Participants stake USDC on YES (campaign succeeds) or
/// NO (campaign fails). Oracle settles after campaign end_ts. Winners split
/// the losers' stake proportionally.
#[account]
#[derive(InitSpace)]
pub struct ConfidencePool {
    /// Parent vault
    pub vault: Pubkey,
    /// Program-derived token account holding staked USDC
    /// Seeds: [b"pool_token", pool.key]
    pub pool_token_account: Pubkey,
    /// Total USDC staked on YES side (in base units)
    pub yes_amount: u64,
    /// Total USDC staked on NO side (in base units)
    pub no_amount: u64,
    /// Number of bettors who have placed a bet
    pub bettor_count: u32,
    /// Current lifecycle status
    pub status: PoolStatus,
    /// Final outcome set by oracle (Some(true) = YES won, Some(false) = NO won)
    pub outcome: Option<bool>,
    /// Unix timestamp when pool was opened
    pub opened_at: i64,
    /// Unix timestamp when pool was settled (0 if not yet settled)
    pub settled_at: i64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, InitSpace)]
pub enum PoolStatus {
    /// Pool is accepting bets
    Open,
    /// Oracle has set the outcome — winners can claim
    Settled,
}
