use anchor_lang::prelude::*;

use crate::constants::{CONFIG_SEED, DEFAULT_FEE_BPS};

/// Repairs a GeckoConfig PDA whose fee_bps / bump / allowed_mints fields were
/// corrupted by the V2→V3 migrate_config instruction.
///
/// migrate_config correctly wrote oracle_authority at [104..136] but overwrote
/// the old fee_bps/bump/mints that lived at those offsets in the V2 layout.
/// The V3 layout expects fee_bps at [136], bump at [138], mints at [139+].
///
/// This instruction re-writes those fields at the correct V3 offsets using raw
/// byte manipulation (UncheckedAccount) to bypass the broken deserialization.
///
/// Callable only by the original authority stored in the account ([8..40]).
pub(crate) fn process(
    ctx: Context<RepairConfig>,
    oracle_authority: Pubkey,
    allowed_mints: Vec<Pubkey>,
) -> Result<()> {
    let config_info = ctx.accounts.config.to_account_info();

    // --- Verify caller is the stored authority ----------------------------
    {
        let data = config_info.data.borrow();
        require!(
            data.len() >= 143,
            crate::errors::GeckoError::InvalidAccountData
        );
        let stored_authority = Pubkey::try_from(&data[8..40])
            .map_err(|_| error!(crate::errors::GeckoError::InvalidAccountData))?;
        require_keys_eq!(
            stored_authority,
            ctx.accounts.authority.key(),
            crate::errors::GeckoError::Unauthorized
        );
    }

    // --- Derive bump for the config PDA -----------------------------------
    let (_, bump) =
        Pubkey::find_program_address(&[CONFIG_SEED], ctx.program_id);

    // --- Write oracle_authority at its correct V3 offset (104..136) -------
    // (may already be set; writing again is idempotent)
    let oracle_offset: usize = 8 + 32 + 32 + 32; // = 104
    {
        let mut data = config_info.data.borrow_mut();
        data[oracle_offset..oracle_offset + 32].copy_from_slice(oracle_authority.as_ref());
    }

    // --- Write fee_bps at V3 offset (136..138) ----------------------------
    let fee_offset: usize = 8 + 32 + 32 + 32 + 32; // = 136
    {
        let mut data = config_info.data.borrow_mut();
        let fee_bytes = DEFAULT_FEE_BPS.to_le_bytes();
        data[fee_offset..fee_offset + 2].copy_from_slice(&fee_bytes);
    }

    // --- Write bump at V3 offset (138) ------------------------------------
    {
        let mut data = config_info.data.borrow_mut();
        data[138] = bump;
    }

    // --- Write allowed_mints vec at V3 offset (139+) ----------------------
    // Layout: u32 length (4 bytes) + up to 5 Pubkey (32 bytes each) = 164 bytes
    let vec_offset: usize = 139;
    let max_mints: usize = 5;
    let n = allowed_mints.len().min(max_mints);

    {
        let mut data = config_info.data.borrow_mut();
        // vec length prefix
        let len_bytes = (n as u32).to_le_bytes();
        data[vec_offset..vec_offset + 4].copy_from_slice(&len_bytes);

        // mint pubkeys
        for (i, mint) in allowed_mints.iter().take(max_mints).enumerate() {
            let start = vec_offset + 4 + i * 32;
            data[start..start + 32].copy_from_slice(mint.as_ref());
        }
        // zero-pad remaining slots
        for i in n..max_mints {
            let start = vec_offset + 4 + i * 32;
            data[start..start + 32].copy_from_slice(&[0u8; 32]);
        }
    }

    msg!(
        "Config repaired: fee_bps={} bump={} mints={} oracle={}",
        DEFAULT_FEE_BPS,
        bump,
        allowed_mints.len(),
        oracle_authority
    );

    Ok(())
}

#[derive(Accounts)]
pub struct RepairConfig<'info> {
    /// CHECK: Cannot use Account<GeckoConfig> — Borsh deserialization fails on
    /// the corrupted account. We verify seeds, discriminator bytes, and authority
    /// manually inside the instruction.
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump,
        owner = crate::ID,
    )]
    pub config: UncheckedAccount<'info>,

    /// Must match the authority stored at bytes [8..40] of the config account.
    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}
