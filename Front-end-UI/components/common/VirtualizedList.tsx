"use client";

import { forwardRef, memo } from "react";
import { useVirtualization } from "@/hooks/use-virtualization";

interface VirtualizedListProps<T> {
  items: T[];
  itemHeight: number;
  renderItem: (item: T, index: number) => React.ReactNode;
  className?: string;
  containerClassName?: string;
  containerHeight?: number;
  overscan?: number;
  emptyMessage?: string;
}

/**
 * A component for efficiently rendering long lists with virtualization
 */
function VirtualizedList<T>({
  items,
  itemHeight,
  renderItem,
  className = "",
  containerClassName = "",
  containerHeight,
  overscan = 3,
  emptyMessage = "No items to display",
}: VirtualizedListProps<T>) {
  const { containerRef, virtualItems, totalHeight } = useVirtualization({
    items,
    itemHeight,
    overscan,
    containerHeight,
  });

  // Show empty state message if no items
  if (items.length === 0) {
    return (
      <div className="py-8 text-center text-muted-foreground">
        {emptyMessage}
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className={`relative overflow-auto ${containerClassName}`}
      style={{ height: containerHeight || "100%" }}
    >
      <div
        className="relative"
        style={{ height: totalHeight }}
      >
        {virtualItems.map(({ item, index, offsetTop }) => (
          <div
            key={index}
            className={`absolute w-full ${className}`}
            style={{
              height: itemHeight,
              top: offsetTop,
            }}
          >
            {renderItem(item, index)}
          </div>
        ))}
      </div>
    </div>
  );
}

export default memo(VirtualizedList) as typeof VirtualizedList;
