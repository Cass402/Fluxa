/**
 * Data validation schemas using Zod
 *
 * This file contains schemas for validating data received from the Solana blockchain
 * to ensure type safety and catch potential issues early.
 */

import { z } from "zod";
import { PublicKey } from "@solana/web3.js";
import { BN } from "bn.js";

// Helper for PublicKey validation
const publicKeySchema = z.instanceof(PublicKey).or(
  z.string().refine(
    (val) => {
      try {
        new PublicKey(val);
        return true;
      } catch {
        return false;
      }
    },
    {
      message: "Invalid Solana public key",
    }
  )
);

// Helper for BN validation
const bnSchema = z
  .instanceof(BN)
  .or(z.union([z.string(), z.number()]).transform((val) => new BN(val)));

// Token Schema
export const tokenSchema = z.object({
  address: publicKeySchema,
  symbol: z.string(),
  name: z.string(),
  logo: z.string().optional(),
  decimals: z.number().int().min(0).max(18),
  balance: z.number().optional(),
  priceUsd: z.number().optional(),
});

// Pool Schema
export const poolSchema = z.object({
  id: z.string(),
  token0: tokenSchema,
  token1: tokenSchema,
  feeTier: z.number().int().min(1).max(10000),
  tickSpacing: z.number().int(),
  liquidity: z.string().or(bnSchema),
  sqrtPrice: z.string().or(bnSchema),
  currentTick: z.number().int(),
  token0Vault: publicKeySchema,
  token1Vault: publicKeySchema,
  tvl: z.number().optional(),
  volume24h: z.number().optional(),
  apr: z.number().optional(),
});

// Position Schema
export const positionSchema = z.object({
  id: z.string(),
  owner: publicKeySchema,
  pool: z.string(),
  token0: tokenSchema,
  token1: tokenSchema,
  token0Amount: z.number(),
  token1Amount: z.number(),
  liquidity: z.string().or(bnSchema),
  tickLower: z.number().int(),
  tickUpper: z.number().int(),
  minPrice: z.string(),
  maxPrice: z.string(),
  inRange: z.boolean(),
  valueUSD: z.number().optional(),
  apr: z.number().optional(),
  earnedFees: z.number().optional(),
  feeTier: z.number().optional(),
});

// Transaction Schema
export const transactionSchema = z.object({
  hash: z.string(),
  type: z.enum(["swap", "add", "remove"]),
  fromToken: tokenSchema,
  toToken: tokenSchema,
  fromAmount: z.number(),
  toAmount: z.number(),
  valueUSD: z.number().optional(),
  timestamp: z.string().datetime(),
  status: z.enum(["confirmed", "pending", "failed"]),
});

// Pool State Schema (from Solana program)
export const poolStateSchema = z.object({
  bump: z.number().int(),
  factory: publicKeySchema,
  token0Mint: publicKeySchema,
  token1Mint: publicKeySchema,
  token0Vault: publicKeySchema,
  token1Vault: publicKeySchema,
  feeRate: z.number().int(),
  tickSpacing: z.number().int(),
  liquidity: bnSchema,
  sqrtPriceQ64: bnSchema,
  currentTick: z.number().int(),
  tickBitmapData: z.any(),
});

// Position State Schema (from Solana program)
export const positionStateSchema = z.object({
  bump: z.number().int(),
  owner: publicKeySchema,
  pool: publicKeySchema,
  tickLower: z.number().int(),
  tickUpper: z.number().int(),
  liquidity: bnSchema,
  token0Amount: bnSchema,
  token1Amount: bnSchema,
  feeGrowthInside0LastQ64: bnSchema,
  feeGrowthInside1LastQ64: bnSchema,
  tokensOwed0: bnSchema,
  tokensOwed1: bnSchema,
});

// Error Mapping for common Solana program errors
export const errorCodeMapping: Record<number, string> = {
  // Standard Solana error codes
  1: "Insufficient funds",
  2: "Invalid instruction",
  3: "Custom program error",
  4: "Invalid account data",
  5: "Account data too small",

  // Custom Fluxa error codes
  6000: "Mints are not in canonical order",
  6001: "Invalid tick spacing",
  6002: "Invalid initial price",
  6003: "Swap amount exceeds available liquidity",
  6004: "Price limit reached",
  6005: "Position out of range",
  6006: "Insufficient output amount",
  6007: "Insufficient liquidity",
  6008: "Fee calculation error",
  6009: "Tick not initialized",
  6010: "Invalid tick range",
  6011: "Position does not exist",
  6012: "Unauthorized operation",
  // Add more error codes as needed
};

// Safe parsing utilities to handle validation errors gracefully
export function safeParseToken(data: unknown) {
  return tokenSchema.safeParse(data);
}

export function safeParsePool(data: unknown) {
  return poolSchema.safeParse(data);
}

export function safeParsePosition(data: unknown) {
  return positionSchema.safeParse(data);
}

export function safeParseTransaction(data: unknown) {
  return transactionSchema.safeParse(data);
}

export function safeParsePoolState(data: unknown) {
  return poolStateSchema.safeParse(data);
}

export function safeParsePositionState(data: unknown) {
  return positionStateSchema.safeParse(data);
}

/**
 * Get a user-friendly error message for a specific Solana program error code
 * @param code Error code from Solana program
 * @returns User-friendly error message
 */
export function getErrorMessage(code: number): string {
  return errorCodeMapping[code] || `Unknown error (code: ${code})`;
}
