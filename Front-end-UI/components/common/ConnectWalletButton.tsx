"use client";

import { useState } from "react";
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
import { Wallet, Copy, ExternalLink, LogOut, ChevronDown } from "lucide-react";
import { useWallet } from "@/contexts/WalletContext";
import { useToast } from "@/hooks/use-toast";

declare global {
  interface Window {
    solana?: {
      isPhantom?: boolean;
      isSolflare?: boolean;
      connect(): Promise<{ publicKey: { toString(): string } }>;
    };
    ethereum?: {
      isMetaMask?: boolean;
      request(args: { method: string }): Promise<string[]>;
    };
  }
}

export default function ConnectWalletButton() {
  const { connected, address, walletType, connect, disconnect } = useWallet();
  const [showConnectModal, setShowConnectModal] = useState(false);
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

  return (
    <>
      {connected ? (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" className="gap-2">
              <div className="h-4 w-4 rounded-full bg-green-500"></div>
              {truncateAddress(address || "")}
              <ChevronDown className="h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuLabel>Connected to {walletType}</DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={copyAddress}>
              <Copy className="mr-2 h-4 w-4" />
              Copy Address
            </DropdownMenuItem>
            <DropdownMenuItem>
              <ExternalLink className="mr-2 h-4 w-4" />
              View on Explorer
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={disconnect} className="text-red-500">
              <LogOut className="mr-2 h-4 w-4" />
              Disconnect
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      ) : (
        <Dialog open={showConnectModal} onOpenChange={setShowConnectModal}>
          <DialogTrigger asChild>
            <Button>
              <Wallet className="mr-2 h-4 w-4" />
              Connect
            </Button>
          </DialogTrigger>
          <DialogContent>
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
                  className="flex items-center justify-between p-6"
                  onClick={() => handleConnect(wallet.type)}
                >
                  <div className="flex items-center gap-3">
                    <div className="flex h-10 w-10 items-center justify-center rounded-full border">
                      {wallet.icon}
                    </div>
                    <div className="text-lg font-medium">{wallet.name}</div>
                  </div>
                  <ArrowRight className="h-5 w-5" />
                </Button>
              ))}
            </div>
          </DialogContent>
        </Dialog>
      )}
    </>
  );
}

// Import icons
import { ArrowRight } from "lucide-react";

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