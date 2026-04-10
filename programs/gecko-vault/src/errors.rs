use anchor_lang::prelude::*;

#[error_code]
pub enum GeckoError {
    #[msg("Unsupported stablecoin mint — only USDC and USDT are accepted")]
    UnsupportedMint,

    #[msg("Campaign duration too short — minimum is 30 days")]
    DurationTooShort,

    #[msg("End timestamp must be after cliff timestamp")]
    InvalidTimestamps,

    #[msg("Cliff period has not elapsed — principal is still locked")]
    CliffNotElapsed,

    #[msg("Vault is not in an active state")]
    VaultNotActive,

    #[msg("Vault is already closed")]
    VaultAlreadyClosed,

    #[msg("Creator is already a member of this squad")]
    CreatorAlreadyInSquad,

    #[msg("Allocation basis points must be between 1 and 10000")]
    InvalidAllocationBps,

    #[msg("Total squad allocation would exceed 10000 bps")]
    TotalAllocationExceeded,

    #[msg("Cannot remove the only member of a squad — close the vault instead")]
    CannotRemoveLastMember,

    #[msg("Squad allocation must total exactly 10000 bps to activate streams")]
    AllocationNotFull,

    #[msg("Arithmetic overflow")]
    Overflow,

    #[msg("Zero amount not allowed")]
    ZeroAmount,

    #[msg("Insufficient vault balance")]
    InsufficientBalance,

    #[msg("Unauthorized — only the vault sponsor can perform this action")]
    Unauthorized,
}
