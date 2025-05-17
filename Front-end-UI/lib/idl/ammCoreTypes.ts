/**
 * TypeScript type definitions for the AMM Core program IDL
 */
import { PublicKey } from "@solana/web3.js";
import { Idl } from "@coral-xyz/anchor";
import { BN } from "bn.js";
import { Program } from "@coral-xyz/anchor";

/**
 * Interface for AMM Core program IDL
 */
export type AmmCoreIdl = Idl & {
  version: string;
  name: string;
  instructions: AmmCoreInstruction[];
  accounts: AmmCoreAccount[];
  types: AmmCoreType[];
  errors: AmmCoreError[];
};

/**
 * Type definition for program instructions
 */
export interface AmmCoreInstruction {
  name: string;
  accounts: {
    name: string;
    isMut: boolean;
    isSigner: boolean;
  }[];
  args: {
    name: string;
    type: string;
  }[];
}

/**
 * Type definition for program accounts
 */
export interface AmmCoreAccount {
  name: string;
  type: {
    kind: string;
    fields: {
      name: string;
      type: string;
    }[];
  };
}

/**
 * Type definition for program custom types
 */
export interface AmmCoreType {
  name: string;
  type: {
    kind: string;
    fields?: {
      name: string;
      type: string;
    }[];
    variants?: {
      name: string;
      fields?: {
        name?: string;
        type: string;
      }[];
    }[];
  };
}

/**
 * Type definition for program errors
 */
export interface AmmCoreError {
  code: number;
  name: string;
  msg: string;
}

/**
 * Represents a mapping from compressed tick word indices to bitmap values
 * This is a TypeScript representation of the Rust BTreeMap<i16, u64> used in the program
 */
export interface TickBitmap {
  /** Map from compressed tick word indices (16-bit integers) to bitmap values (64-bit integers) */
  [wordIndex: number]: string | number; // u64 values are represented as strings or numbers in JS
}

/**
 * Pool state as returned from the program
 */
export interface PoolState {
  /** Bump seed for PDA */
  bump: number;

  /** The factory that created this pool */
  factory: PublicKey;

  /** The mint address of the first token (token0) */
  token0Mint: PublicKey;

  /** The mint address of the second token (token1) */
  token1Mint: PublicKey;

  /** The vault holding token0 for this pool */
  token0Vault: PublicKey;

  /** The vault holding token1 for this pool */
  token1Vault: PublicKey;

  /** Fee rate in basis points (e.g., 30 for 0.3%) */
  feeRate: number;

  /** The spacing between usable ticks */
  tickSpacing: number;

  /** The total active liquidity within the current tick's price range */
  liquidity: typeof BN;

  /**
   * The current square root of the price in Q64.64 fixed-point format
   * Represents sqrt(price) * 2^64 where price is token1/token0
   */
  sqrtPriceQ64: typeof BN;

  /** The current tick index */
  currentTick: number;

  /**
   * Serialized BTreeMap<i16, u64> mapping compressed tick word indices to bitmap
   * When deserialized, this becomes a map where each bit in the bitmap represents
   * whether a specific tick is initialized
   */
  tickBitmapData: Uint8Array | Buffer;
}

/**
 * Position state as returned from the program
 */
export interface PositionState {
  /** Bump seed for PDA */
  bump: number;

  /** The owner of this position */
  owner: PublicKey;

  /** The pool this position belongs to */
  pool: PublicKey;

  /** The lower tick boundary of this position */
  tickLower: number;

  /** The upper tick boundary of this position */
  tickUpper: number;

  /** The amount of liquidity in this position */
  liquidity: typeof BN;

  /** The amount of token0 in this position */
  token0Amount: typeof BN;

  /** The amount of token1 in this position */
  token1Amount: typeof BN;

  /**
   * Fee growth inside the position's range for token0, stored in Q64.64 fixed-point format
   * Represents accumulated fees per unit of liquidity
   */
  feeGrowthInside0LastQ64: typeof BN;

  /**
   * Fee growth inside the position's range for token1, stored in Q64.64 fixed-point format
   * Represents accumulated fees per unit of liquidity
   */
  feeGrowthInside1LastQ64: typeof BN;

  /** Uncollected token0 fees owed to this position */
  tokensOwed0: typeof BN;

  /** Uncollected token1 fees owed to this position */
  tokensOwed1: typeof BN;
}

/**
 * Typescript type for the AMM Core program
 * Represents the instance of the Fluxa AMM Core program with its IDL interface
 */
export type AmmCoreProgram = Program<AmmCoreIdl>;

/**
 * Utility functions for working with tick bitmap data
 */
export const TickBitmapUtils = {
  /**
   * Deserializes a binary tick bitmap into a JavaScript object
   *
   * @param tickBitmapData - The raw binary data from the pool's tickBitmapData field
   * @returns A TickBitmap object with word indices mapped to bitmap values
   */
  deserialize(tickBitmapData: Uint8Array | Buffer): TickBitmap {
    // This function is a placeholder for actual deserialization logic
    // In a real implementation, this would use a Borsh deserializer
    return {} as TickBitmap;
  },

  /**
   * Checks if a specific tick is initialized based on the bitmap
   *
   * @param tickBitmap - The deserialized tick bitmap
   * @param tick - The tick index to check
   * @param tickSpacing - The tick spacing of the pool
   * @returns True if the tick is initialized, false otherwise
   */
  isTickInitialized(
    tickBitmap: TickBitmap,
    tick: number,
    tickSpacing: number
  ): boolean {
    // Compress tick index by dividing by tick spacing
    const compressedTick = Math.floor(tick / tickSpacing);

    // Calculate word index and bit position
    const wordSize = 64; // Size of each bitmap word in bits
    const wordIndex = Math.floor(compressedTick / wordSize);
    const bitPosition = compressedTick % wordSize;

    // Check if the bit is set in the bitmap
    if (tickBitmap[wordIndex] === undefined) {
      return false;
    }

    const bitmapWord =
      typeof tickBitmap[wordIndex] === "string"
        ? BigInt(tickBitmap[wordIndex] as string)
        : BigInt(tickBitmap[wordIndex] as number);

    return (bitmapWord & (BigInt(1) << BigInt(bitPosition))) !== BigInt(0);
  },

  /**
   * Finds the next initialized tick in the bitmap
   *
   * @param tickBitmap - The deserialized tick bitmap
   * @param tick - The starting tick index
   * @param tickSpacing - The tick spacing of the pool
   * @param lte - If true, search for ticks less than or equal to the starting tick;
   *              if false, search for ticks greater than or equal
   * @returns The next initialized tick index or null if none found
   */
  nextInitializedTick(
    tickBitmap: TickBitmap,
    tick: number,
    tickSpacing: number,
    lte: boolean
  ): number | null {
    // This function is a placeholder for the actual implementation
    // In a real implementation, this would mimic the logic in the Solana program
    // by traversing the bitmap to find the next initialized tick
    return null;
  },
};
