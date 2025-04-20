/**
 * Fluxa Pool Initialization Script
 *
 * This script demonstrates how to create a new liquidity pool in the Fluxa AMM.
 * It walks through the process of:
 * 1. Creating token pair accounts (if they don't exist)
 * 2. Creating token vaults
 * 3. Initializing the pool with a specified fee tier and initial price
 *
 * Usage:
 * ts-node initialize-pool.ts [token-a-mint] [token-b-mint] [fee-tier] [initial-price]
 *
 * Fee tiers:
 * - 100 = 0.01% (stable pairs)
 * - 500 = 0.05% (standard pairs)
 * - 3000 = 0.3% (exotic pairs)
 *
 * Example:
 * ts-node initialize-pool.ts EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v 7vfCXTUXx5WJV5JADk17DUJ4ksgau7utNKj4b963voxs 500 1.0
 */

import * as web3 from "@solana/web3.js";
import * as anchor from "@project-serum/anchor";
import * as spl from "@solana/spl-token";
import BN from "bn.js";
import fs from "fs";
import path from "path";
import { program } from "commander";

// Constants from our AMM
const FEE_TIER_LOW = 100;
const FEE_TIER_MEDIUM = 500;
const FEE_TIER_HIGH = 3000;
const MIN_TICK = -887272;
const MAX_TICK = 887272;
const Q64 = new BN(1).shln(64);

// Utility Functions
const parseTokenMint = (value: string): web3.PublicKey => {
  try {
    return new web3.PublicKey(value);
  } catch (error) {
    throw new Error(`Invalid token mint: ${value}`);
  }
};

const parseFeeTier = (value: string): number => {
  const feeTier = parseInt(value);
  if (![FEE_TIER_LOW, FEE_TIER_MEDIUM, FEE_TIER_HIGH].includes(feeTier)) {
    throw new Error(
      `Invalid fee tier: ${value}. Must be one of: 100, 500, or 3000`
    );
  }
  return feeTier;
};

const parseInitialPrice = (value: string): number => {
  const price = parseFloat(value);
  if (isNaN(price) || price <= 0) {
    throw new Error(`Invalid initial price: ${value}`);
  }
  return price;
};

// Convert a price to sqrt price in Q64.64 format
function priceToSqrtPriceX64(price: number): BN {
  // We're using BN.js for large number arithmetic
  // First convert the price to a BN
  const priceInteger = Math.floor(price);
  const priceFraction = price - priceInteger;

  // Handle integer part
  let result = new BN(priceInteger);
  result = result.mul(Q64);

  // Handle fractional part (with precision)
  if (priceFraction > 0) {
    const precision = 10000; // 4 decimal places
    const fractionBN = new BN(Math.floor(priceFraction * precision));
    const fractionScaled = fractionBN.mul(Q64).div(new BN(precision));
    result = result.add(fractionScaled);
  }

  // Take the square root using Newton's method
  let x = result.div(new BN(2)); // Initial guess
  let y = result.clone();

  while (x.lt(y)) {
    y = x.clone();
    x = result.div(x).add(x).div(new BN(2));
  }

  return y.mul(Q64);
}

