/**
 * init-devnet.ts — One-time GeckoConfig initialization on devnet.
 * Run: npx ts-node --transpile-only scripts/init-devnet.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { PublicKey, SystemProgram, Keypair, Connection } from "@solana/web3.js";
import { readFileSync } from "fs";
import { homedir } from "os";
import { join } from "path";

// ---------------------------------------------------------------------------
// Config — update before mainnet
// ---------------------------------------------------------------------------

const DEVNET_USDC = new PublicKey("4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU");
const TREASURY_PUBKEY: PublicKey | null = null;       // null → admin wallet
const AUTOMATION_AUTHORITY: PublicKey | null = null;  // null → admin wallet

// ---------------------------------------------------------------------------
// Bootstrap
// ---------------------------------------------------------------------------

async function main() {
  const walletPath = `${homedir()}/.config/solana/id.json`;
  const adminKeypair = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(readFileSync(walletPath, "utf-8")))
  );

  const connection = new Connection("https://api.devnet.solana.com", "confirmed");
  const wallet = new anchor.Wallet(adminKeypair);
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });

  const idl = JSON.parse(
    readFileSync(join(__dirname, "../target/idl/gecko_vault.json"), "utf-8")
  );
  const programId = new PublicKey(idl.address);
  const program = new anchor.Program(idl, provider);

  const treasury = TREASURY_PUBKEY ?? adminKeypair.publicKey;
  const automationAuthority = AUTOMATION_AUTHORITY ?? adminKeypair.publicKey;

  const [configPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    programId
  );

  // Check if already initialized
  try {
    const existing = await program.account["geckoConfig"].fetch(configPda);
    console.log("⚠️  Config already initialized:");
    console.log("   Authority  :", existing["authority"].toBase58());
    console.log("   Treasury   :", existing["treasury"].toBase58());
    console.log("   Automation :", existing["automationAuthority"].toBase58());
    console.log("   Fee bps    :", existing["feeBps"]);
    console.log("   Mints      :", existing["allowedMints"].map((m: PublicKey) => m.toBase58()));
    return;
  } catch {
    // Not initialized — proceed
  }

  console.log("🚀 Initializing GeckoConfig on devnet...");
  console.log("   Program ID  :", programId.toBase58());
  console.log("   Config PDA  :", configPda.toBase58());
  console.log("   Admin       :", adminKeypair.publicKey.toBase58());
  console.log("   Treasury    :", treasury.toBase58());
  console.log("   Automation  :", automationAuthority.toBase58());
  console.log("   Allowed mint:", DEVNET_USDC.toBase58(), "(devnet USDC)");

  const tx = await program.methods
    .initConfig(treasury, automationAuthority, [DEVNET_USDC])
    .accounts({ authority: adminKeypair.publicKey })
    .signers([adminKeypair])
    .rpc();

  console.log("\n✅ GeckoConfig initialized");
  console.log("   Tx         :", tx);
  console.log("   Explorer   :", `https://explorer.solana.com/tx/${tx}?cluster=devnet`);
  console.log("\n--- Copy to .env ---");
  console.log(`GECKO_CONFIG_PDA=${configPda.toBase58()}`);
  console.log(`GECKO_PROGRAM_ID=${programId.toBase58()}`);
  console.log(`GECKO_TREASURY=${treasury.toBase58()}`);
  console.log(`GECKO_AUTOMATION_AUTHORITY=${automationAuthority.toBase58()}`);
}

main().catch((err) => {
  console.error("❌ Init failed:", err);
  process.exit(1);
});
