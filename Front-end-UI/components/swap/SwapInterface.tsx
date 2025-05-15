"use client";

import { lazy, Suspense } from "react";
import { Loader2 } from "lucide-react";

// Dynamically import the optimized swap interface with code splitting
const OptimizedSwapInterface = lazy(() => import("./OptimizedSwapInterface"));

export default function SwapInterface() {
  return (
    <Suspense fallback={
      <div className="flex flex-col items-center justify-center py-12 space-y-4">
        <Loader2 className="h-10 w-10 animate-spin text-primary" />
        <p>Loading Swap Interface...</p>
      </div>
    }>
      <OptimizedSwapInterface />
    </Suspense>
  );
}