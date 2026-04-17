/**
 * repair-devnet.ts — Repair the corrupted GeckoConfig PDA on devnet.
 *
 * The migrate_config instruction overwrote fee_bps/bump/allowed_mints bytes
 * when it wrote oracle_authority at the V3 offset. This script calls the new
 * repair_config instruction to fix those fields.
 *
 * Run: npx ts-node --transpile-only scripts/repair-devnet.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { PublicKey, Connection, Keypair } from "@solana/web3.js";
import { readFileSync } from "fs";
import { homedir } from "os";
import { join } from "path";

const DEVNET_USDC = new PublicKey("4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU");

async function main() {
  const walletPath = `${homedir()}/.config/solana/id.json`;
  const adminKeypair = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(readFileSync(walletPath, "utf-8")))
  );

  const connection = new Connection("https://api.devnet.solana.com", "confirmed");
  const wallet = new anchor.Wallet(adminKeypair);
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });

  // Load fresh IDL (now includes repair_config)
  const idl = JSON.parse(
    readFileSync(join(__dirname, "../target/idl/gecko_vault.json"), "utf-8")
  );
  const programId = new PublicKey(idl.address);
  const program = new anchor.Program(idl, provider);

  const [configPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    programId
  );

  console.log("🔧 Repairing GeckoConfig on devnet...");
  console.log("   Program ID  :", programId.toBase58());
  console.log("   Config PDA  :", configPda.toBase58());
  console.log("   Admin       :", adminKeypair.publicKey.toBase58());
  console.log("   Oracle auth :", adminKeypair.publicKey.toBase58(), "(admin = oracle on devnet)");
  console.log("   Mint        :", DEVNET_USDC.toBase58());

  // Call repair_config to fix the corrupted fee_bps/bump/allowed_mints
  const tx = await (program.methods as any)
    .repairConfig(
      adminKeypair.publicKey, // oracle_authority = admin on devnet
      [DEVNET_USDC]           // allowed_mints
    )
    .accounts({
      config: configPda,
      authority: adminKeypair.publicKey,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .signers([adminKeypair])
    .rpc();

  console.log("\n✅ GeckoConfig repaired");
  console.log("   Tx       :", tx);
  console.log("   Explorer :", `https://explorer.solana.com/tx/${tx}?cluster=devnet`);

  // Verify — attempt full deserialization
  console.log("\n🔍 Verifying deserialization...");
  const config = await (program.account as any)["geckoConfig"].fetch(configPda);
  console.log("   Authority         :", config["authority"].toBase58());
  console.log("   Treasury          :", config["treasury"].toBase58());
  console.log("   Automation        :", config["automationAuthority"].toBase58());
  console.log("   Oracle            :", config["oracleAuthority"].toBase58());
  console.log("   Fee bps           :", config["feeBps"]);
  console.log("   Bump              :", config["bump"]);
  console.log("   Allowed mints     :", config["allowedMints"].map((m: PublicKey) => m.toBase58()));
  console.log("\n✅ Config deserializes correctly — oracle operations are now unblocked.");
}

main().catch((err) => {
  console.error("❌ Repair failed:", err);
  process.exit(1);
});
