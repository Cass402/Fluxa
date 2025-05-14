import Link from "next/link";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { buttonVariants } from "@/components/ui/button";
import { ArrowRight } from "lucide-react";
import { mockPools } from "@/lib/mock-data";
import TokenPair from "@/components/common/TokenPair";

export default function FeaturedPools() {
  return (
    <section className="w-full py-12 md:py-24 lg:py-32 bg-gradient-to-b from-background to-background/90">
      <div className="container px-4 md:px-6">
        <div className="flex flex-col items-center justify-center space-y-4 text-center">
          <div className="space-y-2">
            <h2 className="text-3xl font-bold tracking-tighter sm:text-4xl md:text-5xl">
              Featured Pools
            </h2>
            <p className="max-w-[800px] text-muted-foreground md:text-lg">
              Explore our top-performing liquidity pools and start earning fees today
            </p>
          </div>
        </div>
        
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 mt-12">
          {mockPools.slice(0, 3).map((pool) => (
            <Card key={pool.id} className="border bg-card/50 backdrop-blur-sm overflow-hidden">
              <CardHeader className="pb-3">
                <div className="flex items-center gap-2">
                  <TokenPair 
                    token1={pool.token1} 
                    token2={pool.token2} 
                    size="md" 
                  />
                  <CardTitle className="text-xl">
                    {pool.token1.symbol}/{pool.token2.symbol}
                  </CardTitle>
                </div>
              </CardHeader>
              <CardContent>
                <div className="space-y-4">
                  <div className="grid grid-cols-2 gap-4">
                    <div>
                      <p className="text-sm text-muted-foreground">TVL</p>
                      <p className="text-lg font-medium">${pool.tvl.toLocaleString()}</p>
                    </div>
                    <div>
                      <p className="text-sm text-muted-foreground">APR</p>
                      <p className="text-lg font-medium text-green-500">{pool.apr}%</p>
                    </div>
                    <div>
                      <p className="text-sm text-muted-foreground">24h Volume</p>
                      <p className="text-lg font-medium">${pool.volume24h.toLocaleString()}</p>
                    </div>
                    <div>
                      <p className="text-sm text-muted-foreground">Fee Tier</p>
                      <p className="text-lg font-medium">{pool.feeTier}%</p>
                    </div>
                  </div>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
        
        <div className="flex justify-center mt-12">
          <Link 
            href="/pools" 
            className={buttonVariants({ variant: "outline", size: "lg" })}
          >
            View all pools
            <ArrowRight className="ml-2 h-4 w-4" />
          </Link>
        </div>
      </div>
    </section>
  );
}