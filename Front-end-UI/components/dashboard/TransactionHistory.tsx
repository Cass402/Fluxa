"use client";

import {
  ArrowDownUp,
  ArrowRight,
  ExternalLink,
  Filter,
  Search,
} from "lucide-react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { formatDistanceToNow } from "date-fns";
import { mockTransactions } from "@/lib/mock-data";
import { useState } from "react";
import TokenPair from "@/components/common/TokenPair";

export default function TransactionHistory() {
  const [filter, setFilter] = useState("all");
  const [searchQuery, setSearchQuery] = useState("");
  
  // Filter transactions based on type and search query
  const filteredTransactions = mockTransactions.filter((tx) => {
    const matchesType = filter === "all" || tx.type === filter;
    const matchesSearch = 
      searchQuery === "" ||
      tx.fromToken.symbol.toLowerCase().includes(searchQuery.toLowerCase()) ||
      tx.toToken.symbol.toLowerCase().includes(searchQuery.toLowerCase());
    
    return matchesType && matchesSearch;
  });

  return (
    <div className="space-y-4">
      <div className="flex flex-col sm:flex-row justify-between gap-4">
        <div className="relative flex-1">
          <Search className="absolute left-2 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search transactions"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="pl-8"
          />
        </div>
        <Select value={filter} onValueChange={setFilter}>
          <SelectTrigger className="w-[180px]">
            <SelectValue placeholder="Filter transactions" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All Transactions</SelectItem>
            <SelectItem value="swap">Swaps</SelectItem>
            <SelectItem value="add">Add Liquidity</SelectItem>
            <SelectItem value="remove">Remove Liquidity</SelectItem>
          </SelectContent>
        </Select>
      </div>
      
      <div className="rounded-md border overflow-hidden">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Type</TableHead>
              <TableHead>Details</TableHead>
              <TableHead>Time</TableHead>
              <TableHead>Status</TableHead>
              <TableHead className="text-right">Value</TableHead>
              <TableHead></TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {filteredTransactions.length > 0 ? (
              filteredTransactions.map((tx) => (
                <TableRow key={tx.hash}>
                  <TableCell>
                    <div className="flex items-center gap-2">
                      <div className="flex items-center justify-center w-8 h-8 rounded-full bg-primary/10">
                        <ArrowDownUp className="h-4 w-4 text-primary" />
                      </div>
                      <span className="capitalize">{tx.type}</span>
                    </div>
                  </TableCell>
                  <TableCell>
                    <div className="flex items-center gap-2">
                      <TokenPair 
                        token1={tx.fromToken} 
                        token2={tx.toToken}
                        size="sm" 
                      />
                      <div>
                        <div className="text-sm flex items-center">
                          {tx.fromAmount} {tx.fromToken.symbol}
                          <ArrowRight className="inline mx-1 h-3 w-3" />
                          {tx.toAmount} {tx.toToken.symbol}
                        </div>
                      </div>
                    </div>
                  </TableCell>
                  <TableCell>
                    {formatDistanceToNow(new Date(tx.timestamp), { addSuffix: true })}
                  </TableCell>
                  <TableCell>
                    <span className={
                      tx.status === "confirmed" 
                        ? "text-green-500" 
                        : tx.status === "pending" 
                        ? "text-yellow-500" 
                        : "text-red-500"
                    }>
                      {tx.status.charAt(0).toUpperCase() + tx.status.slice(1)}
                    </span>
                  </TableCell>
                  <TableCell className="text-right">
                    ${tx.valueUSD.toLocaleString()}
                  </TableCell>
                  <TableCell>
                    <Button variant="ghost" size="icon" className="h-8 w-8">
                      <ExternalLink className="h-4 w-4" />
                    </Button>
                  </TableCell>
                </TableRow>
              ))
            ) : (
              <TableRow>
                <TableCell colSpan={6} className="h-24 text-center">
                  No transactions found
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}