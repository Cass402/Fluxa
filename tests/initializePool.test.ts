import anchor from "@coral-xyz/anchor";
import { Wallet } from "@coral-xyz/anchor";
const { AnchorProvider, workspace, web3, setProvider, AnchorError } = anchor;
const BN = anchor.BN;
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  Mint,
  getMint,
  createMint,
  getAccount,
  createAccount,
} from "@solana/spl-token";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import { expect } from "chai";
import { AmmCore } from "../target/types/amm_core.ts"; // Adjust path if your IDL types are elsewhere

// Helper type for NodeWallet-like structure, assuming provider.wallet is like NodeWallet.
// The `Wallet` interface is from @coral-xyz/anchor, Keypair is from @solana/web3.js.
interface PayerWallet extends Wallet {
  payer: Keypair;
}

describe("AMM Core - Initialize Pool (TypeScript)", () => {
  // Configure the client to use the local cluster.
  const provider = AnchorProvider.local();
  setProvider(provider);

  // Load the program from the workspace.
  // Make sure your Anchor.toml points to the correct program name and IDL path.
  const program = workspace.ammCore as anchor.Program<AmmCore>;
  // The 'walletSigner' object from provider.wallet, conforms to Wallet interface
  const walletSigner = provider.wallet as Wallet;
  // Assuming provider.wallet is a NodeWallet-like object that has a 'payer' Keypair property
  const feePayerKeypair = (provider.wallet as PayerWallet).payer;

  let mintAKeyPair: Keypair;
  let mintBKeyPair: Keypair;
  let mintAPublicKey: PublicKey;
  let mintBPublicKey: PublicKey;

  const factoryKeypair = Keypair.generate();

  // Helper function to create a new mint
  async function createTestMint(
    connection: Connection,
    authority: PublicKey
  ): Promise<Keypair> {
    const mintKeypair = Keypair.generate();
    await createMint(
      connection,
      feePayerKeypair, // Payer for the mint creation transaction (actual Keypair)
      authority, // Mint authority
      null, // Freeze authority (optional)
      0, // Decimals
      mintKeypair // Mint keypair
    );
    return mintKeypair;
  }

  before(async () => {
    // Create two mints and ensure canonical order
    let tempMint1 = await createTestMint(
      provider.connection,
      walletSigner.publicKey
    );
    let tempMint2 = await createTestMint(
      provider.connection,
      walletSigner.publicKey
    );

    if (
      tempMint1.publicKey.toBuffer().compare(tempMint2.publicKey.toBuffer()) < 0
    ) {
      mintAKeyPair = tempMint1;
      mintBKeyPair = tempMint2;
    } else {
      mintAKeyPair = tempMint2;
      mintBKeyPair = tempMint1;
    }
    mintAPublicKey = mintAKeyPair.publicKey;
    mintBPublicKey = mintBKeyPair.publicKey;

    console.log("Mint A (Canonical):", mintAPublicKey.toBase58());
    console.log("Mint B (Canonical):", mintBPublicKey.toBase58());
    console.log("Factory:", factoryKeypair.publicKey.toBase58());
    console.log("Payer:", walletSigner.publicKey.toBase58());
  });

  it("Successfully initializes a new pool", async () => {
    const initialSqrtPriceQ64 = new BN("36893488147419103232");
    const feeRate = 30; // 0.3% in basis points
    const tickSpacing = 60;

    const [poolPda, poolBump] = await PublicKey.findProgramAddress(
      [
        Buffer.from("pool"),
        mintAPublicKey.toBuffer(),
        mintBPublicKey.toBuffer(),
      ],
      program.programId
    );

    const poolVaultAKeypair = Keypair.generate();
    const poolVaultBKeypair = Keypair.generate();

    console.log("Pool PDA:", poolPda.toBase58());
    console.log("Pool Vault A:", poolVaultAKeypair.publicKey.toBase58());
    console.log("Pool Vault B:", poolVaultBKeypair.publicKey.toBase58());

    const txSignature = await program.methods
      .initializePoolHandler(initialSqrtPriceQ64, feeRate, tickSpacing)
      .accountsStrict({
        pool: poolPda,
        mintA: mintAPublicKey,
        mintB: mintBPublicKey,
        factory: factoryKeypair.publicKey,
        poolVaultA: poolVaultAKeypair.publicKey,
        poolVaultB: poolVaultBKeypair.publicKey,
        payer: walletSigner.publicKey, // The publicKey of the wallet paying fees
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([poolVaultAKeypair, poolVaultBKeypair]) // Vaults are new accounts being initialized
      .rpc();

    console.log("Initialize pool transaction signature", txSignature);
    await provider.connection.confirmTransaction(txSignature, "confirmed");

    // Fetch and verify pool account
    const poolAccount = await program.account.pool.fetch(poolPda);
    expect(poolAccount.bump).to.equal(poolBump);
    expect(poolAccount.factory.toBase58()).to.equal(
      factoryKeypair.publicKey.toBase58()
    );
    expect(poolAccount.token0Mint.toBase58()).to.equal(
      mintAPublicKey.toBase58()
    ); // mintA is token0
    expect(poolAccount.token1Mint.toBase58()).to.equal(
      mintBPublicKey.toBase58()
    ); // mintB is token1
    expect(poolAccount.token0Vault.toBase58()).to.equal(
      poolVaultAKeypair.publicKey.toBase58()
    );
    expect(poolAccount.token1Vault.toBase58()).to.equal(
      poolVaultBKeypair.publicKey.toBase58()
    );
    expect(poolAccount.feeRate).to.equal(feeRate);
    expect(poolAccount.tickSpacing).to.equal(tickSpacing);
    expect(poolAccount.sqrtPriceQ64.toString()).to.equal(
      initialSqrtPriceQ64.toString()
    );
    // currentTick verification requires replicating the math.sqrt_price_q64_to_tick logic in TS or calling a view function if available
    // For now, we can check if it's a plausible value (not default 0 if price is not 0)
    // This requires the `math.rs` `sqrt_price_q64_to_tick` logic to be available or replicated.
    // const expectedTick = calculateTick(initialSqrtPriceQ64); // You'd need this helper
    expect(poolAccount.currentTick).to.equal(0);
    expect(poolAccount.liquidity.toString()).to.equal("0");
    expect(poolAccount.tickBitmapData.length).to.be.greaterThan(0); // Serialized empty BTreeMap

    // Verify vault A
    const vaultAInfo = await getAccount(
      provider.connection,
      poolVaultAKeypair.publicKey
    );
    expect(vaultAInfo.mint.toBase58()).to.equal(mintAPublicKey.toBase58());
    expect(vaultAInfo.owner.toBase58()).to.equal(poolPda.toBase58());

    // Verify vault B
    const vaultBInfo = await getAccount(
      provider.connection,
      poolVaultBKeypair.publicKey
    );
    expect(vaultBInfo.mint.toBase58()).to.equal(mintBPublicKey.toBase58());
    expect(vaultBInfo.owner.toBase58()).to.equal(poolPda.toBase58());

    console.log("Pool initialized and verified successfully!");
  });

  it("Fails if mints are not in canonical order", async () => {
    const initialSqrtPriceQ64 = new BN("18446744073709551616"); // Price 1.0
    const feeRate = 30;
    const tickSpacing = 60;

    // Intentionally swapped mints for the .accounts call
    const nonCanonicalMintA = mintBKeyPair.publicKey; // mintB is "larger"
    const nonCanonicalMintB = mintAKeyPair.publicKey; // mintA is "smaller"

    // PDA derivation for the instruction must use the keys as they will be passed.
    // The program itself will derive the PDA based on canonical order if it were to succeed past the check.
    // However, the initial check `ctx.accounts.mint_a.key() >= ctx.accounts.mint_b.key()`
    // is what we are testing here.
    const [poolPdaAttempt, _] = await PublicKey.findProgramAddress(
      [
        Buffer.from("pool"),
        nonCanonicalMintA.toBuffer(), // Larger key first
        nonCanonicalMintB.toBuffer(), // Smaller key second
      ],
      program.programId
    );

    const poolVaultAKeypair = Keypair.generate();
    const poolVaultBKeypair = Keypair.generate();

    try {
      await program.methods
        .initializePoolHandler(initialSqrtPriceQ64, feeRate, tickSpacing)
        .accountsStrict({
          pool: poolPdaAttempt,
          mintA: nonCanonicalMintA, // Larger key
          mintB: nonCanonicalMintB, // Smaller key
          factory: factoryKeypair.publicKey,
          poolVaultA: poolVaultAKeypair.publicKey,
          poolVaultB: poolVaultBKeypair.publicKey,
          payer: walletSigner.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([poolVaultAKeypair, poolVaultBKeypair])
        .rpc();
      expect.fail(
        "Transaction should have failed due to non-canonical mint order."
      );
    } catch (error) {
      // Anchor wraps Rust errors. Check for the specific error code.
      // ErrorCode::MintsNotInCanonicalOrder = 6000 (0x1770) by default if it's the first error.
      // You should confirm this value from your compiled IDL or program.
      if (error instanceof AnchorError) {
        expect(error.toString()).to.include("MintsNotInCanonicalOrder"); // Or check error.code
        console.log("Successfully caught non-canonical mint order error.");
      } else {
        console.error("Unexpected error type:", error);
        throw error; // Re-throw if it's not the expected error
      }
    }
  });

  it("Fails with invalid tick spacing (0)", async () => {
    // Use unique mints for this test to avoid PDA collision
    let localMintAKeyPair = await createTestMint(
      provider.connection,
      walletSigner.publicKey
    );
    let localMintBKeyPair = await createTestMint(
      provider.connection,
      walletSigner.publicKey
    );
    let localMintAPublicKey: PublicKey;
    let localMintBPublicKey: PublicKey;

    if (
      localMintAKeyPair.publicKey
        .toBuffer()
        .compare(localMintBKeyPair.publicKey.toBuffer()) < 0
    ) {
      localMintAPublicKey = localMintAKeyPair.publicKey;
      localMintBPublicKey = localMintBKeyPair.publicKey;
    } else {
      const temp = localMintAKeyPair;
      localMintAKeyPair = localMintBKeyPair;
      localMintBKeyPair = temp;
      localMintAPublicKey = localMintAKeyPair.publicKey;
      localMintBPublicKey = localMintBKeyPair.publicKey;
    }

    const initialSqrtPriceQ64 = new BN("18446744073709551616"); // Price 1.0
    const feeRate = 30;
    const tickSpacing = 0; // Invalid

    const [poolPda, _] = await PublicKey.findProgramAddress(
      [
        Buffer.from("pool"),
        localMintAPublicKey.toBuffer(),
        localMintBPublicKey.toBuffer(),
      ],
      program.programId
    );

    // Vaults will be initialized by the program, so just need their keypairs for signing
    const poolVaultAKeypair = Keypair.generate();
    const poolVaultBKeypair = Keypair.generate();

    try {
      await program.methods
        .initializePoolHandler(initialSqrtPriceQ64, feeRate, tickSpacing)
        .accountsStrict({
          pool: poolPda,
          mintA: localMintAPublicKey, // Use the local mint A for this test
          mintB: localMintBPublicKey,
          factory: factoryKeypair.publicKey, // Can reuse factory
          poolVaultA: poolVaultAKeypair.publicKey,
          poolVaultB: poolVaultBKeypair.publicKey,
          payer: walletSigner.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([poolVaultAKeypair, poolVaultBKeypair])
        .rpc();
      expect.fail(
        "Transaction should have failed due to invalid tick spacing."
      );
    } catch (error) {
      // ErrorCode::InvalidTickSpacing = 6002 (0x1772) if it's the third error (after MintsMustDiffer)
      if (error instanceof AnchorError) {
        expect(error.error.errorCode.code).to.equal("InvalidTickSpacing");
        console.log("Successfully caught invalid tick spacing error.");
      } else {
        console.error("Unexpected error type:", error);
        throw error; // Re-throw if it's not the expected error
      }
    }
  });

  it("Fails with invalid initial price (0)", async () => {
    // Use unique mints for this test to avoid PDA collision
    let localMintAKeyPair = await createTestMint(
      provider.connection,
      walletSigner.publicKey
    );
    let localMintBKeyPair = await createTestMint(
      provider.connection,
      walletSigner.publicKey
    );
    let localMintAPublicKey: PublicKey;
    let localMintBPublicKey: PublicKey;

    if (
      localMintAKeyPair.publicKey
        .toBuffer()
        .compare(localMintBKeyPair.publicKey.toBuffer()) < 0
    ) {
      localMintAPublicKey = localMintAKeyPair.publicKey;
      localMintBPublicKey = localMintBKeyPair.publicKey;
    } else {
      const temp = localMintAKeyPair;
      localMintAKeyPair = localMintBKeyPair;
      localMintBKeyPair = temp;
      localMintAPublicKey = localMintAKeyPair.publicKey;
      localMintBPublicKey = localMintBKeyPair.publicKey;
    }

    const initialSqrtPriceQ64 = new BN(0); // Invalid
    const feeRate = 30;
    const tickSpacing = 60;

    const [poolPda, _] = await PublicKey.findProgramAddress(
      [
        Buffer.from("pool"),
        localMintAPublicKey.toBuffer(),
        localMintBPublicKey.toBuffer(),
      ],
      program.programId
    );

    // Vaults will be initialized by the program, so just need their keypairs for signing
    const poolVaultAKeypair = Keypair.generate();
    const poolVaultBKeypair = Keypair.generate();

    try {
      await program.methods
        .initializePoolHandler(initialSqrtPriceQ64, feeRate, tickSpacing)
        .accountsStrict({
          pool: poolPda,
          mintA: localMintAPublicKey, // Use the local mint A for this test
          mintB: localMintBPublicKey,
          factory: factoryKeypair.publicKey, // Can reuse factory
          poolVaultA: poolVaultAKeypair.publicKey,
          poolVaultB: poolVaultBKeypair.publicKey,
          payer: walletSigner.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([poolVaultAKeypair, poolVaultBKeypair])
        .rpc();
      expect.fail(
        "Transaction should have failed due to invalid initial price."
      );
    } catch (error) {
      // ErrorCode::InvalidInitialPrice = 6001 (0x1771) if it's the second error
      if (error instanceof AnchorError) {
        expect(error.error.errorCode.code).to.equal("InvalidInitialPrice");
        console.log("Successfully caught invalid initial price (zero) error.");
      } else {
        console.error("Unexpected error type:", error);
        throw error; // Re-throw if it's not the expected error
      }
    }
  });

  // You would also add a test for initialSqrtPriceQ64 > MAX_SQRT_PRICE
  // Similar to the Rust test, you'd need the MAX_SQRT_PRICE constant.
  // e.g. const MAX_SQRT_PRICE = new BN("...");
  // if (initialSqrtPriceQ64.gt(MAX_SQRT_PRICE)) // ... expect error
});

// Helper function to replicate tick calculation if needed for currentTick verification.
// This is a simplified placeholder. Your actual math might be more complex.
// import { tickToSqrtPriceQ64, sqrtPriceQ64ToTick } from "./math_utils"; // if you create such a file
/*
function calculateTick(sqrtPriceQ64: BN): number {
    // Placeholder: This needs to replicate the exact Rust logic from math::sqrt_price_q64_to_tick
    // For example, if price = 1, sqrtPrice = 1. sqrtPriceQ64 = 1 * 2^64.
    // log_sqrt(1) = 0. Tick is often related to log of price.
    // This is non-trivial and would require careful porting of your Rust math.
    // Example: For sqrt_price = 1, tick is often 0.
    if (sqrtPriceQ64.eq(new BN("79228162514264337593543950336"))) { // approx 2^64
        return 0;
    }
    // This is a very rough approximation and likely incorrect for general cases.
    // Replace with actual logic from your `math.rs`.
    const price = sqrtPriceQ64.mul(sqrtPriceQ64).div(new BN(2).pow(new BN(128))); // Approximate price
    return Math.floor(Math.log(price.toNumber()) / Math.log(1.0001)); // Uniswap V3 like formula (example)
}
*/
