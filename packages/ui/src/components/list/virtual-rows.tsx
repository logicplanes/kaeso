import { useLayoutEffect, type ReactNode, type RefObject } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";

export interface VirtualRowsProps {
  count: number;
  estimateSize: (index: number) => number;
  getScrollElement: () => HTMLElement | null;
  scrollTo: RefObject<((index: number) => void) | null>;
  row: (index: number) => ReactNode;
  measure?: boolean | undefined;
}

export function VirtualRows({ count, estimateSize, getScrollElement, scrollTo, row, measure }: VirtualRowsProps) {
  const virtualizer = useVirtualizer({ count, getScrollElement, estimateSize, overscan: 12 });
  useLayoutEffect(() => {
    scrollTo.current = (index) => virtualizer.scrollToIndex(index, { align: "auto" });
    return () => {
      scrollTo.current = null;
    };
  }, [virtualizer, scrollTo]);
  return (
    <div role="presentation" className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
      {virtualizer.getVirtualItems().map((v) => (
        <div
          key={v.key}
          {...(measure ? { ref: virtualizer.measureElement, "data-index": v.index } : {})}
          role="presentation"
          className="absolute top-0 left-0 w-full"
          style={{ transform: `translateY(${Math.round(v.start)}px)` }}
        >
          {row(v.index)}
        </div>
      ))}
    </div>
  );
}
