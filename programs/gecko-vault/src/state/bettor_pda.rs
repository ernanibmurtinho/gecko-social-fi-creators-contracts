use anchor_lang::prelude::*;

/// One bettor's position in a confidence pool.
/// Seeds: [b"bettor", pool.key, bettor.key]
///
/// Using a separate PDA per bettor (not Vec inside ConfidencePool) because
/// Solana accounts have a 10MB size cap and bettors are unbounded.
/// Each bettor can only place one bet per pool.
#[account]
#[derive(InitSpace)]
pub struct BettorPda {
    /// The confidence pool this bet belongs to
    pub pool: Pubkey,
    /// Bettor's wallet
    pub bettor: Pubkey,
    /// true = bettor chose YES, false = bettor chose NO
    pub side: bool,
    /// Amount staked in USDC base units
    pub amount: u64,
    /// Whether winnings have been claimed (prevents double-claim)
    pub claimed: bool,
    pub bump: u8,
}
