"use client";

import React, { useState, useEffect } from 'react';
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { useToast } from "@/hooks/use-toast";
import { useWallet } from "@/contexts/WalletContext";
import { BN } from "bn.js";
import { usePools, useCreatePosition } from "@/hooks/use-solana-data";
import { priceToSqrtPriceQ64, tickToPrice, priceToTick } from "@/lib/solanaUtils";
import { Loader2 } from "lucide-react";

const AddLiquidity = () => {
  const { toast } = useToast();
  const { connected } = useWallet();
  
  // Fetch available pools
  const { data: pools = [], isLoading: poolsLoading } = usePools();
  
  // States for the form
  const [selectedPool, setSelectedPool] = useState<string>("");
  const [amount0, setAmount0] = useState("");
  const [amount1, setAmount1] = useState("");
  const [feeTier, setFeeTier] = useState("30"); // Default to 0.3%
  const [priceRange, setPriceRange] = useState<[number, number]>([0.8, 1.2]); // Default price range
  const [currentPrice, setCurrentPrice] = useState(1);
  const [isLoading, setIsLoading] = useState(false);
  
  // Use the create position mutation
  const { mutate: createPosition, isLoading: isCreatingPosition } = useCreatePosition();
  
  // Update current price when pool is selected
  useEffect(() => {
    if (selectedPool) {
      const pool = pools.find((p: any) => p.id === selectedPool);
      if (pool) {
        setCurrentPrice(pool.token1Price || 1);
        // Update default price range based on current price
        setPriceRange([pool.token1Price * 0.8, pool.token1Price * 1.2]);
      }
    }
  }, [selectedPool, pools]);
  
  // Handle form submission
  const handleAddLiquidity = async (event: React.FormEvent) => {
    event.preventDefault();
    
    if (!connected) {
      toast({
        title: "Wallet not connected",
        description: "Please connect your wallet to add liquidity",
        variant: "destructive",
      });
      return;
    }
    
    if (!selectedPool || !amount0 || !amount1) {
      toast({
        title: "Missing information",
        description: "Please select a pool and enter amounts",
        variant: "destructive",
      });
      return;
    }
    
    try {
      setIsLoading(true);
      
      // Calculate ticks from price range
      const tickLower = priceToTick(priceRange[0]);
      const tickUpper = priceToTick(priceRange[1]);
      
      // Convert token amounts to the appropriate format (with the correct decimal places)
      const pool = pools.find((p: any) => p.id === selectedPool);
      const decimals0 = pool?.token1.decimals || 9;
      const decimals1 = pool?.token2.decimals || 9;
      
      const amount0BN = new BN(parseFloat(amount0) * Math.pow(10, decimals0));
      const amount1BN = new BN(parseFloat(amount1) * Math.pow(10, decimals1));
      
      // 5% slippage tolerance
      const amount0Min = amount0BN.muln(95).divn(100);
      const amount1Min = amount1BN.muln(95).divn(100);
      
      // Create position using the mutation
      createPosition(
        {
          poolAddress: selectedPool,
          tickLower,
          tickUpper,
          amount0Desired: amount0BN,
          amount1Desired: amount1BN,
          amount0Min,
          amount1Min,
        },
        {
          onSuccess: () => {
            toast({
              title: "Liquidity Added",
              description: "Your liquidity has been successfully added to the pool",
            });
            
            // Reset form after successful submission
            setAmount0("");
            setAmount1("");
          },
          onError: (error) => {
            console.error("Error adding liquidity:", error);
            toast({
              title: "Failed to add liquidity",
              description: error instanceof Error ? error.message : "An unknown error occurred",
              variant: "destructive",
            });
          },
          onSettled: () => {
            setIsLoading(false);
          }
        }
      );
      
    } catch (error) {
      setIsLoading(false);
      toast({
        title: "Error Adding Liquidity",
        description: error instanceof Error ? error.message : "Unknown error occurred",
        variant: "destructive",
      });
      console.error("Error adding liquidity:", error);
    }
  };
  
  // Get token pair display for the selected pool
  const getSelectedPoolLabel = () => {
    if (!selectedPool) return "Select a pool";
    
    const pool = pools.find((p: any) => p.id === selectedPool);
    if (!pool) return "Select a pool";
    
    return `${pool.token1.symbol}/${pool.token2.symbol} - ${pool.feeTier}%`;
  };
  
  return (
    <Card className="w-full max-w-2xl mx-auto">
      <CardHeader>
        <CardTitle>Add Liquidity</CardTitle>
        <CardDescription>
          Provide liquidity to earn fees and yield
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form onSubmit={handleAddLiquidity} className="space-y-6">
          <div className="space-y-2">
            <label htmlFor="pool">Select Pool</label>
            <Select
              value={selectedPool}
              onValueChange={setSelectedPool}
              disabled={isLoading || isCreatingPosition}
            >
              <SelectTrigger id="pool">
                <SelectValue placeholder={poolsLoading ? "Loading pools..." : "Select a pool"} />
              </SelectTrigger>
              <SelectContent>
                {poolsLoading ? (
                  <SelectItem value="loading" disabled>
                    <Loader2 className="mr-2 h-4 w-4 inline animate-spin" />
                    Loading pools...
                  </SelectItem>
                ) : pools.length === 0 ? (
                  <SelectItem value="none" disabled>
                    No pools available
                  </SelectItem>
                ) : (
                  pools.map((pool: any) => (
                    <SelectItem key={pool.id} value={pool.id}>
                      {pool.token1.symbol}/{pool.token2.symbol} - {pool.feeTier}%
                    </SelectItem>
                  ))
                )}
              </SelectContent>
            </Select>
          </div>
          
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <label htmlFor="amount0">Amount {selectedPool ? pools.find((p: any) => p.id === selectedPool)?.token1.symbol : "Token A"}</label>
              <Input
                id="amount0"
                type="number"
                placeholder="0.0"
                value={amount0}
                onChange={(e) => setAmount0(e.target.value)}
                min="0"
                step="0.000001"
                disabled={!selectedPool || isLoading || isCreatingPosition}
              />
            </div>
            <div className="space-y-2">
              <label htmlFor="amount1">Amount {selectedPool ? pools.find((p: any) => p.id === selectedPool)?.token2.symbol : "Token B"}</label>
              <Input
                id="amount1"
                type="number"
                placeholder="0.0"
                value={amount1}
                onChange={(e) => setAmount1(e.target.value)}
                min="0"
                step="0.000001"
                disabled={!selectedPool || isLoading || isCreatingPosition}
              />
            </div>
          </div>
          
          <div className="space-y-4">
            <div className="flex justify-between">
              <span>Price Range</span>
              <span>{priceRange[0].toFixed(6)} - {priceRange[1].toFixed(6)}</span>
            </div>
            <Slider
              value={[priceRange[0], priceRange[1]]}
              min={currentPrice * 0.1}
              max={currentPrice * 5}
              step={currentPrice * 0.01}
              onValueChange={(value) => setPriceRange(value as [number, number])}
              disabled={!selectedPool || isLoading || isCreatingPosition}
            />
            <div className="text-sm text-muted-foreground">
              Your position will earn fees within this price range.
            </div>
          </div>
          
          {selectedPool && (
            <div className="bg-muted p-4 rounded-md space-y-2">
              <div className="flex justify-between">
                <span>Position Summary</span>
              </div>
              <div className="flex justify-between">
                <span>Pool:</span>
                <span>{getSelectedPoolLabel()}</span>
              </div>
              <div className="flex justify-between">
                <span>Current Price:</span>
                <span>{currentPrice.toFixed(6)}</span>
              </div>
              <div className="flex justify-between">
                <span>Price Range:</span>
                <span>{priceRange[0].toFixed(6)} - {priceRange[1].toFixed(6)}</span>
              </div>
              <div className="flex justify-between">
                <span>Estimated APR:</span>
                <span>
                  {selectedPool ? 
                    `${(pools.find((p: any) => p.id === selectedPool)?.apr || 0).toFixed(2)}%` : 
                    "N/A"}
                </span>
              </div>
            </div>
          )}
        </form>
      </CardContent>
      <CardFooter>
        <Button 
          className="w-full" 
          type="submit"
          disabled={!connected || !selectedPool || !amount0 || !amount1 || isLoading || isCreatingPosition}
          onClick={handleAddLiquidity}
        >
          {isLoading || isCreatingPosition ? (
            <>
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              Adding Liquidity...
            </>
          ) : (
            "Add Liquidity"
          )}
        </Button>
      </CardFooter>
    </Card>
  );
};

export default AddLiquidity;