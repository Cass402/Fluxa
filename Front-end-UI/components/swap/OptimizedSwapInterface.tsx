"use client";

import { useState, useCallback, useMemo, memo, useEffect } from "react";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import {
  ArrowDownUp,
  Settings,
  ChevronDown,
  Info,
  RefreshCw,
  Loader2,
  AlertCircle,
} from "lucide-react";
import { useWallet } from "@/contexts/WalletContext";
import { Token, Pool } from "@/lib/types";
import TokenSelector from "@/components/swap/TokenSelector";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { useToast } from "@/hooks/use-toast";
import { useSwap } from "@/hooks/use-solana-data";
import { BN } from "bn.js";
import { usePools } from "@/hooks/use-solana-data";
import { Badge } from "@/components/ui/badge";

// Define default SOL token
const SOL_TOKEN: Token = {
  address: "So11111111111111111111111111111111111111112",
  symbol: "SOL",
  name: "Solana",
  decimals: 9,
  logo: "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/So11111111111111111111111111111111111111112/logo.png",
  balance: 0, // This will be updated when account is connected
  priceUsd: 0, // Will be updated from API
};

// Define memoized sub-components for better performance
const MemoizedTokenSelector = memo(TokenSelector);

const TokenInput = memo(({ 
  label, 
  token, 
  amount, 
  onChange, 
  showMaxButton = false, 
  onSelectToken,
  otherToken,
  isLoading,
}: {
  label: string;
  token: Token | null;
  amount: string;
  onChange: (value: string) => void;
  showMaxButton?: boolean;
  onSelectToken: (token: Token) => void;
  otherToken: Token | null;
  isLoading?: boolean;
}) => {
  return (
    <div className="p-4 rounded-lg border bg-card/50 backdrop-blur-sm shadow-sm">
      <div className="flex items-center justify-between mb-2">
        <Label className="text-sm font-medium">{label}</Label>
        {token && (
          <span className="text-xs text-muted-foreground">
            Balance: {token.balance.toFixed(6)} {token.symbol}
          </span>
        )}
      </div>
      <div className="flex items-center space-x-3">
        <div className="relative flex-1">
          <Input
            type="number"
            placeholder="0.0"
            value={amount}
            onChange={(e) => onChange(e.target.value)}
            className={cn(
              "text-lg font-medium h-12 px-3",
              showMaxButton ? "pr-20" : "",
              isLoading ? "text-muted-foreground" : ""
            )}
            disabled={isLoading}
          />
          {showMaxButton && token && (
            <Button
              variant="ghost"
              size="sm"
              className="absolute right-1 top-1/2 -translate-y-1/2 h-8 text-xs font-medium"
              onClick={() => onChange(token.balance.toString())}
              disabled={isLoading}
            >
              MAX
            </Button>
          )}
        </div>
        <MemoizedTokenSelector
          selectedToken={token}
          onSelectToken={onSelectToken}
          otherToken={otherToken}
          disabled={isLoading}
        />
      </div>
    </div>
  );
});
TokenInput.displayName = "TokenInput";

