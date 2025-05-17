"use client";

import { ArrowDownUp, ArrowRight, ExternalLink } from "lucide-react";
import { mockTransactions } from "@/lib/mock-data";
import { Button } from "@/components/ui/button";
import { formatDistanceToNow } from "date-fns";
import { useWallet } from "@/contexts/WalletContext";
import TokenPair from "@/components/common/TokenPair";

export default function RecentTransactions() {
  const { connected } = useWallet();
  
  if (!connected) {
    return (
      <div className="flex flex-col items-center justify-center p-6 text-center">
        <ArrowDownUp className="h-12 w-12 text-muted-foreground mb-4" />
        <h3 className="text-lg font-medium mb-2">No recent transactions</h3>
        <p className="text-sm text-muted-foreground mb-4">
          Connect your wallet to view your transaction history
        </p>
        <Button variant="outline" onClick={() => document.getElementById('connect-wallet-btn')?.click()}>Connect Wallet</Button>
      </div>
    );
  }
  
  // Display mock transactions
  return (
    <div className="space-y-4">
      {mockTransactions.length === 0 ? (
        <div className="flex flex-col items-center justify-center p-6 text-center">
          <ArrowDownUp className="h-12 w-12 text-muted-foreground mb-4" />
          <h3 className="text-lg font-medium mb-2">No transactions yet</h3>
          <p className="text-sm text-muted-foreground">
            Your transactions will appear here
          </p>
        </div>
      ) : (
        <div className="space-y-4">
          {mockTransactions.map((tx) => (
            <div
              key={tx.hash}
              className="flex items-center justify-between p-3 rounded-lg border bg-card/50 backdrop-blur-sm transition-colors hover:bg-accent/10"
            >
              <div className="flex items-center gap-3">
                <div className="flex items-center justify-center w-10 h-10 rounded-full bg-primary/10">
                  <ArrowDownUp className="h-5 w-5 text-primary" />
                </div>
                <div>
                  <div className="flex items-center gap-1">
                    <span className="font-medium">Swap</span>
                    <TokenPair 
                      token1={tx.fromToken} 
                      token2={tx.toToken} 
                      size="sm" 
                    />
                  </div>
                  <div className="text-sm text-muted-foreground flex items-center gap-1">
                    {formatDistanceToNow(new Date(tx.timestamp), { addSuffix: true })}
                    <span>•</span>
                    <span className={tx.status === "confirmed" ? "text-green-500" : tx.status === "pending" ? "text-yellow-500" : "text-red-500"}>
                      {tx.status.charAt(0).toUpperCase() + tx.status.slice(1)}
                    </span>
                  </div>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <div className="text-sm text-right">
                  <div>
                    {tx.fromAmount} {tx.fromToken.symbol}
                    <ArrowRight className="inline mx-1 h-3 w-3" />
                    {tx.toAmount} {tx.toToken.symbol}
                  </div>
                  <div className="text-xs text-muted-foreground">
                    ${tx.valueUSD.toLocaleString()}
                  </div>
                </div>
                <Button variant="ghost" size="icon" className="h-8 w-8">
                  <ExternalLink className="h-4 w-4" />
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}