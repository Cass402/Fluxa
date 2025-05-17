"use client";

// Define global window types for wallet extensions
declare global {
  interface Window {
    solana?: {
      isPhantom?: boolean;
      isSolflare?: boolean;
      isConnected?: boolean;
      publicKey?: { toString(): string };
      connect(options?: { onlyIfTrusted?: boolean }): Promise<{ publicKey: { toString(): string } }>;
      disconnect(): Promise<void>;
    };
    solflare?: {
      isConnected?: boolean;
      publicKey?: { toString(): string };
      connect(): Promise<{ publicKey: { toString(): string } }>;
      disconnect(): Promise<void>;
    };
    ethereum?: {
      isMetaMask?: boolean;
      request(args: { method: string }): Promise<string[]>;
    };
  }
}

import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { 
  Wallet, 
  Copy, 
  ExternalLink, 
  LogOut, 
  ChevronDown, 
  Check, 
  ArrowRight,
  RefreshCw
} from "lucide-react";
import { useWallet } from "@/contexts/WalletContext";
import { useToast } from "@/hooks/use-toast";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";

export default function ConnectWalletButton() {
  const { connected, connecting, address, walletType, balance, connect, disconnect } = useWallet();
  const [showConnectModal, setShowConnectModal] = useState(false);
  const [showSuccess, setShowSuccess] = useState(false);
  const [connectionAnimation, setConnectionAnimation] = useState(false);
  const [forceUpdate, setForceUpdate] = useState(0); // Add a force update state
  const [isRefreshing, setIsRefreshing] = useState(false);
  const { toast } = useToast();
  
  // Function to copy address to clipboard
  const copyAddress = () => {
    if (address) {
      navigator.clipboard.writeText(address);
      toast({
        title: "Address copied",
        description: "Wallet address copied to clipboard",
      });
    }
  };
  
  // Function to truncate address for display
  const truncateAddress = (addr: string) => {
    return addr ? `${addr.slice(0, 6)}...${addr.slice(-4)}` : "";
  };
  
  // Function to manually refresh wallet connection
  const refreshConnection = async () => {
    if (!walletType) return;
    
    setIsRefreshing(true);
    try {
      await connect(walletType);
      toast({
        title: "Connection refreshed",
        description: "Wallet connection has been refreshed",
      });
    } catch (error: any) {
      toast({
        title: "Refresh failed",
        description: error.message || "Failed to refresh connection",
        variant: "destructive",
      });
    } finally {
      setIsRefreshing(false);
    }
  };

  // Show success animation when wallet connects
  useEffect(() => {
    if (connected && address) {
      setShowSuccess(true);
      setConnectionAnimation(true);
      const timer = setTimeout(() => {
        setShowSuccess(false);
        // Keep connection animation for longer
        setTimeout(() => {
          setConnectionAnimation(false);
        }, 2000);
      }, 3000); // Show success animation for 3 seconds
      return () => clearTimeout(timer);
    }
  }, [connected, address]);
  
  // Additional debug logging for wallet connection state
  useEffect(() => {
    console.log("[ConnectWalletButton] Connection state changed:", { 
      connected, 
      address, 
      walletType 
    });
    
    // Check for Phantom wallet connection specifically
    if (typeof window !== 'undefined' && window.solana) {
      const isActuallyConnected = window.solana.isConnected && !!window.solana.publicKey;
      console.log("[ConnectWalletButton] Phantom connection check:", {
        isPhantom: window.solana.isPhantom,
        isConnected: window.solana.isConnected,
        hasPublicKey: !!window.solana.publicKey,
        actualState: isActuallyConnected
      });
      
      // If there's a discrepancy, log it
      if (connected !== isActuallyConnected) {
        console.warn("[ConnectWalletButton] Connection state mismatch:", {
          contextState: connected,
          actualState: isActuallyConnected
        });
      }
    }
    
    // Force update the component to make sure UI reflects the current state
    setForceUpdate(prev => prev + 1);
  }, [connected, address, walletType]);

  const handleConnect = async (type: "phantom" | "solflare" | "metamask" | "walletconnect" | "coinbase") => {
    try {
      await connect(type);
      setShowConnectModal(false);
      toast({
        title: "Wallet connected",
        description: `Successfully connected to ${type} wallet`,
      });
    } catch (error: any) {
      toast({
        title: "Connection failed",
        description: error.message,
        variant: "destructive",
      });
    }
  };
  
  // Get wallet icon based on wallet type
  const getWalletIcon = () => {
    switch(walletType) {
      case "phantom":
        return <PhantomIcon />;
      case "solflare":
        return <SolflareIcon />;
      case "metamask":
        return <MetaMaskIcon />;
      case "walletconnect":
        return <WalletConnectIcon />;
      case "coinbase":
        return <CoinbaseWalletIcon />;
      default:
        return <Wallet className="h-4 w-4" />;
    }
  };

  // Style for the connected button animation
  const connectedButtonStyle = `
    @keyframes pulse {
      0% { box-shadow: 0 0 0 0 rgba(74, 222, 128, 0.6); }
      70% { box-shadow: 0 0 0 6px rgba(74, 222, 128, 0); }
      100% { box-shadow: 0 0 0 0 rgba(74, 222, 128, 0); }
    }
    
    @keyframes fadeInUp {
      0% { 
        opacity: 0;
        transform: translateY(10px);
      }
      100% { 
        opacity: 1;
        transform: translateY(0);
      }
    }
    
    @keyframes shine {
      0% { background-position: 0% 50%; }
      50% { background-position: 100% 50%; }
      100% { background-position: 0% 50%; }
    }
    
    .connected-pulse {
      animation: pulse 2s infinite;
      transition: all 0.3s ease;
      border-width: 2px;
    }
    
    .animate-fade-in-up {
      animation: fadeInUp 0.4s ease-out forwards;
    }
    
    .wallet-gradient {
      background: linear-gradient(90deg, #f0fdf4, #dcfce7, #bbf7d0, #dcfce7, #f0fdf4);
      background-size: 200% 100%;
      animation: shine 3s linear infinite;
    }
    
    .dark .wallet-gradient {
      background: linear-gradient(90deg, #052e16, #065f46, #064e3b, #065f46, #052e16);
      background-size: 200% 100%;
      animation: shine 3s linear infinite;
    }
  `;

  return (
    <>
      <style jsx>{connectedButtonStyle}</style>
      {connected ? (
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger asChild>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button 
                    variant="outline" 
                    className="gap-2 wallet-gradient border-green-500 connected-pulse transition-all duration-300 font-medium shadow-md hover:shadow-lg"
                    onClick={() => console.log("Connected button clicked, address:", address)}
                  >
                    <div className="flex items-center justify-center h-8 w-8 mr-2 animate-fade-in-up bg-white dark:bg-gray-800 rounded-full p-1 shadow-sm">
                      {getWalletIcon()}
                    </div>
                    <div className="h-2.5 w-2.5 rounded-full bg-green-500 animate-pulse mr-2"></div>
                    <span className="font-medium text-green-800 dark:text-green-200">{address ? truncateAddress(address) : "Address not available"}</span>
                    <ChevronDown className="h-4 w-4 ml-1" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="w-72">
                  <DropdownMenuLabel>
                    <div className="flex items-center gap-2">
                      <div className="flex items-center justify-center h-6 w-6">{getWalletIcon()}</div>
                      <span>Connected to {walletType || "unknown wallet"}</span>
                    </div>
                  </DropdownMenuLabel>
                  <DropdownMenuLabel className="text-xs break-all font-mono bg-muted p-2 rounded">{address || "No address"}</DropdownMenuLabel>
                  {balance !== null && (
                    <DropdownMenuLabel className="text-sm py-1">
                      <span className="font-medium">Balance:</span> {balance.toFixed(4)} SOL
                    </DropdownMenuLabel>
                  )}
                  <DropdownMenuSeparator />
                  <DropdownMenuItem onClick={copyAddress} className="cursor-pointer">
                    <Copy className="mr-2 h-4 w-4" />
                    Copy Address
                  </DropdownMenuItem>
                  <DropdownMenuItem 
                    onClick={refreshConnection} 
                    className="cursor-pointer"
                    disabled={isRefreshing}
                  >
                    <RefreshCw className={`mr-2 h-4 w-4 ${isRefreshing ? 'animate-spin' : ''}`} />
                    {isRefreshing ? 'Refreshing...' : 'Refresh Connection'}
                  </DropdownMenuItem>
                  <DropdownMenuItem className="cursor-pointer">
                    <ExternalLink className="mr-2 h-4 w-4" />
                    View on Explorer
                  </DropdownMenuItem>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem onClick={disconnect} className="text-red-500 cursor-pointer">
                    <LogOut className="mr-2 h-4 w-4" />
                    Disconnect
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </TooltipTrigger>
            <TooltipContent>
              <p className="font-mono text-xs">{address || "No address available"}</p>
              {balance !== null && <p className="text-xs mt-1">{balance.toFixed(4)} SOL</p>}
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>
      ) : (
        <Dialog open={showConnectModal} onOpenChange={setShowConnectModal}>
          <DialogTrigger asChild>
            <Button id="connect-wallet-btn" className="bg-gradient-to-r from-blue-500 to-purple-500 hover:from-blue-600 hover:to-purple-600 transition-all duration-300 shadow-md hover:shadow-lg">
              <Wallet className="mr-2 h-4 w-4" />
              Connect Wallet
            </Button>
          </DialogTrigger>
          <DialogContent className="sm:max-w-md">
            <DialogHeader>
              <DialogTitle>Connect your wallet</DialogTitle>
              <DialogDescription>
                Choose your wallet provider to connect to the app
              </DialogDescription>
            </DialogHeader>
            <div className="grid gap-4 py-4">
              {walletOptions.map((wallet) => (
                <Button
                  key={wallet.name}
                  variant="outline"
                  className={`flex items-center justify-between p-6 hover:bg-muted/50 hover:border-primary/50 transition-all duration-300 ${
                    connecting && walletType === wallet.type ? 'border-primary' : ''
                  }`}
                  onClick={() => handleConnect(wallet.type)}
                  disabled={connecting && walletType !== wallet.type}
                >
                  <div className="flex items-center gap-3">
                    <div className={`flex h-10 w-10 items-center justify-center rounded-full border ${
                      connecting && walletType === wallet.type ? 'border-primary border-2' : ''
                    }`}>
                      {wallet.icon}
                    </div>
                    <div className="flex flex-col">
                      <div className="text-lg font-medium">{wallet.name}</div>
                      {connecting && walletType === wallet.type && (
                        <div className="text-xs text-primary animate-pulse">Connecting...</div>
                      )}
                    </div>
                  </div>
                  {connecting && walletType === wallet.type ? (
                    <div className="h-5 w-5 rounded-full border-2 border-t-transparent border-primary animate-spin" />
                  ) : (
                    <ArrowRight className="h-5 w-5" />
                  )}
                </Button>
              ))}
            </div>
          </DialogContent>
        </Dialog>
      )}
      {showSuccess && (
        <div className="fixed bottom-4 right-4 p-4 bg-green-500 text-white rounded-lg shadow-lg z-50 animate-fade-in-up flex items-center gap-2">
          <div className="flex items-center justify-center h-6 w-6">{getWalletIcon()}</div>
          <div>
            <div className="font-bold">Wallet connected successfully!</div>
            <div className="text-sm">{address && truncateAddress(address)}</div>
          </div>
          <Check className="h-5 w-5 ml-2" />
        </div>
      )}
    </>
  );
}

