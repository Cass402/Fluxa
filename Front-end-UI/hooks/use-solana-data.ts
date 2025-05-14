// Custom hooks for Solana data fetching with React Query
import { useQuery, useMutation, useQueryClient } from "react-query";
import { BN } from "bn.js";
import { toast } from "@/hooks/use-toast";
import solanaService from "@/services/solanaService";
import { useWallet } from "@/contexts/WalletContext";
import { handleSolanaError } from "@/lib/solanaUtils";

// Query keys for caching and invalidation
export const QUERY_KEYS = {
  POOLS: "pools",
  POOL: "pool",
  POSITIONS: "positions",
  POSITION: "position",
  TOKEN_BALANCES: "tokenBalances",
  TRANSACTION_HISTORY: "transactionHistory",
  CHART_DATA: "chartData",
};

/**
 * Hook to fetch all liquidity pools
 */
export const usePools = () => {
  const { connected } = useWallet();

  return useQuery([QUERY_KEYS.POOLS], async () => solanaService.fetchPools(), {
    enabled: connected && solanaService.isInitialized(),
    onError: (error: any) => {
      const errorMessage = handleSolanaError(error);
      toast({
        title: "Error Fetching Pools",
        description: errorMessage,
        variant: "destructive",
      });
    },
  });
};

/**
 * Hook to fetch a specific pool by token mints
 */
export const usePool = (mintA: string | null, mintB: string | null) => {
  const { connected } = useWallet();

  return useQuery(
    [QUERY_KEYS.POOL, mintA, mintB],
    async () => {
      if (!mintA || !mintB) return null;
      return solanaService.fetchPoolByMints(mintA, mintB);
    },
    {
      enabled: connected && solanaService.isInitialized() && !!mintA && !!mintB,
      onError: (error: any) => {
        const errorMessage = handleSolanaError(error);
        toast({
          title: "Error Fetching Pool",
          description: errorMessage,
          variant: "destructive",
        });
      },
    }
  );
};

/**
 * Hook to fetch user positions
 */
export const usePositions = () => {
  const { connected } = useWallet();

  return useQuery(
    [QUERY_KEYS.POSITIONS],
    async () => solanaService.fetchUserPositions(),
    {
      enabled: connected && solanaService.isInitialized(),
      onError: (error: any) => {
        const errorMessage = handleSolanaError(error);
        toast({
          title: "Error Fetching Positions",
          description: errorMessage,
          variant: "destructive",
        });
      },
    }
  );
};

/**
 * Hook to create a new pool
 */
export const useCreatePool = () => {
  const queryClient = useQueryClient();

  return useMutation(
    async ({
      mintA,
      mintB,
      initialSqrtPriceQ64,
      feeRate,
      tickSpacing,
    }: {
      mintA: string;
      mintB: string;
      initialSqrtPriceQ64: BN;
      feeRate: number;
      tickSpacing: number;
    }) => {
      return solanaService.initializePool(
        mintA,
        mintB,
        initialSqrtPriceQ64,
        feeRate,
        tickSpacing
      );
    },
    {
      onSuccess: () => {
        // Invalidate pools query to refetch with new pool
        queryClient.invalidateQueries([QUERY_KEYS.POOLS]);
      },
      onError: (error: any) => {
        const errorMessage = handleSolanaError(error);
        toast({
          title: "Error Creating Pool",
          description: errorMessage,
          variant: "destructive",
        });
      },
    }
  );
};

/**
 * Hook to create a new position
 */
export const useCreatePosition = () => {
  const queryClient = useQueryClient();

  return useMutation(
    async ({
      poolAddress,
      tickLower,
      tickUpper,
      amount0Desired,
      amount1Desired,
      amount0Min,
      amount1Min,
    }: {
      poolAddress: string;
      tickLower: number;
      tickUpper: number;
      amount0Desired: BN;
      amount1Desired: BN;
      amount0Min: BN;
      amount1Min: BN;
    }) => {
      return solanaService.createPosition(
        poolAddress,
        tickLower,
        tickUpper,
        amount0Desired,
        amount1Desired,
        amount0Min,
        amount1Min
      );
    },
    {
      onSuccess: () => {
        // Invalidate relevant queries
        queryClient.invalidateQueries([QUERY_KEYS.POSITIONS]);
        queryClient.invalidateQueries([QUERY_KEYS.TOKEN_BALANCES]);
      },
      onError: (error: any) => {
        const errorMessage = handleSolanaError(error);
        toast({
          title: "Error Creating Position",
          description: errorMessage,
          variant: "destructive",
        });
      },
    }
  );
};

/**
 * Hook to perform a swap
 */
export const useSwap = () => {
  const queryClient = useQueryClient();

  return useMutation(
    async ({
      poolAddress,
      exactInput,
      amountSpecified,
      sqrtPriceLimitQ64,
    }: {
      poolAddress: string;
      exactInput: boolean;
      amountSpecified: BN;
      sqrtPriceLimitQ64: BN;
    }) => {
      return solanaService.swap(
        poolAddress,
        exactInput,
        amountSpecified,
        sqrtPriceLimitQ64
      );
    },
    {
      onSuccess: () => {
        // Invalidate token balances and transaction history
        queryClient.invalidateQueries([QUERY_KEYS.TOKEN_BALANCES]);
        queryClient.invalidateQueries([QUERY_KEYS.TRANSACTION_HISTORY]);
      },
      onError: (error: any) => {
        const errorMessage = handleSolanaError(error);
        toast({
          title: "Swap Failed",
          description: errorMessage,
          variant: "destructive",
        });
      },
    }
  );
};

/**
 * Hook to collect fees from a position
 */
export const useCollectFees = () => {
  const queryClient = useQueryClient();

  return useMutation(
    async ({ positionAddress }: { positionAddress: string }) => {
      return solanaService.collectFees(positionAddress);
    },
    {
      onSuccess: (_, variables) => {
        // Invalidate specific position and token balances
        queryClient.invalidateQueries([
          QUERY_KEYS.POSITION,
          variables.positionAddress,
        ]);
        queryClient.invalidateQueries([QUERY_KEYS.TOKEN_BALANCES]);
        queryClient.invalidateQueries([QUERY_KEYS.POSITIONS]); // Refresh all positions
      },
      onError: (error: any) => {
        const errorMessage = handleSolanaError(error);
        toast({
          title: "Error Collecting Fees",
          description: errorMessage,
          variant: "destructive",
        });
      },
    }
  );
};
