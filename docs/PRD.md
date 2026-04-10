# Gecko Vault — Product Requirements Document

**Version:** 1.0 · Phase 1 (Hackathon)
**Program:** `gecko-vault` · Anchor 0.32 · Solana Devnet
**Program ID:** `Eeyc1AXnQxmbMoKhJRz8g6soBpCkjwfi79DrhWwNeSh3`

---

## 1. Problem

Creator marketing runs on trust: brands transfer budgets after delivery or, in better deals, upfront via wire. Neither arrangement is enforceable.

- **Brands** send briefs over email, budgets over bank wire, and have no leverage once a creator ghosts or delivers below spec.
- **Creators** deliver first and invoice later — net-30 becomes net-never, scope creep is punished on their side, and there is no verifiable record of what was agreed.

The status quo is a handshake economy operating at web3 scale.

---

## 2. Goal

Replace the handshake with an on-chain deal primitive that is **enforceable by the program, not by policy**.

- Sponsor funds cannot be withdrawn before the cliff — **the program rejects the instruction**, not a terms-of-service clause.
- Creator yield share is determined by basis points written on-chain at campaign launch, not by a spreadsheet sent three weeks later.
- Every state transition — deposit, add/remove creator, yield route, close — is a signed, auditable Solana transaction.

---

## 3. Users

| Role | Description |
|------|-------------|
| **Sponsor (brand)** | Creates campaign vaults, locks USDC/USDT, assembles creator squads, reviews deliverables, closes vault after cliff |
| **Creator** | Accepts squad membership, delivers campaign posts, receives proportional yield routed by automation |
| **Automation authority** | Helius webhook relayer; the only signer allowed to call `route_yield` — never a user key |
| **Protocol admin** | Deploys and owns the singleton `GeckoConfig`; sets treasury, fee, and allowed mints |

---

## 4. Core Concepts

### 4.1 Campaign Vault (`SponsorVault`)

One PDA per campaign. Holds locked principal in a program-owned SPL token account. The sponsor cannot touch the tokens before `cliff_ts` — `close_vault` enforces this on-chain.

```
Seeds: ["vault", sponsor_pubkey, campaign_id_le_bytes]
```

### 4.2 Creator Squad (`SquadMember`)

One PDA per creator per vault. Stores `allocation_bps` — this creator's fractional claim on yield. All squad members' allocations must sum to exactly **10,000 bps** before `route_yield` is callable. This prevents partial squads from draining the vault.

```
Seeds: ["member", vault_pubkey, creator_pubkey]
```

### 4.3 Protocol Config (`GeckoConfig`)

Singleton PDA. Set once after deploy. Holds:
- Treasury wallet (receives protocol fee)
- `automation_authority` (the only signer for `route_yield`)
- `fee_bps` (default 2%)
- `allowed_mints` (USDC, USDT — whitelist, max 5)

```
Seeds: ["config"]
```

---

## 5. Instruction Set

| Instruction | Signer | Description |
|-------------|--------|-------------|
| `init_config` | Admin | Deploy singleton config; one-time post-deploy |
| `init_vault` | Sponsor | Create campaign vault with cliff/end durations |
| `deposit` | Sponsor | Lock stablecoins into the vault token account |
| `add_creator` | Sponsor | Add creator to squad with `allocation_bps` |
| `remove_creator` | Sponsor | Remove creator, reclaim allocation and rent |
| `route_yield` | Automation authority | Distribute one creator's yield share for an epoch |
| `close_vault` | Sponsor | Reclaim principal after cliff; closes vault token account |

---

## 6. Vault Lifecycle

```
init_vault
    │
    ▼
[Active] ──deposit──► principal locked
    │
    ├──add_creator / remove_creator (squad management)
    │
    ├──route_yield (epoch-by-epoch, per member, automation only)
    │
    └──close_vault (after cliff_ts) ──► [Closed]
                                        principal returned to sponsor
                                        vault_token_account closed (rent reclaimed)
```

**Cliff enforcement:** `close_vault` reads `Clock::get()?.unix_timestamp` and rejects with `CliffNotElapsed` if `now < cliff_ts`. The UI may surface a countdown, but the program is the final gate.

