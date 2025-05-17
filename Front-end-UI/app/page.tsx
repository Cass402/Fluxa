import Link from 'next/link';
import { buttonVariants } from '@/components/ui/button';
import { ArrowRight, Zap, Shield, LineChart, RefreshCw } from 'lucide-react';
import HeroSection from '@/components/home/HeroSection';
import StatsSection from '@/components/home/StatsSection';
import FeaturedPools from '@/components/home/FeaturedPools';

export default function Home() {
  return (
    <div className="flex flex-col items-center">
      <HeroSection />
      
      <section className="w-full py-12 md:py-24 lg:py-32 bg-gradient-to-b from-background to-background/80">
        <div className="container px-4 md:px-6">
          <div className="grid gap-6 lg:grid-cols-2 lg:gap-12 items-center">
            <div className="flex flex-col justify-center space-y-4">
              <div className="space-y-2">
                <h2 className="text-3xl font-bold tracking-tighter sm:text-5xl">
                  Swap tokens instantly
                </h2>
                <p className="max-w-[600px] text-muted-foreground md:text-xl/relaxed lg:text-base/relaxed xl:text-xl/relaxed">
                  Trade tokens with minimal slippage and low fees. Our smart order routing ensures you get the best price across multiple liquidity pools.
                </p>
              </div>
              <div className="flex flex-col gap-2 min-[400px]:flex-row">
                <Link href="/swap" className={buttonVariants({ size: "lg" })}>
                  Start swapping
                  <ArrowRight className="ml-2 h-4 w-4" />
                </Link>
              </div>
            </div>
            <div className="flex justify-center lg:justify-end">
              <div className="relative w-full max-w-[500px] aspect-video overflow-hidden rounded-xl bg-gradient-to-br from-primary/10 via-secondary/20 to-accent/10 p-1">
                <div className="absolute inset-0 bg-grid-white/5 [mask-image:linear-gradient(to_bottom,white,transparent)]" />
                <div className="relative flex h-full items-center justify-center rounded-lg bg-background/90 p-6 backdrop-blur-sm">
                  <div className="space-y-4 text-center">
                    <div className="inline-block rounded-full bg-primary/10 p-2 text-primary">
                      <Zap className="h-10 w-10" />
                    </div>
                    <h3 className="text-xl font-bold">Lightning Fast Swaps</h3>
                    <p className="text-sm text-muted-foreground">Enjoy seamless, instant trades with minimum slippage</p>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>
      
      <StatsSection />
      
      <section className="w-full py-12 md:py-24 lg:py-32">
        <div className="container px-4 md:px-6">
          <div className="flex flex-col items-center justify-center space-y-4 text-center">
            <div className="space-y-2">
              <h2 className="text-3xl font-bold tracking-tighter sm:text-5xl">
                Why Choose Fluxa
              </h2>
              <p className="max-w-[900px] text-muted-foreground md:text-xl/relaxed lg:text-base/relaxed xl:text-xl/relaxed">
                Our platform combines the best features of DeFi with a beautiful user experience
              </p>
            </div>
          </div>
          <div className="mx-auto grid max-w-5xl grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-3 mt-12">
            {features.map((feature, index) => (
              <div key={index} className="flex flex-col items-center space-y-4 rounded-lg border p-6 shadow-sm transition-all hover:shadow-md">
                <div className="rounded-full bg-primary/10 p-3 text-primary">
                  {feature.icon}
                </div>
                <h3 className="text-xl font-bold">{feature.title}</h3>
                <p className="text-center text-muted-foreground">{feature.description}</p>
              </div>
            ))}
          </div>
        </div>
      </section>
      
      <FeaturedPools />
    </div>
  );
}

const features = [
  {
    icon: <Shield className="h-6 w-6" />,
    title: "Secure & Non-Custodial",
    description: "Your funds stay in your wallet. We never take custody of your assets.",
  },
  {
    icon: <LineChart className="h-6 w-6" />,
    title: "Advanced Analytics",
    description: "Access detailed charts and data to make informed trading decisions.",
  },
  {
    icon: <RefreshCw className="h-6 w-6" />,
    title: "Capital Efficiency",
    description: "Our concentrated liquidity model maximizes returns for liquidity providers.",
  },
];