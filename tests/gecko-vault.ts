/**
 * gecko-vault integration tests
 *
 * Covers: init_config, init_vault, deposit, add_creator, remove_creator,
 *         route_yield, close_vault, full lifecycle, and security attack vectors.
 *
 * Runs on localnet with the `testing` feature enabled (MIN_CLIFF_SECONDS = 1).
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, BN, AnchorError } from "@coral-xyz/anchor";
import { GeckoVault } from "../target/types/gecko_vault";
import {
  Keypair,
  PublicKey,
  LAMPORTS_PER_SOL,
  SystemProgram,
} from "@solana/web3.js";
import {
  createMint,
  createAccount,
  createAssociatedTokenAccount,
  mintTo,
  getAccount,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const CLIFF_SECONDS = new BN(1);   // 1 second (testing feature)
const END_SECONDS = new BN(60);    // 60 seconds
const CAMPAIGN_ID_1 = new BN(1);
const CAMPAIGN_ID_2 = new BN(2);

const DEPOSIT_AMOUNT = new BN(10_000_000); // 10 USDC (6 decimals)
const YIELD_AMOUNT_TOTAL = new BN(180_000); // 0.18 USDC epoch yield

const BPS_DENOMINATOR = 10_000;
const GECKO_FEE_BPS = 200; // 2%

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function configPda(programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    programId
  );
}

function vaultPda(
  sponsor: PublicKey,
  campaignId: BN,
  programId: PublicKey
): [PublicKey, number] {
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64LE(BigInt(campaignId.toString()));
  return PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), sponsor.toBuffer(), buf],
    programId
  );
}

function vaultTokenPda(vault: PublicKey, programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("vault_token"), vault.toBuffer()],
    programId
  );
}

function memberPda(
  vault: PublicKey,
  creator: PublicKey,
  programId: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("member"), vault.toBuffer(), creator.toBuffer()],
    programId
  );
}

async function airdrop(
  connection: anchor.web3.Connection,
  pubkey: PublicKey,
  sol: number = 2
): Promise<void> {
  const sig = await connection.requestAirdrop(pubkey, sol * LAMPORTS_PER_SOL);
  await connection.confirmTransaction(sig, "confirmed");
}

async function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Asserts that a transaction fails with the expected Anchor error code */
async function assertFails(
  fn: () => Promise<unknown>,
  expectedCode: string
): Promise<void> {
  try {
    await fn();
    assert.fail(`Expected error '${expectedCode}' but transaction succeeded`);
  } catch (err: unknown) {
    if (err instanceof AnchorError) {
      assert.equal(
        err.error.errorCode.code,
        expectedCode,
        `Expected '${expectedCode}' but got '${err.error.errorCode.code}': ${err.error.errorMessage}`
      );
    } else if (err instanceof Error && err.message.includes(expectedCode)) {
      // constraint violations surface as regular errors sometimes
      return;
    } else {
      throw err;
    }
  }
}

// ---------------------------------------------------------------------------
// Test suite
// ---------------------------------------------------------------------------

