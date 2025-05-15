"use client";

import { useEffect, useState } from "react";
import { useWallet } from "@/contexts/WalletContext";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Button, buttonVariants } from "@/components/ui/button";
import { ArrowRight, Wallet, AlertCircle, RefreshCw } from "lucide-react";
import Link from "next/link";
import DashboardStats from "@/components/dashboard/DashboardStats";
import TransactionHistory from "@/components/dashboard/TransactionHistory";
import PortfolioChart from "@/components/dashboard/PortfolioChart";
import TokenBalances from "@/components/dashboard/TokenBalances";
import { solanaService } from "@/services/solanaService";

export default function DashboardPage() {
  const { connected, address, walletType, connect, serviceInitialized } = useWallet();
  const [isRetrying, setIsRetrying] = useState<boolean>(false);

  // Retry wallet connection
  const handleRetryConnection = async () => {
    if (!walletType) return;
    
    setIsRetrying(true);
    try {
      await connect(walletType);
    } catch (error) {
      console.error("[Dashboard] Error retrying connection:", error);
    } finally {
      setIsRetrying(false);
    }
  };

  if (!connected) {
    return (
      <div className="container flex flex-col items-center justify-center min-h-[70vh] px-4 py-8 text-center">
        <Wallet className="h-16 w-16 mb-6 text-primary" />
        <h1 className="text-3xl font-bold tracking-tight mb-4">Connect your wallet</h1>
        <p className="text-muted-foreground max-w-md mb-8">
          Connect your wallet to view your dashboard, track your portfolio, and manage your positions.
        </p>
        <Link href="/" className={buttonVariants({ size: "lg" })}>
          Go to home
          <ArrowRight className="ml-2 h-4 w-4" />
        </Link>
      </div>
    );
  }

  if (connected && !serviceInitialized) {
    return (
      <div className="container flex flex-col items-center justify-center min-h-[70vh] px-4 py-8 text-center">
        <div className="bg-amber-100 dark:bg-amber-900/20 p-3 rounded-full mb-6">
          <AlertCircle className="h-12 w-12 text-amber-600 dark:text-amber-500" />
        </div>
        <h1 className="text-3xl font-bold tracking-tight mb-4">Service Connection Issue</h1>
        <p className="text-muted-foreground max-w-md mb-8">
          Your wallet is connected, but we couldn't initialize the Solana service. This could be due to
          network issues or program deployment problems.
        </p>
        <div className="space-y-4">
          <Button 
            onClick={handleRetryConnection} 
            size="lg" 
            className="w-full sm:w-auto"
            disabled={isRetrying}
          >
            {isRetrying ? (
              <>
                <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
                Retrying...
              </>
            ) : (
              <>
                <RefreshCw className="mr-2 h-4 w-4" />
                Retry Connection
              </>
            )}
          </Button>
          <div className="flex flex-col sm:flex-row gap-4">
            <Link href="/" className={buttonVariants({ variant: "outline", size: "lg" })}>
              Return Home
            </Link>
            <Link href="/swap" className={buttonVariants({ variant: "outline", size: "lg" })}>
              Go to Swap
            </Link>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="container max-w-10xl px-4 py-8 md:px-6 md:py-12">
      <div className="space-y-8">
        <div className="flex items-center justify-between">
          <h1 className="text-3xl font-bold tracking-tight">Dashboard</h1>
        </div>
        
        <DashboardStats />
        
        <div className="grid gap-6 lg:grid-cols-3">
          <Card className="border bg-card/50 backdrop-blur-sm col-span-2">
            <CardHeader>
              <CardTitle>Portfolio Value</CardTitle>
            </CardHeader>
            <CardContent>
              <PortfolioChart />
            </CardContent>
          </Card>
          
          <Card className="border bg-card/50 backdrop-blur-sm">
            <CardHeader>
              <CardTitle>Token Balances</CardTitle>
            </CardHeader>
            <CardContent>
              <TokenBalances />
            </CardContent>
          </Card>
        </div>
        
        <Card className="border bg-card/50 backdrop-blur-sm">
          <CardHeader>
            <CardTitle>Transaction History</CardTitle>
          </CardHeader>
          <CardContent>
            <TransactionHistory />
          </CardContent>
        </Card>
      </div>
    </div>
  );
}