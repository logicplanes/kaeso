import { forwardRef, type ReactNode } from "react";
import { SwapValue, textOf } from "./swap-value.js";

export interface LegendEntry {
  id: string;
  label: ReactNode;
  value?: ReactNode | undefined;
  color: string;
  dashed?: boolean | undefined;
}

export const Legend = forwardRef<HTMLUListElement, { items: LegendEntry[] }>(function Legend({ items }, ref) {
  return (
    <ul ref={ref} className="flex flex-wrap items-center gap-x-20 gap-y-6">
      {items.map((i) => (
        <li key={i.id} className="flex items-center gap-8 text-small text-fg-secondary">
          <svg aria-hidden width="16" height="2" className="shrink-0 overflow-visible">
            <line
              x1="0"
              x2="16"
              y1="1"
              y2="1"
              stroke={i.color}
              strokeWidth={i.dashed ? 1.5 : 2}
              strokeDasharray={i.dashed ? "3 3" : undefined}
              strokeLinecap="round"
            />
          </svg>
          <span>{i.label}</span>
          {i.value !== undefined && (
            <SwapValue text={textOf(i.value)} className="font-medium text-fg tabular">
              {i.value}
            </SwapValue>
          )}
        </li>
      ))}
    </ul>
  );
});
Legend.displayName = "Legend";