const SwapDetails = memo(({ 
  fromToken, 
  toToken, 
  fromAmount, 
  toAmount,
  slippage, 
  priceImpact,
  isLoading,
}: {
  fromToken: Token | null;
  toToken: Token | null;
  fromAmount: string;
  toAmount: string;
  slippage: number;
  priceImpact: string;
  isLoading?: boolean;
}) => {
  // Don't render if we don't have complete data
  if (!fromToken || !toToken || !fromAmount || !toAmount) return null;
  
  // Calculate minimum received based on slippage
  const minReceived = parseFloat(toAmount) * (1 - slippage / 100);
  
  return (
    <div className="space-y-2 rounded-lg bg-muted/50 p-4 text-sm backdrop-blur-sm">
      <div className="flex items-center justify-between">
        <div className="flex items-center text-muted-foreground">
          Rate
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Info className="ml-1 h-3 w-3" />
              </TooltipTrigger>
              <TooltipContent className="max-w-xs">
                <p>The current exchange rate between selected tokens</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
        <div className="flex items-center">
          {isLoading ? (
            <span className="text-muted-foreground">Calculating...</span>
          ) : (
            <>
              1 {fromToken.symbol} = {(parseFloat(toAmount) / parseFloat(fromAmount)).toFixed(6)} {toToken.symbol}
              <RefreshCw className="ml-1 h-3 w-3 cursor-pointer" />
            </>
          )}
        </div>
      </div>
      
      <div className="flex items-center justify-between">
        <div className="flex items-center text-muted-foreground">
          Price Impact
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Info className="ml-1 h-3 w-3" />
              </TooltipTrigger>
              <TooltipContent className="max-w-xs">
                <p>The difference between the market price and estimated price due to trade size</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
        <div className={cn(
          isLoading ? "text-muted-foreground" : "",
          parseFloat(priceImpact.replace('%', '')) < 0.5 ? "text-green-500" : 
          parseFloat(priceImpact.replace('%', '')) < 2.0 ? "text-yellow-500" : "text-red-500"
        )}>
          {isLoading ? "Calculating..." : priceImpact}
        </div>
      </div>
      
      <div className="flex items-center justify-between">
        <div className="flex items-center text-muted-foreground">
          Minimum Received
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Info className="ml-1 h-3 w-3" />
              </TooltipTrigger>
              <TooltipContent className="max-w-xs">
                <p>The minimum amount you are guaranteed to receive. If the price slips more than your slippage tolerance, your transaction will revert.</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
        <div>
          {isLoading ? (
            <span className="text-muted-foreground">Calculating...</span>
          ) : (
            <>{minReceived.toFixed(6)} {toToken.symbol}</>
          )}
        </div>
      </div>
      
      <div className="flex items-center justify-between">
        <div className="flex items-center text-muted-foreground">
          Route
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Info className="ml-1 h-3 w-3" />
              </TooltipTrigger>
              <TooltipContent className="max-w-xs">
                <p>The path your swap takes through liquidity pools</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
        <div className="flex items-center">
          <Badge variant="secondary" className="text-xs mr-1">{fromToken.symbol}</Badge>
          <ChevronDown className="h-3 w-3 rotate-90" />
          <Badge variant="secondary" className="text-xs">{toToken.symbol}</Badge>
        </div>
      </div>
    </div>
  );
});
SwapDetails.displayName = "SwapDetails";

