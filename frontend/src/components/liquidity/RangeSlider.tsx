'use client';

import React, { useState, useEffect, useRef } from "react";
import { cn } from "@/lib/utils";

export interface RangeSliderProps {
  minValue: number;
  maxValue: number;
  initialLowerValue?: number;
  initialUpperValue?: number;
  step?: number;
  onChange?: (lowerValue: number, upperValue: number) => void;
  formatValue?: (value: number) => string;
  label?: string;
  className?: string;
}

/**
 * Range Slider component for setting price ranges in concentrated liquidity positions
 */
export const RangeSlider = ({
  minValue,
  maxValue,
  initialLowerValue,
  initialUpperValue,
  step = 1,
  onChange,
  formatValue = (value) => value.toString(),
  label,
  className,
}: RangeSliderProps) => {
  const [lowerValue, setLowerValue] = useState(initialLowerValue || minValue);
  const [upperValue, setUpperValue] = useState(initialUpperValue || maxValue);
  
  const rangeRef = useRef<HTMLDivElement>(null);

  // Update internal state when prop values change
  useEffect(() => {
    if (initialLowerValue !== undefined) setLowerValue(initialLowerValue);
    if (initialUpperValue !== undefined) setUpperValue(initialUpperValue);
  }, [initialLowerValue, initialUpperValue]);

  // Calculate percentages for UI rendering
  const lowerPercentage = ((lowerValue - minValue) / (maxValue - minValue)) * 100;
  const upperPercentage = ((upperValue - minValue) / (maxValue - minValue)) * 100;
  
  const handleLowerChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = Math.min(Number(e.target.value), upperValue - step);
    setLowerValue(newValue);
    onChange?.(newValue, upperValue);
  };

  const handleUpperChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = Math.max(Number(e.target.value), lowerValue + step);
    setUpperValue(newValue);
    onChange?.(lowerValue, newValue);
  };

  return (
    <div className={cn("w-full", className)}>
      {label && (
        <label className="mb-2 block text-sm font-medium text-text-dark">
          {label}
        </label>
      )}
      
      <div className="px-2">
        <div className="flex justify-between mb-2 text-sm text-gray-600">
          <span>{formatValue(lowerValue)}</span>
          <span>{formatValue(upperValue)}</span>
        </div>
      </div>

      <div className="relative h-14" ref={rangeRef}>
        {/* Track background */}
        <div className="absolute h-2 w-full bg-gray-200 rounded-full top-4"></div>
        
        {/* Selected range */}
        <div 
          className="absolute h-2 bg-primary rounded-full top-4"
          style={{
            left: `${lowerPercentage}%`,
            width: `${upperPercentage - lowerPercentage}%`
          }}
        ></div>
        
        {/* Lower thumb */}
        <input
          type="range"
          min={minValue}
          max={maxValue}
          step={step}
          value={lowerValue}
          onChange={handleLowerChange}
          className="absolute w-full h-2 opacity-0 cursor-pointer z-10 top-4"
          aria-label="Lower bound"
        />
        
        {/* Upper thumb */}
        <input
          type="range"
          min={minValue}
          max={maxValue}
          step={step}
          value={upperValue}
          onChange={handleUpperChange}
          className="absolute w-full h-2 opacity-0 cursor-pointer z-20 top-4"
          aria-label="Upper bound"
        />
        
        {/* Visible thumbs */}
        <div 
          className="absolute z-30 w-4 h-4 bg-white border-2 border-primary rounded-full shadow-md top-3" 
          style={{ left: `calc(${lowerPercentage}% - 0.5rem)` }}
          aria-hidden="true"
        ></div>
        
        <div 
          className="absolute z-30 w-4 h-4 bg-white border-2 border-primary rounded-full shadow-md top-3" 
          style={{ left: `calc(${upperPercentage}% - 0.5rem)` }}
          aria-hidden="true"
        ></div>
      </div>

      <div className="flex justify-between mt-4 px-2">
        <div className="w-28">
          <input
            type="number"
            value={lowerValue}
            min={minValue}
            max={upperValue - step}
            step={step}
            onChange={(e) => {
              const val = Math.max(minValue, Math.min(upperValue - step, Number(e.target.value)));
              setLowerValue(val);
              onChange?.(val, upperValue);
            }}
            className="w-full h-8 px-2 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-1 focus:ring-primary"
            aria-label="Lower bound value"
          />
        </div>
        <div className="w-28">
          <input
            type="number"
            value={upperValue}
            min={lowerValue + step}
            max={maxValue}
            step={step}
            onChange={(e) => {
              const val = Math.min(maxValue, Math.max(lowerValue + step, Number(e.target.value)));
              setUpperValue(val);
              onChange?.(lowerValue, val);
            }}
            className="w-full h-8 px-2 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-1 focus:ring-primary"
            aria-label="Upper bound value"
          />
        </div>
      </div>
    </div>
  );
};

export default RangeSlider;