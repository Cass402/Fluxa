"use client";

import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Loader2 } from "lucide-react";
import { useCreatePool } from "@/hooks/use-solana-data";
import { BN } from "bn.js";
import { priceToSqrtPriceQ64 } from "@/lib/solanaUtils";
import { TICK_SPACINGS, FEE_TIERS } from "@/lib/config";
import { useWallet } from "@/contexts/WalletContext";
import { useToast } from "@/hooks/use-toast";

interface InitializePoolFormProps {
  onSuccess: () => void;
}

// List of supported tokens - in a real app, these would be fetched dynamically
const TOKEN_LIST = [
  {
    address: "So11111111111111111111111111111111111111112",
    symbol: "SOL",
    name: "Solana",
  },
  {
    address: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    symbol: "USDC",
    name: "USD Coin",
  },
  {
    address: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
    symbol: "USDT",
    name: "Tether USD",
  },
  // Add more tokens as needed
];

export default function InitializePoolForm({
  onSuccess,
}: InitializePoolFormProps) {
  const { connected } = useWallet();
  const { toast } = useToast();
  const [token0, setToken0] = useState("");
  const [token1, setToken1] = useState("");
  const [initialPrice, setInitialPrice] = useState("1");
  const [feeTier, setFeeTier] = useState("30"); // Default to 0.3%
  const [debugging, setDebugging] = useState(false);

  const { mutate: createPool, isLoading } = useCreatePool();

  // Sort tokens in canonical order when both are selected
  useEffect(() => {
    if (token0 && token1) {
      // Sort tokens alphabetically by address to ensure canonical order
      const shouldSwap = token0.localeCompare(token1) > 0;

      if (shouldSwap) {
        const temp = token0;
        setToken0(token1);
        setToken1(temp);

        // When swapping tokens, invert the price
        if (initialPrice && parseFloat(initialPrice) !== 0) {
          setInitialPrice((1 / parseFloat(initialPrice)).toString());
        }
      }
    }
  }, [token0, token1, initialPrice]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();

    if (!connected) {
      toast({
        title: "Wallet not connected",
        description: "Please connect your wallet to initialize a pool",
        variant: "destructive",
      });
      return;
    }

    if (!token0 || !token1 || !initialPrice || !feeTier) {
      toast({
        title: "Missing information",
        description: "Please fill in all the required fields",
        variant: "destructive",
      });
      return;
    }

    // Ensure tokens are different
    if (token0 === token1) {
      toast({
        title: "Invalid token selection",
        description: "Please select different tokens for the pool",
        variant: "destructive",
      });
      return;
    }

    try {
      // Convert price to the expected format (sqrtPriceQ64)
      const price = parseFloat(initialPrice);
      if (price <= 0) {
        throw new Error("Price must be positive");
      }

      // Log the parameters
      console.log("Initializing pool with parameters:", {
        token0,
        token1,
        initialPrice: price,
        feeTier,
      });

      // Simplify initial price if it's a complex number
      const simplifiedPrice = parseFloat(price.toFixed(6));
      console.log("Using simplified price:", simplifiedPrice);

      // Calculate the sqrtPriceQ64
      let sqrtPriceQ64;
      try {
        sqrtPriceQ64 = priceToSqrtPriceQ64(simplifiedPrice);
        console.log("Calculated sqrtPriceQ64:", sqrtPriceQ64.toString());
      } catch (sqrtError) {
        console.error("Failed to calculate sqrtPriceQ64:", sqrtError);
        toast({
          title: "Price Conversion Error",
          description:
            "Failed to convert price to internal format. Try using a simpler value like 1.0",
          variant: "destructive",
        });
        return;
      }

      const feeRateBps = parseInt(feeTier); // Fee rate in basis points
      const tickSpacing = TICK_SPACINGS[feeTier as keyof typeof TICK_SPACINGS];

      if (!tickSpacing) {
        throw new Error(`Invalid fee tier: ${feeTier}`);
      }

      console.log("Using tick spacing:", tickSpacing);

      // Log final parameters to be sent
      console.log("Sending to createPool:", {
        mintA: token0,
        mintB: token1,
        sqrtPriceQ64: sqrtPriceQ64.toString(),
        feeRate: feeRateBps,
        tickSpacing,
      });

      // Set debugging flag to true to enable additional debug info
      setDebugging(true);

      createPool(
        {
          mintA: token0,
          mintB: token1,
          initialSqrtPriceQ64: sqrtPriceQ64,
          feeRate: feeRateBps,
          tickSpacing,
        },
        {
          onSuccess: () => {
            toast({
              title: "Pool initialized",
              description: "Your liquidity pool has been successfully created",
            });
            onSuccess();
            setDebugging(false);
          },
          onError: (error) => {
            console.error("Error creating pool:", error);
            let errorMessage = "An unknown error occurred";

            if (error instanceof Error) {
              errorMessage = error.message;

              // Add additional context for common errors
              if (errorMessage.includes("Invalid program id")) {
                errorMessage +=
                  " - Check that the PROGRAM_ID is correctly set in config.ts";
              } else if (errorMessage.includes("insufficient funds")) {
                errorMessage +=
                  " - Make sure your wallet has enough SOL to create the pool";
              } else if (
                errorMessage.includes("Assertion failed") ||
                errorMessage.includes("Error code: 6")
              ) {
                // Handle likely numeric overflow or precision errors
                errorMessage =
                  "Error in price calculation. Try a simpler initial price like 1.0";
                console.error("Possible numeric precision error. Debug info:", {
                  price,
                  sqrtPriceQ64: sqrtPriceQ64?.toString(),
                  feeTier,
                  tickSpacing,
                });
              }
            }

            toast({
              title: "Pool initialization failed",
              description: errorMessage,
              variant: "destructive",
            });
            setDebugging(false);
          },
        }
      );
    } catch (error) {
      console.error("Error preparing pool creation:", error);
      setDebugging(false);

      let errorMessage = "An unknown error occurred";
      if (error instanceof Error) {
        errorMessage = error.message;
        // Add context for token conversion errors
        if (errorMessage.includes("Assertion failed")) {
          errorMessage =
            "Error converting price value. Try using a simpler price like 1.0";
        }
      }

      toast({
        title: "Pool initialization failed",
        description: errorMessage,
        variant: "destructive",
      });
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4 py-4">
      <div className="space-y-2">
        <label htmlFor="token0">Token A</label>
        <Select value={token0} onValueChange={setToken0} disabled={isLoading}>
          <SelectTrigger id="token0">
            <SelectValue placeholder="Select token" />
          </SelectTrigger>
          <SelectContent>
            {TOKEN_LIST.map((token) => (
              <SelectItem
                key={token.address}
                value={token.address}
                disabled={token.address === token1}
              >
                {token.symbol} - {token.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div className="space-y-2">
        <label htmlFor="token1">Token B</label>
        <Select value={token1} onValueChange={setToken1} disabled={isLoading}>
          <SelectTrigger id="token1">
            <SelectValue placeholder="Select token" />
          </SelectTrigger>
          <SelectContent>
            {TOKEN_LIST.map((token) => (
              <SelectItem
                key={token.address}
                value={token.address}
                disabled={token.address === token0}
              >
                {token.symbol} - {token.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div className="space-y-2">
        <label htmlFor="initialPrice">
          Initial Price (Token B per Token A)
        </label>
        <Input
          id="initialPrice"
          type="number"
          placeholder="1.0"
          value={initialPrice}
          onChange={(e) => setInitialPrice(e.target.value)}
          min="0.000001"
          step="0.000001"
          disabled={isLoading}
        />
      </div>

      <div className="space-y-2">
        <label htmlFor="feeTier">Fee Tier</label>
        <Select value={feeTier} onValueChange={setFeeTier} disabled={isLoading}>
          <SelectTrigger id="feeTier">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {FEE_TIERS.map((tier) => (
              <SelectItem key={tier.value} value={tier.value}>
                {tier.label} - {tier.description}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <Button
        type="submit"
        className="w-full"
        disabled={
          isLoading || !connected || !token0 || !token1 || !initialPrice
        }
      >
        {isLoading ? (
          <>
            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            Initializing...
          </>
        ) : (
          "Initialize Pool"
        )}
      </Button>
    </form>
  );
}
