use anchor_lang::prelude::*;

/// Per-creator, per-vault performance score — updated by the oracle service.
/// Seeds: [b"score", vault.key, creator.key]
///
/// Initialized automatically when a creator is added to a vault via add_creator.
/// Updated by the oracle after each scoring epoch (off-chain metric aggregation).
#[account]
#[derive(InitSpace)]
pub struct SquadScore {
    /// Parent vault
    pub vault: Pubkey,
    /// Creator wallet
    pub creator: Pubkey,
    /// Composite score 0–100 (weighted blend of approval_rate, on_time_delivery, engagement)
    pub score: u8,
    /// Number of campaigns completed by this creator (across all vaults)
    pub campaigns_completed: u16,
    /// Submission approval rate 0–100 (percentage)
    pub approval_rate: u8,
    /// On-time delivery rate 0–100 (percentage)
    pub on_time_delivery: u8,
    /// Unix timestamp of last oracle update
    pub updated_at: i64,
    pub bump: u8,
}
