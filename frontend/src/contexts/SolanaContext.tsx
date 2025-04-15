"use client";

import {
  createContext,
  useContext,
  useState,
  useEffect,
  ReactNode,
} from "react";
import { Connection } from "@solana/web3.js";
import {
  useAnchorWallet,
  AnchorWallet,
  useConnection,
} from "@solana/wallet-adapter-react";

interface SolanaContextType {
  connection: Connection | null;
  wallet: AnchorWallet | null | undefined;
  isConnected: boolean;
  isLoading: boolean;
}

const SolanaContext = createContext<SolanaContextType>({
  connection: null,
  wallet: null,
  isConnected: false,
  isLoading: true,
});

export const useSolana = () => useContext(SolanaContext);

export const SolanaProvider = ({ children }: { children: ReactNode }) => {
  const { connection } = useConnection();
  const wallet = useAnchorWallet();
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    // Connection initialization logic
    setIsLoading(false);
  }, [connection, wallet]);

  return (
    <SolanaContext.Provider
      value={{
        connection,
        wallet,
        isConnected: !!wallet,
        isLoading,
      }}
    >
      {children}
    </SolanaContext.Provider>
  );
};
