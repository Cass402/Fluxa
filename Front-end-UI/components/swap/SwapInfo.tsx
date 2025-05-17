import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { mockPoolStats } from "@/lib/mock-data";
import { useState } from "react";
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
import { cn } from "@/lib/utils";

// Chart timeframe options
const TIMEFRAME_OPTIONS = [
  { value: "1H", label: "1H" },
  { value: "1D", label: "1D" },
  { value: "1W", label: "1W" },
  { value: "1M", label: "1M" },
  { value: "ALL", label: "All" },
];

// CustomTooltip component for better styling and more information
const CustomTooltip = ({ active, payload, label }: any) => {
  if (active && payload && payload.length) {
    return (
      <div className="bg-background/90 border rounded-md shadow-md p-3 backdrop-blur-sm">
        <p className="text-sm text-muted-foreground">{label}</p>
        <p className="text-base font-semibold text-foreground">
          Volume: ${payload[0].value.toLocaleString()}
        </p>
        {payload[1] && (
          <p className="text-sm text-primary">
            Price: ${payload[1].value.toLocaleString()}
          </p>
        )}
      </div>
    );
  }

  return null;
};

export default function SwapInfo() {
  // State for chart timeframe
  const [timeframe, setTimeframe] = useState("1D");

  // Generate more realistic data for the chart
  const getChartData = () => {
    // Start with the mock data
    const baseData = [...mockPoolStats.volumeData];
    
    // Add price data to each entry
    return baseData.map(entry => ({
      ...entry,
      price: entry.volume * (0.8 + Math.random() * 0.4) / 1000, // Generate related but not identical price data
    }));
  };

  const chartData = getChartData();

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-2 gap-4">
        <Card className="border bg-card/50 backdrop-blur-sm">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm">TVL</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">${mockPoolStats.tvl.toLocaleString()}</div>
            <CardDescription className="text-xs flex items-center">
              <span className="text-green-500">+2.5%</span> (24h)
            </CardDescription>
          </CardContent>
        </Card>
        
        <Card className="border bg-card/50 backdrop-blur-sm">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm">24h Volume</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">${mockPoolStats.volume24h.toLocaleString()}</div>
            <CardDescription className="text-xs flex items-center">
              <span className="text-red-500">-0.8%</span> (24h)
            </CardDescription>
          </CardContent>
        </Card>
        
        <Card className="border bg-card/50 backdrop-blur-sm">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm">24h Fees</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">${mockPoolStats.fees24h.toLocaleString()}</div>
            <CardDescription className="text-xs flex items-center">
              <span className="text-green-500">+1.2%</span> (24h)
            </CardDescription>
          </CardContent>
        </Card>
        
        <Card className="border bg-card/50 backdrop-blur-sm">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm">Pool Count</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{mockPoolStats.poolCount.toLocaleString()}</div>
            <CardDescription className="text-xs flex items-center">
              <span className="text-green-500">+5</span> new today
            </CardDescription>
          </CardContent>
        </Card>
      </div>
      
      <Card className="border bg-card/50 backdrop-blur-sm">
        <CardHeader className="pb-2">
          <div className="flex items-center justify-between">
            <CardTitle className="text-md">Market Data</CardTitle>
            <div className="flex space-x-1">
              {TIMEFRAME_OPTIONS.map((option) => (
                <Button 
                  key={option.value}
                  variant={timeframe === option.value ? "default" : "outline"}
                  size="sm"
                  onClick={() => setTimeframe(option.value)}
                  className="h-8 px-3"
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
                data={chartData}
                margin={{ top: 5, right: 20, left: 10, bottom: 25 }}
              >
                <defs>
                  <linearGradient id="colorVolume" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="hsl(var(--chart-1))" stopOpacity={0.8}/>
                    <stop offset="95%" stopColor="hsl(var(--chart-1))" stopOpacity={0}/>
                  </linearGradient>
                  <linearGradient id="colorPrice" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="hsl(var(--chart-2))" stopOpacity={0.8}/>
                    <stop offset="95%" stopColor="hsl(var(--chart-2))" stopOpacity={0}/>
                  </linearGradient>
                </defs>
                
                <CartesianGrid stroke="hsl(var(--border))" strokeDasharray="3 3" opacity={0.2} />
                
                <XAxis 
                  dataKey="name" 
                  stroke="hsl(var(--muted-foreground))"
                  tick={{ fontSize: 12 }}
                  tickLine={{ stroke: 'hsl(var(--border))' }}
                  label={{ 
                    value: 'Date', 
                    position: 'insideBottomRight', 
                    offset: -10,
                    fill: 'hsl(var(--muted-foreground))'
                  }}
                />
                
                <YAxis 
                  stroke="hsl(var(--muted-foreground))"
                  tick={{ fontSize: 12 }}
                  tickLine={{ stroke: 'hsl(var(--border))' }}
                  label={{ 
                    value: 'Price ($)', 
                    angle: -90, 
                    position: 'insideLeft',
                    fill: 'hsl(var(--muted-foreground))'
                  }}
                  yAxisId="right"
                  orientation="right"
                  domain={['auto', 'auto']}
                />
                
                <YAxis 
                  stroke="hsl(var(--muted-foreground))"
                  tick={{ fontSize: 12 }}
                  tickLine={{ stroke: 'hsl(var(--border))' }}
                  label={{ 
                    value: 'Volume ($K)', 
                    angle: -90, 
                    position: 'insideLeft',
                    fill: 'hsl(var(--muted-foreground))'
                  }}
                  yAxisId="left"
                  orientation="left"
                  domain={['auto', 'auto']}
                />
                
                <Tooltip content={<CustomTooltip />} />
                
                <Legend 
                  verticalAlign="top" 
                  height={36}
                  wrapperStyle={{ paddingTop: 10 }}
                />
                
                <Area
                  type="monotone" 
                  dataKey="volume" 
                  stroke="hsl(var(--chart-1))" 
                  fillOpacity={0.3}
                  fill="url(#colorVolume)"
                  name="Volume"
                  yAxisId="left"
                  activeDot={{ r: 6 }}
                />
                
                <Line
                  type="monotone"
                  dataKey="price"
                  stroke="hsl(var(--chart-2))"
                  name="Price"
                  yAxisId="right"
                  dot={false}
                  activeDot={{ r: 6 }}
                />
                
                <Brush 
                  dataKey="name"
                  height={30}
                  stroke="hsl(var(--primary))"
                  fill="hsl(var(--background))"
                  startIndex={chartData.length - 5}
                />
                
                {/* Reference line for current price */}
                <ReferenceLine
                  y={chartData[chartData.length - 1]?.price}
                  yAxisId="right"
                  stroke="hsl(var(--primary))"
                  strokeDasharray="3 3"
                  label={{
                    value: 'Current',
                    fill: 'hsl(var(--primary))',
                    position: 'right'
                  }}
                />
              </LineChart>
            </ResponsiveContainer>
          </div>
          <div className="text-xs text-muted-foreground text-center mt-4">
            Hover over the chart for detailed information. Use the brush below to zoom in on specific time periods.
          </div>
        </CardContent>
      </Card>
    </div>
  );
}