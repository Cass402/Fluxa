"use client";

import {
  createContext,
  useContext,
  useState,
  useEffect,
  ReactNode,
  useMemo,
  useCallback,
} from "react";
import { Connection, PublicKey } from "@solana/web3.js";
import { SOLANA_NETWORK, WALLET_CONNECTION_TIMEOUT_MS } from "@/lib/config";
import { solanaService } from "@/services/solanaService";
import { toast } from "@/hooks/use-toast";

// Define supported wallet types
type WalletType = "phantom" | "solflare" | "metamask" | "walletconnect" | "coinbase";

interface WalletContextType {
  connected: boolean;
  connecting: boolean;
  address: string | null;
  walletType: WalletType | null;
  balance: number | null;
  connect: (type: WalletType) => Promise<void>;
  disconnect: () => void;
  publicKey: PublicKey | null;
  sendTransaction: (transaction: any) => Promise<string>;
  signMessage: (message: Uint8Array) => Promise<Uint8Array>;
}

const WalletContext = createContext<WalletContextType>({
  connected: false,
  connecting: false,
  address: null,
  walletType: null,
  balance: null,
  connect: async () => {},
  disconnect: () => {},
  publicKey: null,
  sendTransaction: async () => "",
  signMessage: async () => new Uint8Array(),
});

export const useWallet = () => useContext(WalletContext);

interface WalletProviderProps {
  children: ReactNode;
}

