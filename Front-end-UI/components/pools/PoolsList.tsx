"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Search, ChevronDown, ChevronUp, Loader2 } from "lucide-react";
import TokenPair from "@/components/common/TokenPair";
import { usePools } from "@/hooks/use-solana-data";
import VirtualizedList from "@/components/common/VirtualizedList";

export default function PoolsList() {
  const [searchTerm, setSearchTerm] = useState("");
  const [sortField, setSortField] = useState<string>("tvl");
  const [sortDirection, setSortDirection] = useState<"asc" | "desc">("desc");
  const [feeTierFilter, setFeeTierFilter] = useState<string>("all");

  // Fetch pools from API using React Query
  const { data: pools = [], isLoading, isError } = usePools();

  // Filter pools based on search term and fee tier
  const filteredPools = pools.filter((pool: any) => {
    const searchMatch = 
      pool.token1.symbol.toLowerCase().includes(searchTerm.toLowerCase()) ||
      pool.token2.symbol.toLowerCase().includes(searchTerm.toLowerCase());
    
    const feeMatch = 
      feeTierFilter === "all" || 
      pool.feeTier.toString() === feeTierFilter;
    
    return searchMatch && feeMatch;
  });

  // Sort pools based on selected field and direction
  const sortedPools = [...filteredPools].sort((a, b) => {
    let aValue: any, bValue: any;
    
    switch (sortField) {
      case "feeTier":
        aValue = parseFloat(a.feeTier);
        bValue = parseFloat(b.feeTier);
        break;
      case "tvl":
        aValue = a.tvl;
        bValue = b.tvl;
        break;
      case "volume24h":
        aValue = a.volume24h;
        bValue = b.volume24h;
        break;
      case "apr":
        aValue = parseFloat(a.apr);
        bValue = parseFloat(b.apr);
        break;
      default:
        aValue = a.tvl;
        bValue = b.tvl;
    }
    
    if (sortDirection === "asc") {
      return aValue > bValue ? 1 : -1;
    } else {
      return aValue < bValue ? 1 : -1;
    }
  });

  // Handle sort toggle
  const handleSort = (field: string) => {
    if (sortField === field) {
      setSortDirection(sortDirection === "asc" ? "desc" : "asc");
    } else {
      setSortField(field);
      setSortDirection("desc");
    }
  };

  // Loading state
  if (isLoading) {
    return (
      <div className="w-full flex flex-col items-center justify-center py-12">
        <Loader2 className="h-10 w-10 animate-spin text-primary mb-4" />
        <p className="text-muted-foreground">Loading pools data...</p>
      </div>
    );
  }
  
  // Error state
  if (isError) {
    return (
      <div className="w-full flex flex-col items-center justify-center py-12">
        <p className="text-destructive mb-2">Failed to load pools</p>
        <Button variant="outline" onClick={() => window.location.reload()}>
          Retry
        </Button>
      </div>
    );
  }

  return (
    <div className="w-full space-y-6">
      <div className="flex flex-col md:flex-row gap-4 md:items-center justify-between mb-6">
        <div className="flex items-center gap-2 flex-1">
          <Search className="h-4 w-4 text-muted-foreground shrink-0" />
          <Input
            placeholder="Search by token..."
            className="max-w-sm"
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
          />
        </div>
        <div className="flex items-center gap-4">
          <Select
            value={feeTierFilter}
            onValueChange={setFeeTierFilter}
          >
            <SelectTrigger className="w-[180px]">
              <SelectValue placeholder="Fee tier" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All fee tiers</SelectItem>
              <SelectItem value="0.01">0.01%</SelectItem>
              <SelectItem value="0.05">0.05%</SelectItem>
              <SelectItem value="0.3">0.3%</SelectItem>
              <SelectItem value="1">1%</SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline">My Positions</Button>
        </div>
      </div>

      <div className="rounded-md border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Pool</TableHead>
              <TableHead 
                className="cursor-pointer"
                onClick={() => handleSort("feeTier")}
              >
                Fee tier
                {sortField === "feeTier" && (
                  sortDirection === "asc" ? 
                    <ChevronUp className="inline ml-1 h-4 w-4" /> : 
                    <ChevronDown className="inline ml-1 h-4 w-4" />
                )}
              </TableHead>
              <TableHead 
                className="cursor-pointer"
                onClick={() => handleSort("tvl")}
              >
                TVL
                {sortField === "tvl" && (
                  sortDirection === "asc" ? 
                    <ChevronUp className="inline ml-1 h-4 w-4" /> : 
                    <ChevronDown className="inline ml-1 h-4 w-4" />
                )}
              </TableHead>
              <TableHead 
                className="cursor-pointer"
                onClick={() => handleSort("volume24h")}
              >
                Volume 24h
                {sortField === "volume24h" && (
                  sortDirection === "asc" ? 
                    <ChevronUp className="inline ml-1 h-4 w-4" /> : 
                    <ChevronDown className="inline ml-1 h-4 w-4" />
                )}
              </TableHead>
              <TableHead 
                className="cursor-pointer"
                onClick={() => handleSort("apr")}
              >
                APR
                {sortField === "apr" && (
                  sortDirection === "asc" ? 
                    <ChevronUp className="inline ml-1 h-4 w-4" /> : 
                    <ChevronDown className="inline ml-1 h-4 w-4" />
                )}
              </TableHead>
              <TableHead className="text-right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          {/* Virtualized list only for large datasets */}
          {sortedPools.length > 50 ? (
            <div className="overflow-auto" style={{ height: '400px' }}>
              <VirtualizedList
                items={sortedPools}
                itemHeight={56}
                renderItem={(pool) => (
                  <TableRow key={pool.id}>
                    <TableCell>
                      <TokenPair 
                        token1={pool.token1}
                        token2={pool.token2}
                        size="sm"
                      />
                    </TableCell>
                    <TableCell>{pool.feeTier}%</TableCell>
                    <TableCell>${pool.tvl.toLocaleString()}</TableCell>
                    <TableCell>${pool.volume24h.toLocaleString()}</TableCell>
                    <TableCell>{pool.apr}%</TableCell>
                    <TableCell className="text-right space-x-2">
                      <Button size="sm" variant="outline">Swap</Button>
                      <Button size="sm">Add</Button>
                    </TableCell>
                  </TableRow>
                )}
                containerHeight={400}
              />
            </div>
          ) : (
            <TableBody>
              {sortedPools.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={6} className="h-24 text-center">
                    No pools found matching your search criteria
                  </TableCell>
                </TableRow>
              ) : (
                sortedPools.map((pool) => (
                  <TableRow key={pool.id}>
                    <TableCell>
                      <TokenPair 
                        token1={pool.token1}
                        token2={pool.token2}
                        size="sm"
                      />
                    </TableCell>
                    <TableCell>{pool.feeTier}%</TableCell>
                    <TableCell>${pool.tvl.toLocaleString()}</TableCell>
                    <TableCell>${pool.volume24h.toLocaleString()}</TableCell>
                    <TableCell>{pool.apr}%</TableCell>
                    <TableCell className="text-right space-x-2">
                      <Button size="sm" variant="outline">Swap</Button>
                      <Button size="sm">Add</Button>
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          )}
        </Table>
      </div>
    </div>
  );
}
