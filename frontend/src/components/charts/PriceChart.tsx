'use client';

import React, { useRef, useEffect } from "react";
import { cn } from "@/lib/utils";

export interface PriceDataPoint {
  price: number;
  timestamp: number;
  volume?: number;
}

export interface LiquidityDataPoint {
  price: number;
  liquidity: number;
}

export interface PriceChartProps {
  priceData: PriceDataPoint[];
  liquidityData?: LiquidityDataPoint[];
  selectedRange?: { min: number; max: number };
  width?: number;
  height?: number;
  showVolume?: boolean;
  tokenSymbol?: string;
  className?: string;
  timeframe?: "1H" | "1D" | "1W" | "1M" | "1Y";
  onTimeframeChange?: (timeframe: "1H" | "1D" | "1W" | "1M" | "1Y") => void;
}

/**
 * Price Chart component for visualizing price and liquidity data
 * This is a simplified version that would typically use a chart library like d3 or recharts
 */
export const PriceChart: React.FC<PriceChartProps> = ({
  priceData,
  liquidityData,
  selectedRange,
  width = 600,
  height = 300,
  showVolume = true,
  tokenSymbol = "Token",
  className,
  timeframe = "1D",
  onTimeframeChange
}) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  
  // Draw chart on canvas
  useEffect(() => {
    if (!canvasRef.current || priceData.length === 0) return;
    
    const canvas = canvasRef.current;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    
    // Clear canvas
    ctx.clearRect(0, 0, width, height);
    
    // Set up dimensions
    const padding = { top: 20, right: 20, bottom: 30, left: 50 };
    const chartWidth = width - padding.left - padding.right;
    const chartHeight = height - padding.top - padding.bottom;
    
    // Find min/max values
    const prices = priceData.map(p => p.price);
    const minPrice = Math.min(...prices) * 0.95;
    const maxPrice = Math.max(...prices) * 1.05;
    
    const timestamps = priceData.map(p => p.timestamp);
    const minTimestamp = Math.min(...timestamps);
    const maxTimestamp = Math.max(...timestamps);
    
    // Scale functions
    const scaleX = (timestamp: number) => 
      padding.left + ((timestamp - minTimestamp) / (maxTimestamp - minTimestamp)) * chartWidth;
      
    const scaleY = (price: number) => 
      height - padding.bottom - ((price - minPrice) / (maxPrice - minPrice)) * chartHeight;
    
    // Draw axes
    ctx.beginPath();
    ctx.strokeStyle = "#ccc";
    ctx.moveTo(padding.left, padding.top);
    ctx.lineTo(padding.left, height - padding.bottom);
    ctx.lineTo(width - padding.right, height - padding.bottom);
    ctx.stroke();
    
    // Draw price labels
    ctx.fillStyle = "#666";
    ctx.font = "10px sans-serif";
    ctx.textAlign = "right";
    ctx.textBaseline = "middle";
    
    [0.2, 0.4, 0.6, 0.8, 1].forEach(percent => {
      const price = minPrice + (maxPrice - minPrice) * percent;
      const y = scaleY(price);
      ctx.fillText(price.toFixed(2), padding.left - 5, y);
      
      // Draw horizontal grid line
      ctx.beginPath();
      ctx.strokeStyle = "#eee";
      ctx.moveTo(padding.left, y);
      ctx.lineTo(width - padding.right, y);
      ctx.stroke();
    });
    
    // Draw time labels
    ctx.textAlign = "center";
    ctx.textBaseline = "top";
    [0, 0.25, 0.5, 0.75, 1].forEach(percent => {
      const timestamp = minTimestamp + (maxTimestamp - minTimestamp) * percent;
      const x = scaleX(timestamp);
      const date = new Date(timestamp);
      ctx.fillText(
        timeframe === "1H" ? `${date.getHours()}:${date.getMinutes()}` :
        timeframe === "1D" ? `${date.getHours()}:00` :
        timeframe === "1W" ? date.toLocaleDateString(undefined, { weekday: 'short' }) :
        timeframe === "1M" ? date.toLocaleDateString(undefined, { day: '2-digit' }) :
        date.toLocaleDateString(undefined, { month: 'short' }),
        x, height - padding.bottom + 5
      );
      
      // Draw vertical grid line
      ctx.beginPath();
      ctx.strokeStyle = "#eee";
      ctx.moveTo(x, padding.top);
      ctx.lineTo(x, height - padding.bottom);
      ctx.stroke();
    });
    
    // Draw price line
    ctx.beginPath();
    ctx.strokeStyle = "#3B82F6"; // primary color
    ctx.lineWidth = 2;
    ctx.moveTo(scaleX(priceData[0].timestamp), scaleY(priceData[0].price));
    
    for (let i = 1; i < priceData.length; i++) {
      ctx.lineTo(scaleX(priceData[i].timestamp), scaleY(priceData[i].price));
    }
    ctx.stroke();
    
    // Draw area under price line
    ctx.lineTo(scaleX(priceData[priceData.length - 1].timestamp), height - padding.bottom);
    ctx.lineTo(scaleX(priceData[0].timestamp), height - padding.bottom);
    ctx.closePath();
    ctx.fillStyle = "rgba(59, 130, 246, 0.1)"; // primary color with transparency
    ctx.fill();
    
    // Draw selected range if provided
    if (selectedRange) {
      const rangeMinY = scaleY(selectedRange.min);
      const rangeMaxY = scaleY(selectedRange.max);
      
      ctx.fillStyle = "rgba(124, 58, 237, 0.1)"; // accent color with transparency
      ctx.fillRect(padding.left, rangeMaxY, chartWidth, rangeMinY - rangeMaxY);
      
      // Draw range labels
      ctx.fillStyle = "#7C3AED"; // accent color
      ctx.textAlign = "left";
      ctx.fillText(`Max: ${selectedRange.max.toFixed(2)}`, padding.left + 5, rangeMaxY - 5);
      ctx.fillText(`Min: ${selectedRange.min.toFixed(2)}`, padding.left + 5, rangeMinY + 15);
    }
    
    // Draw volume bars if requested
    if (showVolume && priceData.some(p => p.volume !== undefined)) {
      const volumes = priceData.map(p => p.volume || 0);
      const maxVolume = Math.max(...volumes);
      
      const volumeHeight = chartHeight * 0.15; // 15% of chart height for volume
      const volumeScaleY = (vol: number) => 
        height - padding.bottom - ((vol / maxVolume) * volumeHeight);
      
      for (let i = 0; i < priceData.length; i++) {
        const volume = priceData[i].volume || 0;
        if (volume === 0) continue;
        
        const x = scaleX(priceData[i].timestamp);
        const barWidth = chartWidth / priceData.length * 0.8;
        
        ctx.fillStyle = priceData[i].price > (priceData[i-1]?.price || priceData[i].price) 
          ? "rgba(16, 185, 129, 0.7)" // positive color (green)
          : "rgba(239, 68, 68, 0.7)"; // negative color (red)
        
        ctx.fillRect(
          x - barWidth/2, 
          volumeScaleY(volume), 
          barWidth, 
          height - padding.bottom - volumeScaleY(volume)
        );
      }
    }
    
  }, [priceData, liquidityData, selectedRange, width, height, showVolume, timeframe]);
  
  // Placeholder data for when no data is provided
  if (priceData.length === 0) {
    return (
      <div 
        className={cn("flex items-center justify-center border rounded-lg", className)}
        style={{ width, height }}
      >
        <p className="text-gray-400">No price data available</p>
      </div>
    );
  }
  
  return (
    <div className={cn("relative", className)}>
      <div className="flex justify-between items-center mb-2">
        <div className="font-medium">{tokenSymbol} Price Chart</div>
        
        <div className="flex space-x-1 text-sm">
          {["1H", "1D", "1W", "1M", "1Y"].map((tf) => (
            <button
              key={tf}
              className={cn(
                "px-2 py-1 rounded",
                timeframe === tf 
                  ? "bg-primary text-white" 
                  : "hover:bg-gray-100"
              )}
              onClick={() => onTimeframeChange?.(tf as any)}
            >
              {tf}
            </button>
          ))}
        </div>
      </div>
      
      <canvas 
        ref={canvasRef}
        width={width}
        height={height}
        className="w-full h-auto rounded-lg border border-gray-200"
      />
      
      <div className="mt-2 flex justify-between text-xs text-gray-500">
        <span>Last Price: {priceData[priceData.length - 1].price.toFixed(2)}</span>
        <span>
          24h Change: 
          {priceData.length > 1 && (
            <span 
              className={
                priceData[priceData.length - 1].price > priceData[0].price
                  ? "text-success ml-1"
                  : "text-critical ml-1"
              }
            >
              {(((priceData[priceData.length - 1].price / priceData[0].price) - 1) * 100).toFixed(2)}%
            </span>
          )}
        </span>
      </div>
    </div>
  );
};

export default PriceChart;