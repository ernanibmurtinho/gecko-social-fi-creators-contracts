use anchor_lang::prelude::*;

/// A performance milestone set by the sponsor for a specific vault.
/// Seeds: [b"milestone", vault.key, index.to_le_bytes()]
///
/// When a creator's SquadScore meets the threshold, the automation authority
/// calls release_milestone to transfer the payout from the vault principal.
#[account]
#[derive(InitSpace)]
pub struct PerformanceMilestone {
    /// Parent vault
    pub vault: Pubkey,
    /// Sponsor who created this milestone
    pub sponsor: Pubkey,
    /// Human-readable description of the milestone goal
    #[max_len(200)]
    pub description: String,
    /// Minimum SquadScore required to unlock this milestone (0–100)
    pub score_threshold: u8,
    /// Payout as a fraction of vault principal in basis points (e.g. 500 = 5%)
    pub payout_bps: u16,
    /// Target creator — if Pubkey::default(), applies to all squad members equally
    pub target_creator: Pubkey,
    /// Monotonic index within this vault (used in PDA seed)
    pub index: u8,
    /// Current lifecycle status
    pub status: MilestoneStatus,
    /// Unix timestamp when milestone was created
    pub created_at: i64,
    /// Unix timestamp when milestone was released (0 if not yet released)
    pub released_at: i64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, InitSpace)]
pub enum MilestoneStatus {
    /// Awaiting score threshold to be met
    Pending,
    /// Payout transferred — terminal state
    Released,
}
