/**
 * Utility functions for Solana operations
 * Provides core functionality for interacting with the Solana blockchain
 */
import {
  Connection,
  PublicKey,
  Transaction,
  SendTransactionError,
  Keypair,
  SystemProgram,
  ConfirmOptions,
  Signer,
  BlockheightBasedTransactionConfirmationStrategy,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import {
  AnchorProvider,
  Program,
  web3,
  BN,
  ProgramError,
  Idl,
} from "@coral-xyz/anchor";
import {
  PROGRAM_ID,
  SOLANA_NETWORK,
  FALLBACK_ENDPOINTS,
  SOLANA_CLUSTER,
  MAX_TRANSACTION_RETRIES,
  TRANSACTION_TIMEOUT_MS,
  CONNECTION_COMMITMENT,
} from "./config";
import {
  handleSolanaError as handleError,
  notifyTransactionSuccess as notifySuccess,
} from "./errorHandling";
import { AmmCoreIdl } from "./idl/ammCoreTypes";
import { toast } from "@/hooks/use-toast";

// Connection singleton for reuse with failover support
let connectionInstance: Connection | null = null;
let currentEndpointIndex = 0;

/**
 * Get or create a Solana connection with failover support
 * @returns A Solana connection instance
 */
export const getConnection = (): Connection => {
  if (!connectionInstance) {
    connectionInstance = new Connection(SOLANA_NETWORK, {
      commitment: CONNECTION_COMMITMENT,
      confirmTransactionInitialTimeout: TRANSACTION_TIMEOUT_MS,
    });
  }
  return connectionInstance;
};

/**
 * Switch to a different RPC endpoint if the current one fails
 * @returns A new Solana connection instance
 */
export const rotateRpcEndpoint = (): Connection => {
  // Only rotate if we have fallback endpoints
  if (FALLBACK_ENDPOINTS.length === 0) {
    console.warn("No fallback RPC endpoints available");
    return getConnection();
  }

  // Update the index and get the next endpoint
  currentEndpointIndex = (currentEndpointIndex + 1) % FALLBACK_ENDPOINTS.length;
  const newEndpoint = FALLBACK_ENDPOINTS[currentEndpointIndex];

  console.log(`Switching to RPC endpoint: ${newEndpoint}`);

  // Create a new connection with the new endpoint
  connectionInstance = new Connection(newEndpoint, {
    commitment: CONNECTION_COMMITMENT,
    confirmTransactionInitialTimeout: TRANSACTION_TIMEOUT_MS,
  });

  return connectionInstance;
};

/**
 * Create an AnchorProvider from a wallet adapter
 * @param wallet - Wallet adapter instance
 * @returns An AnchorProvider instance
 */
export const createAnchorProvider = (wallet: any): AnchorProvider => {
  const connection = getConnection();
  const provider = new AnchorProvider(connection, wallet, {
    commitment: CONNECTION_COMMITMENT,
    skipPreflight: false,
    preflightCommitment: CONNECTION_COMMITMENT,
  });
  return provider;
};

/**
 * Get a Program instance with the given IDL
 * @param idl - Program IDL
 * @param programId - Program ID (optional, defaults to config)
 * @param provider - AnchorProvider instance
 * @returns A Program instance
 */
export const getProgram = <T extends Idl = Idl>(
  idl: T,
  programId: PublicKey = PROGRAM_ID,
  provider: AnchorProvider
): Program<T> => {
  try {
    console.log("[getProgram] Creating program with ID:", programId.toString());
    console.log("[getProgram] IDL name:", (idl as any).name);
    console.log("[getProgram] IDL version:", (idl as any).version);

    // Check if IDL has account definitions
    if (!idl.accounts || idl.accounts.length === 0) {
      console.warn("[getProgram] Warning: IDL has no account definitions");
    } else {
      console.log(
        "[getProgram] IDL has",
        idl.accounts.length,
        "account definitions"
      );
    }

    // Add safety check for accounts needing the 'size' field
    idl.accounts?.forEach((account: any, idx) => {
      if (account.type?.kind === "struct" && !account.type.size) {
        console.warn(
          `[getProgram] Account ${account.name} (index ${idx}) is missing size property`
        );
      }
    });

    // Create the program instance with additional error wrapping
    // @ts-ignore - Anchor's type definitions are sometimes inconsistent
    return new Program<T>(idl, programId, provider);
  } catch (error) {
    console.error("[getProgram] Error creating program:", error);
    // Re-throw with more context to help debugging
    throw new Error(
      `Failed to initialize Anchor Program: ${
        error instanceof Error ? error.message : String(error)
      }`
    );
  }
};

/**
 * Send a transaction with retry logic and proper error handling
 * @param transaction - The transaction to send
 * @param signers - Signers for the transaction
 * @param connection - Solana connection
 * @param options - Confirm options
 * @returns Transaction signature
 */
export const sendAndConfirmTransaction = async (
  transaction: Transaction | VersionedTransaction,
  signers: Signer[],
  connection: Connection = getConnection(),
  options: ConfirmOptions = { commitment: CONNECTION_COMMITMENT }
): Promise<string> => {
  let signature: string = "";
  let retries = 0;

  while (retries < MAX_TRANSACTION_RETRIES) {
    try {
      if (transaction instanceof VersionedTransaction) {
        signature = await connection.sendTransaction(transaction, options);
      } else {
        signature = await connection.sendTransaction(
          transaction,
          signers,
          options
        );
      }

      // Wait for confirmation
      const { blockhash, lastValidBlockHeight } =
        await connection.getLatestBlockhash();

      const confirmation: BlockheightBasedTransactionConfirmationStrategy = {
        blockhash,
        lastValidBlockHeight,
        signature,
      };

      await connection.confirmTransaction(confirmation);

      // Transaction confirmed successfully
      console.log(`Transaction confirmed: ${signature}`);
      return signature;
    } catch (error) {
      retries++;

      // On the last retry, throw the error
      if (retries >= MAX_TRANSACTION_RETRIES) {
        console.error(
          `Transaction failed after ${MAX_TRANSACTION_RETRIES} retries:`,
          error
        );
        throw error;
      }

      // Exponential backoff for retries
      const delay = 1000 * 2 ** (retries - 1);
      console.warn(
        `Transaction failed, retrying (${retries}/${MAX_TRANSACTION_RETRIES}) in ${delay}ms...`
      );

      // If we get an RPC error, rotate the endpoint
      if (
        error instanceof Error &&
        (error.message.includes("RPC") || error.message.includes("network"))
      ) {
        rotateRpcEndpoint();
      }

      // Wait before retrying
      await new Promise((resolve) => setTimeout(() => resolve(null), delay));
    }
  }

  throw new Error("Transaction failed after max retries");
};

/**
 * Find PDA for pool account
 * @param mintA - First token mint
 * @param mintB - Second token mint
 * @returns [poolPda, poolBump]
 */
export const findPoolPda = async (
  mintA: PublicKey,
  mintB: PublicKey
): Promise<[PublicKey, number]> => {
  // Ensure canonical order by comparing toString values
  if (mintA.toString() > mintB.toString()) {
    [mintA, mintB] = [mintB, mintA];
  }

  return PublicKey.findProgramAddress(
    [Buffer.from("pool"), mintA.toBuffer(), mintB.toBuffer()],
    PROGRAM_ID
  );
};

/**
 * Find PDA for position account
 * @param pool - Pool public key
 * @param owner - Owner public key
 * @param tickLower - Lower tick
 * @param tickUpper - Upper tick
 * @returns [positionPda, positionBump]
 */
export const findPositionPda = async (
  pool: PublicKey,
  owner: PublicKey,
  tickLower: number,
  tickUpper: number
): Promise<[PublicKey, number]> => {
  return PublicKey.findProgramAddress(
    [
      Buffer.from("position"),
      pool.toBuffer(),
      owner.toBuffer(),
      new BN(tickLower).toArrayLike(Buffer, "le", 4),
      new BN(tickUpper).toArrayLike(Buffer, "le", 4),
    ],
    PROGRAM_ID
  );
};

/**
 * Find PDA for tick account
 * @param pool - Pool public key
 * @param tick - Tick index
 * @returns [tickPda, tickBump]
 */
export const findTickPda = async (
  pool: PublicKey,
  tick: number
): Promise<[PublicKey, number]> => {
  return PublicKey.findProgramAddress(
    [
      Buffer.from("tick"),
      pool.toBuffer(),
      new BN(tick).toArrayLike(Buffer, "le", 4),
    ],
    PROGRAM_ID
  );
};

/**
 * Convert a price to sqrt price Q64 representation
 * @param price - Price as a decimal number
 * @returns BN representation of sqrt price Q64
 */
export const priceToSqrtPriceQ64 = (price: number): BN => {
  if (price <= 0) throw new Error("Price must be positive");

  try {
    // Calculate square root of price with high precision
    const sqrtPrice = Math.sqrt(price);

    // Convert to string with high precision to avoid floating point errors
    const sqrtPriceStr = sqrtPrice.toFixed(15);

    // Split into integer and fractional parts
    const [integerPart, fractionalPart = ""] = sqrtPriceStr.split(".");

    // Create BN from integer part (shifted left by 64 bits for Q64 format)
    let result = new BN(integerPart).shln(64);

    // Handle fractional part if it exists
    if (fractionalPart) {
      // Ensure we don't lose precision with very small numbers
      const fractionalStr = fractionalPart.padEnd(20, "0").substring(0, 20);
      const scaleFactor = new BN(10).pow(new BN(fractionalStr.length));

      // Calculate the fractional contribution: (frac / 10^digits) * 2^64
      try {
        const fracValue = new BN(fractionalStr);
        const fracScaled = fracValue
          .mul(new BN(2).pow(new BN(64)))
          .div(scaleFactor);

        // Add the fractional part to the result
        result = result.add(fracScaled);
      } catch (err) {
        console.error("Error calculating fractional part:", err, {
          fractionalStr,
          price,
          sqrtPrice,
        });
        // Fallback to a simpler method that won't cause assertion failures
        const fracValue = Math.floor(
          parseFloat(`0.${fractionalPart}`) * 2 ** 64
        );
        result = result.add(new BN(fracValue));
      }
    }

    console.log("Converted price to sqrt price Q64:", {
      price,
      sqrtPrice,
      sqrtPriceQ64: result.toString(),
    });

    return result;
  } catch (error: unknown) {
    console.error("Error in priceToSqrtPriceQ64:", error, { price });
    const errorMessage = error instanceof Error ? error.message : String(error);
    throw new Error(
      `Failed to convert price ${price} to sqrtPriceQ64: ${errorMessage}`
    );
  }
};

/**
 * Convert sqrt price Q64 to price
 * @param sqrtPriceQ64 - BN representation of sqrt price Q64
 * @returns Price as a decimal number
 */
export const sqrtPriceQ64ToPrice = (sqrtPriceQ64: BN): number => {
  const sqrtPrice = sqrtPriceQ64.toNumber() / 2 ** 64;
  return sqrtPrice * sqrtPrice;
};

/**
 * Convert tick index to price
 * @param tick - Tick index
 * @returns Price as a decimal number
 */
export const tickToPrice = (tick: number): number => {
  return Math.pow(1.0001, tick);
};

/**
 * Convert price to tick index
 * @param price - Price as a decimal number
 * @returns Tick index
 */
export const priceToTick = (price: number): number => {
  return Math.floor(Math.log(price) / Math.log(1.0001));
};

/**
 * Convert lamports to SOL
 * @param lamports - Lamports amount
 * @returns Amount in SOL
 */
export const lamportsToSol = (lamports: number): number => {
  return lamports / web3.LAMPORTS_PER_SOL;
};

/**
 * Convert SOL to lamports
 * @param sol - SOL amount
 * @returns Amount in lamports
 */
export const solToLamports = (sol: number): number => {
  return sol * web3.LAMPORTS_PER_SOL;
};

/**
 * Add compute budget to a transaction to avoid computation limit errors
 * @param tx - The transaction to update
 * @param units - Compute units to allocate (optional, default: 400000)
 * @param priorityFee - Priority fee in micro-lamports (optional, default: 1)
 * @returns The updated transaction
 */
export const addComputeBudget = (
  tx: Transaction,
  units: number = 400000,
  priorityFee: number = 1
): Transaction => {
  // Import locally to avoid import errors
  const { ComputeBudgetProgram } = require("@solana/web3.js");

  tx.add(
    ComputeBudgetProgram.setComputeUnitLimit({ units }),
    ComputeBudgetProgram.setComputeUnitPrice({ microLamports: priorityFee })
  );
  return tx;
};

// Re-export error handling functions
export const handleSolanaError = handleError;
export const notifyTransactionSuccess = notifySuccess;
