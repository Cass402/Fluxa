"use client";

import { useState, useEffect, useRef, useCallback } from "react";

interface UseVirtualizationProps<T> {
  items: T[];
  itemHeight: number;
  overscan?: number;
  containerHeight?: number;
}

interface UseVirtualizationResult<T> {
  containerRef: React.RefObject<HTMLDivElement>;
  virtualItems: {
    item: T;
    index: number;
    offsetTop: number;
  }[];
  totalHeight: number;
  scrollTo: (index: number) => void;
}

/**
 * A hook for virtualizing long lists
 *
 * @param items - The array of items to render
 * @param itemHeight - The fixed height of each item in pixels
 * @param overscan - Number of items to render before and after the visible area
 * @param containerHeight - Optional fixed height for the container
 */
export function useVirtualization<T>({
  items,
  itemHeight,
  overscan = 3,
  containerHeight,
}: UseVirtualizationProps<T>): UseVirtualizationResult<T> {
  const containerRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [height, setHeight] = useState(containerHeight || 0);

  useEffect(() => {
    if (containerHeight) {
      setHeight(containerHeight);
      return;
    }

    if (!containerRef.current) return;

    // Get actual container height if not specified
    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        if (entry.target === containerRef.current) {
          setHeight(entry.contentRect.height);
        }
      }
    });

    resizeObserver.observe(containerRef.current);
    return () => resizeObserver.disconnect();
  }, [containerHeight]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const handleScroll = () => {
      setScrollTop(container.scrollTop);
    };

    container.addEventListener("scroll", handleScroll);
    return () => container.removeEventListener("scroll", handleScroll);
  }, []);

  const scrollTo = useCallback(
    (index: number) => {
      if (!containerRef.current) return;
      containerRef.current.scrollTop = index * itemHeight;
    },
    [itemHeight]
  );

  // Calculate which items should be visible
  const totalHeight = items.length * itemHeight;
  const startIndex = Math.max(0, Math.floor(scrollTop / itemHeight) - overscan);
  const endIndex = Math.min(
    items.length - 1,
    Math.ceil((scrollTop + height) / itemHeight) + overscan
  );

  // Create an array of visible items with positions
  const virtualItems = [];
  for (let i = startIndex; i <= endIndex; i++) {
    virtualItems.push({
      item: items[i],
      index: i,
      offsetTop: i * itemHeight,
    });
  }

  return {
    containerRef,
    virtualItems,
    totalHeight,
    scrollTo,
  };
}
