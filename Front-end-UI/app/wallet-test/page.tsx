"use client";

import WalletTest from "@/components/common/WalletTest";
import WalletDebugPanel from "@/components/common/WalletDebugPanel";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { ArrowLeft } from "lucide-react";
import Link from "next/link";

export default function WalletTestPage() {
  return (
    <div className="container max-w-4xl py-8">
      <Link 
        href="/" 
        className="flex items-center text-primary hover:underline mb-6"
      >
        <ArrowLeft className="h-4 w-4 mr-2" />
        Back to home
      </Link>

      <Card className="mb-8">
        <CardHeader>
          <CardTitle>Wallet Connection Diagnostics</CardTitle>
          <CardDescription>
            This page helps you diagnose any issues with your wallet connection
          </CardDescription>
        </CardHeader>
        <CardContent>
          <WalletDebugPanel />
          <div className="mt-8">
            <WalletTest />
          </div>
        </CardContent>
      </Card>
      
      <Card className="bg-muted">
        <CardHeader>
          <CardTitle>Common Issues and Solutions</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <div>
              <h3 className="text-lg font-medium mb-1">Wallet not connected</h3>
              <ul className="list-disc pl-6 text-sm space-y-1">
                <li>Make sure your Phantom wallet extension is installed and unlocked</li>
                <li>Try clicking the Disconnect button and connecting again</li>
                <li>Check that you're using a compatible browser (Chrome or Brave)</li>
                <li>Ensure you've given permission to the site to connect to your wallet</li>
              </ul>
            </div>
            
            <div>
              <h3 className="text-lg font-medium mb-1">Transaction errors</h3>
              <ul className="list-disc pl-6 text-sm space-y-1">
                <li>Make sure you have SOL in your wallet for transaction fees</li>
                <li>Try connecting to Devnet instead of Mainnet for testing</li>
                <li>Check that your wallet has the right permissions set</li>
                <li>Clear your browser cache and try again</li>
              </ul>
            </div>
            
            <div>
              <h3 className="text-lg font-medium mb-1">Solana service not initialized</h3>
              <ul className="list-disc pl-6 text-sm space-y-1">
                <li>Disconnect and reconnect your wallet</li>
                <li>Try reloading the page after connecting</li>
                <li>Clear local storage and reconnect</li>
              </ul>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
