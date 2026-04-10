use anchor_lang::prelude::*;

/// Protocol-level configuration — singleton PDA, owned by admin.
/// Seeds: [b"config"]
#[account]
#[derive(InitSpace)]
pub struct GeckoConfig {
    /// Admin who can update config
    pub authority: Pubkey,
    /// Wallet that receives Gecko protocol fees
    pub treasury: Pubkey,
    /// Signer authorized to call route_yield (Helius webhook relayer)
    pub automation_authority: Pubkey,
    /// Protocol fee in basis points (e.g. 200 = 2%)
    pub fee_bps: u16,
    pub bump: u8,
    /// Accepted stablecoin mints (USDC, USDT, etc.)
    #[max_len(5)]
    pub allowed_mints: Vec<Pubkey>,
}
