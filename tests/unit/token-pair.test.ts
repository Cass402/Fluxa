import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { AmmCore } from "../target/types/amm_core";
import { PublicKey, Keypair, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, createMint, createAccount } from "@solana/spl-token";
import { assert } from "chai";

describe("Token Pair Account Structure", () => {
  // Configure the client
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.AmmCore as Program<AmmCore>;

  // Generate keypairs for test
  const authority = provider.wallet;
  const poolKeypair = anchor.web3.Keypair.generate();

  // Token mints and decimals
  let tokenAMint: PublicKey;
  let tokenBMint: PublicKey;
  const tokenADecimals = 6; // USDC-like
  const tokenBDecimals = 9; // SOL-like

  // Accounts for token vaults (will be PDAs)
  let tokenAVault: PublicKey;
  let tokenBVault: PublicKey;

  // Token pair PDA
  let tokenPairPDA: PublicKey;
  let tokenPairBump: number;

  // Initial sqrt price (represents price = 20.0)
  // Convert to Q64.64 fixed-point format
  const initialSqrtPrice = new anchor.BN(Math.sqrt(20.0) * 2 ** 64).toString();

  // Fee tier (0.3%)
  const feeTier = 3000;

  before(async () => {
    console.log("Setting up test environment...");

    // Create two token mints
    const payer = provider.wallet.publicKey;
    tokenAMint = await createMint(
      provider.connection,
      provider.wallet.payer,
      payer,
      payer,
      tokenADecimals
    );

    tokenBMint = await createMint(
      provider.connection,
      provider.wallet.payer,
      payer,
      payer,
      tokenBDecimals
    );

    console.log(
      "Created token mints:",
      tokenAMint.toBase58(),
      tokenBMint.toBase58()
    );

    // Find token pair PDA
    [tokenPairPDA, tokenPairBump] = await PublicKey.findProgramAddress(
      [Buffer.from("token_pair"), tokenAMint.toBuffer(), tokenBMint.toBuffer()],
      program.programId
    );

    console.log("Token pair PDA:", tokenPairPDA.toBase58());

    // Derive PDA addresses for token vaults
    [tokenAVault] = await PublicKey.findProgramAddress(
      [
        Buffer.from("token_vault"),
        poolKeypair.publicKey.toBuffer(),
        tokenAMint.toBuffer(),
      ],
      program.programId
    );

    [tokenBVault] = await PublicKey.findProgramAddress(
      [
        Buffer.from("token_vault"),
        poolKeypair.publicKey.toBuffer(),
        tokenBMint.toBuffer(),
      ],
      program.programId
    );

    console.log(
      "Derived token vault PDAs:",
      tokenAVault.toBase58(),
      tokenBVault.toBase58()
    );
  });

  it("Creates a token pair", async () => {
    // Create the token pair
    await program.methods
      .createTokenPair()
      .accounts({
        authority: authority.publicKey,
        tokenPair: tokenPairPDA,
        tokenAMint: tokenAMint,
        tokenBMint: tokenBMint,
        systemProgram: anchor.web3.SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    // Fetch the token pair account to verify it was created properly
    const tokenPair = await program.account.tokenPair.fetch(tokenPairPDA);

    // Verify the token pair data
    assert.ok(tokenPair.authority.equals(authority.publicKey));
    assert.ok(tokenPair.tokenAMint.equals(tokenAMint));
    assert.ok(tokenPair.tokenBMint.equals(tokenBMint));
    assert.equal(tokenPair.tokenADecimals, tokenADecimals);
    assert.equal(tokenPair.tokenBDecimals, tokenBDecimals);
    assert.equal(tokenPair.pools.length, 0);
    assert.equal(tokenPair.isVerified, false);
    assert.equal(tokenPair.version, 1);

    console.log("Token pair created successfully!");
  });

  it("Initializes a liquidity pool and registers it with the token pair", async () => {
    // Initialize the pool with PDAs for token vaults
    await program.methods
      .initializePool(new anchor.BN(initialSqrtPrice), feeTier)
      .accounts({
        payer: authority.publicKey,
        pool: poolKeypair.publicKey,
        tokenPair: tokenPairPDA,
        tokenAMint: tokenAMint,
        tokenBMint: tokenBMint,
        tokenAVault: tokenAVault,
        tokenBVault: tokenBVault,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([poolKeypair])
      .rpc();

    // Fetch the pool account to verify it was created properly
    const pool = await program.account.pool.fetch(poolKeypair.publicKey);

    // Verify the pool data
    assert.ok(pool.authority.equals(authority.publicKey));
    assert.ok(pool.tokenAMint.equals(tokenAMint));
    assert.ok(pool.tokenBMint.equals(tokenBMint));
    assert.ok(pool.tokenAVault.equals(tokenAVault));
    assert.ok(pool.tokenBVault.equals(tokenBVault));
    assert.equal(pool.sqrtPrice.toString(), initialSqrtPrice);
    assert.ok(
      pool.currentTick !== 0,
      "Current tick should be calculated from sqrt price"
    );
    assert.equal(pool.feeTier, feeTier);
    assert.equal(pool.liquidity.toString(), "0");
    assert.equal(pool.positionCount.toString(), "0");

    console.log("Pool initialized with current tick:", pool.currentTick);

    // Fetch the token pair account to verify the pool was registered
    const tokenPair = await program.account.tokenPair.fetch(tokenPairPDA);

    // Check that the pool was added to the token pair
    assert.equal(tokenPair.pools.length, 1);
    assert.ok(tokenPair.pools[0][0].equals(poolKeypair.publicKey));
    assert.equal(tokenPair.pools[0][1], feeTier);

    console.log(
      "Pool initialized and registered with token pair successfully!"
    );
  });

  it("Can find all pools for a token pair", async () => {
    // Fetch the token pair account
    const tokenPair = await program.account.tokenPair.fetch(tokenPairPDA);

    // Verify we can find the pool
    assert.equal(tokenPair.pools.length, 1);

    // For each pool, get its details
    for (const [poolAddress, feeTier] of tokenPair.pools) {
      const poolAccount = await program.account.pool.fetch(poolAddress);

      console.log(`Pool: ${poolAddress.toBase58()}`);
      console.log(`  Fee tier: ${feeTier / 10000}%`);
      console.log(`  Current tick: ${poolAccount.currentTick}`);
      console.log(`  Liquidity: ${poolAccount.liquidity.toString()}`);
      console.log(`  Positions: ${poolAccount.positionCount.toString()}`);
    }
  });
});
