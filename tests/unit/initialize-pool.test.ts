/// <reference types="mocha" />
import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  getOrCreateAssociatedTokenAccount,
} from "@solana/spl-token";
import { expect } from "chai";
import BN from "bn.js";

// Define the type for the accounts to avoid TypeScript errors
type TokenPairAccount = {
  tokenAMint: PublicKey;
  tokenBMint: PublicKey;
  pools: Array<[PublicKey, number]>;
};

type PoolAccount = {
  tokenAMint: PublicKey;
  tokenBMint: PublicKey;
  tokenAVault: PublicKey;
  tokenBVault: PublicKey;
  feeTier: number;
  liquidity: BN;
  sqrtPriceX64: BN;
};

// Define a generic AnchorProgram type to allow account access
interface AnchorProgram {
  account: {
    tokenPair: {
      fetch: (address: PublicKey) => Promise<any>;
    };
    pool: {
      fetch: (address: PublicKey) => Promise<any>;
    };
  };
  methods: {
    [key: string]: (...args: any[]) => any;
  };
  programId: PublicKey;
}

describe("Fluxa AMM Core - Pool Initialization", () => {
  // Configure the client to use the local cluster
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  // Reference the deployed program
  const program = anchor.workspace.AmmCore as unknown as AnchorProgram;
  const wallet = provider.wallet;

  // Test constants
  const FEE_TIER_LOW = 100; // 0.01%
  const FEE_TIER_MEDIUM = 500; // 0.05%
  const FEE_TIER_HIGH = 3000; // 0.3%
  const Q64 = new BN(1).shln(64);

  // Helper function to convert price to sqrt(price) * 2^64
  function priceToSqrtPriceX64(price: number): BN {
    // Convert to BN and scale by 2^64
    const priceScaled = new BN(Math.floor(price * 10000))
      .mul(Q64)
      .div(new BN(10000));

    // Use Newton's method for square root calculation
    let x = priceScaled.div(new BN(2)); // Initial guess
    let y = priceScaled.clone();

    // Run a few iterations until convergence
    for (let i = 0; i < 10; i++) {
      y = x.clone();
      if (priceScaled.isZero()) break;
      x = priceScaled.div(x).add(x).div(new BN(2));
      if (x.gte(y)) break;
    }

    return y.mul(Q64);
  }

  // Test accounts
  let tokenAMint: PublicKey;
  let tokenBMint: PublicKey;
  let tokenPair: PublicKey;
  let tokenPairBump: number;
  let pool: Keypair;
  let tokenAVault: PublicKey;
  let tokenBVault: PublicKey;

  before(async () => {
    // Create test tokens
    console.log("Creating test tokens...");
    const mintAuthority = wallet.publicKey;
    tokenAMint = await createMint(
      provider.connection,
      (wallet as any).payer,
      mintAuthority,
      null,
      6 // Decimals
    );
    console.log("Token A Mint created:", tokenAMint.toString());

    tokenBMint = await createMint(
      provider.connection,
      (wallet as any).payer,
      mintAuthority,
      null,
      9 // Different decimals
    );
    console.log("Token B Mint created:", tokenBMint.toString());

    // Make sure token A < token B in string comparison for canonical ordering
    if (tokenAMint.toString() > tokenBMint.toString()) {
      const temp = tokenAMint;
      tokenAMint = tokenBMint;
      tokenBMint = temp;
      console.log("Swapped token mints for canonical ordering");
    }

    // Find the token pair PDA
    [tokenPair, tokenPairBump] = await PublicKey.findProgramAddress(
      [Buffer.from("token_pair"), tokenAMint.toBuffer(), tokenBMint.toBuffer()],
      program.programId
    );
    console.log("Token Pair PDA:", tokenPair.toString());

    // Create a pool keypair
    pool = Keypair.generate();
    console.log("Pool keypair created:", pool.publicKey.toString());
  });

  it("Should create a token pair", async () => {
    console.log("Creating token pair...");

    // Create the token pair
    await program.methods
      .createTokenPair()
      .accounts({
        authority: wallet.publicKey,
        tokenPair,
        tokenAMint,
        tokenBMint,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    // Verify the token pair was created and has the correct data
    const tokenPairAccount = (await program.account.tokenPair.fetch(
      tokenPair
    )) as TokenPairAccount;
    expect(tokenPairAccount.tokenAMint.toString()).to.equal(
      tokenAMint.toString()
    );
    expect(tokenPairAccount.tokenBMint.toString()).to.equal(
      tokenBMint.toString()
    );
    expect(tokenPairAccount.pools.length).to.equal(0);

    console.log("Token pair created successfully");
  });

  it("Should initialize a pool with low fee tier", async () => {
    // Create token vaults
    console.log("Creating token vaults...");
    const ataA = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      (wallet as any).payer,
      tokenAMint,
      pool.publicKey,
      true // Allow owner off curve
    );
    tokenAVault = ataA.address;

    const ataB = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      (wallet as any).payer,
      tokenBMint,
      pool.publicKey,
      true // Allow owner off curve
    );
    tokenBVault = ataB.address;

    console.log("Token vaults created");

    // Initialize a pool with 1:1 price and low fee tier
    const initialPrice = 1.0; // 1 token B = 1 token A
    const sqrtPrice = priceToSqrtPriceX64(initialPrice);
    console.log(`Initializing pool with sqrt_price: ${sqrtPrice.toString()}`);

    await program.methods
      .initializePool(new BN(sqrtPrice.toString()), FEE_TIER_LOW)
      .accounts({
        payer: wallet.publicKey,
        tokenPair,
        pool: pool.publicKey,
        tokenAMint,
        tokenBMint,
        tokenAVault,
        tokenBVault,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([pool])
      .rpc();

    // Verify the pool was initialized correctly
    const poolAccount = await program.account.pool.fetch(pool.publicKey);
    expect(poolAccount.tokenAMint.toString()).to.equal(tokenAMint.toString());
    expect(poolAccount.tokenBMint.toString()).to.equal(tokenBMint.toString());
    expect(poolAccount.tokenAVault.toString()).to.equal(tokenAVault.toString());
    expect(poolAccount.tokenBVault.toString()).to.equal(tokenBVault.toString());
    expect(poolAccount.feeTier).to.equal(FEE_TIER_LOW);
    expect(poolAccount.liquidity.toString()).to.equal("0"); // No liquidity yet

    // Verify the token pair was updated with the pool
    const tokenPairAccount = (await program.account.tokenPair.fetch(
      tokenPair
    )) as TokenPairAccount;
    expect(tokenPairAccount.pools.length).to.equal(1);
    const [poolAddress, feeTier] = tokenPairAccount.pools[0];
    expect(poolAddress.toString()).to.equal(pool.publicKey.toString());
    expect(feeTier).to.equal(FEE_TIER_LOW);

    console.log("Pool initialized successfully with low fee tier");
  });

  it("Should initialize a pool with medium fee tier", async () => {
    // Create a new pool keypair for the medium fee tier
    const mediumPool = Keypair.generate();

    // Create token vaults for this pool
    const ataAMedium = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      (wallet as any).payer,
      tokenAMint,
      mediumPool.publicKey,
      true
    );
    const mediumTokenAVault = ataAMedium.address;

    const ataBMedium = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      (wallet as any).payer,
      tokenBMint,
      mediumPool.publicKey,
      true
    );
    const mediumTokenBVault = ataBMedium.address;

    // Initialize a pool with 1.5:1 price and medium fee tier
    const initialPrice = 1.5; // 1 token B = 1.5 token A
    const sqrtPrice = priceToSqrtPriceX64(initialPrice);

    await program.methods
      .initializePool(new BN(sqrtPrice.toString()), FEE_TIER_MEDIUM)
      .accounts({
        payer: wallet.publicKey,
        tokenPair,
        pool: mediumPool.publicKey,
        tokenAMint,
        tokenBMint,
        tokenAVault: mediumTokenAVault,
        tokenBVault: mediumTokenBVault,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([mediumPool])
      .rpc();

    // Verify the pool was initialized correctly
    const poolAccount = await program.account.pool.fetch(mediumPool.publicKey);
    expect(poolAccount.feeTier).to.equal(FEE_TIER_MEDIUM);

    // Verify the token pair was updated with both pools
    const tokenPairAccount = (await program.account.tokenPair.fetch(
      tokenPair
    )) as TokenPairAccount;
    expect(tokenPairAccount.pools.length).to.equal(2);

    // Find the medium fee tier pool in the pools array
    const mediumPool2 = tokenPairAccount.pools.find(
      ([_pool, fee]: [PublicKey, number]) => fee === FEE_TIER_MEDIUM
    );
    expect(mediumPool2).to.exist;
    expect(mediumPool2![0].toString()).to.equal(
      mediumPool.publicKey.toString()
    );

    console.log("Pool initialized successfully with medium fee tier");
  });

  it("Should fail initializing a pool with an invalid fee tier", async () => {
    // Create a new pool keypair for the invalid fee tier
    const invalidPool = Keypair.generate();

    // Create token vaults for this pool
    const ataAInvalid = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      (wallet as any).payer,
      tokenAMint,
      invalidPool.publicKey,
      true
    );
    const invalidTokenAVault = ataAInvalid.address;

    const ataBInvalid = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      (wallet as any).payer,
      tokenBMint,
      invalidPool.publicKey,
      true
    );
    const invalidTokenBVault = ataBInvalid.address;

    // Try to initialize a pool with invalid fee tier
    const initialPrice = 1.0; // 1 token B = 1 token A
    const sqrtPrice = priceToSqrtPriceX64(initialPrice);
    const invalidFeeTier = 1234; // Not a valid fee tier

    try {
      await program.methods
        .initializePool(new BN(sqrtPrice.toString()), invalidFeeTier)
        .accounts({
          payer: wallet.publicKey,
          tokenPair,
          pool: invalidPool.publicKey,
          tokenAMint,
          tokenBMint,
          tokenAVault: invalidTokenAVault,
          tokenBVault: invalidTokenBVault,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([invalidPool])
        .rpc();

      expect.fail(
        "Pool initialization should have failed with invalid fee tier"
      );
    } catch (error: unknown) {
      const errorString = String(error);
      expect(errorString).to.include("InvalidTickSpacing");
      console.log("Pool initialization correctly failed with invalid fee tier");
    }
  });

  it("Should fail initializing a pool with the same token for both sides", async () => {
    try {
      // Find the invalid token pair PDA
      const [invalidTokenPair] = await PublicKey.findProgramAddress(
        [
          Buffer.from("token_pair"),
          tokenAMint.toBuffer(),
          tokenAMint.toBuffer(), // Same token for both sides
        ],
        program.programId
      );

      // Try to create the invalid token pair
      await program.methods
        .createTokenPair()
        .accounts({
          authority: wallet.publicKey,
          tokenPair: invalidTokenPair,
          tokenAMint,
          tokenBMint: tokenAMint, // Same token for both sides
          systemProgram: SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      expect.fail("Token pair creation should have failed with same tokens");
    } catch (error: unknown) {
      const errorString = String(error);
      expect(errorString).to.include("MintsMustDiffer");
      console.log("Token pair creation correctly failed with same tokens");
    }
  });
});
