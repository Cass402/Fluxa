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
import { SOLANA_NETWORK, WALLET_CONNECTION_TIMEOUT_MS, STORAGE_KEYS } from "@/lib/config";
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
  serviceInitialized: boolean;
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
  serviceInitialized: false,
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
  const [serviceConnectionStatus, setServiceConnectionStatus] = useState(false);

  const connection = useMemo(() => new Connection(SOLANA_NETWORK, "confirmed"), []);

  // Check for all available wallet types in window object
  const detectAvailableWallets = useCallback(() => {
    const wallets: Record<WalletType, boolean> = {
      phantom: false,
      solflare: false,
      metamask: false,
      walletconnect: false,
      coinbase: false
    };
    
    if (typeof window !== 'undefined') {
      const { solana } = window as any;
      wallets.phantom = !!solana?.isPhantom;
      wallets.solflare = !!solana?.isSolflare || !!(window as any).solflare;
      wallets.metamask = !!(window as any).ethereum?.isMetaMask;
    }
    
    return wallets;
  }, []);

  // Initialize wallet from localStorage on page load
  useEffect(() => {
    const checkExistingConnection = async () => {
      try {
        console.log("[WalletContext] Checking for existing wallet connections...");
        const availableWallets = detectAvailableWallets();
        console.log("[WalletContext] Available wallets:", availableWallets);
        
        // Check if Phantom is already connected
        const { solana } = window as any;
        if (solana?.isPhantom && solana?.isConnected && solana?.publicKey) {
          console.log("[WalletContext] Detected existing Phantom connection:", solana.publicKey.toString());
          
          // Auto-connect using the detected connection
          try {
            await connect("phantom");
            return;
          } catch (error) {
            console.error("[WalletContext] Error auto-connecting to Phantom:", error);
          }
        }
        
        // Check if Solflare is connected
        const { solflare } = window as any;
        if (solflare?.isConnected && solflare?.publicKey) {
          console.log("[WalletContext] Detected existing Solflare connection:", solflare.publicKey.toString());
          
          try {
            await connect("solflare");
            return;
          } catch (error) {
            console.error("[WalletContext] Error auto-connecting to Solflare:", error);
          }
        }
        
        // Fall back to saved wallet type from local storage
        const savedWalletType = localStorage.getItem(STORAGE_KEYS.WALLET_TYPE) as WalletType | null;
        const lastConnected = localStorage.getItem(STORAGE_KEYS.LAST_CONNECTED);
        const connectionAge = lastConnected ? Date.now() - parseInt(lastConnected) : Infinity;
        
        // Only try to reconnect if the last connection was recent (within 1 day)
        if (savedWalletType && connectionAge < 24 * 60 * 60 * 1000) {
          console.log("[WalletContext] Reconnecting to saved wallet type:", savedWalletType);
          await connect(savedWalletType);
        }
      } catch (error) {
        console.error("[WalletContext] Error checking existing connections:", error);
      }
    };
    
    // Delay connection check slightly to ensure browser extension is loaded
    const timer = setTimeout(() => {
      checkExistingConnection();
    }, 500);
    
    return () => clearTimeout(timer);
  }, [detectAvailableWallets]);

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
          console.log("[WalletContext] Phantom detection:", { 
            detected: !!solana,
            isPhantom: solana?.isPhantom,
            isConnected: solana?.isConnected
          });
          
          if (!solana?.isPhantom) {
            window.open("https://phantom.app/", "_blank");
            throw new Error("Please install Phantom wallet");
          }

          // Connect to Phantom
          walletInstance = solana;
          
          // Make sure any previous connection attempt is handled
          if (solana.isConnected) {
            console.log("[WalletContext] Phantom already connected, trying to use existing connection");
            try {
              // First try using the existing connection
              if (solana.publicKey) {
                connectionPromise = Promise.resolve({ publicKey: solana.publicKey });
                break;
              }
              
              // If no public key, disconnect and reconnect
              await solana.disconnect();
              // Small delay to ensure disconnect is complete
              await new Promise(resolve => setTimeout(resolve, 300));
            } catch (e) {
              console.log("[WalletContext] Error disconnecting existing connection:", e);
            }
          }
          
          console.log("[WalletContext] Attempting to connect to Phantom");
          // Attempt connection with explicit options
          connectionPromise = solana.connect({ onlyIfTrusted: false })
            .then((response: any) => {
              console.log("[WalletContext] Phantom connection success:", response);
              return response;
            })
            .catch((error: Error) => {
              console.error("[WalletContext] Phantom connection error:", error);
              throw error;
            });
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
          
          // Try to use existing connection first
          if (solflare.isConnected && solflare.publicKey) {
            connectionPromise = Promise.resolve({ publicKey: solflare.publicKey });
          } else {
            connectionPromise = solflare.connect();
          }
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
      
      console.log("[WalletContext] Connection response:", response);
      
      if (!response || !response.publicKey) {
        throw new Error("Wallet connected but no public key was returned");
      }
      
      const publicKey = response.publicKey;
      console.log("[WalletContext] Public key object:", publicKey);
      
      const addressString = publicKey.toString();
      console.log("[WalletContext] Address string:", addressString);

      // Store in state and localStorage
      setWallet(walletInstance);
      setPublicKey(publicKey);
      setAddress(addressString);
      setWalletType(type);
      setConnected(true);
      
      localStorage.setItem(STORAGE_KEYS.WALLET_TYPE, type);
      localStorage.setItem(STORAGE_KEYS.WALLET_ADDRESS, addressString);
      localStorage.setItem(STORAGE_KEYS.LAST_CONNECTED, Date.now().toString());

      // Initialize Solana service with the wallet
      try {
        // Log the wallet instance to debug
        console.log("[WalletContext] Wallet instance methods:", Object.keys(walletInstance));
        
        // Make sure methods are available
        const hasSigning = !!walletInstance.signTransaction && 
                          typeof walletInstance.signTransaction === 'function';
        
        console.log("[WalletContext] Wallet has signing capabilities:", hasSigning);
        
        // Create a proper wallet adapter object with complete interface
        const walletAdapter = {
          publicKey,
          connected: true,
          signTransaction: walletInstance.signTransaction?.bind(walletInstance),
          signAllTransactions: walletInstance.signAllTransactions?.bind(walletInstance),
          signMessage: walletInstance.signMessage?.bind(walletInstance),
        };
        
        console.log("[WalletContext] Initializing Solana service with wallet:", addressString);
        
        // Initialize the Solana service with the wallet
        try {
          // This may throw if there's an issue with the program ID, RPC connection, or IDL
          solanaService.initialize(walletAdapter);
          // If we get here, service initialization was successful
          setServiceConnectionStatus(true);
        } catch (serviceError) {
          console.error("[WalletContext] Error initializing Solana service:", serviceError);
          console.log("[WalletContext] Continuing with wallet connection despite service initialization failure");
          
          // Still allow wallet to connect, even if service fails
          setServiceConnectionStatus(false);
          
          // Show toast notification
          toast({
            title: "Service Connection Warning",
            description: "Connected to wallet, but backend services may be limited.",
            variant: "destructive",
          });
        }
      } catch (error) {
        console.error("[WalletContext] Error initializing wallet adapter:", error);
        throw new Error("Failed to initialize wallet adapter");
      }

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
    
    localStorage.removeItem(STORAGE_KEYS.WALLET_TYPE);
    localStorage.removeItem(STORAGE_KEYS.WALLET_ADDRESS);
    
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
    serviceInitialized: serviceConnectionStatus,
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
    serviceConnectionStatus,
  ]);

  return (
    <WalletContext.Provider value={value}>
      {children}
    </WalletContext.Provider>
  );
}