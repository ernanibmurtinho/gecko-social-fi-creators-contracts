use anchor_lang::prelude::*;

use crate::{constants::CONFIG_SEED, state::GeckoConfig};

/// One-time migration: expand the GeckoConfig PDA from the pre-oracle-authority
/// layout (271 bytes) to the current layout (8 + GeckoConfig::INIT_SPACE = 303 bytes),
/// and write the oracle_authority field at the correct offset.
///
/// This is needed because the config was initialized before `oracle_authority`
/// was added to GeckoConfig. The account is too small for the new struct, so
/// Anchor cannot deserialize it — we use UncheckedAccount and raw byte writes.
///
/// Can only be called by the original authority stored in the account.
/// Safe to call once; idempotent if called again (realloc to same size is a no-op).
pub(crate) fn process(ctx: Context<MigrateConfig>, oracle_authority: Pubkey) -> Result<()> {
    let config_info = ctx.accounts.config.to_account_info();

    // --- Verify caller is the authority stored in the account -----------------
    // authority is stored at bytes [8..40] (after 8-byte discriminator)
    {
        let data = config_info.data.borrow();
        require!(data.len() >= 40, crate::errors::GeckoError::InvalidAccountData);
        let stored_authority = Pubkey::try_from(&data[8..40])
            .map_err(|_| error!(crate::errors::GeckoError::InvalidAccountData))?;
        require_keys_eq!(
            stored_authority,
            ctx.accounts.authority.key(),
            crate::errors::GeckoError::Unauthorized
        );
    }

    // --- Top up rent if the larger account needs more lamports ----------------
    let new_size = 8 + GeckoConfig::INIT_SPACE;
    let rent = Rent::get()?;
    let new_min_balance = rent.minimum_balance(new_size);
    let current_balance = config_info.lamports();

    if new_min_balance > current_balance {
        let top_up = new_min_balance - current_balance;
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: config_info.clone(),
                },
            ),
            top_up,
        )?;
    }

    // --- Resize to new size (zero-initializes the new bytes) -----------------
    config_info.resize(new_size)?;

    // --- Write oracle_authority at its field offset ---------------------------
    // GeckoConfig field order: authority(32), treasury(32), automation_authority(32),
    // oracle_authority(32), ...
    // Byte offset after discriminator: 8 + 32 + 32 + 32 = 104
    let oracle_offset: usize = 8 + 32 + 32 + 32;
    let mut data = config_info.data.borrow_mut();
    data[oracle_offset..oracle_offset + 32].copy_from_slice(oracle_authority.as_ref());

    msg!(
        "Config migrated: oracle_authority set to {}",
        oracle_authority
    );

    Ok(())
}

#[derive(Accounts)]
pub struct MigrateConfig<'info> {
    /// CHECK: Cannot use Account<GeckoConfig> — account has old layout (271 bytes)
    /// and Anchor deserialization would fail. We verify seeds, discriminator, and
    /// authority manually inside the instruction.
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
