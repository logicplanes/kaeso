import { useLayoutEffect, useRef, type ReactNode, type RefObject } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useScrollMargin } from "../../hooks/use-scroll-margin.js";

export interface VirtualRowsProps {
  count: number;
  estimateSize: (index: number) => number;
  getScrollElement: () => HTMLElement | null;
  scrollTo: RefObject<((index: number) => void) | null>;
  row: (index: number) => ReactNode;
  measure?: boolean | undefined;
}

export function VirtualRows({ count, estimateSize, getScrollElement, scrollTo, row, measure }: VirtualRowsProps) {
  const body = useRef<HTMLDivElement | null>(null);
  const scrollMargin = useScrollMargin(body, getScrollElement, count);
  const virtualizer = useVirtualizer({ count, getScrollElement, estimateSize, overscan: 12, scrollMargin, indexAttribute: "data-v-index" });
  useLayoutEffect(() => {
    scrollTo.current = (index) => virtualizer.scrollToIndex(index, { align: "auto" });
    return () => {
      scrollTo.current = null;
    };
  }, [virtualizer, scrollTo]);
  return (
    <div ref={body} role="presentation" className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
      {virtualizer.getVirtualItems().map((v) => (
        <div
          key={v.key}
          {...(measure ? { ref: virtualizer.measureElement, "data-v-index": v.index } : {})}
          role="presentation"
          className="absolute top-0 left-0 w-full"
          style={{ transform: `translateY(${Math.round(v.start - scrollMargin)}px)` }}
        >
          {row(v.index)}
        </div>
      ))}
    </div>
  );
}
