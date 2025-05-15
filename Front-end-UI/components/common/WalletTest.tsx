"use client";

import { useEffect, useState } from 'react';
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { useWallet } from '@/contexts/WalletContext';
import { solanaService } from '@/services/solanaService';
import { AlertCircle, CheckCircle2, Wallet, Copy } from 'lucide-react';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";

export default function WalletTest() {
  const { connected, publicKey, walletType, address } = useWallet();
  const [testResult, setTestResult] = useState<{success: boolean, message: string} | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  // Function to get the wallet icon based on wallet type
  const getWalletIcon = () => {
    if (!walletType) return <Wallet className="h-5 w-5" />;
    
    switch(walletType) {
      case "phantom":
        return <div className="bg-purple-400 rounded-full p-1">
          <svg width="18" height="18" viewBox="0 0 128 128" fill="none">
            <path d="M110.584 64.9142C110.584 63.0419 109.059 61.5172 107.187 61.5172H89.8091C87.937 61.5172 86.4121 63.0419 86.4121 64.9142V82.2921C86.4121 84.1642 87.937 85.689 89.8091 85.689H107.187C109.059 85.689 110.584 84.1642 110.584 82.2921V64.9142Z" fill="white"/>
            <path fillRule="evenodd" clipRule="evenodd" d="M27.8502 47.0453C27.8502 44.0913 30.2412 41.7003 33.1952 41.7003H95.6642C98.6182 41.7003 101.009 44.0913 101.009 47.0453V83.3913C101.009 86.3453 98.6182 88.7363 95.6642 88.7363H33.1952C30.2412 88.7363 27.8502 86.3453 27.8502 83.3913V47.0453ZM39.7502 57.7363C39.7502 55.5273 41.5412 53.7363 43.7502 53.7363H85.1092C87.3182 53.7363 89.1092 55.5273 89.1092 57.7363V72.7003C89.1092 74.9093 87.3182 76.7003 85.1092 76.7003H43.7502C41.5412 76.7003 39.7502 74.9093 39.7502 72.7003V57.7363Z" fill="white"/>
          </svg>
        </div>;
      case "solflare":
        return <div className="bg-orange-500 rounded-full p-1">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
            <path d="M16.2322 7H9.57747C9.12101 7 8.73227 7.38873 8.73227 7.8452C8.73227 8.30166 9.12101 8.6904 9.57747 8.6904H16.2322C16.6887 8.6904 17.0774 8.30166 17.0774 7.8452C17.0774 7.38873 16.6887 7 16.2322 7Z" fill="white"/>
            <path d="M16.2322 10.4082H9.57747C9.12101 10.4082 8.73227 10.7969 8.73227 11.2534C8.73227 11.7098 9.12101 12.0986 9.57747 12.0986H16.2322C16.6887 12.0986 17.0774 11.7098 17.0774 11.2534C17.0774 10.7969 16.6887 10.4082 16.2322 10.4082Z" fill="white"/>
            <path d="M15.126 13.8164H7.8452C7.38873 13.8164 7 14.2051 7 14.6616C7 15.1181 7.38873 15.5068 7.8452 15.5068H15.126C15.5825 15.5068 15.9712 15.1181 15.9712 14.6616C15.9712 14.2051 15.5825 13.8164 15.126 13.8164Z" fill="white"/>
            <path d="M15.126 17.2247H7.8452C7.38873 17.2247 7 17.6134 7 18.0699C7 18.5263 7.38873 18.9151 7.8452 18.9151H15.126C15.5825 18.9151 15.9712 18.5263 15.9712 18.0699C15.9712 17.6134 15.5825 17.2247 15.126 17.2247Z" fill="white"/>
          </svg>
        </div>;
      default:
        return <Wallet className="h-5 w-5" />;
    }
  };
  
  // Function to truncate address for display
  const truncateAddress = (addr: string) => {
    return addr ? `${addr.slice(0, 6)}...${addr.slice(-4)}` : "";
  };

  // Function to copy address to clipboard
  const copyAddress = () => {
    if (address) {
      navigator.clipboard.writeText(address);
      alert("Address copied to clipboard!");
    }
  };

  // Function to test the wallet connection
  const testWalletConnection = async () => {
    setIsLoading(true);
    setTestResult(null);
    
    try {
      // Check if wallet is connected
      if (!connected || !publicKey) {
        setTestResult({
          success: false,
          message: "Wallet is not connected. Please connect your wallet first."
        });
        return;
      }
      
      // Check if Solana service is initialized
      if (!solanaService.isInitialized()) {
        setTestResult({
          success: false,
          message: "Solana service is not initialized with the wallet."
        });
        return;
      }
      
      // Try to get the wallet balance as a test
      const connection = solanaService.getConnection();
      const balance = await connection.getBalance(publicKey);
      
      setTestResult({
        success: true,
        message: `Connection successful! Your wallet has ${balance / 1e9} SOL.`
      });
      
    } catch (error: any) {
      console.error("Wallet test failed:", error);
      setTestResult({
        success: false,
        message: `Test failed: ${error.message || "Unknown error"}`
      });
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <Card className="w-full max-w-md mx-auto">
      <CardHeader>
        <CardTitle>Wallet Connection Test</CardTitle>
        <CardDescription>
          Test your wallet connection to make sure everything is working properly.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="space-y-4">
          {connected ? (
            <div className="bg-green-50 dark:bg-green-900/20 border border-green-200 rounded-md p-4">
              <div className="flex items-center justify-between mb-3">
                <div className="flex items-center space-x-2">
                  {getWalletIcon()}
                  <span className="font-medium">{walletType || 'Unknown'} Wallet</span>
                </div>
                <div className="flex items-center space-x-1">
                  <div className="h-2 w-2 rounded-full bg-green-500"></div>
                  <span className="text-xs text-green-600 dark:text-green-400">Connected</span>
                </div>
              </div>
              
              <TooltipProvider>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <div 
                      className="text-sm bg-slate-100 dark:bg-slate-800 px-3 py-2 rounded-md flex justify-between items-center cursor-pointer"
                      onClick={copyAddress}
                    >
                      <div className="truncate">{address || 'No address available'}</div>
                      <Copy className="h-4 w-4 ml-2 flex-shrink-0" />
                    </div>
                  </TooltipTrigger>
                  <TooltipContent>
                    <p>Click to copy full address</p>
                  </TooltipContent>
                </Tooltip>
              </TooltipProvider>
            </div>
          ) : (
            <div className="bg-slate-50 dark:bg-slate-900/20 border border-slate-200 rounded-md p-4 text-center">
              <Wallet className="h-10 w-10 mx-auto text-slate-400 mb-2" />
              <p className="text-slate-600 dark:text-slate-400">No wallet connected.</p>
              <p className="text-xs text-slate-500 mt-1">Connect your wallet to continue.</p>
            </div>
          )}
          
          <div className="grid grid-cols-2 gap-2 text-sm">            
            <div className="font-medium">Solana Service:</div>
            <div>{solanaService.isInitialized() ? '✅ Initialized' : '❌ Not initialized'}</div>
            
            <div className="font-medium">Public Key:</div>
            <div className="truncate">{publicKey ? publicKey.toString() : 'None'}</div>
          </div>
          
          {testResult && (
            <Alert variant={testResult.success ? "default" : "destructive"} className="mt-4">
              {testResult.success ? (
                <CheckCircle2 className="h-4 w-4" />
              ) : (
                <AlertCircle className="h-4 w-4" />
              )}
              <AlertTitle>
                {testResult.success ? 'Success' : 'Error'}
              </AlertTitle>
              <AlertDescription>
                {testResult.message}
              </AlertDescription>
            </Alert>
          )}
        </div>
      </CardContent>
      <CardFooter>
        <div className="flex gap-2 w-full">
          <Button 
            onClick={testWalletConnection} 
            disabled={isLoading || !connected}
            className="flex-1"
          >
            {isLoading ? "Testing..." : "Test Wallet Connection"}
          </Button>
          
          <Button 
            variant="outline"
            onClick={() => {
              // Direct Phantom connection for testing
              if (typeof window !== 'undefined' && window.solana) {
                window.solana.connect({ onlyIfTrusted: false })
                  .then(res => {
                    console.log("Direct connection result:", res);
                    window.location.reload();  // Refresh to update UI
                  })
                  .catch(err => {
                    console.error("Direct connection error:", err);
                    alert(`Error: ${err.message}`);
                  });
              } else {
                alert("Phantom wallet extension not detected");
              }
            }}
          >
            Direct Connect
          </Button>
        </div>
      </CardFooter>
    </Card>
  );
}
