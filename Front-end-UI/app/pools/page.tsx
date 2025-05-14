"use client";

import { useState } from "react";
import { Card } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import PoolsList from "@/components/pools/PoolsList";
import AddLiquidity from "@/components/pools/AddLiquidity";
import RemoveLiquidity from "@/components/pools/RemoveLiquidity";
import UserPositions from "@/components/pools/UserPositions";

export default function PoolsPage() {
  return (
    <div className="container max-w-10xl px-4 py-8 md:px-6 md:py-12">
      <div className="space-y-8">
        <div className="flex items-center justify-between">
          <h1 className="text-3xl font-bold tracking-tight">Liquidity Pools</h1>
        </div>
        
        <Card className="border bg-card/50 backdrop-blur-sm overflow-hidden">
          <Tabs defaultValue="explore" className="w-full">
            <TabsList className="grid w-full grid-cols-4">
              <TabsTrigger value="explore">Explore Pools</TabsTrigger>
              <TabsTrigger value="add">Add Liquidity</TabsTrigger>
              <TabsTrigger value="remove">Remove Liquidity</TabsTrigger>
              <TabsTrigger value="positions">My Positions</TabsTrigger>
            </TabsList>
            <TabsContent value="explore" className="p-4 min-h-[500px]">
              <PoolsList />
            </TabsContent>
            <TabsContent value="add" className="p-4 min-h-[500px]">
              <AddLiquidity />
            </TabsContent>
            <TabsContent value="remove" className="p-4 min-h-[500px]">
              <RemoveLiquidity />
            </TabsContent>
            <TabsContent value="positions" className="p-4 min-h-[500px]">
              <UserPositions />
            </TabsContent>
          </Tabs>
        </Card>
      </div>
    </div>
  );
}