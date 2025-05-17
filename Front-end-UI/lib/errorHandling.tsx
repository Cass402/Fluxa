/**
 * Error handling utilities for Solana transactions
 */
import {
  SendTransactionError,
  TransactionError,
  ComputeBudgetProgram,
  Transaction,
} from "@solana/web3.js";
import { ProgramError } from "@coral-xyz/anchor";
import { toast } from "@/hooks/use-toast";
import { DEFAULT_ERROR_MESSAGE } from "@/lib/config";
import { errorCodeMapping } from "@/lib/schemas";

/**
 * Enum of common Solana error types
 */
export enum SolanaErrorType {
  WALLET_CONNECTION = "WALLET_CONNECTION",
  TRANSACTION_REJECTED = "TRANSACTION_REJECTED",
  INSUFFICIENT_FUNDS = "INSUFFICIENT_FUNDS",
  INSTRUCTION_ERROR = "INSTRUCTION_ERROR",
  PROGRAM_ERROR = "PROGRAM_ERROR",
  RPC_ERROR = "RPC_ERROR",
  TIMEOUT = "TIMEOUT",
  VALIDATION_ERROR = "VALIDATION_ERROR",
  UNKNOWN = "UNKNOWN",
}

/**
 * Interface for a structured Solana error with more context
 */
export interface StructuredSolanaError {
  type: SolanaErrorType;
  message: string;
  code?: number;
  originalError: Error | unknown;
  logs?: string[];
  suggestion?: string;
}

/**
 * Parse a Solana error and return a structured error object
 * @param error - The error to parse
 * @returns A structured error object with more context
 */
export function parseSolanaError(error: unknown): StructuredSolanaError {
  console.error("Raw Solana error:", error);

  // Default error
  const defaultError: StructuredSolanaError = {
    type: SolanaErrorType.UNKNOWN,
    message: DEFAULT_ERROR_MESSAGE,
    originalError: error,
  };

  if (!error) {
    return defaultError;
  }

  // Handle Anchor program errors
  if (error instanceof ProgramError) {
    return {
      type: SolanaErrorType.PROGRAM_ERROR,
      message: error.message || "Program error",
      code: error.code,
      originalError: error,
      logs: error.logs,
      suggestion: "Try again or contact support if the issue persists.",
    };
  }

  // Handle SendTransactionError which might include instruction logs
  if (error instanceof SendTransactionError) {
    // Extract logs if available
    const logs = error.logs || [];

    // Look for specific program error messages in the logs
    const programErrorLog = logs.find(
      (log) =>
        log.includes("Program log: Error:") ||
        log.includes("Program log: Instruction:")
    );

    // Extract error code if present in the form "Error code: 6XXX"
    const errorCodeMatch = logs.join(" ").match(/Error code: (\d+)/i);
    const errorCode = errorCodeMatch
      ? parseInt(errorCodeMatch[1], 10)
      : undefined;

    // Get human-readable message for the error code
    const errorMessage = errorCode ? errorCodeMapping[errorCode] : undefined;

    // Check for common transaction errors
    if (logs.some((log) => log.includes("insufficient funds"))) {
      return {
        type: SolanaErrorType.INSUFFICIENT_FUNDS,
        message: "Insufficient SOL to complete this transaction",
        originalError: error,
        logs,
        suggestion: "Add more SOL to your wallet and try again.",
      };
    }

    // Check for computation limit errors
    if (
      logs.some(
        (log) => log.includes("ComputeBudget") && log.includes("exceeded")
      )
    ) {
      return {
        type: SolanaErrorType.INSTRUCTION_ERROR,
        message: "Transaction exceeded compute budget",
        originalError: error,
        logs,
        suggestion: "Try increasing the compute budget for this transaction.",
      };
    }

    // If we found a program error in the logs
    if (programErrorLog) {
      return {
        type: SolanaErrorType.PROGRAM_ERROR,
        message:
          errorMessage ||
          programErrorLog.split("Program log: ")[1] ||
          "Program execution failed",
        code: errorCode,
        originalError: error,
        logs,
        suggestion: "Check your inputs and try again.",
      };
    }

    return {
      type: SolanaErrorType.TRANSACTION_REJECTED,
      message: "Transaction failed to confirm",
      originalError: error,
      logs,
      suggestion: "Try again with a higher priority fee.",
    };
  }

  // Regular Error object with message
  if (error instanceof Error) {
    const message = error.message.toLowerCase();

    // Check for specific error messages
    if (message.includes("user rejected")) {
      return {
        type: SolanaErrorType.TRANSACTION_REJECTED,
        message: "Transaction rejected by user",
        originalError: error,
        suggestion: "You need to approve the transaction in your wallet.",
      };
    }

    if (message.includes("timeout")) {
      return {
        type: SolanaErrorType.TIMEOUT,
        message: "Transaction timed out",
        originalError: error,
        suggestion: "The network might be congested. Try again later.",
      };
    }

    if (message.includes("wallet adapter")) {
      return {
        type: SolanaErrorType.WALLET_CONNECTION,
        message: "Wallet connection error",
        originalError: error,
        suggestion: "Try reconnecting your wallet or use a different wallet.",
      };
    }

    if (message.includes("insufficient") && message.includes("balance")) {
      return {
        type: SolanaErrorType.INSUFFICIENT_FUNDS,
        message: "Insufficient balance",
        originalError: error,
        suggestion: "Add more funds to your wallet and try again.",
      };
    }

    if (message.includes("rpc")) {
      return {
        type: SolanaErrorType.RPC_ERROR,
        message: "RPC connection error",
        originalError: error,
        suggestion:
          "Check your internet connection or try a different RPC endpoint.",
      };
    }

    // Generic error
    return {
      type: SolanaErrorType.UNKNOWN,
      message: error.message || DEFAULT_ERROR_MESSAGE,
      originalError: error,
    };
  }

  return defaultError;
}

