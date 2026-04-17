/// Devnet USDC mint
pub const USDC_MINT_DEVNET: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";
/// Mainnet USDC mint
pub const USDC_MINT_MAINNET: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
/// Mainnet USDT mint
pub const USDT_MINT_MAINNET: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";

/// Default protocol fee: 2% of yield
pub const DEFAULT_FEE_BPS: u16 = 200;

/// Minimum cliff period: 30 days in seconds (1 second in testing mode)
#[cfg(not(feature = "testing"))]
pub const MIN_CLIFF_SECONDS: i64 = 30 * 24 * 60 * 60;
#[cfg(feature = "testing")]
pub const MIN_CLIFF_SECONDS: i64 = 1;

/// Basis points denominator
pub const BPS_DENOMINATOR: u16 = 10_000;

/// PDA seeds
pub const CONFIG_SEED: &[u8] = b"config";
pub const VAULT_SEED: &[u8] = b"vault";
pub const MEMBER_SEED: &[u8] = b"member";
pub const VAULT_TOKEN_SEED: &[u8] = b"vault_token";

/// V2 PDA seeds
pub const SCORE_SEED: &[u8] = b"score";
pub const MILESTONE_SEED: &[u8] = b"milestone";

/// V3 PDA seeds
pub const REPUTATION_SEED: &[u8] = b"reputation";
pub const POOL_SEED: &[u8] = b"pool";
pub const POOL_TOKEN_SEED: &[u8] = b"pool_token";
pub const BETTOR_SEED: &[u8] = b"bettor";
