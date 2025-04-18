'use client';

import { WalletMultiButton } from '@solana/wallet-adapter-react-ui';
import { useSolana } from '@/contexts/SolanaContext';
import { useState, useEffect } from 'react';
import Link from 'next/link';

export default function Home() {
  const { wallet, connection, isConnected } = useSolana();
  const [balance, setBalance] = useState<number | null>(null);

  useEffect(() => {
    const getBalance = async () => {
      if (wallet && connection) {
        try {
          const bal = await connection.getBalance(wallet.publicKey);
          setBalance(bal / 1000000000); // Convert lamports to SOL
        } catch (error) {
          console.error('Error fetching balance:', error);
        }
      }
    };

    if (isConnected) {
      getBalance();
    } else {
      setBalance(null);
    }
  }, [wallet, connection, isConnected]);

  return (
    <main className="flex min-h-screen flex-col items-center justify-center p-6 bg-gray-50 dark:bg-zinc-900">
      <div className="w-full max-w-md bg-white dark:bg-zinc-800 rounded-xl shadow-md overflow-hidden p-8">
        <div className="text-center mb-8">
          <h1 className="text-3xl font-bold bg-gradient-to-r from-primary to-accent inline-block text-transparent bg-clip-text">
            Fluxa
          </h1>
          <p className="text-gray-600 dark:text-gray-300 mt-2">
            Solana AMM Protocol
          </p>
        </div>

        <div className="flex justify-center mb-8">
          <WalletMultiButton className="!bg-accent hover:!bg-accent-dark transition-all" />
        </div>

        {isConnected && balance !== null && (
          <div className="bg-gray-50 dark:bg-zinc-700/30 p-4 rounded-lg text-center">
            <p className="text-gray-500 dark:text-gray-400 text-sm">Connected Wallet</p>
            <p className="font-mono text-sm truncate mt-1">
              {wallet?.publicKey.toString()}
            </p>
            <p className="font-bold mt-2 text-primary">
              {balance.toFixed(4)} SOL
            </p>
          </div>
        )}

        <div className="mt-8 text-center">
          <Link href="/components" className="text-accent underline text-sm">
            View Components
          </Link>
        </div>
      </div>
    </main>
  );
}