'use client';

import { WalletMultiButton } from '@solana/wallet-adapter-react-ui';
import { useSolana } from '@/contexts/SolanaContext';
import { useState, useEffect } from 'react';

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
    <main className="flex min-h-screen flex-col items-center justify-between p-24">
      <div className="z-10 w-full max-w-5xl items-center justify-between font-mono text-sm flex">
        <p className="fixed left-0 top-0 flex w-full justify-center border-b border-gray-300 bg-gradient-to-b from-zinc-200 pb-6 pt-8 backdrop-blur-2xl dark:border-neutral-800 dark:bg-zinc-800/30 dark:from-inherit">
          Fluxa AMM Protocol
        </p>
        <div className="fixed bottom-0 left-0 flex h-48 w-full items-end justify-center bg-gradient-to-t from-white via-white dark:from-black dark:via-black">
          <div className="pointer-events-none flex place-items-center gap-2 p-8 lg:pointer-events-auto">
            <WalletMultiButton />
          </div>
        </div>
      </div>

      <div className="relative flex place-items-center">
        <h1 className="text-4xl font-bold">Fluxa</h1>
      </div>

      <div className="mb-32 grid text-center lg:mb-0 lg:w-full lg:max-w-5xl lg:grid-cols-3 lg:text-left">
        {isConnected ? (
          <div className="group rounded-lg border border-transparent px-5 py-4">
            <h2 className="mb-3 text-2xl font-semibold">
              Connected to Solana
            </h2>
            <p className="m-0 max-w-[30ch] text-sm opacity-50">
              Wallet: {wallet?.publicKey.toString().slice(0, 8)}...
            </p>
            {balance !== null && (
              <p className="m-0 max-w-[30ch] text-sm opacity-50">
                Balance: {balance.toFixed(4)} SOL
              </p>
            )}
          </div>
        ) : (
          <div className="group rounded-lg border border-transparent px-5 py-4">
            <h2 className="mb-3 text-2xl font-semibold">
              Connect Your Wallet
            </h2>
            <p className="m-0 max-w-[30ch] text-sm opacity-50">
              Please connect your Solana wallet to use the application.
            </p>
          </div>
        )}
      </div>
    </main>
  );
}