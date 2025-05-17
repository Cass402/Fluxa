"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { Check, ChevronDown, Search } from "lucide-react";
import { Token } from "@/lib/types";
import { mockTokens } from "@/lib/mock-data";
import { cn } from "@/lib/utils";

interface TokenSelectorProps {
  selectedToken: Token | null;
  onSelectToken: (token: Token) => void;
  otherToken?: Token | null;
  disabled?: boolean;
}

export default function TokenSelector({
  selectedToken,
  onSelectToken,
  otherToken,
  disabled = false,
}: TokenSelectorProps) {
  const [open, setOpen] = useState(false);
  
  // Filter out the other selected token if provided
  const availableTokens = otherToken
    ? mockTokens.filter((token) => token.address !== otherToken.address)
    : mockTokens;

  return (
    <Popover open={open && !disabled} onOpenChange={(isOpen) => !disabled && setOpen(isOpen)}>
      <PopoverTrigger asChild>
        <Button 
          variant="outline" 
          className="gap-2" 
          disabled={disabled}
        >
          {selectedToken ? (
            <div className="flex items-center gap-2">
              {selectedToken.logo ? (
                <img
                  src={selectedToken.logo}
                  alt={selectedToken.name}
                  className="h-5 w-5 rounded-full"
                  onError={(e) => {
                    // Fallback in case the image fails to load
                    e.currentTarget.src = "https://via.placeholder.com/20";
                  }}
                />
              ) : (
                <div className="h-5 w-5 rounded-full bg-muted flex items-center justify-center">
                  <span className="text-xs">{selectedToken.symbol.charAt(0)}</span>
                </div>
              )}
              <span>{selectedToken.symbol}</span>
            </div>
          ) : (
            <span>Select token</span>
          )}
          <ChevronDown className="h-4 w-4 text-muted-foreground" />
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-[200px] p-0" align="end">
        <Command>
          <CommandInput placeholder="Search tokens..." />
          <CommandList>
            <CommandEmpty>No tokens found.</CommandEmpty>
            <CommandGroup>
              {availableTokens.map((token) => (
                <CommandItem
                  key={token.address}
                  value={token.symbol}
                  onSelect={() => {
                    onSelectToken(token);
                    setOpen(false);
                  }}
                >
                  <div className="flex items-center gap-2 w-full">
                    {token.logo ? (
                      <img
                        src={token.logo}
                        alt={token.name}
                        className="h-5 w-5 rounded-full"
                        onError={(e) => {
                          // Fallback in case image fails to load
                          e.currentTarget.src = "https://via.placeholder.com/20";
                        }}
                      />
                    ) : (
                      <div className="h-5 w-5 rounded-full bg-muted flex items-center justify-center">
                        <span className="text-xs">{token.symbol.charAt(0)}</span>
                      </div>
                    )}
                    <div className="flex flex-col">
                      <span>{token.symbol}</span>
                      <span className="text-xs text-muted-foreground">
                        {token.name}
                      </span>
                    </div>
                    <Check
                      className={cn(
                        "ml-auto h-4 w-4",
                        selectedToken && selectedToken.address === token.address
                          ? "opacity-100"
                          : "opacity-0"
                      )}
                    />
                  </div>
                </CommandItem>
              ))}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}