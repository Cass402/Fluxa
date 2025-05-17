"use client";

import { useState } from "react";
import SwapInterface from "@/components/swap/SwapInterface";
import SwapInfo from "@/components/swap/SwapInfo";
import PriceChart from "@/components/common/PriceChart";
import RecentTransactions from "@/components/swap/RecentTransactions";
import { Card } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

export default function SwapPage() {
  const [activeTab, setActiveTab] = useState("swap");

  return (
    <div className="container max-w-10xl px-4 py-8 md:px-6 md:py-12">
      <div className="grid gap-8 lg:grid-cols-[1fr_400px]">
        <div className="space-y-8">
          <div className="flex items-center justify-between">
            <h1 className="text-3xl font-bold tracking-tight">Swap</h1>
          </div>
          
          <Card className="overflow-hidden border bg-card/50 backdrop-blur-sm">
            <PriceChart />
          </Card>
          
          <Card className="border bg-card/50 backdrop-blur-sm">
            <Tabs defaultValue="history" className="w-full">
              <TabsList className="grid w-full grid-cols-2">
                <TabsTrigger value="history">Transaction History</TabsTrigger>
                <TabsTrigger value="stats">Pool Statistics</TabsTrigger>
              </TabsList>
              <TabsContent value="history" className="p-4">
                <RecentTransactions />
              </TabsContent>
              <TabsContent value="stats" className="p-4">
                <SwapInfo />
              </TabsContent>
            </Tabs>
          </Card>
        </div>
        
        <div className="flex flex-col space-y-6">
          <Card className="border bg-card/50 backdrop-blur-sm p-6">
            <SwapInterface />
          </Card>
        </div>
      </div>
    </div>
  );
}