---

## 7. Yield Routing — Phase 1

`route_yield` receives a `yield_amount` parameter (oracle-provided in Phase 1) representing one member's pre-calculated share for the epoch:

```
yield_amount = total_epoch_yield × (member.allocation_bps / 10_000)
```

The program then:
1. Checks `vault_token_account.amount − principal ≥ yield_amount` — principal is never touched.
2. Deducts `gecko_fee = yield_amount × config.fee_bps / 10_000` → treasury token account.
3. Transfers remainder → creator token account.
4. Updates `member.total_received` and `vault.total_yield_routed`.

**Phase 2 (post-hackathon):** Replace step 3 with a Streamflow CPI that tops up the creator's real-time payment stream. The `SquadMember.stream_id` field is reserved for the stream account.

---

## 8. Yield Source — Phase 1 vs Phase 2

| Phase | Yield source | Delivery mechanism |
|-------|-------------|-------------------|
| **Phase 1 (current)** | Oracle-provided `yield_amount` parameter (simulated on devnet) | Direct SPL token transfer |
| **Phase 2** | Kamino lending position delta (actual DeFi yield on principal) | Streamflow CPI stream top-up |

In Phase 1 the vault holds raw stablecoins. In Phase 2, `deposit` will CPI into Kamino so principal earns real on-chain yield; the `vault_token_account` will hold kTokens instead.

---

## 9. Protocol Fees

- Default fee: **2% of yield** (`fee_bps = 200`)
- Fee is snapshotted into `vault.gecko_fee_bps` at vault creation — mid-campaign fee changes do not affect existing vaults.
- Fee flows to the `GeckoConfig.treasury` wallet on each `route_yield` call.

---

## 10. Security Model

| Threat | Mitigation |
|--------|-----------|
| Sponsor rug-pull before cliff | `close_vault` enforces `now ≥ cliff_ts` on-chain |
| Unauthorized yield drain | `route_yield` requires `config.automation_authority` as signer |
| Unsupported mint deposit | `init_vault` checks mint against `config.allowed_mints` |
| Yield dipping into principal | `route_yield` checks `balance − principal ≥ yield_amount` |
| Unauthorized vault modification | All vault mutations check `has_one = sponsor` |
| Over-allocation | `add_creator` rejects if `total_allocation_bps + new > 10_000` |

---

## 11. Constants & Limits

| Constant | Value | Notes |
|----------|-------|-------|
| `MIN_CLIFF_SECONDS` | 2,592,000 (30 days) | 1 second with `testing` feature |
| `DEFAULT_FEE_BPS` | 200 | 2% — set at config init |
| `BPS_DENOMINATOR` | 10,000 | All allocations must sum to this |
| `allowed_mints` max | 5 | `#[max_len(5)]` on `GeckoConfig` |
| Max creators per vault | Unlimited | Each `SquadMember` is a separate PDA |

---

## 12. Out of Scope (Phase 1)

- **Oracle-driven post verification** — deliverable proof is off-chain (sponsor review). No Twitter/Instagram/YouTube attestation yet.
- **Kamino CPI** — principal earns no real yield on devnet in Phase 1; yield amounts are oracle-provided.
- **Streamflow integration** — `SquadMember.stream_id` is `Pubkey::default()` until Phase 2.
- **Mainnet deployment** — Phase 1 targets Solana devnet.
- **Governance / fee updates** — `init_config` sets fee once; no update instruction in Phase 1.

---

## 13. Deployment Checklist

- [ ] `anchor build` (without `testing` feature) produces a clean `.so`
- [ ] `anchor deploy --provider.cluster devnet` succeeds
- [ ] `scripts/init-devnet.ts` initializes `GeckoConfig` with correct treasury, automation authority, and USDC mint
- [ ] `security.json` written on-chain via `@solana-program/program-metadata`
- [ ] IDL synced to API and app repos via `scripts/sync-idl.sh --devnet`
- [ ] Program verified on Solana Explorer (devnet)

---

*Gecko — enforcement-first creator campaigns on Solana*
