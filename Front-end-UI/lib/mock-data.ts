import { Token, Pool, Position, Transaction, PoolStats, DashboardStats } from "./types";

// Mock tokens
export const mockTokens: Token[] = [
  {
    address: "0x1234...5678",
    symbol: "ETH",
    name: "Ethereum",
    logo: "https://cryptologos.cc/logos/ethereum-eth-logo.png",
    decimals: 18,
    balance: 1.25,
    priceUsd: 2950,
  },
  {
    address: "0x2345...6789",
    symbol: "BTC",
    name: "Bitcoin",
    logo: "https://cryptologos.cc/logos/bitcoin-btc-logo.png",
    decimals: 8,
    balance: 0.08,
    priceUsd: 62000,
  },
  {
    address: "0x3456...7890",
    symbol: "USDC",
    name: "USD Coin",
    logo: "https://cryptologos.cc/logos/usd-coin-usdc-logo.png",
    decimals: 6,
    balance: 4500,
    priceUsd: 1,
  },
  {
    address: "0x4567...8901",
    symbol: "USDT",
    name: "Tether",
    logo: "https://cryptologos.cc/logos/tether-usdt-logo.png",
    decimals: 6,
    balance: 3200,
    priceUsd: 1,
  },
  {
    address: "0x5678...9012",
    symbol: "DAI",
    name: "Dai Stablecoin",
    logo: "https://cryptologos.cc/logos/multi-collateral-dai-dai-logo.png",
    decimals: 18,
    balance: 2800,
    priceUsd: 1,
  },
  {
    address: "0x6789...0123",
    symbol: "LINK",
    name: "Chainlink",
    logo: "https://cryptologos.cc/logos/chainlink-link-logo.png",
    decimals: 18,
    balance: 75,
    priceUsd: 15.5,
  },
  {
    address: "0x7890...1234",
    symbol: "UNI",
    name: "Uniswap",
    logo: "https://cryptologos.cc/logos/uniswap-uni-logo.png",
    decimals: 18,
    balance: 120,
    priceUsd: 10.2,
  },
  {
    address: "0x8901...2345",
    symbol: "AAVE",
    name: "Aave",
    logo: "https://cryptologos.cc/logos/aave-aave-logo.png",
    decimals: 18,
    balance: 10,
    priceUsd: 105,
  },
];

// Mock pools
export const mockPools: Pool[] = [
  {
    id: "pool-1",
    token1: mockTokens[0], // ETH
    token2: mockTokens[2], // USDC
    tvl: 45000000,
    volume24h: 12500000,
    apr: 24.5,
    feeTier: 0.3,
  },
  {
    id: "pool-2",
    token1: mockTokens[1], // BTC
    token2: mockTokens[2], // USDC
    tvl: 78000000,
    volume24h: 18700000,
    apr: 18.2,
    feeTier: 0.3,
  },
  {
    id: "pool-3",
    token1: mockTokens[0], // ETH
    token2: mockTokens[1], // BTC
    tvl: 32000000,
    volume24h: 8500000,
    apr: 12.8,
    feeTier: 0.05,
  },
  {
    id: "pool-4",
    token1: mockTokens[2], // USDC
    token2: mockTokens[3], // USDT
    tvl: 125000000,
    volume24h: 32000000,
    apr: 5.4,
    feeTier: 0.01,
  },
  {
    id: "pool-5",
    token1: mockTokens[0], // ETH
    token2: mockTokens[6], // UNI
    tvl: 18000000,
    volume24h: 5400000,
    apr: 32.1,
    feeTier: 0.3,
  },
  {
    id: "pool-6",
    token1: mockTokens[0], // ETH
    token2: mockTokens[5], // LINK
    tvl: 12000000,
    volume24h: 3800000,
    apr: 28.5,
    feeTier: 0.3,
  },
];

// Mock positions
export const mockPositions: Position[] = [
  {
    id: "position-1",
    token1: mockTokens[0], // ETH
    token2: mockTokens[2], // USDC
    token1Amount: 0.5,
    token2Amount: 1475,
    valueUSD: 2950,
    apr: 24.5,
    feeTier: 0.3,
    inRange: true,
    minPrice: "2500",
    maxPrice: "3500",
    earnedFees: 120,
  },
  {
    id: "position-2",
    token1: mockTokens[1], // BTC
    token2: mockTokens[2], // USDC
    token1Amount: 0.02,
    token2Amount: 1240,
    valueUSD: 2480,
    apr: 18.2,
    feeTier: 0.3,
    inRange: false,
    minPrice: "58000",
    maxPrice: "68000",
    earnedFees: 85,
  },
  {
    id: "position-3",
    token1: mockTokens[2], // USDC
    token2: mockTokens[3], // USDT
    token1Amount: 1000,
    token2Amount: 1000,
    valueUSD: 2000,
    apr: 5.4,
    feeTier: 0.01,
    inRange: true,
    minPrice: "0.99",
    maxPrice: "1.01",
    earnedFees: 12,
  },
];

