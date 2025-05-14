import { ArrowRight } from "lucide-react";
import Link from "next/link";
import { buttonVariants } from "@/components/ui/button";

export default function HeroSection() {
  return (
    <section className="w-full min-h-[85vh] flex flex-col items-center justify-center py-12 md:py-24 lg:py-32 relative overflow-hidden">
      {/* Background gradient effects */}
      <div className="absolute inset-0 bg-gradient-to-b from-primary/5 via-background to-background pointer-events-none" />
      <div className="absolute -top-40 -left-40 w-80 h-80 bg-primary/10 rounded-full blur-3xl" />
      <div className="absolute -bottom-40 -right-40 w-80 h-80 bg-chart-2/10 rounded-full blur-3xl" />
      
      <div className="container relative px-4 md:px-6 flex flex-col items-center text-center space-y-10">
        <div className="space-y-4 max-w-[850px]">
          <h1 className="text-4xl font-bold tracking-tighter sm:text-5xl md:text-6xl lg:text-7xl">
            Trade and earn on the most{" "}
            <span className="bg-clip-text text-transparent bg-gradient-to-r from-chart-1 to-chart-2">
              efficient
            </span>{" "}
            DEX protocol
          </h1>
          <p className="mx-auto max-w-[700px] text-muted-foreground md:text-xl">
            Swap tokens, provide liquidity, and build your portfolio with our
            advanced trading platform and concentrated liquidity pools.
          </p>
        </div>
        <div className="flex flex-col sm:flex-row gap-4">
          <Link href="/swap" className={buttonVariants({ size: "lg" })}>
            Launch App
            <ArrowRight className="ml-2 h-4 w-4" />
          </Link>
          <Link
            href="/pools"
            className={buttonVariants({ variant: "outline", size: "lg" })}
          >
            Explore Pools
          </Link>
        </div>
        
        {/* Animated gradient card for visual interest */}
        <div className="relative w-full max-w-[900px] aspect-[16/9] sm:aspect-[21/9] mt-12 rounded-lg overflow-hidden border bg-card/30 backdrop-blur-sm p-1">
          <div className="absolute inset-0 bg-gradient-to-br from-chart-1/10 via-chart-2/10 to-chart-3/10 animate-gradient-xy" />
          <div className="absolute inset-0 bg-grid-white/[0.02] [mask-image:linear-gradient(to_bottom,white,transparent)]" />
          <div className="relative h-full rounded-md bg-black/40 p-6 flex items-center justify-center backdrop-blur-md">
            {/* Mock Trading Interface */}
            <div className="w-full max-w-[400px] aspect-[4/3] rounded-lg border border-white/10 bg-black/30 shadow-xl overflow-hidden">
              <div className="h-10 bg-background/90 border-b border-white/10 flex items-center px-4">
                <div className="w-3 h-3 rounded-full bg-red-500 mr-2" />
                <div className="w-3 h-3 rounded-full bg-yellow-500 mr-2" />
                <div className="w-3 h-3 rounded-full bg-green-500" />
              </div>
              <div className="p-4 space-y-4">
                <div className="h-32 bg-gradient-to-r from-chart-1/30 to-chart-2/30 rounded-md animate-pulse" />
                <div className="space-y-2">
                  <div className="h-8 bg-white/10 rounded-md w-full" />
                  <div className="h-8 bg-white/10 rounded-md w-3/4" />
                </div>
                <div className="h-10 bg-primary/80 rounded-md w-full mt-4" />
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}