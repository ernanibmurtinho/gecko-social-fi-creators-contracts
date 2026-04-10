# Gecko Vault — On-chain deal primitive for creator campaigns

[![Solana](https://img.shields.io/badge/Solana-Devnet-14F195?logo=solana)](https://explorer.solana.com/address/Eeyc1AXnQxmbMoKhJRz8g6soBpCkjwfi79DrhWwNeSh3?cluster=devnet)
[![Anchor](https://img.shields.io/badge/Anchor-0.32-9945FF)](https://www.anchor-lang.com/)
[![Rust](https://img.shields.io/badge/Rust-1.89-orange?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-ISC-blue)](LICENSE)

**Gecko Vault** is the on-chain program powering the Gecko protocol — a programmable, enforceable **deal primitive** on Solana for brand–creator campaigns. Sponsors lock stablecoins in smart contract vaults with **cliff enforcement**; creators earn **proportional yield** routed by automation, settled after the campaign period. The program is the final authority: no policy document, no terms of service — the instruction either succeeds or the program rejects it.

---

## The problem

Creator marketing is **trust-based**: brands fear creators ghosting after briefs; creators fear brands renegotiating or delaying pay after delivery. Deals run on email, screenshots, and PDFs — not verifiable commitment.

---

## The solution (Gecko inversion)

**Lock the brand's commitment from day one.** Funds sit in an on-chain vault; **early withdrawal is rejected by the program** (`CliffNotElapsed`), not by policy. Creators work knowing capital cannot be pulled arbitrarily; sponsors close the vault only when the cliff period has elapsed — enforced by `Clock::get()`, not a human.

| Side | Before | With Gecko |
|------|--------|------------|
| Brand | Upfront risk, weak recourse | **On-chain lock** + full audit trail |
| Creator | Net-30 → net-never | **Binding yield floor** tied to squad allocation |

---

## What this repository is

This repo contains the **Anchor smart contract** (`gecko-vault`) deployed on Solana, plus the TypeScript integration tests and deployment scripts. On-chain state is the source of truth for all money and cliff logic.

**See also:** the sibling repos for the API (transaction builders + vault reads) and the Next.js frontend. The program ID used by both is configuration; only this repo owns the deployed bytecode.

---

## End-to-end flow

```
Deploy    Admin     anchor deploy → scripts/init-devnet.ts
                    → GeckoConfig: treasury, automation_authority, allowed_mints

Day 0     Sponsor   init_vault (campaign_id, cliff_seconds, end_seconds)
                    deposit (USDC/USDT → vault_token_account)
                    add_creator × N  (allocation_bps, must sum to 10_000)

Active    Epoch     route_yield (automation_authority signs, once per creator per epoch)
                    → protocol fee → treasury
                    → creator share → creator token account

Close     Sponsor   close_vault (only after cliff_ts elapsed)
                    → principal returned, vault_token_account closed, rent reclaimed
```

### Roles and guarantees

| Role | Responsibility | Enforcement |
|------|----------------|-------------|
| **Sponsor (brand)** | Lock USDC, set cliff/squad, review deliverables | Cannot withdraw before cliff — **program** |
| **Creator** | Deliver campaign posts; wallet registered in squad | Yield share fixed by `allocation_bps` at add time |
| **Automation authority** | Calls `route_yield` per epoch per creator | Only signer allowed — `config.automation_authority` checked on-chain |
| **Protocol admin** | Owns `GeckoConfig`; sets fee, treasury, allowed mints | One-time `init_config`; no update path in Phase 1 |

---

## Architecture

### Account model (PDAs)

```
GeckoConfig ["config"]
│   Singleton — admin, treasury, fee_bps, allowed_mints
│
└── SponsorVault ["vault", sponsor, campaign_id_le]
    │   Principal, cliff_ts, end_ts, member_count, total_allocation_bps, status
    │
    ├── vault_token_account ["vault_token", vault]
    │   Program-owned SPL token account — holds locked principal
    │
    └── SquadMember ["member", vault, creator]  × N
            allocation_bps, total_received, stream_id (Phase 2)
```

### Vault lifecycle

```
Active ──(cliff_ts elapsed)──► close_vault ──► Closed
  │
  └── route_yield (per epoch, per member)
      Constraint: vault_balance − principal ≥ yield_amount
      (principal is never touched by automation)
```

### Yield routing — Phase 1 vs Phase 2

| Phase | Yield source | Delivery |
|-------|-------------|---------|
| **Phase 1 (current)** | Oracle-provided `yield_amount` (simulated on devnet) | Direct SPL transfer |
| **Phase 2** | Kamino lending position delta on locked principal | Streamflow CPI stream top-up |

---

## Instruction reference

| Instruction | Signer | Key constraint |
|-------------|--------|---------------|
| `init_config` | Admin | One-time; initializes singleton |
| `init_vault` | Sponsor | `cliff_seconds ≥ MIN_CLIFF_SECONDS`; mint must be in `allowed_mints` |
| `deposit` | Sponsor | Vault must be `Active`; increments `principal` |
| `add_creator` | Sponsor | `total_allocation_bps + new ≤ 10_000` |
| `remove_creator` | Sponsor | Cannot remove last member; frees allocation, closes PDA |
| `route_yield` | Automation authority | `total_allocation_bps == 10_000`; balance check protects principal |
| `close_vault` | Sponsor | `now ≥ cliff_ts`; closes token account, marks `Closed` |

---

## Stack

| Layer | Choice |
|-------|--------|
| Smart contract | Anchor 0.32, Rust 1.89 |
| Token standard | SPL Token (USDC / USDT) |
| Automation | Helius webhook relayer (`automation_authority`) |
| Yield (Phase 2) | Kamino lending CPI |
| Streaming (Phase 2) | Streamflow protocol CPI |
| Test framework | Mocha + Chai via `ts-mocha` |
| Package manager | Yarn (contracts) |

---

## Prerequisites

- **Rust** 1.89 (pinned in `rust-toolchain.toml` — `rustup` will auto-install)
- **Solana CLI** ≥ 1.18 with a funded devnet wallet at `~/.config/solana/id.json`
- **Anchor CLI** 0.32 (`cargo install --git https://github.com/coral-xyz/anchor avm --locked && avm install 0.32.0`)
- **Node.js** ≥ 18 and **Yarn**

---

## Quickstart

```bash
yarn install

# Run tests on localnet (builds with testing feature — MIN_CLIFF_SECONDS = 1s)
anchor test

# Build for devnet (no testing feature)
anchor build

# Deploy to devnet
anchor deploy --provider.cluster devnet

# Initialize GeckoConfig (one-time, post-deploy)
npx ts-node --transpile-only scripts/init-devnet.ts

# Write security.txt on-chain
npx @solana-program/program-metadata@latest write security \
  Eeyc1AXnQxmbMoKhJRz8g6soBpCkjwfi79DrhWwNeSh3 \
  ./security.json

# Sync IDL + TypeScript types to sibling repos
./scripts/sync-idl.sh --devnet
```

---

## Environment

Copy `.env.example` to `.env`:

| Variable | Role |
|----------|------|
| `ANCHOR_PROVIDER_URL` | Solana RPC endpoint (devnet or localnet) |
| `ANCHOR_WALLET` | Path to deployer keypair |
| `HELIUS_RPC_DEVNET` | Helius RPC for faster devnet access |
| `HELIUS_RPC_MAINNET` | Helius RPC for mainnet (Phase 2) |

---

## Program ID

```
Eeyc1AXnQxmbMoKhJRz8g6soBpCkjwfi79DrhWwNeSh3
```

[Explorer (devnet)](https://explorer.solana.com/address/Eeyc1AXnQxmbMoKhJRz8g6soBpCkjwfi79DrhWwNeSh3?cluster=devnet)

---

## Repository layout

```
gecko-social-fi-creators-contracts/
├── programs/gecko-vault/src/
│   ├── lib.rs                  # Program entry, instruction dispatch
│   ├── constants.rs            # Seeds, fee defaults, cliff minimum
│   ├── errors.rs               # GeckoError enum
│   ├── state/                  # GeckoConfig, SponsorVault, SquadMember
│   └── instructions/           # One file per instruction
├── tests/gecko-vault.ts        # Full lifecycle integration tests
├── scripts/
│   ├── init-devnet.ts          # One-time GeckoConfig bootstrap
│   └── sync-idl.sh             # Distribute IDL + types to sibling repos
├── migrations/deploy.ts        # Anchor deploy hook (extend as needed)
├── security.json               # On-chain security.txt metadata
├── Anchor.toml                 # Program IDs, cluster, test command
├── Cargo.toml                  # Workspace definition
└── rust-toolchain.toml         # Pinned Rust version
```

---

## Documentation map

| Doc | Contents |
|-----|----------|
| [`CLAUDE.md`](CLAUDE.md) | Commands, architecture runbook for AI contributors |
| [`docs/PRD.md`](docs/PRD.md) | Full product requirements, security model, phase roadmap |
| [`security.json`](security.json) | On-chain security disclosure metadata |

---

## Implementation status

- **Cliff enforcement:** Fully implemented — `close_vault` checks `Clock::get()` on-chain; cliff is **30 days** minimum in production, **1 second** in the `testing` feature build.
- **Yield routing (Phase 1):** Implemented — direct SPL transfers with 2% protocol fee; `yield_amount` is oracle-provided (simulated on devnet).
- **Streamflow streams:** `SquadMember.stream_id` is reserved — `Pubkey::default()` until Phase 2 CPI is wired.
- **Kamino CPI:** Not yet — `deposit` holds raw stablecoins in Phase 1; Phase 2 will deposit into Kamino and hold kTokens.
- **Oracle / post verification:** Not implemented — creator deliverable proof is off-chain; no automated social media attestation yet.
- **Mainnet:** Targeting Solana devnet for the hackathon; mainnet readiness requires audit.

---

## Security

- Program keypair (`target/deploy/gecko_vault-keypair.json`) is **gitignored** — never commit it.
- The `automation_authority` in `GeckoConfig` is the only signer allowed to call `route_yield` — it is a server keypair, never a user's embedded wallet.
- All vault mutations verify `has_one = sponsor` — no cross-vault interference is possible.
- Report vulnerabilities via [`security.json`](security.json) contacts or open a [GitHub Security Advisory](https://github.com/ernanibmurtinho/gecko-social-fi-creators-contracts/security/advisories/new).

---

## License

See [LICENSE](LICENSE).

---

*Gecko — enforcement-first creator campaigns on Solana · Anchor · Helius · Privy*
