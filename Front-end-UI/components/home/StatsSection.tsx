"use client"

import { Card, CardContent } from "@/components/ui/card";
import { LineChart, BarChart, PieChart, ActivitySquare } from "lucide-react";
import CountUp from "@/components/ui/count-up";

export default function StatsSection() {
  // Define formatter function inside the component
  const numberFormatter = (value: number) => value.toLocaleString();
  
  return (
    <section className="w-full py-12 md:py-24 lg:py-32 bg-background">
      <div className="container px-4 md:px-6">
        <div className="flex flex-col items-center justify-center space-y-4 text-center">
          <div className="space-y-2 max-w-[800px]">
            <h2 className="text-3xl font-bold tracking-tighter sm:text-4xl md:text-5xl">
              Platform Statistics
            </h2>
            <p className="text-muted-foreground md:text-lg">
              Track our protocol metrics and performance across multiple networks
            </p>
          </div>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 md:gap-8 mt-12">
          {stats.map((stat, index) => (
            <Card key={index} className="border bg-card/50 backdrop-blur-sm overflow-hidden">
              <CardContent className="p-6 flex flex-col space-y-2">
                <div className="flex items-center space-x-2">
                  <div className="text-primary">{stat.icon}</div>
                  <h3 className="text-sm font-medium text-muted-foreground">
                    {stat.title}
                  </h3>
                </div>
                <div className="text-2xl md:text-3xl font-bold">
                  {stat.prefix}
                  <CountUp
                    value={stat.value}
                    duration={2.5}
                    formatter={numberFormatter}
                  />
                  {stat.suffix}
                </div>
                <p className="text-xs text-muted-foreground">{stat.subtitle}</p>
              </CardContent>
            </Card>
          ))}
        </div>
      </div>
    </section>
  );
}

const stats = [
  {
    title: "Total Value Locked",
    value: 2850000000,
    prefix: "$",
    suffix: "",
    subtitle: "Across all networks and pools",
    icon: <LineChart className="h-4 w-4" />,
  },
  {
    title: "24h Trading Volume",
    value: 436000000,
    prefix: "$",
    suffix: "",
    subtitle: "Last 24 hours trading activity",
    icon: <BarChart className="h-4 w-4" />,
  },
  {
    title: "Active Pools",
    value: 3200,
    prefix: "",
    suffix: "+",
    subtitle: "Liquidity pools across all chains",
    icon: <PieChart className="h-4 w-4" />,
  },
  {
    title: "Unique Users",
    value: 185000,
    prefix: "",
    suffix: "+",
    subtitle: "Total unique wallets",
    icon: <ActivitySquare className="h-4 w-4" />,
  },
];