const walletOptions = [
  {
    name: "Phantom",
    type: "phantom" as const,
    icon: <PhantomIcon />,
  },
  {
    name: "Solflare",
    type: "solflare" as const,
    icon: <SolflareIcon />,
  },
];

// Wallet Icons Components
function PhantomIcon() {
  return (
    <svg width="24" height="24" viewBox="0 0 128 128" fill="none">
      <rect width="128" height="128" rx="64" fill="#AB9FF2"/>
      <path d="M110.584 64.9142C110.584 63.0419 109.059 61.5172 107.187 61.5172H89.8091C87.937 61.5172 86.4121 63.0419 86.4121 64.9142V82.2921C86.4121 84.1642 87.937 85.689 89.8091 85.689H107.187C109.059 85.689 110.584 84.1642 110.584 82.2921V64.9142Z" fill="white"/>
      <path fillRule="evenodd" clipRule="evenodd" d="M27.8502 47.0453C27.8502 44.0913 30.2412 41.7003 33.1952 41.7003H95.6642C98.6182 41.7003 101.009 44.0913 101.009 47.0453V83.3913C101.009 86.3453 98.6182 88.7363 95.6642 88.7363H33.1952C30.2412 88.7363 27.8502 86.3453 27.8502 83.3913V47.0453ZM39.7502 57.7363C39.7502 55.5273 41.5412 53.7363 43.7502 53.7363H85.1092C87.3182 53.7363 89.1092 55.5273 89.1092 57.7363V72.7003C89.1092 74.9093 87.3182 76.7003 85.1092 76.7003H43.7502C41.5412 76.7003 39.7502 74.9093 39.7502 72.7003V57.7363Z" fill="white"/>
    </svg>
  );
}

