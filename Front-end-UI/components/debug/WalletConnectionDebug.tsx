"use client";

import { useEffect, useState } from 'react';
import { Connection } from '@solana/web3.js';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { useWallet } from '@/contexts/WalletContext';
import { SOLANA_NETWORK } from '@/lib/config';
import { Wallet, RefreshCw, AlertCircle, CheckCircle, ExternalLink } from 'lucide-react';

export default function WalletConnectionDebug() {
  const wallet = useWallet();
  const [phantomDetails, setPhantomDetails] = useState<any>({});
  const [connectionState, setConnectionState] = useState<string>("Not checked");
  const [isChecking, setIsChecking] = useState(false);

  // Render wallet icon based on wallet type
  const getWalletIcon = () => {
    if (!wallet.walletType) return <Wallet className="h-5 w-5" />;
    
    switch(wallet.walletType) {
      case "phantom":
        return (
          <div className="bg-purple-400 rounded-full p-1 flex items-center justify-center">
            <svg width="20" height="20" viewBox="0 0 128 128" fill="none">
              <path d="M110.584 64.9142C110.584 63.0419 109.059 61.5172 107.187 61.5172H89.8091C87.937 61.5172 86.4121 63.0419 86.4121 64.9142V82.2921C86.4121 84.1642 87.937 85.689 89.8091 85.689H107.187C109.059 85.689 110.584 84.1642 110.584 82.2921V64.9142Z" fill="white"/>
              <path fillRule="evenodd" clipRule="evenodd" d="M27.8502 47.0453C27.8502 44.0913 30.2412 41.7003 33.1952 41.7003H95.6642C98.6182 41.7003 101.009 44.0913 101.009 47.0453V83.3913C101.009 86.3453 98.6182 88.7363 95.6642 88.7363H33.1952C30.2412 88.7363 27.8502 86.3453 27.8502 83.3913V47.0453ZM39.7502 57.7363C39.7502 55.5273 41.5412 53.7363 43.7502 53.7363H85.1092C87.3182 53.7363 89.1092 55.5273 89.1092 57.7363V72.7003C89.1092 74.9093 87.3182 76.7003 85.1092 76.7003H43.7502C41.5412 76.7003 39.7502 74.9093 39.7502 72.7003V57.7363Z" fill="white"/>
            </svg>
          </div>
        );
      case "solflare":
        return (
          <div className="bg-orange-500 rounded-full p-1 flex items-center justify-center">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
              <path d="M16.2322 7H9.57747C9.12101 7 8.73227 7.38873 8.73227 7.8452C8.73227 8.30166 9.12101 8.6904 9.57747 8.6904H16.2322C16.6887 8.6904 17.0774 8.30166 17.0774 7.8452C17.0774 7.38873 16.6887 7 16.2322 7Z" fill="white"/>
              <path d="M16.2322 10.4082H9.57747C9.12101 10.4082 8.73227 10.7969 8.73227 11.2534C8.73227 11.7098 9.12101 12.0986 9.57747 12.0986H16.2322C16.6887 12.0986 17.0774 11.7098 17.0774 11.2534C17.0774 10.7969 16.6887 10.4082 16.2322 10.4082Z" fill="white"/>
              <path d="M15.126 13.8164H7.8452C7.38873 13.8164 7 14.2051 7 14.6616C7 15.1181 7.38873 15.5068 7.8452 15.5068H15.126C15.5825 15.5068 15.9712 15.1181 15.9712 14.6616C15.9712 14.2051 15.5825 13.8164 15.126 13.8164Z" fill="white"/>
              <path d="M15.126 17.2247H7.8452C7.38873 17.2247 7 17.6134 7 18.0699C7 18.5263 7.38873 18.9151 7.8452 18.9151H15.126C15.5825 18.9151 15.9712 18.5263 15.9712 18.0699C15.9712 17.6134 15.5825 17.2247 15.126 17.2247Z" fill="white"/>
            </svg>
          </div>
        );
      default:
        return <Wallet className="h-5 w-5" />;
    }
  };

  // Function to truncate address for display
  const truncateAddress = (addr: string) => {
    if (!addr) return "";
    return `${addr.slice(0, 8)}...${addr.slice(-8)}`;
  };

  // Debug function to check Phantom connection
  const checkPhantomConnection = () => {
    setIsChecking(true);
    
    try {
      const solana = (window as any).solana;
      if (!solana) {
        setConnectionState("Phantom not found in window");
        setIsChecking(false);
        return;
      }

      const details = {
        exists: !!solana,
        isPhantom: !!solana.isPhantom,
        isConnected: !!solana.isConnected,
        publicKey: solana.publicKey?.toString() || 'None',
        autoApprove: !!solana.autoApprove,
        api_version: solana._version || 'Unknown',
        methods: Object.keys(solana).filter(k => typeof solana[k] === 'function')
      };

      setPhantomDetails(details);
      setConnectionState(details.isConnected ? "Connected in Phantom" : "Not connected in Phantom");
      
      // Try to get actual connection
      solana.connect({ onlyIfTrusted: false })
        .then((resp: any) => {
          console.log("Raw Phantom response:", resp);
          setConnectionState(`Connected: ${resp.publicKey?.toString() || 'No key'}`);
          setIsChecking(false);
        })
        .catch((err: any) => {
          setConnectionState(`Error: ${err.message}`);
          console.error("Phantom connection error:", err);
          setIsChecking(false);
        });
    } catch (error: any) {
      console.error("Error checking Phantom:", error);
      setConnectionState(`Error: ${error.message}`);
      setIsChecking(false);
    }
  };

  // Check if the wallet context has a functioning connection
  const checkWalletContext = async () => {
    setIsChecking(true);
    try {
      if (!wallet.publicKey) {
        setConnectionState("No publicKey in wallet context");
        setIsChecking(false);
        return;
      }
      
      // Create a new connection
      const connection = new Connection(SOLANA_NETWORK);
      const balance = await connection.getBalance(wallet.publicKey);
      
      setConnectionState(`Wallet context working: ${balance / 1e9} SOL`);
    } catch (error: any) {
      console.error("Error checking wallet context:", error);
      setConnectionState(`Wallet context error: ${error.message}`);
    } finally {
      setIsChecking(false);
    }
  };

  // Force a clean reconnect
  const forceReconnect = async () => {
    setIsChecking(true);
    try {
      // First disconnect
      if (wallet.connected) {
        await wallet.disconnect();
      }
      
      // Clear any stored data
      localStorage.removeItem("walletType");
      localStorage.removeItem("walletAddress");
      
      // Wait a moment
      await new Promise(resolve => setTimeout(resolve, 1000));
      
      // Reconnect
      await wallet.connect("phantom");
      setConnectionState("Forced reconnection completed");
    } catch (error: any) {
      console.error("Force reconnect error:", error);
      setConnectionState(`Reconnect error: ${error.message}`);
    } finally {
      setIsChecking(false);
    }
  };

  return (
    <Card className="mb-6 bg-red-50 dark:bg-red-900/10 border-red-200">
      <CardHeader className="pb-2">
        <div className="flex justify-between items-center">
          <CardTitle className="text-red-600 dark:text-red-400 text-lg">
            <span>🚨 Wallet Debug Panel</span>
          </CardTitle>
          {wallet.connected && (
            <div className="flex items-center space-x-1 bg-green-100 dark:bg-green-900/30 px-2 py-0.5 rounded-full text-green-700 dark:text-green-400">
              <div className="h-2 w-2 rounded-full bg-green-500"></div>
              <span className="text-xs">Connected</span>
            </div>
          )}
        </div>
      </CardHeader>
      <CardContent>
        {wallet.connected ? (
          <div className="mb-4 p-3 bg-white dark:bg-slate-800 rounded-md border border-red-100 dark:border-red-900/30">
            <div className="flex items-center gap-2 mb-2">
              {getWalletIcon()}
              <span className="font-medium capitalize">{wallet.walletType} Wallet</span>
            </div>
            <div className="text-sm bg-slate-50 dark:bg-slate-900 p-2 rounded border border-slate-200 dark:border-slate-700 break-all">
              {wallet.address}
            </div>
          </div>
        ) : (
          <div className="mb-4 p-3 bg-slate-50 dark:bg-slate-800 rounded-md border border-slate-200 dark:border-slate-700 text-center">
            <AlertCircle className="h-8 w-8 mx-auto text-red-500 mb-2" />
            <p className="text-red-600 dark:text-red-400">Wallet not connected</p>
          </div>
        )}
        
        <div className="space-y-2 mb-4">
          <div className="flex justify-between items-center py-1 border-b border-red-100 dark:border-red-900/20">
            <span className="font-medium">Connection State:</span>
            <span className="text-right flex items-center">
              {isChecking && <RefreshCw className="h-3 w-3 mr-1 animate-spin" />}
              {connectionState}
            </span>
          </div>
          <div className="flex justify-between items-center py-1 border-b border-red-100 dark:border-red-900/20">
            <span className="font-medium">Public Key:</span>
            <span className="text-right">
              {wallet.publicKey ? truncateAddress(wallet.publicKey.toString()) : 'None'}
            </span>
          </div>
        </div>
        
        {phantomDetails.exists && (
          <div className="mb-4 p-2 bg-slate-100 dark:bg-slate-800 rounded text-xs">
            <div className="text-sm font-medium mb-1">Phantom Details:</div>
            <div className="grid grid-cols-2 gap-1">
              <div><span className="font-medium">isPhantom:</span> {phantomDetails.isPhantom ? 'Yes' : 'No'}</div>
              <div><span className="font-medium">isConnected:</span> {phantomDetails.isConnected ? 'Yes' : 'No'}</div>
              <div className="col-span-2"><span className="font-medium">Public Key:</span> {phantomDetails.publicKey}</div>
              <div className="col-span-2"><span className="font-medium">API Version:</span> {phantomDetails.api_version}</div>
              <div className="col-span-2">
                <span className="font-medium">Methods:</span> 
                <div className="mt-1 bg-slate-200 dark:bg-slate-700 p-1 rounded overflow-x-auto">
                  {phantomDetails.methods?.join(', ')}
                </div>
              </div>
            </div>
          </div>
        )}
        
        <div className="flex flex-wrap gap-2">
          <Button 
            variant="destructive" 
            size="sm" 
            onClick={checkPhantomConnection} 
            disabled={isChecking}
            className="flex items-center gap-1"
          >
            {isChecking ? <RefreshCw className="h-3 w-3 animate-spin" /> : <Wallet className="h-4 w-4" />}
            Check Phantom
          </Button>
          
          <Button 
            variant="destructive" 
            size="sm" 
            onClick={checkWalletContext}
            disabled={isChecking}
            className="flex items-center gap-1"
          >
            {isChecking ? <RefreshCw className="h-3 w-3 animate-spin" /> : <CheckCircle className="h-4 w-4" />}
            Test Wallet Context
          </Button>
          
          <Button 
            variant="destructive" 
            size="sm" 
            onClick={forceReconnect}
            disabled={isChecking}
            className="flex items-center gap-1"
          >
            {isChecking ? <RefreshCw className="h-3 w-3 animate-spin" /> : <RefreshCw className="h-4 w-4" />}
            Force Reconnect
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