// Mock transactions
export const mockTransactions: Transaction[] = [
  {
    hash: "0xabcd...1234",
    type: "swap",
    fromToken: mockTokens[0], // ETH
    toToken: mockTokens[2], // USDC
    fromAmount: 0.1,
    toAmount: 295,
    valueUSD: 295,
    timestamp: new Date(Date.now() - 1000 * 60 * 5).toISOString(), // 5 minutes ago
    status: "confirmed",
  },
  {
    hash: "0xbcde...2345",
    type: "add",
    fromToken: mockTokens[0], // ETH
    toToken: mockTokens[2], // USDC
    fromAmount: 0.2,
    toAmount: 590,
    valueUSD: 1180,
    timestamp: new Date(Date.now() - 1000 * 60 * 60 * 2).toISOString(), // 2 hours ago
    status: "confirmed",
  },
  {
    hash: "0xcdef...3456",
    type: "swap",
    fromToken: mockTokens[2], // USDC
    toToken: mockTokens[6], // UNI
    fromAmount: 100,
    toAmount: 9.8,
    valueUSD: 100,
    timestamp: new Date(Date.now() - 1000 * 60 * 60 * 12).toISOString(), // 12 hours ago
    status: "confirmed",
  },
  {
    hash: "0xdefg...4567",
    type: "remove",
    fromToken: mockTokens[1], // BTC
    toToken: mockTokens[2], // USDC
    fromAmount: 0.01,
    toAmount: 620,
    valueUSD: 1240,
    timestamp: new Date(Date.now() - 1000 * 60 * 60 * 24).toISOString(), // 1 day ago
    status: "confirmed",
  },
  {
    hash: "0xefgh...5678",
    type: "swap",
    fromToken: mockTokens[5], // LINK
    toToken: mockTokens[0], // ETH
    fromAmount: 20,
    toAmount: 0.1,
    valueUSD: 310,
    timestamp: new Date(Date.now() - 1000 * 60 * 60 * 36).toISOString(), // 1.5 days ago
    status: "failed",
  },
];

// Mock pool stats
export const mockPoolStats: PoolStats = {
  tvl: 2850000000,
  volume24h: 436000000,
  fees24h: 1308000,
  poolCount: 3200,
  volumeData: [
    { name: "Mon", volume: 380000000 },
    { name: "Tue", volume: 420000000 },
    { name: "Wed", volume: 390000000 },
    { name: "Thu", volume: 450000000 },
    { name: "Fri", volume: 436000000 },
    { name: "Sat", volume: 410000000 },
    { name: "Sun", volume: 385000000 },
  ],
};

// Mock chart data
export const mockChartData = {
  "1h": Array.from({ length: 60 }, (_, i) => ({
    time: `${i}m`,
    price: 2950 + Math.random() * 20 - 10,
    volume: 100000 + Math.random() * 50000,
  })),
  "1d": Array.from({ length: 24 }, (_, i) => ({
    time: `${i}h`,
    price: 2950 + Math.random() * 50 - 25,
    volume: 500000 + Math.random() * 200000,
  })),
  "1w": Array.from({ length: 7 }, (_, i) => ({
    time: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][i],
    price: 2950 + Math.random() * 100 - 50,
    volume: 2000000 + Math.random() * 1000000,
  })),
  "1m": Array.from({ length: 30 }, (_, i) => ({
    time: `Day ${i + 1}`,
    price: 2950 + Math.sin(i / 5) * 200 + Math.random() * 50 - 25,
    volume: 5000000 + Math.random() * 3000000,
  })),
  "1y": Array.from({ length: 12 }, (_, i) => ({
    time: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][i],
    price: 2950 + Math.sin(i / 2) * 600 + Math.random() * 100 - 50,
    volume: 20000000 + Math.random() * 10000000,
  })),
};

// Mock portfolio data
export const mockPortfolioData = {
  "1d": Array.from({ length: 24 }, (_, i) => ({
    date: `${i}h`,
    value: 8500 + Math.random() * 200 - 100,
  })),
  "1w": Array.from({ length: 7 }, (_, i) => ({
    date: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][i],
    value: 8200 + (i * 100) + (Math.random() * 200 - 100),
  })),
  "1m": Array.from({ length: 30 }, (_, i) => ({
    date: `Day ${i + 1}`,
    value: 7800 + (i * 30) + (Math.random() * 300 - 150),
  })),
  "3m": Array.from({ length: 12 }, (_, i) => ({
    date: `Week ${i + 1}`,
    value: 7000 + (i * 200) + (Math.random() * 400 - 200),
  })),
  "1y": Array.from({ length: 12 }, (_, i) => ({
    date: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][i],
    value: 5000 + (i * 300) + (Math.random() * 500 - 250),
  })),
  "all": Array.from({ length: 10 }, (_, i) => ({
    date: `${2016 + i}`,
    value: 1000 + (i * i * 200) + (Math.random() * 1000 - 500),
  })),
};

// Mock dashboard stats
export const mockDashboardStats: DashboardStats = {
  portfolioValue: 8630,
  totalEarnings: 1245,
  activePositions: 3,
  totalTransactions: 28,
};