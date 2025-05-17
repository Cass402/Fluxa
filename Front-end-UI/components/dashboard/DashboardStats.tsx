"use client"

import { Card, CardContent } from "@/components/ui/card";
import { LineChart, BarChart, PieChart, ActivitySquare } from "lucide-react";
import CountUp from "@/components/ui/count-up";
import { mockDashboardStats } from "@/lib/mock-data";

export default function DashboardStats() {
  // Define formatter function inside the component
  const numberFormatter = (value: number) => value.toLocaleString();
  
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
      {stats.map((stat, index) => (
        <Card key={index} className="border bg-card/50 backdrop-blur-sm">
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
                value={mockDashboardStats[stat.key] as number}
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
  );
}

const stats = [
  {
    title: "Portfolio Value",
    key: "portfolioValue",
    prefix: "$",
    suffix: "",
    subtitle: "Total value across all positions",
    icon: <LineChart className="h-4 w-4" />,
  },
  {
    title: "Total Earnings",
    key: "totalEarnings",
    prefix: "$",
    suffix: "",
    subtitle: "Fees and rewards earned",
    icon: <BarChart className="h-4 w-4" />,
  },
  {
    title: "Active Positions",
    key: "activePositions",
    prefix: "",
    suffix: "",
    subtitle: "Across all pools",
    icon: <PieChart className="h-4 w-4" />,
  },
  {
    title: "Total Transactions",
    key: "totalTransactions",
    prefix: "",
    suffix: "",
    subtitle: "All-time transaction count",
    icon: <ActivitySquare className="h-4 w-4" />,
  },
];