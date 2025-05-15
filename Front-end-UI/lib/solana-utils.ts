// This file centralizes Solana imports to avoid circular dependencies or import issues
import { Connection, PublicKey } from "@solana/web3.js";

// Types for wallet adapter
export interface SolanaWallet {
  publicKey: PublicKey | null;
  connected: boolean;
  signTransaction: (transaction: any) => Promise<any>;
  signAllTransactions?: (transactions: any[]) => Promise<any[]>;
  signMessage?: (message: Uint8Array) => Promise<Uint8Array>;
  connect: (options?: any) => Promise<any>;
  disconnect: () => Promise<void> | void;
}

// Check if wallet is installed and properly injected
export function checkWalletAvailable(type: "phantom" | "solflare"): boolean {
  if (typeof window === "undefined") return false;

  if (type === "phantom") {
    return !!(window as any).solana?.isPhantom;
  } else if (type === "solflare") {
    return !!(window as any).solflare;
  }

  return false;
}

// Get a properly typed wallet instance
export function getWalletInstance(
  type: "phantom" | "solflare"
): SolanaWallet | null {
  if (typeof window === "undefined") return null;

  let wallet: any;

  if (type === "phantom") {
    wallet = (window as any).solana;
    if (!wallet?.isPhantom) return null;
  } else if (type === "solflare") {
    wallet = (window as any).solflare;
    if (!wallet) return null;
  } else {
    return null;
  }

  return wallet as SolanaWallet;
}

// Helper to serialize wallet for debugging
export function serializeWalletState(wallet: any): Record<string, any> {
  try {
    return {
      publicKey: wallet.publicKey?.toString() || null,
      connected: !!wallet.connected,
      isPhantom: !!wallet.isPhantom,
      hasSigningCapability: typeof wallet.signTransaction === "function",
      availableMethods: Object.keys(wallet).filter(
        (key) => typeof wallet[key] === "function"
      ),
    };
  } catch (error) {
    return { error: "Could not serialize wallet state" };
  }
}