function SwapInterface() {
  const { connected, publicKey } = useWallet();
  const { toast } = useToast();
  
  // Fetch available tokens
  const { data: pools = [], isLoading: isLoadingPools } = usePools();
  
  // Extract available tokens from pools
  const availableTokens = useMemo(() => {
    const tokenMap = new Map<string, Token>();
    
    // Always ensure SOL is available
    tokenMap.set(SOL_TOKEN.address, { ...SOL_TOKEN });
    
    pools.forEach((pool: Pool) => {
      if (!tokenMap.has(pool.token1.address)) {
        tokenMap.set(pool.token1.address, pool.token1);
      }
      if (!tokenMap.has(pool.token2.address)) {
        tokenMap.set(pool.token2.address, pool.token2);
      }
    });
    
    return Array.from(tokenMap.values());
  }, [pools]);
  
  // Default tokens - use SOL and the first USDC-like token if available
  const defaultTokens = useMemo(() => {
    const solToken = availableTokens.find(t => t.symbol === "SOL");
    let stableToken = availableTokens.find(t => t.symbol === "USDC" || t.symbol === "USDT");
    
    if (!stableToken && availableTokens.length > 1) {
      stableToken = availableTokens.find(t => t.symbol !== "SOL");
    }
    
    if (!stableToken && availableTokens.length > 0) {
      stableToken = availableTokens[0];
    }
    
    return [solToken, stableToken].filter(Boolean) as Token[];
  }, [availableTokens]);
  
  // State management
  const [fromToken, setFromToken] = useState<Token | null>(null);
  const [toToken, setToToken] = useState<Token | null>(null);
  const [fromAmount, setFromAmount] = useState<string>("");
  const [toAmount, setToAmount] = useState<string>("");
  const [slippage, setSlippage] = useState<number>(0.5);
  const [showSettings, setShowSettings] = useState(false);
  const [isCalculating, setIsCalculating] = useState(false);
  const [quoteTimer, setQuoteTimer] = useState<NodeJS.Timeout | null>(null);
  
  // Set default tokens once they're loaded
  useEffect(() => {
    if (defaultTokens.length >= 2 && !fromToken && !toToken) {
      setFromToken(defaultTokens[0]);
      setToToken(defaultTokens[1]);
    }
  }, [defaultTokens, fromToken, toToken]);
  
  // Get the swap mutation
  const { mutate: performSwap, isLoading } = useSwap();

  // Get pool for selected token pair
  const selectedPool = useMemo(() => {
    if (!fromToken || !toToken) return null;
    
    return pools.find((pool: Pool) => 
      (pool.token1.address === fromToken.address && pool.token2.address === toToken.address) ||
      (pool.token2.address === fromToken.address && pool.token1.address === toToken.address)
    );
  }, [pools, fromToken, toToken]);

  // Calculate price quote with debounce
  const calculateQuote = useCallback((value: string, isFromAmount: boolean) => {
    // Clear any existing timer
    if (quoteTimer) clearTimeout(quoteTimer);
    
    // Set calculation state
    setIsCalculating(true);
    
    // Create a new timer for debounce effect
    const timer = setTimeout(() => {
      if (!selectedPool || !fromToken || !toToken || !value || parseFloat(value) === 0) {
        if (isFromAmount) {
          setToAmount("");
        } else {
          setFromAmount("");
        }
        setIsCalculating(false);
        return;
      }
      
      // Simple price calculation - in real app this would be more sophisticated
      // and would account for slippage, price impact, etc.
      const valueNum = parseFloat(value);
      const price = selectedPool.token1.address === (isFromAmount ? fromToken.address : toToken.address)
        ? selectedPool.token1Price 
        : 1 / selectedPool.token1Price;
      
      if (isFromAmount) {
        const calculatedToAmount = valueNum * price;
        setToAmount(calculatedToAmount.toFixed(6));
      } else {
        const calculatedFromAmount = valueNum / price;
        setFromAmount(calculatedFromAmount.toFixed(6));
      }
      
      setIsCalculating(false);
    }, 500); // 500ms debounce delay
    
    setQuoteTimer(timer);
    
    return () => {
      if (timer) clearTimeout(timer);
    };
  }, [selectedPool, fromToken, toToken]);
  
  // Handle amount changes with debounced quote calculation
  const handleFromAmountChange = useCallback((value: string) => {
    setFromAmount(value);
    calculateQuote(value, true);
  }, [calculateQuote]);
  
  const handleToAmountChange = useCallback((value: string) => {
    setToAmount(value);
    calculateQuote(value, false);
  }, [calculateQuote]);
  
  // Swap tokens positions
  const handleSwapTokens = useCallback(() => {
    setFromToken(toToken);
    setToToken(fromToken);
    setFromAmount(toAmount);
    setToAmount(fromAmount);
  }, [fromToken, toToken, fromAmount, toAmount]);
  
  // Token swap function
  const handleSwap = useCallback(() => {
    if (!connected) {
      toast({
        title: "Wallet not connected",
        description: "Please connect your wallet to swap tokens",
        variant: "destructive",
      });
      return;
    }
    
    if (!fromToken || !toToken || !fromAmount || !selectedPool) {
      toast({
        title: "Invalid swap",
        description: "Please select tokens and enter an amount",
        variant: "destructive",
      });
      return;
    }
    
    try {
      // Convert amount to the appropriate BN format
      const amountIn = new BN(parseFloat(fromAmount) * Math.pow(10, fromToken.decimals));
      
      // For price limit, we can use a large number to basically accept any price
      // In a real app, this would be calculated based on slippage tolerance
      const sqrtPriceLimitQ64 = new BN(0);
      
      // Determine if this is exactInput (true) or exactOutput (false)
      const exactInput = true;
      
      // Execute the swap
      performSwap(
        {
          poolAddress: selectedPool.id,
          exactInput,
          amountSpecified: amountIn,
          sqrtPriceLimitQ64,
        },
        {
          onSuccess: (txSignature) => {
            toast({
              title: "Swap successful",
              description: `Swapped ${fromAmount} ${fromToken.symbol} for approximately ${toAmount} ${toToken.symbol}`,
            });
            
            // Reset the form
            setFromAmount("");
            setToAmount("");
          },
          onError: (error) => {
            console.error("Swap error:", error);
            toast({
              title: "Swap failed",
              description: error instanceof Error ? error.message : "An unknown error occurred",
              variant: "destructive",
            });
          }
        }
      );
    } catch (error) {
      console.error("Error preparing swap:", error);
      toast({
        title: "Swap preparation failed",
        description: error instanceof Error ? error.message : "An unknown error occurred",
        variant: "destructive",
      });
    }
  }, [connected, fromToken, toToken, fromAmount, toAmount, selectedPool, performSwap, toast]);

  // Calculate the price impact (use more sophisticated calculation for production)
  const priceImpact = useMemo(() => {
    if (!fromAmount || !toAmount || !selectedPool) return "0.00%";
    
    // In a real implementation, this would compare the expected price with the actual price
    // For now we'll use a placeholder that increases with swap size
    const impact = parseFloat(fromAmount) > 10 ? 0.75 : parseFloat(fromAmount) > 1 ? 0.25 : 0.05;
    return impact.toFixed(2) + "%";
  }, [fromAmount, toAmount, selectedPool]);

  // Show loading state while fetching pools
  if (isLoadingPools) {
    return (
      <div className="flex flex-col items-center justify-center py-12 space-y-4">
        <Loader2 className="h-10 w-10 animate-spin text-primary" />
        <p>Loading pools and tokens...</p>
      </div>
    );
  }

  // Display a message if no pools are available
  if (pools.length === 0 && !isLoadingPools) {
    return (
      <div className="flex flex-col items-center justify-center py-8 space-y-4 text-center">
        <AlertCircle className="h-12 w-12 text-yellow-500" />
        <h3 className="text-lg font-medium">No Liquidity Pools Available</h3>
        <p className="text-sm text-muted-foreground max-w-md">
          There are currently no liquidity pools to swap with. Please initialize a new pool first or try again later.
        </p>
        <Button variant="outline" onClick={() => window.location.reload()}>Refresh</Button>
      </div>
    );
  }

  return (
    <div className="flex flex-col space-y-6">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-medium">Swap Tokens</h3>
        <Dialog open={showSettings} onOpenChange={setShowSettings}>
          <DialogTrigger asChild>
            <Button variant="ghost" size="icon" className="rounded-full" title="Swap Settings">
              <Settings className="h-4 w-4" />
            </Button>
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Swap Settings</DialogTitle>
            </DialogHeader>
            <div className="space-y-4 py-4">
              <div className="space-y-2">
                <Label htmlFor="slippage">Slippage Tolerance ({slippage}%)</Label>
                <div className="flex items-center gap-2">
                  <Slider
                    id="slippage"
                    value={[slippage]}
                    max={5}
                    step={0.1}
                    onValueChange={(values) => setSlippage(values[0])}
                    className="flex-1"
                  />
                  <Input
                    type="number"
                    value={slippage}
                    onChange={(e) => setSlippage(Number(e.target.value))}
                    min={0.1}
                    max={5}
                    step={0.1}
                    className="w-20"
                  />
                </div>
              </div>
              <div className="space-y-2">
                <Label>Transaction Deadline</Label>
                <div className="flex items-center gap-2">
                  <Input type="number" defaultValue={20} className="w-20" />
                  <span className="text-sm text-muted-foreground">minutes</span>
                </div>
              </div>
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <Label>Expert Mode</Label>
                  <Switch />
                </div>
                <p className="text-xs text-muted-foreground">
                  Allow high slippage trades and skip confirmation screen
                </p>
              </div>
            </div>
          </DialogContent>
        </Dialog>
      </div>
      
      <TokenInput 
        label="From"
        token={fromToken}
        amount={fromAmount}
        onChange={handleFromAmountChange}
        showMaxButton={true}
        onSelectToken={setFromToken}
        otherToken={toToken}
        isLoading={isLoading || isCalculating}
      />
      
      <div className="flex justify-center -my-2 z-10">
        <Button
          variant="secondary"
          size="icon"
          className="rounded-full h-10 w-10 bg-muted shadow-md"
          onClick={handleSwapTokens}
          disabled={isLoading || isCalculating}
        >
          <ArrowDownUp className="h-4 w-4" />
        </Button>
      </div>
      
      <TokenInput 
        label="To (estimated)"
        token={toToken}
        amount={toAmount}
        onChange={handleToAmountChange}
        onSelectToken={setToToken}
        otherToken={fromToken}
        isLoading={isLoading || isCalculating}
      />
      
      {/* Show swap details only when we have amounts */}
      {(fromAmount && toAmount) ? (
        <SwapDetails
          fromToken={fromToken}
          toToken={toToken}
          fromAmount={fromAmount}
          toAmount={toAmount}
          slippage={slippage}
          priceImpact={priceImpact}
          isLoading={isCalculating}
        />
      ) : (
        selectedPool && fromToken && toToken && (
          <div className="text-sm text-center text-muted-foreground p-2">
            {fromToken.symbol}/{toToken.symbol} - Pool fee: {selectedPool.feeTier}%
          </div>
        )
      )}
      
      <Button 
        className="w-full py-6 text-base"
        disabled={!fromAmount || !toAmount || isLoading || isCalculating || !connected || !fromToken || !toToken || !selectedPool} 
        onClick={handleSwap}
      >
        {isLoading ? (
          <>
            <RefreshCw className="mr-2 h-4 w-4 animate-spin" /> Swapping...
          </>
        ) : isCalculating ? (
          <>
            <Loader2 className="mr-2 h-4 w-4 animate-spin" /> Calculating...
          </>
        ) : !connected ? (
          "Connect Wallet"
        ) : !fromToken || !toToken ? (
          "Select Tokens"
        ) : !selectedPool ? (
          "No Liquidity Pool Available"
        ) : !fromAmount || !toAmount ? (
          "Enter an Amount"
        ) : (
          `Swap ${fromAmount} ${fromToken.symbol} for ${toAmount} ${toToken.symbol}`
        )}
      </Button>
    </div>
  );
}

export default memo(SwapInterface);