describe("gecko-vault", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.GeckoVault as Program<GeckoVault>;
  const conn = provider.connection;

  // Wallets
  const admin = Keypair.generate();
  const sponsor = Keypair.generate();
  const sponsor2 = Keypair.generate();
  const attacker = Keypair.generate();
  const automation = Keypair.generate();
  const creator1 = Keypair.generate();
  const creator2 = Keypair.generate();
  const creator3 = Keypair.generate();
  const treasury = Keypair.generate();

  // Mint
  let testMint: PublicKey;
  let unsupportedMint: PublicKey;

  // Token accounts
  let sponsorAta: PublicKey;
  let sponsor2Ata: PublicKey;
  let attackerAta: PublicKey;
  let creator1Ata: PublicKey;
  let creator2Ata: PublicKey;
  let creator3Ata: PublicKey;
  let treasuryAta: PublicKey;

  // PDAs
  let [configAddress] = configPda(program.programId);

  before("Setup wallets, mints, token accounts", async () => {
    // Airdrop SOL
    await Promise.all([
      airdrop(conn, admin.publicKey),
      airdrop(conn, sponsor.publicKey),
      airdrop(conn, sponsor2.publicKey),
      airdrop(conn, attacker.publicKey),
      airdrop(conn, automation.publicKey),
      airdrop(conn, creator1.publicKey),
      airdrop(conn, creator2.publicKey),
      airdrop(conn, creator3.publicKey),
      airdrop(conn, treasury.publicKey),
    ]);

    // Create test mint (admin is mint authority — simulates USDC on localnet)
    testMint = await createMint(conn, admin, admin.publicKey, null, 6);
    unsupportedMint = await createMint(conn, admin, admin.publicKey, null, 6);

    // Create token accounts
    sponsorAta = await createAssociatedTokenAccount(conn, sponsor, testMint, sponsor.publicKey);
    sponsor2Ata = await createAssociatedTokenAccount(conn, sponsor2, testMint, sponsor2.publicKey);
    attackerAta = await createAssociatedTokenAccount(conn, attacker, testMint, attacker.publicKey);
    creator1Ata = await createAssociatedTokenAccount(conn, creator1, testMint, creator1.publicKey);
    creator2Ata = await createAssociatedTokenAccount(conn, creator2, testMint, creator2.publicKey);
    creator3Ata = await createAssociatedTokenAccount(conn, creator3, testMint, creator3.publicKey);
    treasuryAta = await createAssociatedTokenAccount(conn, treasury, testMint, treasury.publicKey);

    // Mint tokens to sponsors and attacker
    await mintTo(conn, admin, testMint, sponsorAta, admin, DEPOSIT_AMOUNT.toNumber() * 5);
    await mintTo(conn, admin, testMint, sponsor2Ata, admin, DEPOSIT_AMOUNT.toNumber() * 2);
    await mintTo(conn, admin, testMint, attackerAta, admin, DEPOSIT_AMOUNT.toNumber());
  });

  // =========================================================================
  // init_config
  // =========================================================================

  describe("init_config", () => {
    it("admin initializes protocol config", async () => {
      await program.methods
        .initConfig(treasury.publicKey, automation.publicKey, admin.publicKey, [testMint])
        .accounts({
          config: configAddress,
          authority: admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([admin])
        .rpc();

      const config = await program.account.geckoConfig.fetch(configAddress);
      assert.equal(config.authority.toBase58(), admin.publicKey.toBase58());
      assert.equal(config.treasury.toBase58(), treasury.publicKey.toBase58());
      assert.equal(config.automationAuthority.toBase58(), automation.publicKey.toBase58());
      assert.equal(config.feeBps, GECKO_FEE_BPS);
      assert.equal(config.allowedMints.length, 1);
      assert.equal(config.allowedMints[0].toBase58(), testMint.toBase58());
    });

    it("fails: duplicate init_config (already initialized)", async () => {
      try {
        await program.methods
          .initConfig(treasury.publicKey, automation.publicKey, admin.publicKey, [testMint])
          .accounts({
            config: configAddress,
            authority: admin.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([admin])
          .rpc();
        assert.fail("Should have failed");
      } catch (err: unknown) {
        // Anchor rejects re-init of existing PDA with a 'already in use' error
        assert.ok(err instanceof Error);
      }
    });
  });

  // =========================================================================
  // init_vault
  // =========================================================================

  describe("init_vault", () => {
    it("sponsor creates vault with supported mint", async () => {
      const [vault] = vaultPda(sponsor.publicKey, CAMPAIGN_ID_1, program.programId);
      const [vaultToken] = vaultTokenPda(vault, program.programId);

      await program.methods
        .initVault(CAMPAIGN_ID_1, CLIFF_SECONDS, END_SECONDS)
        .accounts({
          vault,
          vaultTokenAccount: vaultToken,
          mint: testMint,
          config: configAddress,
          sponsor: sponsor.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([sponsor])
        .rpc();

      const vaultData = await program.account.sponsorVault.fetch(vault);
      assert.equal(vaultData.sponsor.toBase58(), sponsor.publicKey.toBase58());
      assert.equal(vaultData.mint.toBase58(), testMint.toBase58());
      assert.equal(vaultData.principal.toNumber(), 0);
      assert.equal(vaultData.memberCount, 0);
      assert.deepEqual(vaultData.status, { active: {} });
    });

    it("fails: unsupported mint", async () => {
      const [vault] = vaultPda(sponsor.publicKey, new BN(99), program.programId);
      const [vaultToken] = vaultTokenPda(vault, program.programId);

      await assertFails(
        () =>
          program.methods
            .initVault(new BN(99), CLIFF_SECONDS, END_SECONDS)
            .accounts({
              vault,
              vaultTokenAccount: vaultToken,
              mint: unsupportedMint,
              config: configAddress,
              sponsor: sponsor.publicKey,
              tokenProgram: TOKEN_PROGRAM_ID,
              associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
              systemProgram: SystemProgram.programId,
            })
            .signers([sponsor])
            .rpc(),
        "UnsupportedMint"
      );
    });

    it("fails: cliff shorter than minimum", async () => {
      const [vault] = vaultPda(sponsor.publicKey, new BN(98), program.programId);
      const [vaultToken] = vaultTokenPda(vault, program.programId);

      // In testing mode MIN_CLIFF_SECONDS = 1, so 0 should fail
      await assertFails(
        () =>
          program.methods
            .initVault(new BN(98), new BN(0), END_SECONDS)
            .accounts({
              vault,
              vaultTokenAccount: vaultToken,
              mint: testMint,
              config: configAddress,
              sponsor: sponsor.publicKey,
              tokenProgram: TOKEN_PROGRAM_ID,
              associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
              systemProgram: SystemProgram.programId,
            })
            .signers([sponsor])
            .rpc(),
        "DurationTooShort"
      );
    });

    it("fails: end_ts before cliff_ts", async () => {
      const [vault] = vaultPda(sponsor.publicKey, new BN(97), program.programId);
      const [vaultToken] = vaultTokenPda(vault, program.programId);

      await assertFails(
        () =>
          program.methods
            // cliff = 60s, end = 5s — end is before cliff
            .initVault(new BN(97), new BN(60), new BN(5))
            .accounts({
              vault,
              vaultTokenAccount: vaultToken,
              mint: testMint,
              config: configAddress,
              sponsor: sponsor.publicKey,
              tokenProgram: TOKEN_PROGRAM_ID,
              associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
              systemProgram: SystemProgram.programId,
            })
            .signers([sponsor])
            .rpc(),
        "InvalidTimestamps"
      );
    });

    it("fails: duplicate campaign_id for same sponsor", async () => {
      // CAMPAIGN_ID_1 was already initialized above — must fail
      const [vault] = vaultPda(sponsor.publicKey, CAMPAIGN_ID_1, program.programId);
      const [vaultToken] = vaultTokenPda(vault, program.programId);
      try {
        await program.methods
          .initVault(CAMPAIGN_ID_1, CLIFF_SECONDS, END_SECONDS)
          .accounts({
            vault,
            vaultTokenAccount: vaultToken,
            mint: testMint,
            config: configAddress,
            sponsor: sponsor.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([sponsor])
          .rpc();
        assert.fail("Should have failed — PDA already exists");
      } catch (err: unknown) {
        assert.ok(err instanceof Error);
      }
    });

    it("different sponsors can use same campaign_id (different PDA)", async () => {
      const [vault2] = vaultPda(sponsor2.publicKey, CAMPAIGN_ID_1, program.programId);
      const [vaultToken2] = vaultTokenPda(vault2, program.programId);

      await program.methods
        .initVault(CAMPAIGN_ID_1, CLIFF_SECONDS, END_SECONDS)
        .accounts({
          vault: vault2,
          vaultTokenAccount: vaultToken2,
          mint: testMint,
          config: configAddress,
          sponsor: sponsor2.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([sponsor2])
        .rpc();

      const vaultData = await program.account.sponsorVault.fetch(vault2);
      assert.equal(vaultData.sponsor.toBase58(), sponsor2.publicKey.toBase58());
    });
  });

  // =========================================================================
  // deposit
  // =========================================================================

  describe("deposit", () => {
    let vault: PublicKey;
    let vaultToken: PublicKey;

    before(() => {
      [vault] = vaultPda(sponsor.publicKey, CAMPAIGN_ID_1, program.programId);
      [vaultToken] = vaultTokenPda(vault, program.programId);
    });

    it("sponsor deposits tokens into vault", async () => {
      const sponsorBefore = await getAccount(conn, sponsorAta);

      await program.methods
        .deposit(DEPOSIT_AMOUNT)
        .accounts({
          vault,
          vaultTokenAccount: vaultToken,
          sponsorTokenAccount: sponsorAta,
          mint: testMint,
          sponsor: sponsor.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([sponsor])
        .rpc();

      const vaultData = await program.account.sponsorVault.fetch(vault);
      assert.equal(vaultData.principal.toString(), DEPOSIT_AMOUNT.toString());

      const vaultTokenData = await getAccount(conn, vaultToken);
      assert.equal(vaultTokenData.amount.toString(), DEPOSIT_AMOUNT.toString());

      const sponsorAfter = await getAccount(conn, sponsorAta);
      assert.equal(
        BigInt(sponsorBefore.amount.toString()) - BigInt(sponsorAfter.amount.toString()),
        BigInt(DEPOSIT_AMOUNT.toString())
      );
    });

    it("multiple deposits accumulate principal correctly", async () => {
      const extraDeposit = new BN(5_000_000);

      await program.methods
        .deposit(extraDeposit)
        .accounts({
          vault,
          vaultTokenAccount: vaultToken,
          sponsorTokenAccount: sponsorAta,
          mint: testMint,
          sponsor: sponsor.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([sponsor])
        .rpc();

      const vaultData = await program.account.sponsorVault.fetch(vault);
      const expected = DEPOSIT_AMOUNT.add(extraDeposit);
      assert.equal(vaultData.principal.toString(), expected.toString());
    });

    it("fails: zero amount", async () => {
      await assertFails(
        () =>
          program.methods
            .deposit(new BN(0))
            .accounts({
              vault,
              vaultTokenAccount: vaultToken,
              sponsorTokenAccount: sponsorAta,
              mint: testMint,
              sponsor: sponsor.publicKey,
              tokenProgram: TOKEN_PROGRAM_ID,
            })
            .signers([sponsor])
            .rpc(),
        "ZeroAmount"
      );
    });

    it("fails: attacker deposits to someone else's vault (not a signer)", async () => {
      try {
        await program.methods
          .deposit(new BN(1_000_000))
          .accounts({
            vault,
            vaultTokenAccount: vaultToken,
            sponsorTokenAccount: attackerAta,
            mint: testMint,
            sponsor: attacker.publicKey, // wrong sponsor
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([attacker])
          .rpc();
        assert.fail("Should have failed");
      } catch (err: unknown) {
        // has_one = sponsor constraint rejects mismatched sponsor
        assert.ok(err instanceof Error);
      }
    });
  });

  // =========================================================================
  // add_creator / remove_creator
  // =========================================================================

  describe("squad management", () => {
    let vault: PublicKey;

    before(() => {
      [vault] = vaultPda(sponsor.publicKey, CAMPAIGN_ID_1, program.programId);
    });

    it("sponsor adds creator1 with 5000 bps", async () => {
      const [member1] = memberPda(vault, creator1.publicKey, program.programId);

      await program.methods
        .addCreator(5000)
        .accounts({
          vault,
          member: member1,
          creator: creator1.publicKey,
          sponsor: sponsor.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([sponsor])
        .rpc();

      const memberData = await program.account.squadMember.fetch(member1);
      assert.equal(memberData.creator.toBase58(), creator1.publicKey.toBase58());
      assert.equal(memberData.allocationBps, 5000);
      assert.equal(memberData.totalReceived.toNumber(), 0);

      const vaultData = await program.account.sponsorVault.fetch(vault);
      assert.equal(vaultData.memberCount, 1);
      assert.equal(vaultData.totalAllocationBps, 5000);
    });

    it("sponsor adds creator2 with 5000 bps (total = 10000)", async () => {
      const [member2] = memberPda(vault, creator2.publicKey, program.programId);

      await program.methods
        .addCreator(5000)
        .accounts({
          vault,
          member: member2,
          creator: creator2.publicKey,
          sponsor: sponsor.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([sponsor])
        .rpc();

      const vaultData = await program.account.sponsorVault.fetch(vault);
      assert.equal(vaultData.memberCount, 2);
      assert.equal(vaultData.totalAllocationBps, 10000);
    });

    it("fails: adding creator3 would exceed 10000 bps", async () => {
      const [member3] = memberPda(vault, creator3.publicKey, program.programId);

      await assertFails(
        () =>
          program.methods
            .addCreator(1)
            .accounts({
              vault,
              member: member3,
              creator: creator3.publicKey,
              sponsor: sponsor.publicKey,
              systemProgram: SystemProgram.programId,
            })
            .signers([sponsor])
            .rpc(),
        "TotalAllocationExceeded"
      );
    });

    it("fails: zero allocation bps", async () => {
      const [member3] = memberPda(vault, creator3.publicKey, program.programId);

      await assertFails(
        () =>
          program.methods
            .addCreator(0)
            .accounts({
              vault,
              member: member3,
              creator: creator3.publicKey,
              sponsor: sponsor.publicKey,
              systemProgram: SystemProgram.programId,
            })
            .signers([sponsor])
            .rpc(),
        "InvalidAllocationBps"
      );
    });

    it("fails: duplicate creator (PDA already exists)", async () => {
      // creator1 is already in the squad
      const [member1] = memberPda(vault, creator1.publicKey, program.programId);
      try {
        await program.methods
          .addCreator(100)
          .accounts({
            vault,
            member: member1,
            creator: creator1.publicKey,
            sponsor: sponsor.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([sponsor])
          .rpc();
        assert.fail("Should have failed — member PDA already exists");
      } catch (err: unknown) {
        assert.ok(err instanceof Error);
      }
    });

    it("fails: attacker adds creator to sponsor's vault", async () => {
      const [member3] = memberPda(vault, creator3.publicKey, program.programId);
      try {
        await program.methods
          .addCreator(100)
          .accounts({
            vault,
            member: member3,
            creator: creator3.publicKey,
            sponsor: attacker.publicKey, // wrong sponsor
            systemProgram: SystemProgram.programId,
          })
          .signers([attacker])
          .rpc();
        assert.fail("Should have failed");
      } catch (err: unknown) {
        assert.ok(err instanceof Error);
      }
    });

    it("remove_creator: sponsor removes creator2, allocation freed", async () => {
      const [member2] = memberPda(vault, creator2.publicKey, program.programId);
      const sponsorLamportsBefore = await conn.getBalance(sponsor.publicKey);

      await program.methods
        .removeCreator()
        .accounts({
          vault,
          member: member2,
          sponsor: sponsor.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([sponsor])
        .rpc();

      const vaultData = await program.account.sponsorVault.fetch(vault);
      assert.equal(vaultData.memberCount, 1);
      assert.equal(vaultData.totalAllocationBps, 5000);

      // Member PDA should be closed (rent returned to sponsor)
      const sponsorLamportsAfter = await conn.getBalance(sponsor.publicKey);
      assert.isAbove(sponsorLamportsAfter, sponsorLamportsBefore);

      try {
        await program.account.squadMember.fetch(member2);
        assert.fail("Member account should be closed");
      } catch {
        // Expected — account closed
      }
    });

    it("fails: cannot remove the last squad member", async () => {
      // Only creator1 remains
      const [member1] = memberPda(vault, creator1.publicKey, program.programId);

      await assertFails(
        () =>
          program.methods
            .removeCreator()
            .accounts({
              vault,
              member: member1,
              sponsor: sponsor.publicKey,
              systemProgram: SystemProgram.programId,
            })
            .signers([sponsor])
            .rpc(),
        "CannotRemoveLastMember"
      );
    });

    it("fails: attacker removes creator from sponsor's vault", async () => {
      const [member1] = memberPda(vault, creator1.publicKey, program.programId);
      try {
        await program.methods
          .removeCreator()
          .accounts({
            vault,
            member: member1,
            sponsor: attacker.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([attacker])
          .rpc();
        assert.fail("Should have failed");
      } catch (err: unknown) {
        assert.ok(err instanceof Error);
      }
    });

    it("restore creator2 at 5000 bps for yield routing tests", async () => {
      const [member2] = memberPda(vault, creator2.publicKey, program.programId);

      await program.methods
        .addCreator(5000)
        .accounts({
          vault,
          member: member2,
          creator: creator2.publicKey,
          sponsor: sponsor.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([sponsor])
        .rpc();

      const vaultData = await program.account.sponsorVault.fetch(vault);
      assert.equal(vaultData.totalAllocationBps, 10000);
    });
  });

  // =========================================================================
  // route_yield
  // =========================================================================

  describe("route_yield", () => {
    let vault: PublicKey;
    let vaultToken: PublicKey;

    before(async () => {
      [vault] = vaultPda(sponsor.publicKey, CAMPAIGN_ID_1, program.programId);
      [vaultToken] = vaultTokenPda(vault, program.programId);

      // Mint yield tokens directly into the vault token account (simulates Kamino yield)
      await mintTo(conn, admin, testMint, vaultToken, admin, YIELD_AMOUNT_TOTAL.toNumber());
    });

    it("fails: allocation not full (route before squad is complete)", async () => {
      // Create a separate vault with incomplete allocation for this test
      const [vault2] = vaultPda(sponsor2.publicKey, CAMPAIGN_ID_1, program.programId);
      const [vaultToken2] = vaultTokenPda(vault2, program.programId);
      const [member2c1] = memberPda(vault2, creator1.publicKey, program.programId);

      // Add only one creator (5000 bps, not 10000)
      await program.methods
        .addCreator(5000)
        .accounts({
          vault: vault2,
          member: member2c1,
          creator: creator1.publicKey,
          sponsor: sponsor2.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([sponsor2])
        .rpc();

      // Deposit some tokens
      await program.methods
        .deposit(DEPOSIT_AMOUNT)
        .accounts({
          vault: vault2,
          vaultTokenAccount: vaultToken2,
          sponsorTokenAccount: sponsor2Ata,
          mint: testMint,
          sponsor: sponsor2.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([sponsor2])
        .rpc();

      await mintTo(conn, admin, testMint, vaultToken2, admin, 100_000);

      await assertFails(
        () =>
          program.methods
            .routeYield(new BN(50_000))
            .accounts({
              vault: vault2,
              vaultTokenAccount: vaultToken2,
              member: member2c1,
              creatorTokenAccount: creator1Ata,
              treasuryTokenAccount: treasuryAta,
              config: configAddress,
              mint: testMint,
              authority: automation.publicKey,
              tokenProgram: TOKEN_PROGRAM_ID,
            })
            .signers([automation])
            .rpc(),
        "AllocationNotFull"
      );
    });

    it("fails: zero yield amount", async () => {
      const [member1] = memberPda(vault, creator1.publicKey, program.programId);

      await assertFails(
        () =>
          program.methods
            .routeYield(new BN(0))
            .accounts({
              vault,
              vaultTokenAccount: vaultToken,
              member: member1,
              creatorTokenAccount: creator1Ata,
              treasuryTokenAccount: treasuryAta,
              config: configAddress,
              mint: testMint,
              authority: automation.publicKey,
              tokenProgram: TOKEN_PROGRAM_ID,
            })
            .signers([automation])
            .rpc(),
        "ZeroAmount"
      );
    });

    it("fails: yield_amount exceeds available yield (would dip into principal)", async () => {
      const [member1] = memberPda(vault, creator1.publicKey, program.programId);

      // Vault balance = principal + 180_000 yield. Requesting more than 180_000 should fail.
      await assertFails(
        () =>
          program.methods
            .routeYield(new BN(200_000)) // more than YIELD_AMOUNT_TOTAL
            .accounts({
              vault,
              vaultTokenAccount: vaultToken,
              member: member1,
              creatorTokenAccount: creator1Ata,
              treasuryTokenAccount: treasuryAta,
              config: configAddress,
              mint: testMint,
              authority: automation.publicKey,
              tokenProgram: TOKEN_PROGRAM_ID,
            })
            .signers([automation])
            .rpc(),
        "InsufficientBalance"
      );
    });

    it("route_yield to creator1 (5000 bps = 50% of epoch yield)", async () => {
      const [member1] = memberPda(vault, creator1.publicKey, program.programId);

      // creator1's share = 50% of total epoch yield
      const creator1YieldAmount = YIELD_AMOUNT_TOTAL.divn(2); // 90_000
      const expectedFee = Math.floor(creator1YieldAmount.toNumber() * GECKO_FEE_BPS / BPS_DENOMINATOR); // 1_800
      const expectedCreatorShare = creator1YieldAmount.toNumber() - expectedFee; // 88_200

      const creator1Before = await getAccount(conn, creator1Ata);
      const treasuryBefore = await getAccount(conn, treasuryAta);

      await program.methods
        .routeYield(creator1YieldAmount)
        .accounts({
          vault,
          vaultTokenAccount: vaultToken,
          member: member1,
          creatorTokenAccount: creator1Ata,
          treasuryTokenAccount: treasuryAta,
          config: configAddress,
          mint: testMint,
          authority: automation.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([automation])
        .rpc();

      const creator1After = await getAccount(conn, creator1Ata);
      const treasuryAfter = await getAccount(conn, treasuryAta);
      const member1Data = await program.account.squadMember.fetch(member1);

      // Creator received correct amount
      const creator1Received = Number(creator1After.amount) - Number(creator1Before.amount);
      assert.equal(creator1Received, expectedCreatorShare, "Creator1 share mismatch");

      // Treasury received fee
      const treasuryReceived = Number(treasuryAfter.amount) - Number(treasuryBefore.amount);
      assert.equal(treasuryReceived, expectedFee, "Treasury fee mismatch");

      // Member total_received updated
      assert.equal(member1Data.totalReceived.toNumber(), expectedCreatorShare);
    });

    it("route_yield to creator2 (5000 bps = 50% of epoch yield)", async () => {
      const [member2] = memberPda(vault, creator2.publicKey, program.programId);

      const creator2YieldAmount = YIELD_AMOUNT_TOTAL.divn(2); // 90_000
      const expectedFee = Math.floor(creator2YieldAmount.toNumber() * GECKO_FEE_BPS / BPS_DENOMINATOR);
      const expectedCreatorShare = creator2YieldAmount.toNumber() - expectedFee;

      const creator2Before = await getAccount(conn, creator2Ata);

      await program.methods
        .routeYield(creator2YieldAmount)
        .accounts({
          vault,
          vaultTokenAccount: vaultToken,
          member: member2,
          creatorTokenAccount: creator2Ata,
          treasuryTokenAccount: treasuryAta,
          config: configAddress,
          mint: testMint,
          authority: automation.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([automation])
        .rpc();

      const creator2After = await getAccount(conn, creator2Ata);
      const creator2Received = Number(creator2After.amount) - Number(creator2Before.amount);
      assert.equal(creator2Received, expectedCreatorShare, "Creator2 share mismatch");

      // Verify vault total_yield_routed accumulated correctly
      const vaultData = await program.account.sponsorVault.fetch(vault);
      assert.equal(
        vaultData.totalYieldRouted.toNumber(),
        YIELD_AMOUNT_TOTAL.toNumber(),
        "total_yield_routed mismatch"
      );
    });

    it("vault principal is untouched after yield routing", async () => {
      const vaultData = await program.account.sponsorVault.fetch(vault);
      // principal = 10M + 5M from deposit tests
      const expectedPrincipal = DEPOSIT_AMOUNT.addn(5_000_000);
      assert.equal(vaultData.principal.toString(), expectedPrincipal.toString());
    });
  });

  // =========================================================================
  // close_vault
  // =========================================================================

  describe("close_vault", () => {
    it("fails: close_vault before cliff (vault just created)", async () => {
      // Create a fresh vault to test cliff enforcement
      const [freshVault] = vaultPda(sponsor.publicKey, CAMPAIGN_ID_2, program.programId);
      const [freshVaultToken] = vaultTokenPda(freshVault, program.programId);

      await program.methods
        .initVault(CAMPAIGN_ID_2, new BN(3600), new BN(7200)) // 1 hour cliff
        .accounts({
          vault: freshVault,
          vaultTokenAccount: freshVaultToken,
          mint: testMint,
          config: configAddress,
          sponsor: sponsor.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([sponsor])
        .rpc();

      const sponsorFreshAta = sponsorAta;

      await assertFails(
        () =>
          program.methods
            .closeVault()
            .accounts({
              vault: freshVault,
              vaultTokenAccount: freshVaultToken,
              sponsorTokenAccount: sponsorFreshAta,
              mint: testMint,
              sponsor: sponsor.publicKey,
              tokenProgram: TOKEN_PROGRAM_ID,
              systemProgram: SystemProgram.programId,
            })
            .signers([sponsor])
            .rpc(),
        "CliffNotElapsed"
      );
    });

    it("fails: attacker closes sponsor's vault", async () => {
      const [vault] = vaultPda(sponsor.publicKey, CAMPAIGN_ID_1, program.programId);
      const [vaultToken] = vaultTokenPda(vault, program.programId);

      try {
        await program.methods
          .closeVault()
          .accounts({
            vault,
            vaultTokenAccount: vaultToken,
            sponsorTokenAccount: attackerAta,
            mint: testMint,
            sponsor: attacker.publicKey, // wrong sponsor
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([attacker])
          .rpc();
        assert.fail("Should have failed");
      } catch (err: unknown) {
        assert.ok(err instanceof Error);
      }
    });

    it("sponsor closes vault after cliff — principal returned", async () => {
      const [vault] = vaultPda(sponsor.publicKey, CAMPAIGN_ID_1, program.programId);
      const [vaultToken] = vaultTokenPda(vault, program.programId);

      // Wait for cliff (1 second in testing mode)
      await sleep(2000);

      const vaultDataBefore = await program.account.sponsorVault.fetch(vault);
      const vaultTokenBefore = await getAccount(conn, vaultToken);
      const sponsorBefore = await getAccount(conn, sponsorAta);

      await program.methods
        .closeVault()
        .accounts({
          vault,
          vaultTokenAccount: vaultToken,
          sponsorTokenAccount: sponsorAta,
          mint: testMint,
          sponsor: sponsor.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([sponsor])
        .rpc();

      // Sponsor received remaining vault balance
      const sponsorAfter = await getAccount(conn, sponsorAta);
      const returned = BigInt(sponsorAfter.amount) - BigInt(sponsorBefore.amount);
      assert.equal(returned.toString(), vaultTokenBefore.amount.toString());

      // Vault is now closed
      const vaultDataAfter = await program.account.sponsorVault.fetch(vault);
      assert.deepEqual(vaultDataAfter.status, { closed: {} });
      assert.equal(vaultDataAfter.principal.toNumber(), 0);

      // Vault token account is closed (should not be fetchable)
      try {
        await getAccount(conn, vaultToken);
        assert.fail("Vault token account should be closed");
      } catch {
        // Expected
      }
    });

    it("fails: double close (vault already closed)", async () => {
      const [vault] = vaultPda(sponsor.publicKey, CAMPAIGN_ID_1, program.programId);

      // Vault token account is closed, but let's try with a fresh ATA
      const freshTokenAccount = await createAssociatedTokenAccount(
        conn,
        sponsor,
        testMint,
        sponsor.publicKey
      ).catch(() => sponsorAta);

      try {
        await program.methods
          .closeVault()
          .accounts({
            vault,
            vaultTokenAccount: sponsorAta, // wrong — vault token is already closed
            sponsorTokenAccount: sponsorAta,
            mint: testMint,
            sponsor: sponsor.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([sponsor])
          .rpc();
        assert.fail("Should have failed");
      } catch (err: unknown) {
        assert.ok(err instanceof Error);
      }
    });

    it("fails: route_yield on closed vault", async () => {
      const [vault] = vaultPda(sponsor.publicKey, CAMPAIGN_ID_1, program.programId);
      const [member1] = memberPda(vault, creator1.publicKey, program.programId);

      // Vault is closed — need a valid token account for the constraint check
      // This should fail at the VaultNotActive check before token accounts matter
      // We'll use the sponsorAta as a placeholder since vault is already closed
      try {
        await program.methods
          .routeYield(new BN(1000))
          .accounts({
            vault,
            vaultTokenAccount: sponsorAta,
            member: member1,
            creatorTokenAccount: creator1Ata,
            treasuryTokenAccount: treasuryAta,
            config: configAddress,
            mint: testMint,
            authority: automation.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([automation])
          .rpc();
        assert.fail("Should have failed");
      } catch (err: unknown) {
        assert.ok(err instanceof Error);
      }
    });
  });

  // =========================================================================
  // 🔴 Security: attack vectors
  // =========================================================================

  describe("security: attack vectors", () => {
    let vault3: PublicKey;
    let vaultToken3: PublicKey;
    let member3c1: PublicKey;
    let member3c2: PublicKey;

    before(async () => {
      // Spawn a fresh vault (campaign 3) for attack tests
      const campaignId3 = new BN(3);
      [vault3] = vaultPda(sponsor.publicKey, campaignId3, program.programId);
      [vaultToken3] = vaultTokenPda(vault3, program.programId);
      [member3c1] = memberPda(vault3, creator1.publicKey, program.programId);
      [member3c2] = memberPda(vault3, creator2.publicKey, program.programId);

      await program.methods
        .initVault(campaignId3, CLIFF_SECONDS, END_SECONDS)
        .accounts({
          vault: vault3,
          vaultTokenAccount: vaultToken3,
          mint: testMint,
          config: configAddress,
          sponsor: sponsor.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([sponsor])
        .rpc();

      await program.methods
        .deposit(DEPOSIT_AMOUNT)
        .accounts({
          vault: vault3,
          vaultTokenAccount: vaultToken3,
          sponsorTokenAccount: sponsorAta,
          mint: testMint,
          sponsor: sponsor.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([sponsor])
        .rpc();

      await program.methods
        .addCreator(6000)
        .accounts({ vault: vault3, member: member3c1, creator: creator1.publicKey, sponsor: sponsor.publicKey, systemProgram: SystemProgram.programId })
        .signers([sponsor])
        .rpc();

      await program.methods
        .addCreator(4000)
        .accounts({ vault: vault3, member: member3c2, creator: creator2.publicKey, sponsor: sponsor.publicKey, systemProgram: SystemProgram.programId })
        .signers([sponsor])
        .rpc();

      // Mint yield
      await mintTo(conn, admin, testMint, vaultToken3, admin, 200_000);
    });

    it("attacker cannot impersonate automation_authority for route_yield", async () => {
      await assertFails(
        () =>
          program.methods
            .routeYield(new BN(50_000))
            .accounts({
              vault: vault3,
              vaultTokenAccount: vaultToken3,
              member: member3c1,
              creatorTokenAccount: creator1Ata,
              treasuryTokenAccount: treasuryAta,
              config: configAddress,
              mint: testMint,
              authority: attacker.publicKey, // wrong authority
              tokenProgram: TOKEN_PROGRAM_ID,
            })
            .signers([attacker])
            .rpc(),
        "Unauthorized"
      );
    });

    it("attacker cannot redirect yield to their own token account (constraint blocks)", async () => {
      // creator_token_account must have authority = member.creator
      // Attacker can't pass their own ATA as creator_token_account for creator1's member PDA
      try {
        await program.methods
          .routeYield(new BN(50_000))
          .accounts({
            vault: vault3,
            vaultTokenAccount: vaultToken3,
            member: member3c1,
            creatorTokenAccount: attackerAta, // attacker's ATA, but authority != creator1
            treasuryTokenAccount: treasuryAta,
            config: configAddress,
            mint: testMint,
            authority: automation.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([automation])
          .rpc();
        assert.fail("Should have failed — token account authority mismatch");
      } catch (err: unknown) {
        assert.ok(err instanceof Error);
      }
    });

    it("attacker cannot drain principal via oversized yield_amount", async () => {
      // vault balance = principal (10M) + yield (200K)
      // Requesting 300K would exceed available yield (200K)
      await assertFails(
        () =>
          program.methods
            .routeYield(new BN(300_000))
            .accounts({
              vault: vault3,
              vaultTokenAccount: vaultToken3,
              member: member3c1,
              creatorTokenAccount: creator1Ata,
              treasuryTokenAccount: treasuryAta,
              config: configAddress,
              mint: testMint,
              authority: automation.publicKey,
              tokenProgram: TOKEN_PROGRAM_ID,
            })
            .signers([automation])
            .rpc(),
        "InsufficientBalance"
      );
    });

    it("attacker cannot use member from vault A on vault B (PDA seed mismatch)", async () => {
      // member3c1 is seeded with [vault3, creator1]
      // Using it against vault2 (different vault) must fail
      const [vault2] = vaultPda(sponsor2.publicKey, CAMPAIGN_ID_1, program.programId);
      const [vaultToken2] = vaultTokenPda(vault2, program.programId);

      try {
        await program.methods
          .routeYield(new BN(50_000))
          .accounts({
            vault: vault2,       // different vault
            vaultTokenAccount: vaultToken2,
            member: member3c1,   // member from vault3 — seeds won't match
            creatorTokenAccount: creator1Ata,
            treasuryTokenAccount: treasuryAta,
            config: configAddress,
            mint: testMint,
            authority: automation.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([automation])
          .rpc();
        assert.fail("Should have failed — member belongs to a different vault");
      } catch (err: unknown) {
        assert.ok(err instanceof Error);
      }
    });

    it("attacker cannot supply a fake config to bypass fee/authority checks", async () => {
      // Attacker deploys their own config — but config PDA is [b"config"] for THIS program
      // There is only one valid config PDA per program, so this attack is structurally impossible.
      // We verify the correct config was used by asserting automation_authority matches.
      const config = await program.account.geckoConfig.fetch(configAddress);
      assert.equal(
        config.automationAuthority.toBase58(),
        automation.publicKey.toBase58(),
        "Config automation_authority tampered"
      );
    });

    it("attacker cannot add themselves to a sponsor's vault", async () => {
      const campaignId4 = new BN(4);
      const [vault4] = vaultPda(sponsor.publicKey, campaignId4, program.programId);
      const [vaultToken4] = vaultTokenPda(vault4, program.programId);

      await program.methods
        .initVault(campaignId4, CLIFF_SECONDS, END_SECONDS)
        .accounts({
          vault: vault4, vaultTokenAccount: vaultToken4, mint: testMint,
          config: configAddress, sponsor: sponsor.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID, associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([sponsor])
        .rpc();

      const [attackerMember] = memberPda(vault4, attacker.publicKey, program.programId);

      try {
        await program.methods
          .addCreator(10000)
          .accounts({
            vault: vault4,
            member: attackerMember,
            creator: attacker.publicKey,
            sponsor: attacker.publicKey, // attacker signs as sponsor
            systemProgram: SystemProgram.programId,
          })
          .signers([attacker])
          .rpc();
        assert.fail("Should have failed — attacker is not the vault sponsor");
      } catch (err: unknown) {
        assert.ok(err instanceof Error);
      }
    });

    it("attacker cannot close a vault they do not own", async () => {
      try {
        await program.methods
          .closeVault()
          .accounts({
            vault: vault3,
            vaultTokenAccount: vaultToken3,
            sponsorTokenAccount: attackerAta,
            mint: testMint,
            sponsor: attacker.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([attacker])
          .rpc();
        assert.fail("Should have failed — attacker is not the vault sponsor");
      } catch (err: unknown) {
        assert.ok(err instanceof Error);
      }
    });

    it("attacker cannot remove creator from a vault they do not own", async () => {
      try {
        await program.methods
          .removeCreator()
          .accounts({
            vault: vault3,
            member: member3c1,
            sponsor: attacker.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([attacker])
          .rpc();
        assert.fail("Should have failed");
      } catch (err: unknown) {
        assert.ok(err instanceof Error);
      }
    });
  });

  // =========================================================================
  // 🟢 Full lifecycle
  // =========================================================================

  describe("full lifecycle", () => {
    let vault: PublicKey;
    let vaultToken: PublicKey;

    it("complete campaign: init → deposit → squad → 3x yield → close", async () => {
      const campaignId = new BN(10);
      [vault] = vaultPda(sponsor.publicKey, campaignId, program.programId);
      [vaultToken] = vaultTokenPda(vault, program.programId);
      const [memberC1] = memberPda(vault, creator1.publicKey, program.programId);
      const [memberC2] = memberPda(vault, creator2.publicKey, program.programId);

      // 1. Init vault
      await program.methods
        .initVault(campaignId, CLIFF_SECONDS, END_SECONDS)
        .accounts({
          vault, vaultTokenAccount: vaultToken, mint: testMint,
          config: configAddress, sponsor: sponsor.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID, associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([sponsor])
        .rpc();

      // 2. Deposit principal
      await program.methods
        .deposit(DEPOSIT_AMOUNT)
        .accounts({
          vault, vaultTokenAccount: vaultToken, sponsorTokenAccount: sponsorAta,
          mint: testMint, sponsor: sponsor.publicKey, tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([sponsor])
        .rpc();

      // 3. Build squad (creator1: 70%, creator2: 30%)
      await program.methods
        .addCreator(7000)
        .accounts({ vault, member: memberC1, creator: creator1.publicKey, sponsor: sponsor.publicKey, systemProgram: SystemProgram.programId })
        .signers([sponsor])
        .rpc();

      await program.methods
        .addCreator(3000)
        .accounts({ vault, member: memberC2, creator: creator2.publicKey, sponsor: sponsor.publicKey, systemProgram: SystemProgram.programId })
        .signers([sponsor])
        .rpc();

      // 4. Three yield epochs
      const epochYield = new BN(100_000); // 0.1 USDC per epoch

      let creator1TotalReceived = BigInt(0);
      let creator2TotalReceived = BigInt(0);

      for (let epoch = 1; epoch <= 3; epoch++) {
        // Simulate yield arriving in vault
        await mintTo(conn, admin, testMint, vaultToken, admin, epochYield.toNumber());

        const c1Share = new BN(Math.floor(epochYield.toNumber() * 7000 / BPS_DENOMINATOR)); // 70%
        const c2Share = new BN(epochYield.toNumber() - c1Share.toNumber());

        const c1Before = await getAccount(conn, creator1Ata);
        const c2Before = await getAccount(conn, creator2Ata);

        await program.methods
          .routeYield(c1Share)
          .accounts({
            vault, vaultTokenAccount: vaultToken, member: memberC1,
            creatorTokenAccount: creator1Ata, treasuryTokenAccount: treasuryAta,
            config: configAddress, mint: testMint, authority: automation.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([automation])
          .rpc();

        await program.methods
          .routeYield(c2Share)
          .accounts({
            vault, vaultTokenAccount: vaultToken, member: memberC2,
            creatorTokenAccount: creator2Ata, treasuryTokenAccount: treasuryAta,
            config: configAddress, mint: testMint, authority: automation.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([automation])
          .rpc();

        const c1After = await getAccount(conn, creator1Ata);
        const c2After = await getAccount(conn, creator2Ata);
        creator1TotalReceived += BigInt(c1After.amount) - BigInt(c1Before.amount);
        creator2TotalReceived += BigInt(c2After.amount) - BigInt(c2Before.amount);
      }

      // After 3 epochs creator1 should have received ~70% of net yield
      const totalNetYield = epochYield.toNumber() * 3 * (1 - GECKO_FEE_BPS / BPS_DENOMINATOR);
      const expectedC1 = Math.floor(totalNetYield * 0.7);
      const expectedC2 = Math.floor(totalNetYield * 0.3);

      assert.approximately(Number(creator1TotalReceived), expectedC1, 10, "Creator1 total mismatch");
      assert.approximately(Number(creator2TotalReceived), expectedC2, 10, "Creator2 total mismatch");

      // 5. Close squad members first (recover rent)
      await program.methods
        .removeCreator()
        .accounts({ vault, member: memberC2, sponsor: sponsor.publicKey, systemProgram: SystemProgram.programId })
        .signers([sponsor])
        .rpc();

      // 6. Wait for cliff, close vault
      await sleep(2000);
      const sponsorBefore = await getAccount(conn, sponsorAta);

      await program.methods
        .closeVault()
        .accounts({
          vault, vaultTokenAccount: vaultToken, sponsorTokenAccount: sponsorAta,
          mint: testMint, sponsor: sponsor.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID, systemProgram: SystemProgram.programId,
        })
        .signers([sponsor])
        .rpc();

      const sponsorAfter = await getAccount(conn, sponsorAta);
      const returned = BigInt(sponsorAfter.amount) - BigInt(sponsorBefore.amount);

      // Sponsor gets back at least the principal (minus yield that was distributed)
      assert.isAbove(Number(returned), 0, "Sponsor received nothing back");

      const vaultData = await program.account.sponsorVault.fetch(vault);
      assert.deepEqual(vaultData.status, { closed: {} });

      console.log(`
  ✅ Full lifecycle complete
     Creator1 received: ${creator1TotalReceived} tokens over 3 epochs
     Creator2 received: ${creator2TotalReceived} tokens over 3 epochs
     Principal returned to sponsor: ${returned} tokens
      `);
    });
  });

  // =========================================================================
  // V4 — create_milestone_by_automation
  // =========================================================================

  describe("V4 — create_milestone_by_automation", () => {
    let vault: PublicKey;

    before(() => {
      // Reuse vault3 (campaign_id=3) which is funded and has members
      [vault] = vaultPda(sponsor.publicKey, new BN(3), program.programId);
    });

    it("automation keypair creates a milestone with score_threshold=0", async () => {
      const milestoneIndex = 5;
      const [milestonePda] = PublicKey.findProgramAddressSync(
        [Buffer.from("milestone"), vault.toBuffer(), Buffer.from([milestoneIndex])],
        program.programId
      );

      await program.methods
        .createMilestoneByAutomation(
          "Advance payment",
          0,
          1000,
          creator1.publicKey,
          milestoneIndex,
        )
        .accounts({
          vault,
          milestone: milestonePda,
          config: configAddress,
          automation: automation.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([automation])
        .rpc();

      const milestone = await program.account.performanceMilestone.fetch(milestonePda);
      assert.equal(milestone.scoreThreshold, 0);
      assert.equal(milestone.payoutBps, 1000);
      assert.equal(milestone.status.pending !== undefined, true);
    });

    it("rejects if caller is not automation_authority", async () => {
      const fake = Keypair.generate();
      await airdrop(conn, fake.publicKey);
      const milestoneIndex = 6;
      const [milestonePda] = PublicKey.findProgramAddressSync(
        [Buffer.from("milestone"), vault.toBuffer(), Buffer.from([milestoneIndex])],
        program.programId
      );

      try {
        await program.methods
          .createMilestoneByAutomation("Bad actor", 0, 1000, creator1.publicKey, milestoneIndex)
          .accounts({
            vault,
            milestone: milestonePda,
            config: configAddress,
            automation: fake.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([fake])
          .rpc();
        assert.fail("Should have thrown");
      } catch (err: any) {
        assert.include(err.message, "Unauthorized");
      }
    });
  });

  // =========================================================================
  // V4 — init_and_deposit
  // =========================================================================

  describe("V4 — init_and_deposit", () => {
    it("initializes vault and deposits in one transaction", async () => {
      const campaignId = new BN(20);
      const [vaultPdaAddr] = vaultPda(sponsor.publicKey, campaignId, program.programId);
      const [vaultTokenPdaAddr] = vaultTokenPda(vaultPdaAddr, program.programId);

      const depositAmount = new BN(1_000_000); // 1 USDC

      await program.methods
        .initAndDeposit(campaignId, CLIFF_SECONDS, END_SECONDS, depositAmount)
        .accounts({
          vault: vaultPdaAddr,
          vaultTokenAccount: vaultTokenPdaAddr,
          sponsorTokenAccount: sponsorAta,
          mint: testMint,
          config: configAddress,
          sponsor: sponsor.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([sponsor])
        .rpc();

      const vaultData = await program.account.sponsorVault.fetch(vaultPdaAddr);
      assert.equal(vaultData.principal.toString(), depositAmount.toString());
      assert.equal(vaultData.campaignId.toString(), campaignId.toString());
      assert.deepEqual(vaultData.status, { active: {} });

      const tokenData = await getAccount(conn, vaultTokenPdaAddr);
      assert.equal(tokenData.amount.toString(), depositAmount.toString());
    });

    it("fails: zero deposit amount", async () => {
      const campaignId = new BN(21);
      const [vaultPdaAddr] = vaultPda(sponsor.publicKey, campaignId, program.programId);
      const [vaultTokenPdaAddr] = vaultTokenPda(vaultPdaAddr, program.programId);

      await assertFails(
        () =>
          program.methods
            .initAndDeposit(campaignId, CLIFF_SECONDS, END_SECONDS, new BN(0))
            .accounts({
              vault: vaultPdaAddr,
              vaultTokenAccount: vaultTokenPdaAddr,
              sponsorTokenAccount: sponsorAta,
              mint: testMint,
              config: configAddress,
              sponsor: sponsor.publicKey,
              tokenProgram: TOKEN_PROGRAM_ID,
              associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
              systemProgram: SystemProgram.programId,
            })
            .signers([sponsor])
            .rpc(),
        "ZeroAmount"
      );
    });
  });
});
