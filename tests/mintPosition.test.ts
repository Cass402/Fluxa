import * as anchor from "@coral-xyz/anchor";
import { Wallet } from "@coral-xyz/anchor";
const { AnchorProvider, workspace, web3, setProvider } = anchor;
const BN = anchor.BN;
import {
  TOKEN_PROGRAM_ID,
  createMint,
  getAccount,
  Mint,
} from "@solana/spl-token";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import { expect } from "chai";
import { AmmCore } from "../target/types/amm_core.ts"; // Adjust path if your IDL types are elsewhere

// Helper type for NodeWallet-like structure, assuming provider.wallet is like NodeWallet.
interface PayerWallet extends Wallet {
  payer: Keypair;
}

// Constants from constants.rs
const MIN_TICK = -887272;
const MAX_TICK = 887272;
const MIN_LIQUIDITY = new BN(1000);

describe("AMM Core - Mint Position (TypeScript)", () => {
  const provider = AnchorProvider.local();
  setProvider(provider);

  const program = workspace.ammCore as anchor.Program<AmmCore>;
  const walletSigner = provider.wallet as Wallet;
  // Assuming provider.wallet is a NodeWallet-like object that has a 'payer' Keypair property
  const feePayerKeypair = (provider.wallet as PayerWallet).payer;

  let mintAKeyPair: Keypair;
  let mintBKeyPair: Keypair;
  let mintAPublicKey: PublicKey;
  let mintBPublicKey: PublicKey;

  const factoryKeypair = Keypair.generate();
  let poolPda: PublicKey;
  let poolBump: number;
  let poolVaultAKeypair: Keypair;
  let poolVaultBKeypair: Keypair;

  const tickSpacing = 60; // Must match what the pool is initialized with
  const feeRate = 30; // 0.3%

  // Helper function to create a new mint
  async function createTestMint(
    connection: Connection,
    authority: PublicKey
  ): Promise<Keypair> {
    const mintKeypair = Keypair.generate();
    // Airdrop SOL to payer if needed (for local testing)
    const lamports = await connection.getBalance(feePayerKeypair.publicKey);
    if (lamports < LAMPORTS_PER_SOL) {
      const sig = await connection.requestAirdrop(
        feePayerKeypair.publicKey,
        2 * LAMPORTS_PER_SOL
      );
      await connection.confirmTransaction(sig);
    }
    await new Promise((resolve) => setTimeout(resolve, 1000)); // wait for airdrop confirmation and account update

    await createMint(
      connection,
      feePayerKeypair,
      authority,
      null,
      0, // Decimals
      mintKeypair
    );
    return mintKeypair;
  }

  before(async () => {
    // 1. Create Mints
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

    // 2. Initialize Pool (reusing logic from initializePool.test.ts)
    const initialSqrtPriceQ64 = new BN("79228162514264337593543950336"); // Price 1.0 (1 << 64)

    [poolPda, poolBump] = await PublicKey.findProgramAddress(
      [
        Buffer.from("pool"),
        mintAPublicKey.toBuffer(),
        mintBPublicKey.toBuffer(),
      ],
      program.programId
    );

    poolVaultAKeypair = Keypair.generate();
    poolVaultBKeypair = Keypair.generate();

    console.log("Pool PDA for mint position tests:", poolPda.toBase58());

    await program.methods
      .initializePoolHandler(initialSqrtPriceQ64, feeRate, tickSpacing)
      .accountsStrict({
        pool: poolPda,
        mintA: mintAPublicKey,
        mintB: mintBPublicKey,
        factory: factoryKeypair.publicKey,
        poolVaultA: poolVaultAKeypair.publicKey,
        poolVaultB: poolVaultBKeypair.publicKey,
        payer: walletSigner.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([poolVaultAKeypair, poolVaultBKeypair, feePayerKeypair])
      .rpc();
    console.log("Pool initialized for mint position tests.");

    const poolAccount = await program.account.pool.fetch(poolPda);
    expect(poolAccount.tickSpacing).to.equal(tickSpacing);
  });

  it("Successfully mints a new position", async () => {
    const tickLowerIndex = -tickSpacing * 10; // e.g., -600
    const tickUpperIndex = tickSpacing * 20; // e.g., 1200
    const liquidityAmountDesired = new BN(1000000); // Example liquidity

    const [positionPda, positionBump] = await PublicKey.findProgramAddress(
      [
        Buffer.from("position"),
        poolPda.toBuffer(),
        walletSigner.publicKey.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
        new BN(tickUpperIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );

    const [tickLowerPda, tickLowerBump] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );

    const [tickUpperPda, tickUpperBump] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickUpperIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );

    console.log("Position PDA:", positionPda.toBase58());
    console.log("TickLower PDA:", tickLowerPda.toBase58());
    console.log("TickUpper PDA:", tickUpperPda.toBase58());

    const txSignature = await program.methods
      .mintPositionHandler(
        tickLowerIndex,
        tickUpperIndex,
        liquidityAmountDesired
      )
      .accountsStrict({
        pool: poolPda,
        position: positionPda,
        tickLower: tickLowerPda,
        tickUpper: tickUpperPda,
        owner: walletSigner.publicKey,
        payer: walletSigner.publicKey, // feePayerKeypair.publicKey if different
        systemProgram: SystemProgram.programId,
        rent: web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([feePayerKeypair]) // Payer signs for account creation
      .rpc();

    console.log("Mint position transaction signature", txSignature);
    await provider.connection.confirmTransaction(txSignature, "confirmed");

    // Fetch and verify position account
    const positionAccount = await program.account.positionData.fetch(
      positionPda
    );
    expect(positionAccount.owner.toBase58()).to.equal(
      walletSigner.publicKey.toBase58()
    );
    expect(positionAccount.pool.toBase58()).to.equal(poolPda.toBase58());
    expect(positionAccount.tickLowerIndex).to.equal(tickLowerIndex);
    expect(positionAccount.tickUpperIndex).to.equal(tickUpperIndex);
    expect(positionAccount.liquidity.toString()).to.equal(
      liquidityAmountDesired.toString()
    );

    // Fetch and verify tick accounts (were initialized)
    const tickLowerAccount = await program.account.tickData.fetch(tickLowerPda);
    expect(tickLowerAccount.pool.toBase58()).to.equal(poolPda.toBase58());
    expect(tickLowerAccount.index).to.equal(tickLowerIndex);
    expect(tickLowerAccount.liquidityNet.toString()).to.equal(
      liquidityAmountDesired.toString()
    );

    const tickUpperAccount = await program.account.tickData.fetch(tickUpperPda);
    expect(tickUpperAccount.pool.toBase58()).to.equal(poolPda.toBase58());
    expect(tickUpperAccount.index).to.equal(tickUpperIndex);
    expect(tickUpperAccount.liquidityNet.toString()).to.equal(
      liquidityAmountDesired.neg().toString()
    );

    const poolAccount = await program.account.pool.fetch(poolPda);
    if (
      poolAccount.currentTick >= tickLowerIndex &&
      poolAccount.currentTick < tickUpperIndex
    ) {
      expect(poolAccount.liquidity.toString()).to.equal(
        liquidityAmountDesired.toString()
      );
    } else {
      expect(poolAccount.liquidity.toString()).to.equal("0");
    }
    console.log("Position minted and verified successfully!");
  });

  it("Fails if tick_lower_index >= tick_upper_index", async () => {
    const tickLowerIndex = tickSpacing * 10; // 600
    const tickUpperIndexValid = tickSpacing * 15; // 900
    const tickUpperIndexInvalid = tickSpacing * 5; // 300 (lower > upper)
    const tickUpperIndexSame = tickSpacing * 10; // 600 (lower == upper)
    const liquidityAmountDesired = MIN_LIQUIDITY;

    // Case 1: tick_lower_index > tick_upper_index
    const [positionPdaInvalid] = await PublicKey.findProgramAddress(
      [
        Buffer.from("position"),
        poolPda.toBuffer(),
        walletSigner.publicKey.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
        new BN(tickUpperIndexInvalid).toArrayLike(Buffer, "le", 4), // Invalid upper tick
      ],
      program.programId
    );
    const [tickLowerPdaInvalid] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );
    const [tickUpperPdaInvalid] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickUpperIndexInvalid).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );

    try {
      await program.methods
        .mintPositionHandler(
          tickLowerIndex,
          tickUpperIndexInvalid, // Invalid
          liquidityAmountDesired
        )
        .accountsStrict({
          pool: poolPda,
          position: positionPdaInvalid,
          tickLower: tickLowerPdaInvalid,
          tickUpper: tickUpperPdaInvalid,
          owner: walletSigner.publicKey,
          payer: walletSigner.publicKey,
          systemProgram: SystemProgram.programId,
          rent: web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([feePayerKeypair])
        .rpc();
      expect.fail(
        "Transaction should have failed due to tick_lower_index > tick_upper_index."
      );
    } catch (error) {
      expect(error).to.be.instanceOf(anchor.AnchorError);
      const anchorError = error as anchor.AnchorError;
      expect(anchorError.error.errorCode.code).to.equal("InvalidTickRange");
      console.log(
        "Successfully caught error for tick_lower_index > tick_upper_index."
      );
    }

    // Case 2: tick_lower_index == tick_upper_index
    const [positionPdaSame] = await PublicKey.findProgramAddress(
      [
        Buffer.from("position"),
        poolPda.toBuffer(),
        walletSigner.publicKey.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
        new BN(tickUpperIndexSame).toArrayLike(Buffer, "le", 4), // Same upper tick
      ],
      program.programId
    );
    const [tickUpperPdaSame] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickUpperIndexSame).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );

    try {
      await program.methods
        .mintPositionHandler(
          tickLowerIndex,
          tickUpperIndexSame, // Same as lower
          liquidityAmountDesired
        )
        .accountsStrict({
          pool: poolPda,
          position: positionPdaSame,
          tickLower: tickLowerPdaInvalid, // Can reuse tickLowerPda from previous invalid attempt for diff position
          tickUpper: tickUpperPdaSame,
          owner: walletSigner.publicKey,
          payer: walletSigner.publicKey,
          systemProgram: SystemProgram.programId,
          rent: web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([feePayerKeypair])
        .rpc();
      expect.fail(
        "Transaction should have failed due to tick_lower_index == tick_upper_index."
      );
    } catch (error) {
      expect(error).to.be.instanceOf(anchor.AnchorError);
      const anchorError = error as anchor.AnchorError;
      expect(anchorError.error.errorCode.code).to.equal("InvalidTickRange");
      console.log(
        "Successfully caught error for tick_lower_index == tick_upper_index."
      );
    }
  });

  it("Fails if tick_lower_index < MIN_TICK", async () => {
    const tickLowerIndex = MIN_TICK - 1; // Invalid
    const tickUpperIndex = tickSpacing * 10;
    const liquidityAmountDesired = MIN_LIQUIDITY;

    const [positionPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("position"),
        poolPda.toBuffer(),
        walletSigner.publicKey.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
        new BN(tickUpperIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );
    const [tickLowerPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );
    const [tickUpperPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickUpperIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );

    try {
      await program.methods
        .mintPositionHandler(
          tickLowerIndex,
          tickUpperIndex,
          liquidityAmountDesired
        )
        .accountsStrict({
          pool: poolPda,
          position: positionPda,
          tickLower: tickLowerPda,
          tickUpper: tickUpperPda,
          owner: walletSigner.publicKey,
          payer: walletSigner.publicKey,
          systemProgram: SystemProgram.programId,
          rent: web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([feePayerKeypair])
        .rpc();
      expect.fail(
        "Transaction should have failed due to tick_lower_index < MIN_TICK."
      );
    } catch (error) {
      expect(error).to.be.instanceOf(anchor.AnchorError);
      const anchorError = error as anchor.AnchorError;
      expect(anchorError.error.errorCode.code).to.equal("InvalidTickRange");
      console.log("Successfully caught error for tick_lower_index < MIN_TICK.");
    }
  });

  it("Fails if tick_upper_index > MAX_TICK", async () => {
    const tickLowerIndex = tickSpacing * 10;
    const tickUpperIndex = MAX_TICK + 1; // Invalid
    const liquidityAmountDesired = MIN_LIQUIDITY;

    const [positionPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("position"),
        poolPda.toBuffer(),
        walletSigner.publicKey.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
        new BN(tickUpperIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );
    const [tickLowerPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );
    const [tickUpperPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickUpperIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );

    try {
      await program.methods
        .mintPositionHandler(
          tickLowerIndex,
          tickUpperIndex,
          liquidityAmountDesired
        )
        .accountsStrict({
          pool: poolPda,
          position: positionPda,
          tickLower: tickLowerPda,
          tickUpper: tickUpperPda,
          owner: walletSigner.publicKey,
          payer: walletSigner.publicKey,
          systemProgram: SystemProgram.programId,
          rent: web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([feePayerKeypair])
        .rpc();
      expect.fail(
        "Transaction should have failed due to tick_upper_index > MAX_TICK."
      );
    } catch (error) {
      expect(error).to.be.instanceOf(anchor.AnchorError);
      const anchorError = error as anchor.AnchorError;
      expect(anchorError.error.errorCode.code).to.equal("InvalidTickRange");
      console.log("Successfully caught error for tick_upper_index > MAX_TICK.");
    }
  });

  it("Fails if tick_lower_index is not aligned with tick_spacing", async () => {
    const tickLowerIndex = tickSpacing * 10 + 1; // Invalid, not a multiple of tickSpacing (60)
    const tickUpperIndex = tickSpacing * 20;
    const liquidityAmountDesired = MIN_LIQUIDITY;

    const [positionPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("position"),
        poolPda.toBuffer(),
        walletSigner.publicKey.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
        new BN(tickUpperIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );
    const [tickLowerPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );
    const [tickUpperPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickUpperIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );

    try {
      await program.methods
        .mintPositionHandler(
          tickLowerIndex,
          tickUpperIndex,
          liquidityAmountDesired
        )
        .accountsStrict({
          pool: poolPda,
          position: positionPda,
          tickLower: tickLowerPda,
          tickUpper: tickUpperPda,
          owner: walletSigner.publicKey,
          payer: walletSigner.publicKey,
          systemProgram: SystemProgram.programId,
          rent: web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([feePayerKeypair])
        .rpc();
      expect.fail(
        "Transaction should have failed due to unaligned tick_lower_index."
      );
    } catch (error) {
      expect(error).to.be.instanceOf(anchor.AnchorError);
      const anchorError = error as anchor.AnchorError;
      expect(anchorError.error.errorCode.code).to.equal("InvalidTickSpacing");
      console.log("Successfully caught error for unaligned tick_lower_index.");
    }
  });

  it("Fails if tick_upper_index is not aligned with tick_spacing", async () => {
    const tickLowerIndex = tickSpacing * 10;
    const tickUpperIndex = tickSpacing * 20 + 1; // Invalid, not a multiple of tickSpacing (60)
    const liquidityAmountDesired = MIN_LIQUIDITY;

    const [positionPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("position"),
        poolPda.toBuffer(),
        walletSigner.publicKey.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
        new BN(tickUpperIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );
    const [tickLowerPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );
    const [tickUpperPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickUpperIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );

    try {
      await program.methods
        .mintPositionHandler(
          tickLowerIndex,
          tickUpperIndex,
          liquidityAmountDesired
        )
        .accountsStrict({
          pool: poolPda,
          position: positionPda,
          tickLower: tickLowerPda,
          tickUpper: tickUpperPda,
          owner: walletSigner.publicKey,
          payer: walletSigner.publicKey,
          systemProgram: SystemProgram.programId,
          rent: web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([feePayerKeypair])
        .rpc();
      expect.fail(
        "Transaction should have failed due to unaligned tick_upper_index."
      );
    } catch (error) {
      expect(error).to.be.instanceOf(anchor.AnchorError);
      const anchorError = error as anchor.AnchorError;
      expect(anchorError.error.errorCode.code).to.equal("InvalidTickSpacing");
      console.log("Successfully caught error for unaligned tick_upper_index.");
    }
  });

  it("Fails if liquidity_amount_desired is 0", async () => {
    const tickLowerIndex = tickSpacing * 10;
    const tickUpperIndex = tickSpacing * 20;
    const liquidityAmountDesired = new BN(0); // Invalid

    const [positionPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("position"),
        poolPda.toBuffer(),
        walletSigner.publicKey.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
        new BN(tickUpperIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );
    const [tickLowerPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );
    const [tickUpperPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickUpperIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );

    try {
      await program.methods
        .mintPositionHandler(
          tickLowerIndex,
          tickUpperIndex,
          liquidityAmountDesired
        )
        .accountsStrict({
          pool: poolPda,
          position: positionPda,
          tickLower: tickLowerPda,
          tickUpper: tickUpperPda,
          owner: walletSigner.publicKey,
          payer: walletSigner.publicKey,
          systemProgram: SystemProgram.programId,
          rent: web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([feePayerKeypair])
        .rpc();
      expect.fail(
        "Transaction should have failed due to zero liquidity_amount_desired."
      );
    } catch (error) {
      expect(error).to.be.instanceOf(anchor.AnchorError);
      const anchorError = error as anchor.AnchorError;
      expect(anchorError.error.errorCode.code).to.equal("ZeroLiquidityDelta");
      console.log(
        "Successfully caught error for zero liquidity_amount_desired."
      );
    }
  });

  it("Fails if liquidity_amount_desired < MIN_LIQUIDITY", async () => {
    const tickLowerIndex = tickSpacing * 10;
    const tickUpperIndex = tickSpacing * 20;
    const liquidityAmountDesired = MIN_LIQUIDITY.sub(new BN(1)); // Invalid

    if (liquidityAmountDesired.eqn(0)) {
      console.log(
        "Skipping MIN_LIQUIDITY test as MIN_LIQUIDITY might be 1, causing amount to be 0, which is already tested."
      );
      return; // Already tested by ZeroLiquidityDelta
    }

    const [positionPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("position"),
        poolPda.toBuffer(),
        walletSigner.publicKey.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
        new BN(tickUpperIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );
    const [tickLowerPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickLowerIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );
    const [tickUpperPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("tick"),
        poolPda.toBuffer(),
        new BN(tickUpperIndex).toArrayLike(Buffer, "le", 4),
      ],
      program.programId
    );

    try {
      await program.methods
        .mintPositionHandler(
          tickLowerIndex,
          tickUpperIndex,
          liquidityAmountDesired
        )
        .accountsStrict({
          pool: poolPda,
          position: positionPda,
          tickLower: tickLowerPda,
          tickUpper: tickUpperPda,
          owner: walletSigner.publicKey,
          payer: walletSigner.publicKey,
          systemProgram: SystemProgram.programId,
          rent: web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([feePayerKeypair])
        .rpc();
      expect.fail(
        "Transaction should have failed due to liquidity_amount_desired < MIN_LIQUIDITY."
      );
    } catch (error) {
      expect(error).to.be.instanceOf(anchor.AnchorError);
      const anchorError = error as anchor.AnchorError;
      // Based on mint_position.rs, this should be InvalidInput
      expect(anchorError.error.errorCode.code).to.equal("InvalidInput");
      console.log(
        "Successfully caught error for liquidity_amount_desired < MIN_LIQUIDITY."
      );
    }
  });

  // TODO: Add test cases for when tick_lower or tick_upper are already initialized
  // This requires minting a position, then minting another one that reuses one or both ticks.
  // The expected behavior is that `init_if_needed` doesn't re-initialize,
  // and `liquidity_net` on the TickData accounts should be updated correctly.

  // TODO: Test behavior when `current_tick` is outside, at the boundary, or inside the new position's range
  // This will affect the `pool.liquidity` update.
  // The current "Successfully mints a new position" test has a basic check for this.
  // More specific tests for different current_tick scenarios would be good.
});
