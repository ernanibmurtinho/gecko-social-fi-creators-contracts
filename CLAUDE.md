# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

This is a Solana smart contract project built with the Anchor framework. The `gecko-vault` program implements a sponsor-creator yield-routing protocol: sponsors lock stablecoins (USDC/USDT) in campaign vaults, and yield generated is distributed to a squad of creators proportionally.

## Commands

### Build

```bash
# Standard build
anchor build

# Build with testing feature (relaxes MIN_CLIFF_SECONDS to 1 second)
anchor build -- --features testing
```

### Test

```bash
# Run all integration tests (builds with testing feature automatically)
anchor test

# Run a single test file
yarn run ts-mocha -p ./tsconfig.json -t 1000000 "tests/gecko-vault.ts"

# Run a specific test by name (grep pattern)
yarn run ts-mocha -p ./tsconfig.json -t 1000000 "tests/**/*.ts" --grep "route yield"
```

### Lint / Format

```bash
# Check formatting (TypeScript/JS)
yarn lint

# Fix formatting
yarn lint:fix
```

### Local Validator

```bash
# Start localnet and deploy (uses Anchor.toml cluster = "localnet")
anchor localnet

# Deploy to devnet
anchor deploy --provider.cluster devnet
```

### Toolchain

Rust 1.89.0 (pinned in `rust-toolchain.toml`). Anchor uses `yarn` as package manager (see `Anchor.toml`).

## Architecture

### Program: `gecko-vault`

Program ID: `Eeyc1AXnQxmbMoKhJRz8g6soBpCkjwfi79DrhWwNeSh3`

#### Account Model (PDAs)

| Account | Seeds | Description |
|---|---|---|
| `GeckoConfig` | `["config"]` | Singleton protocol config — admin, treasury, fee_bps, allowed mints |
| `SponsorVault` | `["vault", sponsor, campaign_id_le_bytes]` | One per campaign — holds principal amount, cliff/end timestamps, member count |
| `vault_token_account` | `["vault_token", vault]` | SPL token account owned by the vault PDA, holds locked principal |
| `SquadMember` | `["member", vault, creator]` | One per creator per vault — stores allocation_bps and cumulative yield received |

#### Instruction Flow

1. **`init_config`** — admin deploys once; sets treasury, automation authority (Helius relayer), and allowed stablecoin mints
2. **`init_vault`** — sponsor creates a campaign vault with cliff/end durations and a chosen stablecoin mint
3. **`deposit`** — sponsor transfers stablecoins into the vault token account
4. **`add_creator`** — sponsor adds creators to the squad with `allocation_bps`; all members' bps must sum to exactly 10,000 before yield routing works
5. **`remove_creator`** — sponsor removes a creator; frees their allocation and closes their `SquadMember` PDA (rent reclaimed)
6. **`route_yield`** — called by `automation_authority` (Helius webhook relayer); distributes one creator's yield share per call. Deducts protocol fee → treasury, remainder → creator token account. Enforces `vault_balance - principal >= yield_amount` so principal is never touched
7. **`close_vault`** — sponsor reclaims principal after cliff timestamp; closes the vault token account

#### Key Invariants

- `total_allocation_bps` must equal 10,000 for `route_yield` to execute
- `route_yield` never touches locked principal — only the balance above `vault.principal`
- `route_yield` is called once per member per epoch by the automation authority; `yield_amount` is pre-calculated by the caller (Phase 1: oracle-provided; Phase 2 TODO: derived from Kamino position delta)
- Protocol fee is `config.fee_bps` (default 200 = 2%), snapshotted into `vault.gecko_fee_bps` at vault creation
- The `testing` feature flag reduces `MIN_CLIFF_SECONDS` from 30 days to 1 second — always build with this flag when running tests

#### Vault Lifecycle

`Active` → (cliff elapsed) → `Cliffed` → (close_vault called) → `Closed`

Note: `Cliffed` status is set in `close_vault` logic by checking `now >= cliff_ts`; the vault status in code transitions directly from `Active` to `Closed` via `close_vault`.

#### Phase 2 TODOs

`route_yield` currently transfers tokens directly to creators. Phase 2 will replace this with Streamflow CPI calls to top-up real-time payment streams. `SquadMember.stream_id` is reserved for this (set to `Pubkey::default()` until then).

### Test Structure

All tests are in `tests/gecko-vault.ts` and run on localnet with the `testing` feature. PDA derivation helpers (`configPda`, `vaultPda`, `vaultTokenPda`, `memberPda`) in the test file match the on-chain seeds exactly. Tests use a mock stablecoin mint (not real USDC) to avoid devnet dependencies.

### Environment

Copy `.env.example` to `.env`. Required for devnet/mainnet deployments:
- `HELIUS_RPC_DEVNET` / `HELIUS_RPC_MAINNET` — faster RPC than public endpoints
- `ANCHOR_WALLET` — path to Solana keypair used for signing
