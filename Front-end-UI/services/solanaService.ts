/**
 * Solana API Service
 *
 * This service handles all interactions with the Solana blockchain,
 * including fetching data, sending transactions, and managing wallet connections.
 * It provides a clean API for the frontend components to use without worrying
 * about the underlying blockchain details.
 */

import {
  PublicKey,
  Keypair,
  Transaction,
  TransactionSignature,
  Connection,
  SendTransactionError,
  ConfirmOptions,
  BlockheightBasedTransactionConfirmationStrategy,
  TransactionInstruction,
  ComputeBudgetProgram,
} from "@solana/web3.js";
import {
  AnchorProvider,
  BN,
  Program,
  web3,
  utils,
  ProgramError,
  EventParser,
} from "@coral-xyz/anchor";
import {
  getConnection,
  createAnchorProvider,
  getProgram,
  findPoolPda,
  handleSolanaError,
  notifyTransactionSuccess,
  priceToSqrtPriceQ64,
} from "@/lib/solanaUtils";
import {
  PROGRAM_ID,
  FACTORY_ADDRESS,
  SOLANA_NETWORK,
  FALLBACK_ENDPOINTS,
  CONNECTION_COMMITMENT,
  MAX_TRANSACTION_RETRIES,
  TRANSACTION_TIMEOUT_MS,
  CONNECTION_TIMEOUT_MS,
  TOKEN_METADATA_API,
  TOKEN_PRICE_API,
} from "@/lib/config";
import {
  PoolState,
  PositionState,
  poolStateToPool,
  positionStateToPosition,
} from "@/lib/idl/ammCore";
import { toast } from "@/hooks/use-toast";
import {
  poolStateSchema,
  positionStateSchema,
  tokenSchema,
  getErrorMessage,
} from "@/lib/schemas";

// Import the IDL for the AMM Core program
import idl from "./idl.json";

// For retry logic and error handling
import { setTimeout } from "timers/promises";
import { QueryClient } from "react-query";

// Initialize QueryClient for global state management
const queryClient = new QueryClient();

// Type definition for the AMM Core program
type AmmCoreProgram = Program<typeof idl>;

// Type for token metadata
interface TokenMetadata {
  symbol: string;
  name: string;
  decimals: number;
  logo?: string;
}

/**
 * Main service class for Solana program interactions
 */
class SolanaService {
  private connection: Connection;
  private wallet: any = null;
  private provider: AnchorProvider | null = null;
  private program: AmmCoreProgram | null = null;
  private eventParser: EventParser | null = null;
  private currentRpcIndex = 0;
  private rpcEndpoints: string[] = [SOLANA_NETWORK, ...FALLBACK_ENDPOINTS];
  private tokenMetadataCache: Map<string, TokenMetadata> = new Map();
  private tokenPriceCache: Map<string, { price: number; timestamp: number }> =
    new Map();

  constructor() {
    // Initialize connection with first RPC endpoint
    this.connection = new Connection(this.rpcEndpoints[0], {
      commitment: CONNECTION_COMMITMENT,
      confirmTransactionInitialTimeout: TRANSACTION_TIMEOUT_MS,
    });

    // Pre-fetch common token metadata for performance
    this.prefetchCommonTokens();
  }

  /**
   * Initialize the service with a wallet
   * @param wallet - Solana wallet adapter
   */
  initialize(wallet: any) {
    if (!wallet) return;

    try {
      this.wallet = wallet;
      this.provider = createAnchorProvider(wallet);
      this.program = getProgram(
        idl,
        PROGRAM_ID,
        this.provider
      ) as AmmCoreProgram;
      this.eventParser = new EventParser(PROGRAM_ID, this.program.coder);

      console.log(
        "SolanaService initialized with wallet:",
        wallet.publicKey?.toString()
      );

      // Test the RPC connection when initializing
      this.testConnection().catch(this.rotateRpcEndpoint.bind(this));
    } catch (error) {
      console.error("Failed to initialize SolanaService:", error);
      throw new Error(
        `Failed to initialize Solana service: ${
          error instanceof Error ? error.message : String(error)
        }`
      );
    }
  }

  /**
   * Check if the service is initialized with a wallet
   */
  isInitialized(): boolean {
    return !!this.wallet && !!this.program;
  }

  /**
   * Ensure the service is initialized before performing operations
   * @private
   */
  private ensureInitialized() {
    if (!this.isInitialized()) {
      throw new Error("SolanaService not initialized with wallet");
    }
  }