async function main() {
  // Parse command line arguments
  program
    .name("initialize-pool")
    .description("Initialize a new Fluxa liquidity pool")
    .argument("<tokenAMint>", "Token A mint address", parseTokenMint)
    .argument("<tokenBMint>", "Token B mint address", parseTokenMint)
    .argument("<feeTier>", "Fee tier (100, 500, or 3000)", parseFeeTier)
    .argument(
      "<initialPrice>",
      "Initial price of token B in terms of token A",
      parseInitialPrice
    )
    .option(
      "-n, --network <network>",
      "Solana network to connect to (mainnet-beta, testnet, devnet, localhost)",
      "devnet"
    )
    .option(
      "-k, --keypair <path>",
      "Path to keypair file",
      process.env.HOME + "/.config/solana/id.json"
    )
    .parse(process.argv);

  const options = program.opts();
  const args = program.args;

  // Extract parameters
  const tokenAMint = args[0];
  const tokenBMint = args[1];
  const feeTier = args[2];
  const initialPrice = args[3];
  const networkName = options.network;
  const keypairPath = options.keypair;

  // Connect to the network
  let connection;
  try {
    const endpoint =
      networkName === "localhost"
        ? "http://localhost:8899"
        : web3.clusterApiUrl(networkName);

    connection = new web3.Connection(endpoint, {
      commitment: "confirmed",
      confirmTransactionInitialTimeout: 60000, // 60 seconds
    });

    // Test connection
    await connection.getVersion();
    console.log(`Connected to ${networkName} network`);
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    console.error(`Failed to connect to ${networkName}:`, errorMessage);
    process.exit(1);
  }

  // Load keypair for transaction signing
  let wallet;
  try {
    if (!fs.existsSync(keypairPath)) {
      console.error(`Keypair file not found at ${keypairPath}`);
      console.error(
        "To create a keypair file, run: solana-keygen new -o <path>"
      );
      process.exit(1);
    }

    const secretKey = Uint8Array.from(
      JSON.parse(fs.readFileSync(keypairPath, "utf-8"))
    );
    wallet = web3.Keypair.fromSecretKey(secretKey);
    console.log(`Using wallet: ${wallet.publicKey.toString()}`);

    // Verify the wallet has enough SOL
    const balance = await connection.getBalance(wallet.publicKey);
    if (balance < 10000000) {
      // 0.01 SOL
      console.warn(
        `Warning: Wallet balance is low (${
          balance / web3.LAMPORTS_PER_SOL
        } SOL)`
      );
      console.warn("Pool creation requires SOL for transaction fees and rent");
      // Continue but warn the user
    }
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    console.error("Failed to load wallet:", errorMessage);
    process.exit(1);
  }

  try {
    // Ensure canonical ordering of token mints
    let [orderedTokenAMint, orderedTokenBMint] =
      tokenAMint.toString() < tokenBMint.toString()
        ? [tokenAMint, tokenBMint]
        : [tokenBMint, tokenAMint];

    // Validate tokens exist on-chain
    try {
      await Promise.all([
        connection.getTokenSupply(
          typeof orderedTokenAMint === "string"
            ? new web3.PublicKey(orderedTokenAMint)
            : orderedTokenAMint
        ),
        connection.getTokenSupply(
          typeof orderedTokenBMint === "string"
            ? new web3.PublicKey(orderedTokenBMint)
            : orderedTokenBMint
        ),
      ]);
    } catch (error: unknown) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      console.error("Error validating token mints:", errorMessage);
      console.error("Please ensure both tokens exist on the selected network");
      process.exit(1);
    }

    // Load the Fluxa program
    let program;
    try {
      const idlPath = "./target/idl/amm_core.json";
      if (!fs.existsSync(idlPath)) {
        console.error(`IDL file not found at ${idlPath}`);
        console.error("Please build the program with: anchor build");
        process.exit(1);
      }

      const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
      const programId = new web3.PublicKey(idl.metadata.address);
      const provider = new anchor.AnchorProvider(
        connection,
        new anchor.Wallet(wallet),
        {
          preflightCommitment: "confirmed",
          commitment: "confirmed",
        }
      );
      program = new anchor.Program(idl, programId, provider);

      console.log(`Loaded Fluxa AMM Core program: ${programId.toString()}`);
    } catch (error: unknown) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      console.error("Failed to load Fluxa program:", errorMessage);
      process.exit(1);
    }

    // Step 1: Find the token pair PDA
    console.log("Finding token pair account...");

    // Ensure we're working with PublicKey objects
    const tokenAMintPubkey =
      typeof orderedTokenAMint === "string"
        ? new web3.PublicKey(orderedTokenAMint)
        : orderedTokenAMint;

    const tokenBMintPubkey =
      typeof orderedTokenBMint === "string"
        ? new web3.PublicKey(orderedTokenBMint)
        : orderedTokenBMint;

    const [tokenPairAccount, tokenPairBump] =
      await web3.PublicKey.findProgramAddress(
        [
          Buffer.from("token_pair"),
          tokenAMintPubkey.toBuffer(),
          tokenBMintPubkey.toBuffer(),
        ],
        program.programId
      );

    console.log(`Token pair account: ${tokenPairAccount.toString()}`);

    // Check if token pair already exists
    let tokenPairExists = true;
    try {
      await program.account.tokenPair.fetch(tokenPairAccount);
      console.log("Token pair already exists");
    } catch (e) {
      tokenPairExists = false;
      console.log("Token pair does not exist, creating now...");
    }

    // Create token pair if it doesn't exist
    if (!tokenPairExists) {
      try {
        const createTokenPairTx = await program.methods
          .createTokenPair()
          .accounts({
            authority: wallet.publicKey,
            tokenPair: tokenPairAccount,
            tokenAMint: tokenAMintPubkey,
            tokenBMint: tokenBMintPubkey,
            systemProgram: web3.SystemProgram.programId,
            rent: web3.SYSVAR_RENT_PUBKEY,
          })
          .rpc();

        console.log(`Token pair created: ${createTokenPairTx}`);
      } catch (error: unknown) {
        const errorMessage =
          error instanceof Error ? error.message : String(error);
        console.error("Failed to create token pair:", errorMessage);
        process.exit(1);
      }
    }

    // Step 2: Create a new pool account
    console.log("Creating pool account...");
    const poolKeypair = web3.Keypair.generate();

    // Step 3: Create token vaults
    console.log("Creating token vaults...");
    let tokenAVault, tokenBVault;
    try {
      tokenAVault = await spl.getAssociatedTokenAddress(
        tokenAMintPubkey,
        poolKeypair.publicKey,
        true
      );

      tokenBVault = await spl.getAssociatedTokenAddress(
        tokenBMintPubkey,
        poolKeypair.publicKey,
        true
      );

      console.log(`Token A vault: ${tokenAVault.toString()}`);
      console.log(`Token B vault: ${tokenBVault.toString()}`);
    } catch (error: unknown) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      console.error("Failed to generate vault addresses:", errorMessage);
      process.exit(1);
    }

    // Create token vaults
    try {
      const createVaultsTx = new web3.Transaction();
      createVaultsTx.add(
        spl.createAssociatedTokenAccountInstruction(
          wallet.publicKey,
          tokenAVault,
          poolKeypair.publicKey,
          tokenAMintPubkey
        )
      );
      createVaultsTx.add(
        spl.createAssociatedTokenAccountInstruction(
          wallet.publicKey,
          tokenBVault,
          poolKeypair.publicKey,
          tokenBMintPubkey
        )
      );

      // Sign and send the transaction to create vaults
      console.log("Sending transaction to create token vaults...");
      const vaultsTxId = await web3.sendAndConfirmTransaction(
        connection,
        createVaultsTx,
        [wallet],
        { commitment: "confirmed", maxRetries: 3 }
      );
      console.log(`Vaults created: ${vaultsTxId}`);
    } catch (error: unknown) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      console.error("Failed to create token vaults:", errorMessage);
      process.exit(1);
    }

    // Step 4: Initialize the pool
    console.log(
      `Initializing pool with fee tier: ${feeTier} and initial price: ${initialPrice}`
    );

    // Convert the initial price to sqrt_price_x64
    let sqrtPriceX64;
    try {
      sqrtPriceX64 = priceToSqrtPriceX64(Number(initialPrice));
      console.log(`Calculated sqrt_price_x64: ${sqrtPriceX64.toString()}`);
    } catch (error: unknown) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      console.error("Failed to convert price to sqrt format:", errorMessage);
      process.exit(1);
    }

    // Initialize pool transaction
    try {
      const initPoolTx = await program.methods
        .initializePool(parseInt(sqrtPriceX64.toString()), feeTier)
        .accounts({
          payer: wallet.publicKey,
          tokenPair: tokenPairAccount,
          pool: poolKeypair.publicKey,
          tokenAMint: tokenAMintPubkey,
          tokenBMint: tokenBMintPubkey,
          tokenAVault: tokenAVault,
          tokenBVault: tokenBVault,
          tokenProgram: spl.TOKEN_PROGRAM_ID,
          systemProgram: web3.SystemProgram.programId,
          rent: web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([wallet, poolKeypair])
        .rpc();

      console.log(`Pool initialized successfully: ${initPoolTx}`);
      console.log(`Pool address: ${poolKeypair.publicKey.toString()}`);
    } catch (error: unknown) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      console.error("Failed to initialize pool:", errorMessage);

      // Handle Solana/Anchor specific error with logs property
      const anchorError = error as { logs?: string[] };
      console.error(
        "Transaction simulation error details:",
        anchorError.logs?.join("\n") || "No logs available"
      );
      process.exit(1);
    }

    // Output pool information
    const poolInfo = {
      address: poolKeypair.publicKey.toString(),
      tokenAMint: orderedTokenAMint.toString(),
      tokenBMint: orderedTokenBMint.toString(),
      tokenAVault: tokenAVault.toString(),
      tokenBVault: tokenBVault.toString(),
      feeTier: feeTier,
      initialPrice: initialPrice,
      network: networkName,
      createdAt: new Date().toISOString(),
    };

    console.log("\nPool Information:");
    console.table(poolInfo);

    // Save pool information to a file
    const poolInfoDir = "./poolinfo";
    if (!fs.existsSync(poolInfoDir)) {
      fs.mkdirSync(poolInfoDir);
    }

    const filename = `${poolInfoDir}/pool_${poolKeypair.publicKey
      .toString()
      .substring(0, 8)}.json`;
    fs.writeFileSync(filename, JSON.stringify(poolInfo, null, 2));
    console.log(`Pool information saved to ${filename}`);
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    console.error("Unexpected error during pool initialization:", errorMessage);
    process.exit(1);
  }
}

main().then(
  () => process.exit(0),
  (err) => {
    console.error(err);
    process.exit(1);
  }
);
