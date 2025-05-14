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
 * @param provider - AnchorProvider instance
 * @param programId - Program ID (optional, defaults to config)
 * @returns A Program instance
 */
export const getProgram = <T extends Idl = Idl>(
  idl: T,
  provider: AnchorProvider,
  programId: PublicKey = PROGRAM_ID
): Program<T> => {
  return new Program<T>(idl, programId, provider);
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
  const sqrtPrice = Math.sqrt(price);
  return new BN(sqrtPrice * 2 ** 64);
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