export function WalletProvider({ children }: WalletProviderProps) {
  const [connected, setConnected] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [address, setAddress] = useState<string | null>(null);
  const [walletType, setWalletType] = useState<WalletType | null>(null);
  const [balance, setBalance] = useState<number | null>(null);
  const [wallet, setWallet] = useState<any>(null);
  const [publicKey, setPublicKey] = useState<PublicKey | null>(null);

  const connection = useMemo(() => new Connection(SOLANA_NETWORK, "confirmed"), []);

  // Initialize wallet from localStorage on page load
  useEffect(() => {
    const savedWalletType = localStorage.getItem("walletType") as WalletType | null;
    if (savedWalletType) {
      connect(savedWalletType).catch(console.error);
    }
  }, []);

  // Fetch balance when wallet is connected
  useEffect(() => {
    if (connected && publicKey) {
      const fetchBalance = async () => {
        try {
          const bal = await connection.getBalance(publicKey);
          setBalance(bal / 1_000_000_000); // Convert lamports to SOL
        } catch (error) {
          console.error("Error fetching balance:", error);
        }
      };

      fetchBalance();
      const intervalId = setInterval(fetchBalance, 30000); // Update every 30 seconds

      return () => clearInterval(intervalId);
    } else {
      setBalance(null);
    }
  }, [connected, publicKey, connection]);

  /**
   * Connect to wallet
   */
  const connect = useCallback(async (type: WalletType) => {
    try {
      setConnecting(true);
      
      let walletInstance;
      let connectionPromise;
      let timeoutId: NodeJS.Timeout;
      
      // Setup connection timeout
      const timeoutPromise = new Promise<never>((_, reject) => {
        timeoutId = setTimeout(() => {
          reject(new Error("Wallet connection timed out"));
        }, WALLET_CONNECTION_TIMEOUT_MS);
      });
      
      switch (type) {
        case "phantom": {
          // Check if Phantom is installed
          const { solana } = window as any;
          if (!solana?.isPhantom) {
            window.open("https://phantom.app/", "_blank");
            throw new Error("Please install Phantom wallet");
          }

          // Connect to Phantom
          walletInstance = solana;
          connectionPromise = solana.connect();
          break;
        }

        case "solflare": {
          // Check if Solflare is installed
          const { solflare } = window as any;
          if (!solflare) {
            window.open("https://solflare.com/", "_blank");
            throw new Error("Please install Solflare wallet");
          }

          // Connect to Solflare
          walletInstance = solflare;
          connectionPromise = solflare.connect();
          break;
        }

        case "metamask": {
          // Check if MetaMask is installed
          const { ethereum } = window as any;
          if (!ethereum?.isMetaMask) {
            window.open("https://metamask.io/download/", "_blank");
            throw new Error("Please install MetaMask");
          }

          // Connect to MetaMask
          walletInstance = {
            publicKey: null,
            async connect() {
              const accounts = await ethereum.request({ method: "eth_requestAccounts" });
              this.publicKey = new PublicKey("11111111111111111111111111111111"); // Placeholder
              return { publicKey: this.publicKey };
            },
            async disconnect() {
              this.publicKey = null;
            },
            async signTransaction(tx: any) {
              // Mock implementation
              return tx;
            },
            async signAllTransactions(txs: any[]) {
              // Mock implementation
              return txs;
            },
            async signMessage(message: Uint8Array) {
              // Mock implementation
              return message;
            },
          };
          connectionPromise = walletInstance.connect();
          break;
        }

        case "walletconnect":
        case "coinbase": {
          // Mock implementation for demo purposes
          walletInstance = {
            publicKey: new PublicKey("11111111111111111111111111111111"),
            async connect() {
              return { publicKey: this.publicKey };
            },
            async disconnect() {},
            async signTransaction(tx: any) { return tx; },
            async signAllTransactions(txs: any[]) { return txs; },
            async signMessage(message: Uint8Array) { return message; },
          };
          connectionPromise = Promise.resolve({ publicKey: walletInstance.publicKey });
          break;
        }

        default:
          throw new Error(`Wallet type ${type} not supported yet`);
      }

      // Race connection against timeout
      const response = await Promise.race([connectionPromise, timeoutPromise]);
      clearTimeout(timeoutId!);
      
      const publicKey = response.publicKey;
      const addressString = publicKey.toString();

      // Store in state and localStorage
      setWallet(walletInstance);
      setPublicKey(publicKey);
      setAddress(addressString);
      setWalletType(type);
      setConnected(true);
      
      localStorage.setItem("walletType", type);
      localStorage.setItem("walletAddress", addressString);

      // Initialize Solana service with the wallet
      solanaService.initialize({
        publicKey,
        signTransaction: walletInstance.signTransaction?.bind(walletInstance),
        signAllTransactions: walletInstance.signAllTransactions?.bind(walletInstance),
        signMessage: walletInstance.signMessage?.bind(walletInstance),
        sendTransaction: async (transaction: any) => {
          const signed = await walletInstance.signTransaction(transaction);
          const signature = await connection.sendRawTransaction(signed.serialize());
          return signature;
        },
      });

      toast({
        title: "Wallet Connected",
        description: `Connected to ${type} wallet`,
      });
      
    } catch (error: any) {
      console.error("Wallet connection error:", error);
      
      toast({
        title: "Connection Failed",
        description: error.message || "Could not connect to wallet",
        variant: "destructive",
      });
      
      disconnect();
    } finally {
      setConnecting(false);
    }
  }, [connection]);

  /**
   * Disconnect wallet
   */
  const disconnect = useCallback(() => {
    if (wallet && wallet.disconnect) {
      try {
        wallet.disconnect();
      } catch (error) {
        console.error("Error disconnecting wallet:", error);
      }
    }
    
    setWallet(null);
    setPublicKey(null);
    setAddress(null);
    setWalletType(null);
    setConnected(false);
    setBalance(null);
    
    localStorage.removeItem("walletType");
    localStorage.removeItem("walletAddress");
    
    toast({
      title: "Wallet Disconnected",
      description: "Your wallet has been disconnected",
    });
  }, [wallet]);

  /**
   * Send a transaction
   */
  const sendTransaction = useCallback(async (transaction: any): Promise<string> => {
    if (!wallet || !connected) {
      throw new Error("Wallet not connected");
    }
    
    try {
      const signed = await wallet.signTransaction(transaction);
      const signature = await connection.sendRawTransaction(signed.serialize());
      return signature;
    } catch (error) {
      console.error("Error sending transaction:", error);
      throw error;
    }
  }, [wallet, connected, connection]);

  /**
   * Sign a message
   */
  const signMessage = useCallback(async (message: Uint8Array): Promise<Uint8Array> => {
    if (!wallet || !connected) {
      throw new Error("Wallet not connected");
    }
    
    try {
      const signature = await wallet.signMessage(message);
      return signature;
    } catch (error) {
      console.error("Error signing message:", error);
      throw error;
    }
  }, [wallet, connected]);

  // Context value
  const value = useMemo(() => ({
    connected,
    connecting,
    address,
    walletType,
    balance,
    connect,
    disconnect,
    publicKey,
    sendTransaction,
    signMessage,
  }), [
    connected,
    connecting,
    address,
    walletType,
    balance,
    connect,
    disconnect,
    publicKey,
    sendTransaction,
    signMessage,
  ]);

  return (
    <WalletContext.Provider value={value}>
      {children}
    </WalletContext.Provider>
  );
}