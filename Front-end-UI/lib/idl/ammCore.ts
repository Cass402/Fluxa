// IDL definition for the Fluxa AMM Core program
import { PublicKey } from "@solana/web3.js";
import { BN } from "bn.js";

export type AmmCoreIdl = {
  version: string;
  name: string;
  instructions: {
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
  }[];
  accounts: {
    name: string;
    type: {
      kind: string;
      fields: {
        name: string;
        type: string;
      }[];
    };
  }[];
  types: {
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
  }[];
  errors: {
    code: number;
    name: string;
    msg: string;
  }[];
};

// Pool state as returned from the program
export interface PoolState {
  bump: number;
  factory: PublicKey;
  token0Mint: PublicKey;
  token1Mint: PublicKey;
  token0Vault: PublicKey;
  token1Vault: PublicKey;
  feeRate: number;
  tickSpacing: number;
  liquidity: BN;
  sqrtPriceQ64: BN;
  currentTick: number;
  tickBitmapData: any;
}

// Position state as returned from the program
export interface PositionState {
  bump: number;
  owner: PublicKey;
  pool: PublicKey;
  tickLower: number;
  tickUpper: number;
  liquidity: BN;
  token0Amount: BN;
  token1Amount: BN;
  feeGrowthInside0LastQ64: BN;
  feeGrowthInside1LastQ64: BN;
  tokensOwed0: BN;
  tokensOwed1: BN;
}

// Error codes from the program
export enum ErrorCode {
  MintsNotInCanonicalOrder = 6000,
  InvalidTickSpacing = 6001,
  InvalidInitialPrice = 6002,
  // Add more error codes as needed
}

// Convert from on-chain Pool data to frontend Pool model
export function poolStateToPool(
  poolState: PoolState,
  address: PublicKey,
  token0Data: any,
  token1Data: any
): any {
  return {
    id: address.toBase58(),
    token0: {
      address: poolState.token0Mint.toBase58(),
      symbol: token0Data.symbol,
      name: token0Data.name,
      decimals: token0Data.decimals,
    },
    token1: {
      address: poolState.token1Mint.toBase58(),
      symbol: token1Data.symbol,
      name: token1Data.name,
      decimals: token1Data.decimals,
    },
    feeTier: poolState.feeRate,
    tickSpacing: poolState.tickSpacing,
    liquidity: poolState.liquidity.toString(),
    sqrtPrice: poolState.sqrtPriceQ64.toString(),
    currentTick: poolState.currentTick,
    token0Vault: poolState.token0Vault.toBase58(),
    token1Vault: poolState.token1Vault.toBase58(),
  };
}

// Convert from on-chain Position data to frontend Position model
export function positionStateToPosition(
  positionState: PositionState,
  address: PublicKey,
  poolData: any,
  token0Price: number,
  token1Price: number
): any {
  const token0Amount =
    positionState.token0Amount.toString() /
    Math.pow(10, poolData.token0.decimals);
  const token1Amount =
    positionState.token1Amount.toString() /
    Math.pow(10, poolData.token1.decimals);

  const valueUSD = token0Amount * token0Price + token1Amount * token1Price;

  // Calculate price range from ticks
  const tickToPrice = (tick: number) => {
    return Math.pow(1.0001, tick);
  };

  const minPrice = tickToPrice(positionState.tickLower);
  const maxPrice = tickToPrice(positionState.tickUpper);
  const currentPrice = tickToPrice(poolData.currentTick);

  // Position is in range if current price is within the position's range
  const inRange = currentPrice >= minPrice && currentPrice <= maxPrice;

  return {
    id: address.toBase58(),
    owner: positionState.owner.toBase58(),
    pool: poolData.id,
    token0: poolData.token0,
    token1: poolData.token1,
    token0Amount,
    token1Amount,
    liquidity: positionState.liquidity.toString(),
    tickLower: positionState.tickLower,
    tickUpper: positionState.tickUpper,
    minPrice: minPrice.toFixed(6),
    maxPrice: maxPrice.toFixed(6),
    inRange,
    valueUSD,
    // Estimated values based on liquidity and fees
    apr: 0, // This would need to be calculated based on historical data
    earnedFees: 0, // This would need to be calculated from tokensOwed
    feeTier: poolData.feeTier,
  };
}
