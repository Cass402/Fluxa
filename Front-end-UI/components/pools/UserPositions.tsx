"use client";

import { useState } from "react";
import { useWallet } from "@/contexts/WalletContext";
import { usePositions, useCollectFees } from "@/hooks/use-solana-data";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
import { ExternalLink, Plus, RefreshCw } from "lucide-react";
import Link from "next/link";
import TokenPair from "@/components/common/TokenPair";

// Import the Position type from types.ts or define it inline
import { Position } from "@/lib/types";

export default function UserPositions() {
  const { connected } = useWallet();
  const [activeTab, setActiveTab] = useState("active");
  
  // Fetch positions using our custom hook with React Query
  const { 
    data: positions = [],
    isLoading, 
    isError, 
    refetch 
  } = usePositions();
  
  // Setup mutation for collecting fees
  const { mutate: collectFees, isLoading: isCollectingFees } = useCollectFees();
  
  // Filter positions based on active tab
  const filteredPositions = positions.filter((position: Position) => {
    if (activeTab === "active") return position.inRange;
    if (activeTab === "out-of-range") return !position.inRange;
    return true;
  });

  // Handle loading, error and wallet connection states
  if (!connected) {
    return (
      <div className="flex flex-col items-center justify-center py-12">
        <div className="text-center space-y-4">
          <h3 className="text-xl font-medium">Connect Your Wallet</h3>
          <p className="text-muted-foreground">
            Connect your wallet to view and manage your liquidity positions
          </p>
          <Button>Connect Wallet</Button>
        </div>
      </div>
    );
  }
  
  if (isLoading) {
    return (
      <div className="flex flex-col items-center justify-center py-12">
        <div className="text-center space-y-4">
          <RefreshCw className="h-8 w-8 animate-spin text-primary" />
          <h3 className="text-xl font-medium">Loading Positions</h3>
          <p className="text-muted-foreground">
            Fetching your liquidity positions...
          </p>
        </div>
      </div>
    );
  }
  
  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center py-12">
        <div className="text-center space-y-4">
          <h3 className="text-xl font-medium text-red-500">Error Loading Positions</h3>
          <p className="text-muted-foreground">
            We encountered a problem loading your positions
          </p>
          <Button onClick={() => refetch()}>Retry</Button>
        </div>
      </div>
    );
  }

  if (positions.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-12">
        <div className="text-center space-y-4">
          <h3 className="text-xl font-medium">No Liquidity Positions</h3>
          <p className="text-muted-foreground">
            You don't have any active liquidity positions
          </p>
          <Link href="/pools?tab=add">
            <Button>
              <Plus className="mr-2 h-4 w-4" />
              Add Liquidity
            </Button>
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <Tabs value={activeTab} onValueChange={setActiveTab}>
          <TabsList>
            <TabsTrigger value="active">Active</TabsTrigger>
            <TabsTrigger value="out-of-range">Out of Range</TabsTrigger>
            <TabsTrigger value="all">All Positions</TabsTrigger>
          </TabsList>
        </Tabs>
        
        <Link href="/pools?tab=add">
          <Button size="sm">
            <Plus className="mr-2 h-4 w-4" />
            Add Liquidity
          </Button>
        </Link>
      </div>
      
      <div className="grid gap-4">
        {filteredPositions.map((position: Position) => (
          <Card 
            key={position.id}
            className="border overflow-hidden"
          >
            <CardContent className="p-4">
              <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-4">
                <div className="flex items-center gap-3">
                  <TokenPair 
                    token1={position.token1} 
                    token2={position.token2}
                  />
                  <div>
                    <div className="flex items-center gap-1">
                      <span className="font-medium">
                        {position.token1.symbol}/{position.token2.symbol}
                      </span>
                      <span className="text-sm text-muted-foreground">
                        {position.feeTier}%
                      </span>
                    </div>
                    <div className="text-sm flex items-center gap-2">
                      {position.inRange ? (
                        <span className="text-green-500">In Range</span>
                      ) : (
                        <span className="text-red-500">Out of Range</span>
                      )}
                      <span className="text-muted-foreground">
                        Min: {position.minPrice} • Max: {position.maxPrice}
                      </span>
                    </div>
                  </div>
                </div>
                
                <div className="grid grid-cols-3 gap-4">
                  <div>
                    <div className="text-sm text-muted-foreground">Liquidity</div>
                    <div className="font-medium">${position.valueUSD.toLocaleString()}</div>
                  </div>
                  <div>
                    <div className="text-sm text-muted-foreground">APR</div>
                    <div className="font-medium text-green-500">{position.apr}%</div>
                  </div>
                  <div>
                    <div className="text-sm text-muted-foreground">Earned Fees</div>
                    <div className="font-medium">${position.earnedFees.toLocaleString()}</div>
                  </div>
                </div>
                
                <div className="flex gap-2">
                  <Button 
                    variant="outline" 
                    size="sm"
                    onClick={() => {
                      window.open(`https://explorer.solana.com/address/${position.id}`, '_blank');
                    }}
                  >
                    <ExternalLink className="mr-2 h-4 w-4" />
                    Details
                  </Button>
                  <Button 
                    variant="secondary" 
                    size="sm"
                    onClick={() => collectFees({ positionAddress: position.id })}
                    disabled={isCollectingFees || position.earnedFees <= 0}
                  >
                    {isCollectingFees ? (
                      <>
                        <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
                        Collecting...
                      </>
                    ) : (
                      "Collect Fees"
                    )}
                  </Button>
                  <Button variant="default" size="sm">
                    Manage
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>
        ))}
        
        {filteredPositions.length === 0 && (
          <div className="flex flex-col items-center justify-center py-8">
            <div className="text-center space-y-2">
              <h3 className="text-lg font-medium">No positions found</h3>
              <p className="text-sm text-muted-foreground">
                {activeTab === "active" 
                  ? "You don't have any active positions in range" 
                  : activeTab === "out-of-range" 
                  ? "You don't have any positions that are out of range"
                  : "You don't have any positions"}
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}