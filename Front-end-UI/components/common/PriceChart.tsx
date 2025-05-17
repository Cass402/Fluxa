"use client";

import { useMemo, useState } from "react";
import { 
  AreaChart, 
  Area, 
  LineChart, 
  Line, 
  XAxis, 
  YAxis, 
  Tooltip, 
  ResponsiveContainer, 
  Legend,
  CartesianGrid,
  ReferenceLine,
  ReferenceArea,
  Brush,
} from "recharts";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/lib/utils";

// Chart timeframe options
const TIMEFRAME_OPTIONS = [
  { value: "1H", label: "1H" },
  { value: "1D", label: "1D" },
  { value: "1W", label: "1W" },
  { value: "1M", label: "1M" },
  { value: "ALL", label: "All" },
];

interface PriceChartProps {
  data?: any[];
  tokenPair?: {
    token0: string;
    token1: string;
  };
  className?: string;
}

// CustomTooltip component for better styling and more information
const CustomTooltip = ({ active, payload, label }: any) => {
  if (active && payload && payload.length) {
    const hasVolume = payload.some((p: any) => p.dataKey === 'volume');
    const hasPrice = payload.some((p: any) => p.dataKey === 'price');
    
    return (
      <div className="bg-background/90 border rounded-md shadow-md p-3 backdrop-blur-sm">
        <p className="text-sm text-muted-foreground">{label}</p>
        {hasPrice && (
          <p className="text-base font-semibold text-foreground">
            Price: ${payload.find((p: any) => p.dataKey === 'price')?.value?.toLocaleString() || '0.00'}
          </p>
        )}
        {hasVolume && (
          <p className="text-sm text-primary">
            Volume: ${payload.find((p: any) => p.dataKey === 'volume')?.value?.toLocaleString() || '0.00'}
          </p>
        )}
      </div>
    );
  }

  return null;
};

