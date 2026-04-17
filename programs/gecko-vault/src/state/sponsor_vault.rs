use anchor_lang::prelude::*;

/// A sponsor's campaign vault — one per campaign.
/// Seeds: [b"vault", sponsor.key, campaign_id.to_le_bytes()]
#[account]
#[derive(InitSpace)]
pub struct SponsorVault {
    /// Sponsor who created this vault
    pub sponsor: Pubkey,
    /// Accepted stablecoin mint (USDC or USDT)
    pub mint: Pubkey,
    /// Program-derived token account holding the locked principal
    /// Seeds: [b"vault_token", vault.key]
    pub vault_token_account: Pubkey,
    /// Amount of principal currently locked (in token base units)
    pub principal: u64,
    /// Cumulative yield routed to creators (lifetime)
    pub total_yield_routed: u64,
    /// Yield harvested by Kamino and credited to vault but not yet distributed
    pub accrued_yield: u64,
    /// Fee bps captured at vault creation (snapshot of protocol fee)
    pub gecko_fee_bps: u16,
    /// Unix timestamp after which the sponsor CAN close the vault
    pub cliff_ts: i64,
    /// Unix timestamp of campaign end (informational; close_vault enforces cliff_ts)
    pub end_ts: i64,
    /// Monotonically-increasing campaign ID per sponsor
    pub campaign_id: u64,
    /// Number of SquadMember PDAs attached
    pub member_count: u8,
    /// Sum of all member allocation_bps — must equal 10_000 before route_yield
    pub total_allocation_bps: u16,
    /// Current lifecycle status
    pub status: VaultStatus,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, InitSpace)]
pub enum VaultStatus {
    /// Principal deposited, yield routing allowed
    Active,
    /// Cliff elapsed — sponsor may close and reclaim principal
    Cliffed,
    /// Principal returned to sponsor, vault is terminal
    Closed,
}
