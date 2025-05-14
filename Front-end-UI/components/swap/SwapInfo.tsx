import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { mockPoolStats } from "@/lib/mock-data";
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer } from "recharts";

export default function SwapInfo() {
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
        <CardHeader>
          <CardTitle className="text-md">Volume (7d)</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="h-[200px] w-full">
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart data={mockPoolStats.volumeData}>
                <defs>
                  <linearGradient id="colorVolume" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="hsl(var(--chart-1))" stopOpacity={0.8}/>
                    <stop offset="95%" stopColor="hsl(var(--chart-1))" stopOpacity={0}/>
                  </linearGradient>
                </defs>
                <XAxis dataKey="name" />
                <YAxis />
                <Tooltip
                  contentStyle={{
                    backgroundColor: "hsl(var(--card))",
                    border: "1px solid hsl(var(--border))",
                    borderRadius: "var(--radius)",
                  }}
                />
                <Area
                  type="monotone" 
                  dataKey="volume" 
                  stroke="hsl(var(--chart-1))" 
                  fillOpacity={1}
                  fill="url(#colorVolume)"
                />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}