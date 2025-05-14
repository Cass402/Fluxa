"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { Label } from "@/components/ui/label";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { mockPositions } from "@/lib/mock-data";
import { useWallet } from "@/contexts/WalletContext";
import TokenPair from "@/components/common/TokenPair";

export default function RemoveLiquidity() {
  const { connected } = useWallet();
  const [selectedPosition, setSelectedPosition] = useState(mockPositions[0]);
  const [percentage, setPercentage] = useState(50);
  
  // Calculate values based on percentage
  const token1Amount = selectedPosition.token1Amount * (percentage / 100);
  const token2Amount = selectedPosition.token2Amount * (percentage / 100);
  const valueUSD = selectedPosition.valueUSD * (percentage / 100);

  const handleRemove = () => {
    // Mock implementation for removing liquidity
    alert(`Removed ${percentage}% of ${selectedPosition.token1.symbol}/${selectedPosition.token2.symbol} position`);
    setPercentage(50);
  };

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

  if (mockPositions.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-12">
        <div className="text-center space-y-4">
          <h3 className="text-xl font-medium">No Liquidity Positions</h3>
          <p className="text-muted-foreground">
            You don't have any active liquidity positions
          </p>
          <Button>Add Liquidity</Button>
        </div>
      </div>
    );
  }

  return (
    <div className="grid gap-6 lg:grid-cols-[2fr_1fr]">
      <div className="space-y-6">
        <div className="space-y-4">
          <Label>Select Position</Label>
          <div className="grid gap-4">
            {mockPositions.map((position) => (
              <Card 
                key={position.id}
                className={`border cursor-pointer transition-colors ${
                  selectedPosition.id === position.id 
                    ? "border-primary bg-primary/5" 
                    : ""
                }`}
                onClick={() => setSelectedPosition(position)}
              >
                <CardContent className="p-4">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <TokenPair 
                        token1={position.token1} 
                        token2={position.token2}
                        size="sm" 
                      />
                      <div>
                        <div className="font-medium">
                          {position.token1.symbol}/{position.token2.symbol}
                        </div>
                        <div className="text-sm text-muted-foreground">
                          {position.feeTier}% Fee Tier
                        </div>
                      </div>
                    </div>
                    <div className="text-right">
                      <div className="font-medium">
                        ${position.valueUSD.toLocaleString()}
                      </div>
                      <div className="text-sm text-green-500">
                        {position.apr}% APR
                      </div>
                    </div>
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        </div>
        
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <Label>Amount to Remove ({percentage}%)</Label>
            <div className="flex gap-2">
              {[25, 50, 75, 100].map((value) => (
                <Button
                  key={value}
                  variant={percentage === value ? "default" : "outline"}
                  size="sm"
                  onClick={() => setPercentage(value)}
                >
                  {value}%
                </Button>
              ))}
            </div>
          </div>
          <Slider
            value={[percentage]}
            min={1}
            max={100}
            step={1}
            onValueChange={(values) => setPercentage(values[0])}
          />
        </div>
        
        <Button 
          className="w-full" 
          size="lg"
          onClick={handleRemove}
        >
          Remove Liquidity
        </Button>
      </div>
      
      <div className="space-y-6">
        <Card>
          <CardHeader>
            <CardTitle>Position Summary</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <div className="flex justify-between text-sm">
                <span className="text-muted-foreground">Pool</span>
                <span>{selectedPosition.token1.symbol}/{selectedPosition.token2.symbol}</span>
              </div>
              
              <div className="flex justify-between text-sm">
                <span className="text-muted-foreground">Fee Tier</span>
                <span>{selectedPosition.feeTier}%</span>
              </div>
              
              <div className="flex justify-between text-sm">
                <span className="text-muted-foreground">Current APR</span>
                <span className="text-green-500">{selectedPosition.apr}%</span>
              </div>
              
              <div className="border-t my-2"></div>
              
              <div className="flex justify-between font-medium">
                <span>Total Value</span>
                <span>${selectedPosition.valueUSD.toLocaleString()}</span>
              </div>
            </div>
          </CardContent>
        </Card>
        
        <Card>
          <CardHeader>
            <CardTitle>You'll Receive</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <div className="flex items-center">
                  <img
                    src={selectedPosition.token1.logo}
                    alt={selectedPosition.token1.symbol}
                    className="h-5 w-5 rounded-full mr-2"
                  />
                  <span>{selectedPosition.token1.symbol}</span>
                </div>
                <span>{token1Amount.toFixed(6)}</span>
              </div>
              
              <div className="flex items-center justify-between">
                <div className="flex items-center">
                  <img
                    src={selectedPosition.token2.logo}
                    alt={selectedPosition.token2.symbol}
                    className="h-5 w-5 rounded-full mr-2"
                  />
                  <span>{selectedPosition.token2.symbol}</span>
                </div>
                <span>{token2Amount.toFixed(6)}</span>
              </div>
              
              <div className="border-t my-2"></div>
              
              <div className="flex justify-between font-medium">
                <span>Value</span>
                <span>${valueUSD.toLocaleString()}</span>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}