function SolflareIcon() {
  return (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none">
      <path
        d="M12 24C18.6274 24 24 18.6274 24 12C24 5.37258 18.6274 0 12 0C5.37258 0 0 5.37258 0 12C0 18.6274 5.37258 24 12 24Z"
        fill="#FF6D41"
      />
      <path
        d="M16.2322 7H9.57747C9.12101 7 8.73227 7.38873 8.73227 7.8452C8.73227 8.30166 9.12101 8.6904 9.57747 8.6904H16.2322C16.6887 8.6904 17.0774 8.30166 17.0774 7.8452C17.0774 7.38873 16.6887 7 16.2322 7Z"
        fill="white"
      />
      <path
        d="M16.2322 10.4082H9.57747C9.12101 10.4082 8.73227 10.7969 8.73227 11.2534C8.73227 11.7098 9.12101 12.0986 9.57747 12.0986H16.2322C16.6887 12.0986 17.0774 11.7098 17.0774 11.2534C17.0774 10.7969 16.6887 10.4082 16.2322 10.4082Z"
        fill="white"
      />
      <path
        d="M15.126 13.8164H7.8452C7.38873 13.8164 7 14.2051 7 14.6616C7 15.1181 7.38873 15.5068 7.8452 15.5068H15.126C15.5825 15.5068 15.9712 15.1181 15.9712 14.6616C15.9712 14.2051 15.5825 13.8164 15.126 13.8164Z"
        fill="white"
      />
      <path
        d="M15.126 17.2247H7.8452C7.38873 17.2247 7 17.6134 7 18.0699C7 18.5263 7.38873 18.9151 7.8452 18.9151H15.126C15.5825 18.9151 15.9712 18.5263 15.9712 18.0699C15.9712 17.6134 15.5825 17.2247 15.126 17.2247Z"
        fill="white"
      />
    </svg>
  );
}

