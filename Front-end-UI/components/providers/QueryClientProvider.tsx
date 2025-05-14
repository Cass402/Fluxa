"use client";

import { QueryClient, QueryClientProvider as ReactQueryClientProvider } from 'react-query';
import { ReactNode, useState } from 'react';
import { CACHE_TIME_MS, RETRY_COUNT, STALE_TIME_MS } from '@/lib/config';

interface QueryClientProviderProps {
  children: ReactNode;
}

export function QueryClientProvider({ children }: QueryClientProviderProps) {
  // Create a client for React Query with default options
  const [queryClient] = useState(() => new QueryClient({
    defaultOptions: {
      queries: {
        refetchOnWindowFocus: false,
        refetchOnMount: true,
        refetchOnReconnect: true,
        retry: RETRY_COUNT,
        staleTime: STALE_TIME_MS,
        cacheTime: CACHE_TIME_MS,
        // Structure error responses consistently
        useErrorBoundary: (error: any) => error.statusCode >= 500,
      },
    },
  }));

  return (
    <ReactQueryClientProvider client={queryClient}>
      {children}
    </ReactQueryClientProvider>
  );
}
