export interface Token {
  address: string;
  symbol: string;
  name: string;
  logo: string;
  decimals: number;
  balance: number;
  priceUsd: number;
}

export interface Pool {
  id: string;
  token1: Token;
  token2: Token;
  tvl: number;
  volume24h: number;
  apr: number;
  feeTier: number;
}

export interface Position {
  id: string;
  token1: Token;
  token2: Token;
  token1Amount: number;
  token2Amount: number;
  valueUSD: number;
  apr: number;
  feeTier: number;
  inRange: boolean;
  minPrice: string;
  maxPrice: string;
  earnedFees: number;
}

export interface Transaction {
  hash: string;
  type: "swap" | "add" | "remove";
  fromToken: Token;
  toToken: Token;
  fromAmount: number;
  toAmount: number;
  valueUSD: number;
  timestamp: string;
  status: "confirmed" | "pending" | "failed";
}

export interface PoolStats {
  tvl: number;
  volume24h: number;
  fees24h: number;
  poolCount: number;
  volumeData: { name: string; volume: number }[];
}

export interface DashboardStats {
  portfolioValue: number;
  totalEarnings: number;
  activePositions: number;
  totalTransactions: number;
}