function MetaMaskIcon() {
  return (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none">
      <path
        d="M21.6 4.8L13.2 10.8L14.4 7.2L21.6 4.8Z"
        fill="#E17726"
      />
      <path
        d="M2.4 4.8L10.8 10.8L9.6 7.2L2.4 4.8Z"
        fill="#E27625"
      />
      <path
        d="M18 16.8L16.2 19.2L20.4 20.4L21.6 16.8H18Z"
        fill="#E27625"
      />
      <path
        d="M2.4 16.8L3.6 20.4L7.8 19.2L6 16.8H2.4Z"
        fill="#E27625"
      />
      <path
        d="M7.8 12L6.6 13.8L10.8 14.4L10.8 9.6L7.8 12Z"
        fill="#E27625"
      />
      <path
        d="M16.2 12L13.2 9.6V14.4L17.4 13.8L16.2 12Z"
        fill="#E27625"
      />
    </svg>
  );
}

function WalletConnectIcon() {
  return (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none">
      <path
        d="M6.6 9.6C9.6 6.6 14.4 6.6 17.4 9.6L18 10.2C18.3 10.5 18.3 11.1 18 11.4L16.8 12.6C16.5 12.9 16.2 12.9 15.9 12.6L15 11.7C13.2 9.9 10.8 9.9 9 11.7L8.1 12.6C7.8 12.9 7.5 12.9 7.2 12.6L6 11.4C5.7 11.1 5.7 10.5 6 10.2L6.6 9.6Z"
        fill="#3396FF"
      />
    </svg>
  );
}

function CoinbaseWalletIcon() {
  return (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none">
      <path
        d="M12 22C17.5228 22 22 17.5228 22 12C22 6.47715 17.5228 2 12 2C6.47715 2 2 6.47715 2 12C2 17.5228 6.47715 22 12 22Z"
        fill="#0052FF"
      />
      <path
        d="M12 6.5C9.125 6.5 6.5 9.125 6.5 12C6.5 14.875 9.125 17.5 12 17.5C14.875 17.5 17.5 14.875 17.5 12C17.5 9.125 14.875 6.5 12 6.5ZM9.5 12C9.5 13.375 10.625 14.5 12 14.5C13.375 14.5 14.5 13.375 14.5 12C14.5 10.625 13.375 9.5 12 9.5C10.625 9.5 9.5 10.625 9.5 12Z"
        fill="white"
      />
    </svg>
  );
}