'use client';

import React, { useState } from "react";
import { cn } from "@/lib/utils";
import { Select, SelectOption } from "../ui/select";

export interface Token {
  symbol: string;
  name: string;
  logo?: string;
  address: string;
}

export interface TokenPairSelectorProps {
  availableTokens: Token[];
  baseToken?: string;
  quoteToken?: string;
  onPairChange?: (baseToken: string, quoteToken: string) => void;
  className?: string;
  disabled?: boolean;
  feeTiers?: { value: string; label: string }[];
  selectedFeeTier?: string;
  onFeeTierChange?: (feeTier: string) => void;
}

/**
 * Token Pair Selector component for AMM trading pairs
 * Includes fee tier selection for concentrated liquidity positions
 */
export const TokenPairSelector = ({
  availableTokens,
  baseToken,
  quoteToken,
  onPairChange,
  className,
  disabled,
  feeTiers = [
    { value: "0.01", label: "0.01% - Best for stable pairs" },
    { value: "0.05", label: "0.05% - Best for normal pairs" },
    { value: "0.3", label: "0.3% - Best for volatile pairs" },
    { value: "1", label: "1% - Best for exotic pairs" }
  ],
  selectedFeeTier,
  onFeeTierChange,
}: TokenPairSelectorProps) => {
  const [baseTokenValue, setBaseTokenValue] = useState(baseToken || "");
  const [quoteTokenValue, setQuoteTokenValue] = useState(quoteToken || "");

  // Convert tokens to select options
  const tokenOptions: SelectOption[] = availableTokens.map(token => ({
    value: token.address,
    label: token.symbol,
    icon: token.logo ? (
      <img 
        src={token.logo} 
        alt={token.symbol} 
        className="w-5 h-5 rounded-full"
      />
    ) : (
      <div className="w-5 h-5 bg-gray-200 rounded-full flex items-center justify-center text-xs">
        {token.symbol.charAt(0)}
      </div>
    )
  }));

  // Fee tier options
  const feeTierOptions: SelectOption[] = feeTiers.map(tier => ({
    value: tier.value,
    label: tier.label
  }));

  const handleBaseTokenChange = (value: string) => {
    // Prevent selecting the same token for both sides
    if (value === quoteTokenValue) {
      return;
    }
    
    setBaseTokenValue(value);
    onPairChange?.(value, quoteTokenValue);
  };

  const handleQuoteTokenChange = (value: string) => {
    // Prevent selecting the same token for both sides
    if (value === baseTokenValue) {
      return;
    }
    
    setQuoteTokenValue(value);
    onPairChange?.(baseTokenValue, value);
  };

  // Swap the token positions
  const handleSwapTokens = () => {
    if (disabled || !baseTokenValue || !quoteTokenValue) return;
    
    const tempBase = baseTokenValue;
    setBaseTokenValue(quoteTokenValue);
    setQuoteTokenValue(tempBase);
    onPairChange?.(quoteTokenValue, tempBase);
  };

  return (
    <div className={cn("p-4 rounded-lg border border-gray-200", className)}>
      <div className="space-y-4">
        <h3 className="text-base font-medium">Select Pair</h3>
        
        <div className="grid grid-cols-[1fr,auto,1fr] items-center gap-2">
          {/* Base token selector */}
          <Select
            options={tokenOptions}
            value={baseTokenValue}
            onChange={handleBaseTokenChange}
            placeholder="Select token"
            label="You pay"
            disabled={disabled}
            className="w-full"
          />
          
          {/* Swap button */}
          <button
            onClick={handleSwapTokens}
            disabled={disabled || !baseTokenValue || !quoteTokenValue}
            className="p-2 rounded-full bg-primary/10 hover:bg-primary/20 text-primary disabled:opacity-50 mt-6"
            aria-label="Swap tokens"
          >
            <svg 
              xmlns="http://www.w3.org/2000/svg" 
              width="16" 
              height="16" 
              viewBox="0 0 24 24" 
              fill="none" 
              stroke="currentColor" 
              strokeWidth="2" 
              strokeLinecap="round" 
              strokeLinejoin="round"
            >
              <path d="M7 16V4m0 0L3 8m4-4l4 4M17 8v12m0 0l4-4m-4 4l-4-4" />
            </svg>
          </button>
          
          {/* Quote token selector */}
          <Select
            options={tokenOptions}
            value={quoteTokenValue}
            onChange={handleQuoteTokenChange}
            placeholder="Select token"
            label="You receive"
            disabled={disabled}
            className="w-full"
          />
        </div>

        {/* Fee tier selector - only visible when both tokens are selected */}
        {(baseTokenValue && quoteTokenValue && feeTiers) && (
          <div className="mt-4">
            <Select
              options={feeTierOptions}
              value={selectedFeeTier}
              onChange={(value) => onFeeTierChange?.(value)}
              placeholder="Select fee tier"
              label="Fee tier"
              disabled={disabled}
            />
            <p className="mt-1 text-xs text-gray-500">
              The fee tier represents the cost per trade and affects potential returns
            </p>
          </div>
        )}
      </div>
    </div>
  );
};

export default TokenPairSelector;