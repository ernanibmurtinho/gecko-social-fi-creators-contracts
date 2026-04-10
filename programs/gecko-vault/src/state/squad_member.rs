use anchor_lang::prelude::*;

/// One creator's membership in a sponsor's squad.
/// Seeds: [b"member", vault.key, creator.key]
///
/// Each vault can have unlimited members. Members are added/removed
/// individually without requiring vault reallocation.
#[account]
#[derive(InitSpace)]
pub struct SquadMember {
    /// Parent vault this membership belongs to
    pub vault: Pubkey,
    /// Creator's wallet (their embedded wallet from Privy)
    pub creator: Pubkey,
    /// This member's share of yield in basis points
    /// All members' allocation_bps must sum to 10_000 to activate routing
    pub allocation_bps: u16,
    /// Streamflow stream account for this creator on this vault.
    /// Set to Pubkey::default() until stream is created via CPI.
    pub stream_id: Pubkey,
    /// Cumulative yield received from this vault (in token base units)
    pub total_received: u64,
    pub bump: u8,
}
