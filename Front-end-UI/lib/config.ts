// Configuration for Solana connection and program IDs
import { clusterApiUrl, PublicKey } from "@solana/web3.js";

// Network configuration
export enum SolanaCluster {
  MAINNET = "mainnet-beta",
  TESTNET = "testnet",
  DEVNET = "devnet",
  LOCALNET = "localnet",
}

// Default to devnet for development if not specified
export const SOLANA_CLUSTER =
  (process.env.NEXT_PUBLIC_SOLANA_CLUSTER as SolanaCluster) ||
  SolanaCluster.DEVNET; // Using DEVNET instead of LOCALNET for better compatibility

// Network RPC endpoints with fallbacks
const RPC_ENDPOINTS: Record<SolanaCluster, string[]> = {
  [SolanaCluster.MAINNET]: [
    process.env.NEXT_PUBLIC_MAINNET_RPC || clusterApiUrl(SolanaCluster.MAINNET),
    "https://solana-mainnet.g.alchemy.com/v2/your-api-key", // Replace with your Alchemy API key
    "https://api.mainnet-beta.solana.com",
  ],
  [SolanaCluster.TESTNET]: [
    process.env.NEXT_PUBLIC_TESTNET_RPC || clusterApiUrl(SolanaCluster.TESTNET),
    "https://api.testnet.solana.com",
  ],
  [SolanaCluster.DEVNET]: [
    process.env.NEXT_PUBLIC_DEVNET_RPC || clusterApiUrl(SolanaCluster.DEVNET),
    "https://api.devnet.solana.com",
  ],
  [SolanaCluster.LOCALNET]: [
    process.env.NEXT_PUBLIC_LOCALNET_RPC || "http://127.0.0.1:8899",
  ],
};

// Primary RPC endpoint for the selected network
export const SOLANA_NETWORK = RPC_ENDPOINTS[SOLANA_CLUSTER][0];

// Additional RPC endpoints for fallback
export const FALLBACK_ENDPOINTS = RPC_ENDPOINTS[SOLANA_CLUSTER].slice(1);

// Solana program IDs
export const PROGRAM_ID = new PublicKey(
  process.env.NEXT_PUBLIC_PROGRAM_ID ||
    "AmMCorefQhtQGGKrBj2gNFyzoKKkwAVRrYx9MpyKCFji" // Default program ID
);

export const FACTORY_ADDRESS = new PublicKey(
  process.env.NEXT_PUBLIC_FACTORY_ADDRESS || "11111111111111111111111111111111" // Default to system program ID placeholder
);

// Connection configuration
export const CONNECTION_COMMITMENT = "confirmed";
export const MAX_TRANSACTION_RETRIES = 3;
export const TRANSACTION_TIMEOUT_MS = 30000; // 30 seconds
export const CONNECTION_TIMEOUT_MS = 15000; // 15 seconds

// API caching configuration for React Query
export const CACHE_TIME_MS = 5 * 60 * 1000; // Cache data for 5 minutes
export const STALE_TIME_MS = 30 * 1000; // Consider data stale after 30 seconds
export const RETRY_COUNT = 2; // Number of times to retry failed queries
export const RETRY_DELAY_MS = 1000; // Base delay between retries (will be used for exponential backoff)
export const REFETCH_INTERVAL_MS = 15 * 1000; // Refetch data every 15 seconds
export const BACKGROUND_REFETCH_INTERVAL_MS = 60 * 1000; // Refetch data in background every minute
export const MAX_CACHE_SIZE = 100; // Maximum number of items to keep in cache

// Pool settings
export const FEE_TIERS = [
  {
    value: "1",
    label: "0.01%",
    description: "Best for stable pairs",
    basisPoints: 1,
  },
  {
    value: "5",
    label: "0.05%",
    description: "Best for stable pairs",
    basisPoints: 5,
  },
  {
    value: "30",
    label: "0.3%",
    description: "Best for most pairs",
    basisPoints: 30,
  },
  {
    value: "100",
    label: "1%",
    description: "Best for exotic pairs",
    basisPoints: 100,
  },
];

export const TICK_SPACINGS = {
  "1": 1,
  "5": 10,
  "30": 60,
  "100": 200,
};

// Initial chart timeframes
export const DEFAULT_CHART_TIMEFRAME = "1D";
export const CHART_TIMEFRAMES = ["1H", "1D", "1W", "1M", "1Y"];

// Wallet connection options
export const WALLET_CONNECTION_TIMEOUT_MS = 60 * 1000; // 60 seconds timeout for wallet connections

// Token API endpoints - these would be used to fetch token metadata and prices
export const TOKEN_METADATA_API = "https://token-list-api.solana.com";
export const TOKEN_PRICE_API = "https://price-api.crypto.com/price/v1";
export const TOKEN_PRICE_FALLBACK_API = "https://api.coingecko.com/api/v3";
export const TOKEN_PRICE_CACHE_TIME_MS = 60 * 1000; // Cache token prices for 1 minute

// Performance optimization settings
export const VIRTUALIZATION_THRESHOLD = 100; // Use virtualization for lists longer than this
export const PAGINATION_SIZE = 20; // Number of items per page in paginated lists

// Error messaging
export const DEFAULT_ERROR_MESSAGE =
  "An unexpected error occurred. Please try again.";
export const TRANSACTION_FAILURE_MESSAGE =
  "Transaction failed. Please check your wallet and try again.";

// Analytics and tracking
export const ENABLE_ANALYTICS =
  process.env.NEXT_PUBLIC_ENABLE_ANALYTICS === "true";
export const ANALYTICS_API_KEY = process.env.NEXT_PUBLIC_ANALYTICS_API_KEY;

// Feature flags
export const FEATURES = {
  ADVANCED_TRADING: process.env.NEXT_PUBLIC_FEATURE_ADVANCED_TRADING === "true",
  IL_PROTECTION: process.env.NEXT_PUBLIC_FEATURE_IL_PROTECTION === "true",
  POSITION_MANAGEMENT:
    process.env.NEXT_PUBLIC_FEATURE_POSITION_MANAGEMENT === "true",
  CROSS_CHAIN_SWAPS:
    process.env.NEXT_PUBLIC_FEATURE_CROSS_CHAIN_SWAPS === "true",
};

// Local storage keys
export const STORAGE_KEYS = {
  WALLET_TYPE: "fluxa_wallet_type",
  WALLET_ADDRESS: "fluxa_wallet_address",
  THEME: "fluxa_theme",
  LAST_CONNECTED: "fluxa_last_connected",
  AGREED_TO_TERMS: "fluxa_agreed_to_terms",
  USER_PREFERENCES: "fluxa_user_preferences",
};

// Debug mode
export const DEBUG_MODE = process.env.NODE_ENV !== "production";
export const VERBOSE_LOGGING =
  process.env.NEXT_PUBLIC_VERBOSE_LOGGING === "true";