/**
 * Handle a Solana error and show a toast notification
 * @param error - The error to handle
 * @param customMessage - Optional custom message to show
 * @returns A human-readable error message string
 */
export function handleSolanaError(
  error: unknown,
  customMessage?: string
): string {
  const parsedError = parseSolanaError(error);

  // Construct toast message
  const toastMessage = customMessage || parsedError.message;
  const toastDescription = parsedError.suggestion
    ? `${parsedError.message}. ${parsedError.suggestion}`
    : parsedError.message;

  // Show toast to user
  toast({
    title: toastMessage,
    description: toastDescription,
    variant: "destructive",
  });

  return toastDescription;
}

/**
 * Add compute budget to a transaction to avoid computation limit errors
 * @param tx - The transaction to update
 * @param units - Compute units to allocate (optional, default: 400000)
 * @param priorityFee - Priority fee in micro-lamports (optional, default: 1)
 * @returns The updated transaction
 */
export function addComputeBudget(
  tx: Transaction,
  units: number = 400000,
  priorityFee: number = 1
): Transaction {
  tx.add(
    ComputeBudgetProgram.setComputeUnitLimit({ units }),
    ComputeBudgetProgram.setComputeUnitPrice({ microLamports: priorityFee })
  );
  return tx;
}

/**
 * Show a success toast for a transaction
 * @param signature - Transaction signature
 * @param message - Success message to show
 * @param cluster - Solana cluster name for explorer link
 */
export function notifyTransactionSuccess(
  signature: string,
  message: string = "Transaction confirmed",
  cluster: string = "devnet"
) {
  const explorerUrl = `https://explorer.solana.com/tx/${signature}${
    cluster !== "mainnet-beta" ? `?cluster=${cluster}` : ""
  }`;

  toast({
    title: "Success",
    description: (
      <div className="flex flex-col gap-1">
        <p>{message}</p>
        <a
          href={explorerUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="text-primary hover:underline text-sm"
        >
          View on Solana Explorer
        </a>
      </div>
    ),
    variant: "default",
  });
}
