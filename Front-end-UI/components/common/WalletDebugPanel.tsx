"use client";

import { useState, useEffect } from 'react';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { useWallet } from '@/contexts/WalletContext';
import { solanaService } from '@/services/solanaService';
import { SOLANA_CLUSTER, SOLANA_NETWORK } from '@/lib/config';
import { Wallet, RefreshCw, Trash2, ExternalLink } from 'lucide-react';

export default function WalletDebugPanel() {
  const { connected, address, walletType, connect, disconnect, publicKey } = useWallet();
  const [serviceInitialized, setServiceInitialized] = useState(false);
  const [solanaStatus, setSolanaStatus] = useState<string>('Unknown');
  const [walletDetected, setWalletDetected] = useState<boolean>(false);
  const [phantomConnected, setPhantomConnected] = useState<boolean>(false);
  const [isCheckingStatus, setIsCheckingStatus] = useState<boolean>(false);

  // Function to get wallet icon based on wallet type
  const getWalletIcon = () => {
    if (!walletType) return <Wallet className="h-5 w-5" />;
    
    switch(walletType) {
      case "phantom":
        return (
          <div className="bg-purple-400 dark:bg-purple-500 rounded-full p-1 flex items-center justify-center">
            <svg width="16" height="16" viewBox="0 0 128 128" fill="none">
              <path d="M110.584 64.9142C110.584 63.0419 109.059 61.5172 107.187 61.5172H89.8091C87.937 61.5172 86.4121 63.0419 86.4121 64.9142V82.2921C86.4121 84.1642 87.937 85.689 89.8091 85.689H107.187C109.059 85.689 110.584 84.1642 110.584 82.2921V64.9142Z" fill="white"/>
              <path fillRule="evenodd" clipRule="evenodd" d="M27.8502 47.0453C27.8502 44.0913 30.2412 41.7003 33.1952 41.7003H95.6642C98.6182 41.7003 101.009 44.0913 101.009 47.0453V83.3913C101.009 86.3453 98.6182 88.7363 95.6642 88.7363H33.1952C30.2412 88.7363 27.8502 86.3453 27.8502 83.3913V47.0453ZM39.7502 57.7363C39.7502 55.5273 41.5412 53.7363 43.7502 53.7363H85.1092C87.3182 53.7363 89.1092 55.5273 89.1092 57.7363V72.7003C89.1092 74.9093 87.3182 76.7003 85.1092 76.7003H43.7502C41.5412 76.7003 39.7502 74.9093 39.7502 72.7003V57.7363Z" fill="white"/>
            </svg>
          </div>
        );
      case "solflare":
        return (
          <div className="bg-orange-500 rounded-full p-1 flex items-center justify-center">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
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

  useEffect(() => {
    // Check for wallet extensions
    if (typeof window !== 'undefined') {
      const phantomDetected = !!(window as any).solana?.isPhantom;
      setWalletDetected(phantomDetected);
      
      if (phantomDetected) {
        setPhantomConnected(!!(window as any).solana?.isConnected);
      }
    }
    
    // Check if solana service is initialized
    setServiceInitialized(solanaService?.isInitialized() || false);
    
    // Check Solana network status
    const checkConnection = async () => {
      setIsCheckingStatus(true);
      try {
        // Use solanaService connection to check RPC status
        const connection = solanaService.getConnection();
        const version = await connection.getVersion();
        setSolanaStatus(`Connected (Version: ${version['solana-core']})`);
      } catch (err: any) {
        setSolanaStatus(`Error: ${err?.message || 'Unknown error'}`);
      } finally {
        setIsCheckingStatus(false);
      }
    };
    
    checkConnection();
  }, [connected]);

  // Function to check raw wallet state
  const checkRawWalletState = () => {
    const phantom = (window as any).solana;
    
    if (!phantom) {
      alert('Phantom extension not detected');
      return;
    }
    
    // Log raw wallet state
    console.log('Raw Phantom state:', {
      isPhantom: phantom.isPhantom,
      isConnected: phantom.isConnected,
      publicKey: phantom.publicKey?.toString(),
      provider: phantom._handleDisconnect ? 'Phantom' : 'Unknown'
    });
    
    // Try to directly connect
    phantom.connect({ onlyIfTrusted: false })
      .then((res: any) => {
        console.log('Direct connect result:', res);
        alert(`Direct connection successful! Address: ${res.publicKey.toString()}`);
      })
      .catch((err: any) => {
        console.error('Direct connect error:', err);
        alert(`Direct connection error: ${err.message}`);
      });
  };

  // Function to truncate address for display
  const truncateAddress = (addr: string) => {
    return addr ? `${addr.slice(0, 8)}...${addr.slice(-8)}` : "";
  };

  return (
    <Card className="p-4 mb-4 bg-yellow-50 dark:bg-yellow-900/20 border-yellow-300">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-lg font-semibold">Wallet Debug Info</h3>
        {connected && (
          <div className="flex items-center space-x-1 text-green-600 dark:text-green-400">
            <div className="h-2 w-2 rounded-full bg-green-500"></div>
            <span className="text-xs">Connected</span>
          </div>
        )}
      </div>
      
      {connected ? (
        <div className="mb-4 p-3 bg-white dark:bg-slate-800 rounded-md border border-slate-200 dark:border-slate-700">
          <div className="flex items-center gap-2 mb-2">
            {getWalletIcon()}
            <span className="font-medium">{walletType}</span>
          </div>
          <div className="text-sm break-all bg-slate-50 dark:bg-slate-900 p-2 rounded border border-slate-200 dark:border-slate-800">
            {address || "Not available"}
          </div>
        </div>
      ) : (
        <div className="mb-4 p-3 bg-slate-100 dark:bg-slate-800 rounded-md border border-slate-200 dark:border-slate-700 text-center">
          <Wallet className="h-8 w-8 mx-auto text-slate-400 mb-1" />
          <p className="text-slate-600 dark:text-slate-400 text-sm">No wallet connected</p>
        </div>
      )}
      
      <div className="space-y-2 text-sm">
        <div className="flex justify-between items-center py-1 border-b dark:border-yellow-900/30">
          <span className="font-medium">Network:</span>
          <span className="text-right">{SOLANA_CLUSTER}</span>
        </div>
        
        <div className="flex justify-between items-center py-1 border-b dark:border-yellow-900/30">
          <span className="font-medium">Wallet Connected (Context):</span>
          <span className="text-right">{connected ? '✅' : '❌'}</span>
        </div>
        
        <div className="flex justify-between items-center py-1 border-b dark:border-yellow-900/30">
          <span className="font-medium">Phantom Detected:</span>
          <span className="text-right">{walletDetected ? '✅' : '❌'}</span>
        </div>
        
        <div className="flex justify-between items-center py-1 border-b dark:border-yellow-900/30">
          <span className="font-medium">Phantom Connected (Raw):</span>
          <span className="text-right">{phantomConnected ? '✅' : '❌'}</span>
        </div>
        
        <div className="flex justify-between items-center py-1 border-b dark:border-yellow-900/30">
          <span className="font-medium">PublicKey:</span>
          <span className="text-right">{publicKey ? truncateAddress(publicKey.toString()) : 'None'}</span>
        </div>
        
        <div className="flex justify-between items-center py-1 border-b dark:border-yellow-900/30">
          <span className="font-medium">Solana Service Initialized:</span>
          <span className="text-right">{serviceInitialized ? '✅' : '❌'}</span>
        </div>
        
        <div className="flex justify-between items-center py-1 border-b dark:border-yellow-900/30">
          <span className="font-medium">Solana RPC Status:</span>
          <span className="text-right flex items-center">
            {isCheckingStatus ? (
              <RefreshCw size={14} className="animate-spin mr-1" />
            ) : null}
            {solanaStatus}
          </span>
        </div>
        
        <div className="pt-3 flex flex-wrap gap-2">
          <Button 
            variant="outline" 
            size="sm"
            onClick={() => {
              // Force reconnection
              disconnect();
              setTimeout(() => connect('phantom'), 500);
            }}
            className="flex items-center gap-1"
          >
            <RefreshCw size={14} />
            Force Reconnect
          </Button>
          
          <Button 
            variant="outline" 
            size="sm"
            onClick={() => {
              // Clear storage and refresh
              localStorage.removeItem('walletType');
              localStorage.removeItem('walletAddress');
              window.location.reload();
            }}
            className="flex items-center gap-1"
          >
            <Trash2 size={14} />
            Clear Storage & Refresh
          </Button>
          
          <Button
            variant="outline"
            size="sm"
            onClick={checkRawWalletState}
            className="flex items-center gap-1"
          >
            <ExternalLink size={14} />
            Direct Phantom Check
          </Button>
        </div>
      </div>
    </Card>
  );
}
