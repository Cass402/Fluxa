"use client";

import { mockTokens } from "@/lib/mock-data";

export default function TokenBalances() {
  // Sort tokens by value (balance * price)
  const sortedTokens = [...mockTokens]
    .map(token => ({
      ...token,
      value: token.balance * token.priceUsd
    }))
    .sort((a, b) => b.value - a.value);

  return (
    <div className="space-y-4">
      {sortedTokens.slice(0, 6).map((token) => (
        <div key={token.address} className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <img
              src={token.logo}
              alt={token.name}
              className="h-6 w-6 rounded-full"
            />
            <div>
              <div className="font-medium">{token.symbol}</div>
              <div className="text-xs text-muted-foreground">{token.name}</div>
            </div>
          </div>
          <div className="text-right">
            <div>{token.balance.toFixed(token.balance < 0.01 ? 6 : 4)}</div>
            <div className="text-xs text-muted-foreground">
              ${(token.balance * token.priceUsd).toLocaleString()}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}