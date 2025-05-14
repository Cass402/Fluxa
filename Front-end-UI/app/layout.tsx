import './globals.css';
import type { Metadata } from 'next';
import { Inter } from 'next/font/google';
import { ThemeProvider } from '@/components/providers/ThemeProvider';
import { QueryClientProvider } from '@/components/providers/QueryClientProvider';
import { WalletProvider } from '@/contexts/WalletContext';
import { Toaster } from '@/components/ui/toaster';
import Header from '@/components/layout/Header';
import Footer from '@/components/layout/Footer';

const inter = Inter({ subsets: ['latin'] });

export const metadata: Metadata = {
  title: 'Fluxa | Decentralized Exchange',
  description: 'Trade and provide liquidity in decentralized crypto pools',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className={inter.className}>
        <ThemeProvider
          attribute="class"
          defaultTheme="dark"
          enableSystem
          disableTransitionOnChange
        >
          <QueryClientProvider>
            <WalletProvider>
              <div className="relative flex min-h-screen flex-col">
                <Header />
                <main className="flex-1 mx-auto w-full max-w-screen-2xl">{children}</main>
                <Footer />
              </div>
              <Toaster />
            </WalletProvider>
          </QueryClientProvider>
        </ThemeProvider>
      </body>
    </html>
  );
}