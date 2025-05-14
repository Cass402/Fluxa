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
  selectedToken: Token;
  onSelectToken: (token: Token) => void;
  otherToken?: Token;
}

export default function TokenSelector({
  selectedToken,
  onSelectToken,
  otherToken,
}: TokenSelectorProps) {
  const [open, setOpen] = useState(false);
  
  // Filter out the other selected token if provided
  const availableTokens = otherToken
    ? mockTokens.filter((token) => token.address !== otherToken.address)
    : mockTokens;

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button variant="outline" className="gap-2">
          <div className="flex items-center gap-2">
            <img
              src={selectedToken.logo}
              alt={selectedToken.name}
              className="h-5 w-5 rounded-full"
            />
            <span>{selectedToken.symbol}</span>
          </div>
          <ChevronDown className="h-4 w-4 text-muted-foreground" />
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-[200px] p-0" align="end">
        <Command>
          <CommandInput placeholder="Search tokens..." icon={Search} />
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
                    <img
                      src={token.logo}
                      alt={token.name}
                      className="h-5 w-5 rounded-full"
                    />
                    <div className="flex flex-col">
                      <span>{token.symbol}</span>
                      <span className="text-xs text-muted-foreground">
                        {token.name}
                      </span>
                    </div>
                    <Check
                      className={cn(
                        "ml-auto h-4 w-4",
                        selectedToken.address === token.address
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