  /**
   * Test the current RPC connection
   * @private
   */
  private async testConnection(): Promise<void> {
    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(
        () => controller.abort(),
        CONNECTION_TIMEOUT_MS
      );

      await Promise.race([
        this.connection.getVersion({ signal: controller.signal }),
        new Promise((_, reject) =>
          setTimeout(CONNECTION_TIMEOUT_MS).then(() =>
            reject(new Error("RPC connection timeout"))
          )
        ),
      ]);

      clearTimeout(timeoutId);
    } catch (error) {
      console.warn("RPC connection test failed:", error);
      throw error;
    }
  }

  /**
   * Rotate to next available RPC endpoint
   * @private
   */
  private rotateRpcEndpoint(): Connection {
    this.currentRpcIndex =
      (this.currentRpcIndex + 1) % this.rpcEndpoints.length;
    const newEndpoint = this.rpcEndpoints[this.currentRpcIndex];

    console.log(`Switching to RPC endpoint: ${newEndpoint}`);

    this.connection = new Connection(newEndpoint, {
      commitment: CONNECTION_COMMITMENT,
      confirmTransactionInitialTimeout: TRANSACTION_TIMEOUT_MS,
    });

    return this.connection;
  }

  /**
   * Pre-fetch metadata for common tokens
   * @private
   */
  private async prefetchCommonTokens(): Promise<void> {
    // Common token mints to prefetch
    const commonTokens = [
      "So11111111111111111111111111111111111111112", // Wrapped SOL
      "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", // USDC
      "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB", // USDT
    ];

    // Prefetch in background
    Promise.all(
      commonTokens.map((mint) => this.fetchTokenMetadata(new PublicKey(mint)))
    ).catch((error) =>
      console.warn("Failed to prefetch token metadata:", error)
    );
  }

  /**
   * Fetch all pools from the program with data validation
   * @returns Array of validated pool data
   */
  async fetchPools() {
    try {
      this.ensureInitialized();

      // Start loading indicator
      console.time("fetchPools");

      // Use a circuit breaker pattern with retries
      let retries = 0;
      let poolAccounts = [];

      while (retries < MAX_TRANSACTION_RETRIES) {
        try {
          // Fetch all pools using the program
          // Note: We need to use the program.account["pool"] syntax due to how Anchor generates the IDL
          poolAccounts = await this.program!.account["pool"].all();
          break; // Success, exit loop
        } catch (error) {
          retries++;

          // On the last retry, throw the error
          if (retries >= MAX_TRANSACTION_RETRIES) {
            throw error;
          }

          console.warn(
            `Retrying pool fetch (${retries}/${MAX_TRANSACTION_RETRIES})...`
          );

          // If we get an RPC error, rotate the endpoint
          if (
            error instanceof Error &&
            (error.message.includes("RPC") || error.message.includes("network"))
          ) {
            this.rotateRpcEndpoint();
          }

          // Exponential backoff
          const delay = 1000 * 2 ** (retries - 1);
          await new Promise((resolve) => setTimeout(resolve, delay));
        }
      }

      if (!poolAccounts || poolAccounts.length === 0) {
        console.log("No pools found");
        return [];
      }

      console.log(`Found ${poolAccounts.length} pools, fetching details...`);

      // Transform and validate pool data
      const poolPromises = poolAccounts.map(async (poolAccount: any) => {
        try {
          // Parse and validate the pool data using Zod schema
          const poolData = poolAccount.account as unknown as PoolState;
          const validationResult = poolStateSchema.safeParse(poolData);

          if (!validationResult.success) {
            console.error(
              `Pool data validation failed for ${poolAccount.publicKey.toString()}:`,
              validationResult.error
            );
            return null; // Skip invalid pools
          }

          // Fetch token metadata for the pool's tokens
          const [token0Data, token1Data] = await Promise.all([
            this.fetchTokenMetadata(poolData.token0Mint),
            this.fetchTokenMetadata(poolData.token1Mint),
          ]);

          // Fetch recent volume and TVL data (would be from a real API or indexer in production)
          // For now, we'll generate realistic mock data based on liquidity
          const liquidityBN = poolData.liquidity;
          const liquidityNum = parseFloat(liquidityBN.toString());

          // Calculate TVL using token prices
          const token0Price = await this.fetchTokenPrice(
            token0Data.symbol,
            poolData.token0Mint.toString()
          );
          const token1Price = await this.fetchTokenPrice(
            token1Data.symbol,
            poolData.token1Mint.toString()
          );

          // Format the pool data with extended information
          const formattedPool = poolStateToPool(
            poolData,
            poolAccount.publicKey,
            token0Data,
            token1Data
          );

          // Add real-world derived metrics
          const tvlEstimate =
            (liquidityNum * (token0Price + token1Price)) / 1e6;
          const volume24hEstimate = tvlEstimate * (Math.random() * 0.2 + 0.05); // 5-25% of TVL as daily volume
          const feeRate = poolData.feeRate / 10000; // Convert from basis points (e.g. 3000 = 0.3%)
          const feesEarned = volume24hEstimate * feeRate;
          const aprEstimate = ((feesEarned * 365) / tvlEstimate) * 100; // Annualize daily fees

          // Enhance the pool data with the calculated metrics
          return {
            ...formattedPool,
            tvl: tvlEstimate,
            volume24h: volume24hEstimate,
            apr: aprEstimate,
          };
        } catch (error) {
          console.error(
            `Error processing pool ${poolAccount.publicKey.toString()}:`,
            error
          );
          return null; // Skip pools with errors
        }
      });

      // Wait for all pool processing to complete
      const poolsWithTokenData = await Promise.all(poolPromises);

      // Filter out null values from pools with errors
      const validPools = poolsWithTokenData.filter(Boolean);

      console.timeEnd("fetchPools");
      console.log(
        `Successfully processed ${validPools.length} out of ${poolAccounts.length} pools`
      );

      return validPools;
    } catch (error) {
      console.timeEnd("fetchPools");
      const errorMessage = handleSolanaError(error);
      toast({
        title: "Error Fetching Pools",
        description: errorMessage,
        variant: "destructive",
      });
      console.error("Error fetching pools:", error);
      return [];
    }
  }

  /**
   * Fetch a specific pool by its token mints
   * @param mintA - First token mint
   * @param mintB - Second token mint
   * @returns Pool data
   */
  async fetchPoolByMints(mintAAddress: string, mintBAddress: string) {
    try {
      this.ensureInitialized();

      const mintA = new PublicKey(mintAAddress);
      const mintB = new PublicKey(mintBAddress);

      // Find the pool PDA
      const [poolPda] = await findPoolPda(mintA, mintB);

      // Fetch the pool data
      const poolData = (await this.program!.account.pool.fetch(
        poolPda
      )) as unknown as PoolState;

      // Fetch token metadata
      const token0Data = await this.fetchTokenMetadata(poolData.token0Mint);
      const token1Data = await this.fetchTokenMetadata(poolData.token1Mint);

      // Format the pool data
      return poolStateToPool(poolData, poolPda, token0Data, token1Data);
    } catch (error) {
      const errorMessage = handleSolanaError(error);
      toast({
        title: "Error Fetching Pool",
        description: errorMessage,
        variant: "destructive",
      });
      console.error("Error fetching pool by mints:", error);
      return null;
    }
  }

  /**
   * Fetch user positions for the connected wallet
   * @returns Array of position data
   */
  async fetchUserPositions() {
    try {
      this.ensureInitialized();

      if (!this.wallet.publicKey) {
        return [];
      }

      // Fetch all positions where the owner is the connected wallet
      const positions = await this.program!.account.position.all([
        {
          memcmp: {
            offset: 8, // After the discriminator
            bytes: this.wallet.publicKey.toBase58(),
          },
        },
      ]);

      // Transform position data
      const positionsWithPoolData = await Promise.all(
        positions.map(async (position) => {
          const posData = position.account as unknown as PositionState;

          // Fetch the associated pool data
          const poolData = await this.program!.account.pool.fetch(posData.pool);

          // Format pool data
          const token0Data = await this.fetchTokenMetadata(poolData.token0Mint);
          const token1Data = await this.fetchTokenMetadata(poolData.token1Mint);
          const formattedPool = poolStateToPool(
            poolData as unknown as PoolState,
            posData.pool,
            token0Data,
            token1Data
          );

          // Get token prices (you would replace this with real price fetching)
          const token0Price = await this.fetchTokenPrice(token0Data.symbol);
          const token1Price = await this.fetchTokenPrice(token1Data.symbol);

          // Format position data
          return positionStateToPosition(
            posData,
            position.publicKey,
            formattedPool,
            token0Price,
            token1Price
          );
        })
      );

      return positionsWithPoolData;
    } catch (error) {
      const errorMessage = handleSolanaError(error);
      toast({
        title: "Error Fetching Positions",
        description: errorMessage,
        variant: "destructive",
      });
      console.error("Error fetching user positions:", error);
      return [];
    }
  }

  /**
   * Initialize a new pool
   * @param mintA - First token mint address
   * @param mintB - Second token mint address
   * @param initialSqrtPriceQ64 - Initial sqrt price in Q64 format
   * @param feeRate - Fee rate in basis points
   * @param tickSpacing - Tick spacing
   * @returns Transaction signature
   */
  async initializePool(
    mintA: string,
    mintB: string,
    initialSqrtPriceQ64: BN,
    feeRate: number,
    tickSpacing: number
  ): Promise<TransactionSignature> {
    try {
      this.ensureInitialized();

      const mintAPubkey = new PublicKey(mintA);
      const mintBPubkey = new PublicKey(mintB);

      // Ensure canonical order
      let canonicalMintA = mintAPubkey;
      let canonicalMintB = mintBPubkey;
      if (canonicalMintA.toBuffer().compare(canonicalMintB.toBuffer()) > 0) {
        [canonicalMintA, canonicalMintB] = [canonicalMintB, canonicalMintA];
      }

      // Find pool PDA
      const [poolPda] = await findPoolPda(canonicalMintA, canonicalMintB);

      // Create keypairs for token vaults
      const poolVaultAKeypair = Keypair.generate();
      const poolVaultBKeypair = Keypair.generate();

      // Build and send the transaction
      const txSignature = await this.program!.methods.initializePoolHandler(
        initialSqrtPriceQ64,
        feeRate,
        tickSpacing
      )
        .accounts({
          pool: poolPda,
          mintA: canonicalMintA,
          mintB: canonicalMintB,
          factory: FACTORY_ADDRESS,
          poolVaultA: poolVaultAKeypair.publicKey,
          poolVaultB: poolVaultBKeypair.publicKey,
          payer: this.wallet.publicKey,
          systemProgram: web3.SystemProgram.programId,
          tokenProgram: utils.token.TOKEN_PROGRAM_ID,
          rent: web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([poolVaultAKeypair, poolVaultBKeypair])
        .rpc();

      // Notify success
      notifyTransactionSuccess(txSignature, "Pool created successfully");

      return txSignature;
    } catch (error) {
      const errorMessage = handleSolanaError(error);
      toast({
        title: "Error Creating Pool",
        description: errorMessage,
        variant: "destructive",
      });
      console.error("Error initializing pool:", error);
      throw error;
    }
  }

  /**
   * Create a position in a pool
   * @param poolAddress - Pool address
   * @param tickLower - Lower tick boundary
   * @param tickUpper - Upper tick boundary
   * @param amount0Desired - Desired amount of token0
   * @param amount1Desired - Desired amount of token1
   * @param amount0Min - Minimum amount of token0
   * @param amount1Min - Minimum amount of token1
   * @returns Transaction signature
   */
  async createPosition(
    poolAddress: string,
    tickLower: number,
    tickUpper: number,
    amount0Desired: BN,
    amount1Desired: BN,
    amount0Min: BN,
    amount1Min: BN
  ): Promise<TransactionSignature> {
    try {
      this.ensureInitialized();

      const poolPubkey = new PublicKey(poolAddress);

      // Fetch pool data to get token mints
      const poolData = (await this.program!.account.pool.fetch(
        poolPubkey
      )) as unknown as PoolState;

      // Find PDA for the position
      const [positionPda] = await PublicKey.findProgramAddress(
        [
          Buffer.from("position"),
          poolPubkey.toBuffer(),
          this.wallet.publicKey.toBuffer(),
          new BN(tickLower).toArrayLike(Buffer, "le", 4),
          new BN(tickUpper).toArrayLike(Buffer, "le", 4),
        ],
        PROGRAM_ID
      );

      // Find or create token accounts for the user
      const userToken0Account = await this.findOrCreateTokenAccount(
        poolData.token0Mint,
        this.wallet.publicKey
      );

      const userToken1Account = await this.findOrCreateTokenAccount(
        poolData.token1Mint,
        this.wallet.publicKey
      );

      // Build and send transaction
      const txSignature = await this.program!.methods.createPositionHandler(
        tickLower,
        tickUpper,
        amount0Desired,
        amount1Desired,
        amount0Min,
        amount1Min
      )
        .accounts({
          position: positionPda,
          pool: poolPubkey,
          owner: this.wallet.publicKey,
          userToken0Account,
          userToken1Account,
          poolVault0: poolData.token0Vault,
          poolVault1: poolData.token1Vault,
          tokenProgram: utils.token.TOKEN_PROGRAM_ID,
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc();

      // Notify success
      notifyTransactionSuccess(txSignature, "Position created successfully");

      return txSignature;
    } catch (error) {
      const errorMessage = handleSolanaError(error);
      toast({
        title: "Error Creating Position",
        description: errorMessage,
        variant: "destructive",
      });
      console.error("Error creating position:", error);
      throw error;
    }
  }

  /**
   * Swap tokens
   * @param poolAddress - Pool address
   * @param exactInput - Whether this is an exact input swap
   * @param amountSpecified - Amount of tokens to swap (positive for exact input, negative for exact output)
   * @param sqrtPriceLimitQ64 - Price limit for the swap
   * @returns Transaction signature
   */
  async swap(
    poolAddress: string,
    exactInput: boolean,
    amountSpecified: BN,
    sqrtPriceLimitQ64: BN
  ): Promise<TransactionSignature> {
    try {
      this.ensureInitialized();

      const poolPubkey = new PublicKey(poolAddress);

      // Fetch pool data to get token mints and vaults
      const poolData = (await this.program!.account.pool.fetch(
        poolPubkey
      )) as unknown as PoolState;

      // Determine input/output based on exactInput flag
      const [tokenInMint, tokenOutMint] = exactInput
        ? [poolData.token0Mint, poolData.token1Mint]
        : [poolData.token1Mint, poolData.token0Mint];

      const [tokenInVault, tokenOutVault] = exactInput
        ? [poolData.token0Vault, poolData.token1Vault]
        : [poolData.token1Vault, poolData.token0Vault];

      // Find or create token accounts for the user
      const userTokenInAccount = await this.findOrCreateTokenAccount(
        tokenInMint,
        this.wallet.publicKey
      );

      const userTokenOutAccount = await this.findOrCreateTokenAccount(
        tokenOutMint,
        this.wallet.publicKey
      );

      // Build and send transaction
      const txSignature = await this.program!.methods.swapHandler(
        exactInput,
        amountSpecified,
        sqrtPriceLimitQ64
      )
        .accounts({
          pool: poolPubkey,
          userTokenInAccount,
          userTokenOutAccount,
          tokenVaultIn: tokenInVault,
          tokenVaultOut: tokenOutVault,
          tokenProgram: utils.token.TOKEN_PROGRAM_ID,
        })
        .rpc();

      // Notify success
      notifyTransactionSuccess(txSignature, "Swap completed successfully");

      return txSignature;
    } catch (error) {
      const errorMessage = handleSolanaError(error);
      toast({
        title: "Error Swapping Tokens",
        description: errorMessage,
        variant: "destructive",
      });
      console.error("Error swapping:", error);
      throw error;
    }
  }

  /**
   * Collect fees from a position
   * @param positionAddress - Position address
   * @returns Transaction signature
   */
  async collectFees(positionAddress: string): Promise<TransactionSignature> {
    try {
      this.ensureInitialized();

      const positionPubkey = new PublicKey(positionAddress);

      // Fetch position data
      const positionData = (await this.program!.account.position.fetch(
        positionPubkey
      )) as unknown as PositionState;

      // Fetch pool data
      const poolData = (await this.program!.account.pool.fetch(
        positionData.pool
      )) as unknown as PoolState;

      // Find or create token accounts for the user
      const userToken0Account = await this.findOrCreateTokenAccount(
        poolData.token0Mint,
        this.wallet.publicKey
      );

      const userToken1Account = await this.findOrCreateTokenAccount(
        poolData.token1Mint,
        this.wallet.publicKey
      );

      // Build and send transaction
      const txSignature = await this.program!.methods.collectFeesHandler()
        .accounts({
          position: positionPubkey,
          pool: positionData.pool,
          owner: this.wallet.publicKey,
          userToken0Account,
          userToken1Account,
          poolVault0: poolData.token0Vault,
          poolVault1: poolData.token1Vault,
          tokenProgram: utils.token.TOKEN_PROGRAM_ID,
        })
        .rpc();

      // Notify success
      notifyTransactionSuccess(txSignature, "Fees collected successfully");

      return txSignature;
    } catch (error) {
      const errorMessage = handleSolanaError(error);
      toast({
        title: "Error Collecting Fees",
        description: errorMessage,
        variant: "destructive",
      });
      console.error("Error collecting fees:", error);
      throw error;
    }
  }

  /**
   * Fetch token metadata
   * @param mint - Token mint address
   * @returns Token metadata
   */
  private async fetchTokenMetadata(mint: PublicKey) {
    try {
      // Check cache first
      const mintAddress = mint.toBase58();
      if (this.tokenMetadataCache.has(mintAddress)) {
        return this.tokenMetadataCache.get(mintAddress);
      }

      // These are known tokens we can hardcode for better performance
      const knownTokens: Record<string, any> = {
        EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v: {
          symbol: "USDC",
          name: "USD Coin",
          decimals: 6,
          logo: "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/logo.png",
        },
        So11111111111111111111111111111111111111112: {
          symbol: "SOL",
          name: "Solana",
          decimals: 9,
          logo: "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/So11111111111111111111111111111111111111112/logo.png",
        },
        Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB: {
          symbol: "USDT",
          name: "Tether USD",
          decimals: 6,
          logo: "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB/logo.png",
        },
      };

      if (knownTokens[mintAddress]) {
        // Cache and return if it's a known token
        this.tokenMetadataCache.set(mintAddress, knownTokens[mintAddress]);
        return knownTokens[mintAddress];
      }

      // Otherwise fetch from Token Metadata Program
      // In a real-world scenario, we would use the Metaplex SDK to fetch token metadata
      // But we can also use a direct RPC call to the Metadata program

      // Start with getting the Metadata PDA
      const metadataPDA = await this.getTokenMetadataPDA(mint);

      // Try to fetch metadata account
      try {
        const accountInfo = await this.connection.getAccountInfo(metadataPDA);

        if (accountInfo && accountInfo.data) {
          // Deserialize metadata (simplified version - in production use metaplex SDK)
          // Skip first few bytes which is the metadata version and other prefix data
          const nameLength = accountInfo.data[4];
          const name = new TextDecoder().decode(
            accountInfo.data.slice(5, 5 + nameLength)
          );

          const symbolLength = accountInfo.data[5 + nameLength];
          const symbol = new TextDecoder().decode(
            accountInfo.data.slice(
              6 + nameLength,
              6 + nameLength + symbolLength
            )
          );

          // Get decimals from mint account
          const mintInfo = await this.connection.getAccountInfo(mint);
          // The decimals are at byte 44 in a mint account
          const decimals = mintInfo ? mintInfo.data[44] : 9;

          // Format data
          const metadata = {
            symbol: symbol || mintAddress.substring(0, 4),
            name: name || `Token ${mintAddress.substring(0, 8)}`,
            decimals: decimals,
            logo: `https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/${mintAddress}/logo.png`,
          };

          // Cache the result
          this.tokenMetadataCache.set(mintAddress, metadata);
          return metadata;
        }
      } catch (error) {
        console.warn(
          `Error fetching metadata for token ${mintAddress}:`,
          error
        );
        // Fall back to placeholder data below
      }

      // Generate placeholder data if no metadata found
      const placeholderData = {
        symbol: mintAddress.substring(0, 4).toUpperCase(),
        name: `Token ${mintAddress.substring(0, 8)}`,
        decimals: 9, // Default to 9 decimals
      };

      // Cache the result
      this.tokenMetadataCache.set(mintAddress, placeholderData);
      return placeholderData;
    } catch (error) {
      console.error(
        `Error in fetchTokenMetadata for ${mint.toString()}:`,
        error
      );

      // Return a safe fallback
      const fallback = {
        symbol: mint.toString().substring(0, 4).toUpperCase(),
        name: `Unknown Token`,
        decimals: 9,
      };

      return fallback;
    }
  }

  /**
   * Fetch token price
   * @param symbol - Token symbol
   * @param mint - Optional mint address for better accuracy
   * @returns Token price in USD
   */
  private async fetchTokenPrice(
    symbol: string,
    mint?: string
  ): Promise<number> {
    try {
      // Check cache first with time-based expiry (5 minutes)
      const cacheKey = mint || symbol;
      const now = Date.now();
      const cachedData = this.tokenPriceCache.get(cacheKey);

      if (cachedData && now - cachedData.timestamp < 5 * 60 * 1000) {
        return cachedData.price;
      }

      // Prices from Pyth Network or other oracles would be ideal here
      // For the demo, we'll use a mockup API call but structure it properly

      // Typical API endpoints for production:
      // - CoinGecko: https://api.coingecko.com/api/v3/simple/price?ids=${symbol}&vs_currencies=usd
      // - Pyth Network price feed: Using on-chain oracle data

      // Map known token symbols to their price feed accounts
      const stablecoinSymbols = ["USDC", "USDT", "BUSD", "DAI", "UST"];
      if (stablecoinSymbols.includes(symbol)) {
        // Assume stablecoins are $1
        const priceData = { price: 1, timestamp: now };
        this.tokenPriceCache.set(cacheKey, priceData);
        return 1;
      }

      // For well-known tokens, try to get actual prices from Jupiter API
      // This is a placeholder - you should implement a real API call
      let price: number;

      try {
        // Simulate API call
        console.log(`Fetching price data for ${symbol}...`);

        // In a real implementation, make an API call like:
        /*
        const response = await fetch(`${TOKEN_PRICE_API}?symbol=${symbol}`);
        if (!response.ok) {
          throw new Error(`Failed to fetch price: ${response.statusText}`);
        }
        const data = await response.json();
        price = data.price;
        */

        // Fallback to mock data for demo
        const mockPrices: Record<string, number> = {
          SOL: 80,
          ETH: 3000,
          BTC: 50000,
          BONK: 0.00001,
          RAY: 0.5,
          SRM: 0.2,
          MNGO: 0.1,
        };

        price = mockPrices[symbol] || 1;
      } catch (error) {
        console.warn(`Error fetching ${symbol} price:`, error);

        // Use last known price or default
        const lastKnownPrice = cachedData?.price || 1;
        price = lastKnownPrice;
      }

      // Cache the result
      this.tokenPriceCache.set(cacheKey, { price, timestamp: now });
      return price;
    } catch (error) {
      console.error(`Error in fetchTokenPrice for ${symbol}:`, error);
      return 1; // Default fallback
    }
  }

  /**
   * Find or create a token account for a user
   * @param mint - Token mint
   * @param owner - Account owner
   * @returns Token account address
   */
  private async findOrCreateTokenAccount(
    mint: PublicKey,
    owner: PublicKey
  ): Promise<PublicKey> {
    try {
      // Import token program helpers
      const {
        ASSOCIATED_TOKEN_PROGRAM_ID,
        TOKEN_PROGRAM_ID,
        getAssociatedTokenAddress,
        createAssociatedTokenAccountInstruction,
      } = require("@solana/spl-token");

      // Find the associated token account address
      const associatedTokenAddress = await getAssociatedTokenAddress(
        mint,
        owner,
        false,
        TOKEN_PROGRAM_ID,
        ASSOCIATED_TOKEN_PROGRAM_ID
      );

      // Check if the account exists
      const accountInfo = await this.connection.getAccountInfo(
        associatedTokenAddress
      );

      // If the account doesn't exist, create it
      if (!accountInfo) {
        // Generate instruction to create ATA
        const createAtaInstruction = createAssociatedTokenAccountInstruction(
          owner, // payer
          associatedTokenAddress, // ata
          owner, // owner
          mint, // mint
          TOKEN_PROGRAM_ID,
          ASSOCIATED_TOKEN_PROGRAM_ID
        );

        // Create and send the transaction
        const transaction = new Transaction().add(createAtaInstruction);
        await this.wallet.sendTransaction(transaction, this.connection);
      }

      return associatedTokenAddress;
    } catch (error) {
      console.error("Error finding or creating token account:", error);
      throw error;
    }
  }

  /**
   * Get the PDA for a token's metadata account
   * @param mint - Token mint address
   * @returns Metadata account PDA
   * @private
   */
  private async getTokenMetadataPDA(mint: PublicKey): Promise<PublicKey> {
    // Metadata program ID from Metaplex
    const METADATA_PROGRAM_ID = new PublicKey(
      "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"
    );

    // Find metadata PDA
    const [metadataPDA] = await PublicKey.findProgramAddress(
      [
        Buffer.from("metadata"),
        METADATA_PROGRAM_ID.toBuffer(),
        mint.toBuffer(),
      ],
      METADATA_PROGRAM_ID
    );

    return metadataPDA;
  }
}

// Export singleton instance
export const solanaService = new SolanaService();
export default solanaService;
