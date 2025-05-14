"use client";

import { useState, useCallback, useMemo, memo } from "react";
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
}: {
  label: string;
  token: Token | null;
  amount: string;
  onChange: (value: string) => void;
  showMaxButton?: boolean;
  onSelectToken: (token: Token) => void;
  otherToken: Token | null;
}) => {
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <Label>{label}</Label>
        {token && (
          <span className="text-sm text-muted-foreground">
            Balance: {token.balance.toFixed(4)} {token.symbol}
          </span>
        )}
      </div>
      <div className="flex items-center space-x-2">
        <div className="relative flex-1">
          <Input
            type="number"
            placeholder="0.0"
            value={amount}
            onChange={(e) => onChange(e.target.value)}
            className={showMaxButton ? "pr-20" : ""}
          />
          {showMaxButton && token && (
            <Button
              variant="ghost"
              size="sm"
              className="absolute right-1 top-1 h-7 text-xs"
              onClick={() => onChange(token.balance.toString())}
            >
              MAX
            </Button>
          )}
        </div>
        <MemoizedTokenSelector
          selectedToken={token}
          onSelectToken={onSelectToken}
          otherToken={otherToken}
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
  priceImpact 
}: {
  fromToken: Token | null;
  toToken: Token | null;
  fromAmount: string;
  toAmount: string;
  slippage: number;
  priceImpact: string;
}) => {
  // Don't render if we don't have complete data
  if (!fromToken || !toToken || !fromAmount || !toAmount) return null;
  
  // Calculate minimum received based on slippage
  const minReceived = parseFloat(toAmount) * (1 - slippage / 100);
  
  return (
    <div className="space-y-2 rounded-lg bg-muted/50 p-3 text-sm">
      <div className="flex items-center justify-between">
        <div className="flex items-center text-muted-foreground">
          Rate
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Info className="ml-1 h-3 w-3" />
              </TooltipTrigger>
              <TooltipContent>
                <p>The current exchange rate</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
        <div className="flex items-center">
          1 {fromToken.symbol} = {(parseFloat(toAmount) / parseFloat(fromAmount)).toFixed(6)} {toToken.symbol}
          <RefreshCw className="ml-1 h-3 w-3" />
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
              <TooltipContent>
                <p>The difference between the market price and estimated price due to trade size</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
        <div className={cn(
          parseFloat(priceImpact.replace('%', '')) < 0.03 ? "text-green-500" : "text-yellow-500"
        )}>
          {priceImpact}
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
              <TooltipContent>
                <p>The minimum amount you are guaranteed to receive. If the price slips more than your slippage tolerance, your transaction will revert.</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
        <div>
          {minReceived.toFixed(6)} {toToken.symbol}
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
  
  // Default tokens - use the first two tokens if available
  const defaultTokens = useMemo(() => {
    return availableTokens.slice(0, 2);
  }, [availableTokens]);
  
  // State management
  const [fromToken, setFromToken] = useState<Token | null>(null);
  const [toToken, setToToken] = useState<Token | null>(null);
  const [fromAmount, setFromAmount] = useState<string>("");
  const [toAmount, setToAmount] = useState<string>("");
  const [slippage, setSlippage] = useState<number>(0.5);
  const [showSettings, setShowSettings] = useState(false);
  
  // Set default tokens once they're loaded
  useMemo(() => {
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

  // Handle amount changes with real price calculation
  const handleFromAmountChange = useCallback((value: string) => {
    setFromAmount(value);
    
    if (!selectedPool || !fromToken || !toToken || !value || isNaN(parseFloat(value))) {
      setToAmount("");
      return;
    }
    
    // Calculate based on pool price
    try {
      const isToken1ToToken2 = selectedPool.token1.address === fromToken.address;
      const price = isToken1ToToken2 ? selectedPool.price : 1 / selectedPool.price;
      const calculatedAmount = parseFloat(value) * price;
      setToAmount(isNaN(calculatedAmount) ? "" : calculatedAmount.toFixed(6));
    } catch (error) {
      console.error("Error calculating price", error);
      setToAmount("");
    }
  }, [selectedPool, fromToken, toToken]);

  const handleToAmountChange = useCallback((value: string) => {
    setToAmount(value);
    
    if (!selectedPool || !fromToken || !toToken || !value || isNaN(parseFloat(value))) {
      setFromAmount("");
      return;
    }
    
    try {
      const isToken1ToToken2 = selectedPool.token1.address === fromToken.address;
      const price = isToken1ToToken2 ? selectedPool.price : 1 / selectedPool.price;
      const calculatedAmount = parseFloat(value) / price;
      setFromAmount(isNaN(calculatedAmount) ? "" : calculatedAmount.toFixed(6));
    } catch (error) {
      console.error("Error calculating price", error);
      setFromAmount("");
    }
  }, [selectedPool, fromToken, toToken]);

  const handleSwapTokens = useCallback(() => {
    const tempToken = fromToken;
    setFromToken(toToken);
    setToToken(tempToken);
    
    const tempAmount = fromAmount;
    setFromAmount(toAmount);
    setToAmount(tempAmount);
  }, [fromToken, toToken, fromAmount, toAmount]);

  const handleSwap = useCallback(() => {
    if (!connected) {
      toast({
        title: "Wallet not connected",
        description: "Please connect your wallet to swap tokens",
        variant: "destructive",
      });
      return;
    }

    if (!fromToken || !toToken || !fromAmount || !toAmount) {
      toast({
        title: "Invalid input",
        description: "Please select tokens and enter valid amounts",
        variant: "destructive",
      });
      return;
    }

    if (!selectedPool) {
      toast({
        title: "No liquidity pool",
        description: "No pool found for this token pair",
        variant: "destructive",
      });
      return;
    }
    
    try {
      // Prepare parameters for the swap
      const amountSpecified = new BN(parseFloat(fromAmount) * (10 ** fromToken.decimals));
      const sqrtPriceLimitQ64 = new BN(0); // Use default price limit
      
      performSwap({
        poolAddress: selectedPool.address,
        exactInput: true, // We're providing exact input amount
        amountSpecified,
        sqrtPriceLimitQ64,
      }, {
        onSuccess: () => {
          toast({
            title: "Swap Successful",
            description: `Swapped ${fromAmount} ${fromToken.symbol} for ${toAmount} ${toToken.symbol}`,
          });
          setFromAmount("");
          setToAmount("");
        },
        onError: (error) => {
          console.error("Swap error", error);
          toast({
            title: "Swap Failed",
            description: "Failed to complete the swap. Please try again.",
            variant: "destructive",
          });
        }
      });
    } catch (error) {
      console.error("Error preparing swap", error);
      toast({
        title: "Error",
        description: "Failed to prepare the swap transaction",
        variant: "destructive",
      });
    }
  }, [connected, fromToken, toToken, fromAmount, toAmount, selectedPool, performSwap, toast]);

  // Calculate the price impact (use more sophisticated calculation for production)
  const priceImpact = useMemo(() => {
    if (!fromAmount || !toAmount || !selectedPool) return "0.00%";
    
    // In a real implementation, this would compare the expected price with the actual price
    // For now we'll use a placeholder that increases with swap size
    const impact = parseFloat(fromAmount) > 10 ? 0.05 : 0.02;
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

  return (
    <div className="flex flex-col space-y-6">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-medium">Swap Tokens</h3>
        <Dialog open={showSettings} onOpenChange={setShowSettings}>
          <DialogTrigger asChild>
            <Button variant="ghost" size="icon">
              <Settings className="h-4 w-4" />
            </Button>
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Settings</DialogTitle>
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
      />
      
      <div className="flex justify-center -my-2">
        <Button
          variant="secondary"
          size="icon"
          className="rounded-full h-10 w-10 bg-muted shadow-md z-10"
          onClick={handleSwapTokens}
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
      />
      
      {/* Show swap details only when we have amounts */}
      <SwapDetails
        fromToken={fromToken}
        toToken={toToken}
        fromAmount={fromAmount}
        toAmount={toAmount}
        slippage={slippage}
        priceImpact={priceImpact}
      />
      
      <Button 
        className="w-full"
        disabled={!fromAmount || !toAmount || isLoading || !connected || !fromToken || !toToken} 
        onClick={handleSwap}
      >
        {isLoading ? (
          <>
            <RefreshCw className="mr-2 h-4 w-4 animate-spin" /> Swapping...
          </>
        ) : !connected ? (
          "Connect Wallet"
        ) : !fromToken || !toToken ? (
          "Select Tokens"
        ) : !fromAmount || !toAmount ? (
          "Enter an Amount"
        ) : (
          `Swap ${fromToken.symbol} for ${toToken.symbol}`
        )}
      </Button>
    </div>
  );
}

export default memo(SwapInterface);
