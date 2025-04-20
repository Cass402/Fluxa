"use client";

import React from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
} from "@/components/ui/card";
import { Select } from "@/components/ui/select";
import { RangeSlider } from "@/components/liquidity/RangeSlider";
import { TokenPairSelector } from "@/components/liquidity/TokenPairSelector";
import { PriceChart } from "@/components/charts/PriceChart";

/**
 * Component Showcase Page
 * Demonstrates the Fluxa UI component library
 */
export default function ComponentsPage() {
  // Sample data for demonstration
  const sampleTokens = [
    {
      symbol: "SOL",
      name: "Solana",
      address: "sol-address",
      logo: "https://cryptologos.cc/logos/solana-sol-logo.png",
    },
    {
      symbol: "ETH",
      name: "Ethereum",
      address: "eth-address",
      logo: "https://cryptologos.cc/logos/ethereum-eth-logo.png",
    },
    {
      symbol: "USDC",
      name: "USD Coin",
      address: "usdc-address",
      logo: "https://cryptologos.cc/logos/usd-coin-usdc-logo.png",
    },
    {
      symbol: "BTC",
      name: "Bitcoin",
      address: "btc-address",
      logo: "https://cryptologos.cc/logos/bitcoin-btc-logo.png",
    },
  ];

  const selectOptions = [
    { value: "option1", label: "Option 1" },
    { value: "option2", label: "Option 2" },
    { value: "option3", label: "Option 3" },
  ];

  // Sample price data for chart
  const generateSamplePriceData = () => {
    const data = [];
    const now = Date.now();
    const day = 24 * 60 * 60 * 1000;

    for (let i = 0; i < 24; i++) {
      const basePrice = 100;
      const variation = Math.random() * 20 - 10;
      data.push({
        price: basePrice + variation,
        timestamp: now - ((24 - i) * day) / 24,
        volume: Math.random() * 100 + 50,
      });
    }

    return data;
  };

  // Define the format function outside the JSX to avoid serialization issues
  const formatDollarValue = (value: number) => `$${value.toFixed(2)}`;

  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">Fluxa UI Component Library</h1>

      <section className="mb-12">
        <h2 className="text-2xl font-semibold mb-4">Button Component</h2>
        <div className="flex flex-wrap gap-4">
          <Button variant="default">Default</Button>
          <Button variant="secondary">Secondary</Button>
          <Button variant="accent">Accent</Button>
          <Button variant="outline">Outline</Button>
          <Button variant="ghost">Ghost</Button>
          <Button variant="link">Link</Button>
          <Button variant="success">Success</Button>
          <Button variant="warning">Warning</Button>
          <Button variant="danger">Danger</Button>
          <Button disabled>Disabled</Button>
          <Button isLoading>Loading</Button>
        </div>
        <div className="mt-4 flex flex-wrap gap-4">
          <Button size="sm">Small</Button>
          <Button>Default</Button>
          <Button size="lg">Large</Button>
          <Button size="icon">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="M12 19V5M5 12l7-7 7 7" />
            </svg>
          </Button>
        </div>
      </section>

      <section className="mb-12">
        <h2 className="text-2xl font-semibold mb-4">Input Component</h2>
        <div className="space-y-4 max-w-md">
          <Input placeholder="Basic input" />
          <Input label="Labeled Input" placeholder="Enter some text" />
          <Input
            label="Input with Error"
            placeholder="Invalid input"
            error="This field is required"
          />
          <Input
            label="Input with Prefix"
            placeholder="Enter amount"
            prefixElement={<span className="text-gray-500">$</span>}
          />
          <Input
            label="Input with Suffix"
            placeholder="Enter amount"
            suffixElement={<span className="text-gray-500">SOL</span>}
          />
        </div>
      </section>

      <section className="mb-12">
        <h2 className="text-2xl font-semibold mb-4">Card Component</h2>
        <div className="grid md:grid-cols-2 gap-6">
          <Card>
            <CardHeader>
              <CardTitle>Default Card</CardTitle>
              <CardDescription>This is a basic card component</CardDescription>
            </CardHeader>
            <CardContent>
              <p>
                Cards can contain any content, including text, images, and other
                components.
              </p>
            </CardContent>
            <CardFooter>
              <Button>Action</Button>
            </CardFooter>
          </Card>

          <Card variant="filled">
            <CardHeader>
              <CardTitle>Filled Card</CardTitle>
              <CardDescription>A card with a filled background</CardDescription>
            </CardHeader>
            <CardContent>
              <p>This card has a filled background style.</p>
            </CardContent>
            <CardFooter>
              <Button variant="secondary">Action</Button>
            </CardFooter>
          </Card>

          <Card variant="outline">
            <CardHeader>
              <CardTitle>Outline Card</CardTitle>
              <CardDescription>A card with an outline style</CardDescription>
            </CardHeader>
            <CardContent>
              <p>This card has an outline style.</p>
            </CardContent>
            <CardFooter>
              <Button variant="outline">Action</Button>
            </CardFooter>
          </Card>

          <Card variant="elevated" padding="lg">
            <CardHeader>
              <CardTitle>Elevated Card with Large Padding</CardTitle>
              <CardDescription>
                A card with elevation and larger padding
              </CardDescription>
            </CardHeader>
            <CardContent>
              <p>This card has elevation and larger padding.</p>
            </CardContent>
            <CardFooter>
              <Button variant="accent">Action</Button>
            </CardFooter>
          </Card>
        </div>
      </section>

      <section className="mb-12">
        <h2 className="text-2xl font-semibold mb-4">Select Component</h2>
        <div className="max-w-md space-y-4">
          <Select
            options={selectOptions}
            placeholder="Choose an option"
            label="Basic Select"
          />

          <Select
            options={sampleTokens.map((token) => ({
              value: token.address,
              label: token.symbol,
              icon: token.logo ? (
                <img
                  src={token.logo}
                  alt={token.symbol}
                  className="w-5 h-5 rounded-full"
                />
              ) : undefined,
            }))}
            placeholder="Select a token"
            label="Token Select"
          />
        </div>
      </section>

      <section className="mb-12">
        <h2 className="text-2xl font-semibold mb-4">Range Slider Component</h2>
        <div className="max-w-md">
          <RangeSlider
            minValue={0}
            maxValue={100}
            initialLowerValue={20}
            initialUpperValue={80}
            label="Price Range"
            formatValue={formatDollarValue}
          />
        </div>
      </section>

      <section className="mb-12">
        <h2 className="text-2xl font-semibold mb-4">
          Token Pair Selector Component
        </h2>
        <div className="max-w-xl">
          <TokenPairSelector availableTokens={sampleTokens} />
        </div>
      </section>

      <section className="mb-12">
        <h2 className="text-2xl font-semibold mb-4">Price Chart Component</h2>
        <div>
          <PriceChart
            priceData={generateSamplePriceData()}
            tokenSymbol="SOL/USDC"
            selectedRange={{ min: 95, max: 105 }}
            width={800}
            height={400}
          />
        </div>
      </section>
    </div>
  );
}