export default function PriceChart({ data = [], tokenPair, className }: PriceChartProps) {
  // State for chart timeframe
  const [timeframe, setTimeframe] = useState("1D");
  
  // Generate sample data if none provided
  const chartData = useMemo(() => {
    if (data && data.length > 0) return data;
    
    // Generate mock data if no data provided
    return Array.from({ length: 24 }, (_, i) => {
      const hour = i.toString().padStart(2, '0');
      const basePrice = 25 + Math.random() * 5;
      const volume = 50000 + Math.random() * 30000;
      
      return {
        name: `${hour}:00`,
        price: parseFloat(basePrice.toFixed(2)),
        volume: Math.round(volume),
      };
    });
  }, [data]);
  
  // Filter data based on selected timeframe
  const filteredData = useMemo(() => {
    if (timeframe === "ALL") return chartData;
    
    let startIdx: number;
    switch (timeframe) {
      case "1H":
        startIdx = Math.max(0, chartData.length - 12);
        break;
      case "1D":
        startIdx = Math.max(0, chartData.length - 24);
        break;
      case "1W":
        startIdx = Math.max(0, chartData.length - 7 * 24);
        break;
      case "1M":
        startIdx = Math.max(0, chartData.length - 30 * 24);
        break;
      default:
        startIdx = 0;
    }
    
    return chartData.slice(startIdx);
  }, [chartData, timeframe]);
  
  // Get min and max values for better axis scaling
  const dataStats = useMemo(() => {
    if (!filteredData.length) return { minPrice: 0, maxPrice: 0, minVolume: 0, maxVolume: 0 };
    
    const prices = filteredData.map(d => d.price).filter(Boolean);
    const volumes = filteredData.map(d => d.volume).filter(Boolean);
    
    return {
      minPrice: Math.floor(Math.min(...prices) * 0.95),
      maxPrice: Math.ceil(Math.max(...prices) * 1.05),
      minVolume: 0,
      maxVolume: Math.ceil(Math.max(...volumes) * 1.1),
    };
  }, [filteredData]);
  
  // Format to make Y-axis more readable
  const formatYAxis = (value: number) => {
    if (value >= 1000000) return `$${(value / 1000000).toFixed(1)}M`;
    if (value >= 1000) return `$${(value / 1000).toFixed(1)}K`;
    return `$${value}`;
  };
  
  return (
    <Card className={cn("h-full", className)}>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <CardTitle className="text-md">
            {tokenPair 
              ? `${tokenPair.token0}/${tokenPair.token1} Price` 
              : "Price Chart"}
          </CardTitle>
          <div className="flex space-x-1">
            {TIMEFRAME_OPTIONS.map((option) => (
              <Button 
                key={option.value}
                variant={timeframe === option.value ? "default" : "outline"}
                size="sm"
                onClick={() => setTimeframe(option.value)}
                className="h-7 px-2 text-xs"
              >
                {option.label}
              </Button>
            ))}
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <div className="h-[300px] w-full">
          <ResponsiveContainer width="100%" height="100%">
            <LineChart
              data={filteredData}
              margin={{ top: 5, right: 30, left: 10, bottom: 25 }}
            >
              <defs>
                <linearGradient id="colorPrice" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="hsl(var(--primary))" stopOpacity={0.8}/>
                  <stop offset="95%" stopColor="hsl(var(--primary))" stopOpacity={0}/>
                </linearGradient>
                <linearGradient id="colorVolume" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="hsl(var(--secondary))" stopOpacity={0.8}/>
                  <stop offset="95%" stopColor="hsl(var(--secondary))" stopOpacity={0}/>
                </linearGradient>
              </defs>
              
              <CartesianGrid stroke="hsl(var(--border))" strokeDasharray="3 3" opacity={0.2} />
              
              <XAxis 
                dataKey="name" 
                stroke="hsl(var(--muted-foreground))"
                tick={{ fontSize: 12 }}
                tickLine={{ stroke: 'hsl(var(--border))' }}
                label={{ 
                  value: 'Time', 
                  position: 'insideBottomRight', 
                  offset: -10,
                  fill: 'hsl(var(--muted-foreground))'
                }}
              />
              
              <YAxis 
                yAxisId="price"
                stroke="hsl(var(--muted-foreground))"
                tick={{ fontSize: 12 }}
                tickLine={{ stroke: 'hsl(var(--border))' }}
                label={{ 
                  value: 'Price ($)', 
                  angle: -90, 
                  position: 'insideLeft',
                  fill: 'hsl(var(--muted-foreground))'
                }}
                domain={[dataStats.minPrice, dataStats.maxPrice]}
                tickFormatter={(value) => `$${value}`}
              />
              
              <YAxis 
                yAxisId="volume"
                orientation="right"
                stroke="hsl(var(--muted-foreground))"
                tick={{ fontSize: 12 }}
                tickLine={{ stroke: 'hsl(var(--border))' }}
                label={{ 
                  value: 'Volume', 
                  angle: 90, 
                  position: 'insideRight',
                  fill: 'hsl(var(--muted-foreground))'
                }}
                domain={[0, dataStats.maxVolume]}
                tickFormatter={formatYAxis}
              />
              
              <Tooltip content={<CustomTooltip />} />
              
              <Legend 
                verticalAlign="top" 
                height={36}
                wrapperStyle={{ paddingTop: 10 }}
              />
              
              <Line
                type="monotone"
                dataKey="price"
                stroke="hsl(var(--primary))"
                strokeWidth={2}
                dot={false}
                activeDot={{ r: 6 }}
                name="Price"
                yAxisId="price"
              />
              
              <Area
                type="monotone" 
                dataKey="volume" 
                stroke="hsl(var(--secondary))" 
                fill="url(#colorVolume)"
                strokeWidth={1}
                fillOpacity={0.3}
                name="Volume"
                yAxisId="volume"
              />
              
              {filteredData.length > 30 && (
                <Brush 
                  dataKey="name"
                  height={30}
                  stroke="hsl(var(--primary))"
                  fill="hsl(var(--background))"
                  startIndex={Math.max(0, filteredData.length - 24)}
                  tickFormatter={() => ''}
                />
              )}
              
              {/* Reference line for most recent price */}
              {filteredData.length > 0 && (
                <ReferenceLine
                  y={filteredData[filteredData.length - 1]?.price}
                  yAxisId="price"
                  stroke="hsl(var(--primary))"
                  strokeDasharray="3 3"
                  label={{ 
                    value: 'Current',
                    fill: 'hsl(var(--primary))',
                    position: 'right'
                  }}
                />
              )}
            </LineChart>
          </ResponsiveContainer>
        </div>
        <div className="text-xs text-muted-foreground text-center mt-4">
          Hover over the chart for detailed information. 
          {filteredData.length > 30 && " Use the brush below to zoom in on specific time periods."}
        </div>
      </CardContent>
    </Card>
  );
}