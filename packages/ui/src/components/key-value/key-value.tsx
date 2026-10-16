import { forwardRef, useRef, type ReactNode } from "react";
import { cn } from "../../utils/cn";
import { useTruncated } from "../../hooks/use-truncated";
import { CopyButton } from "../copy-button/copy-button";
import { Tooltip } from "../tooltip/tooltip";

export const KAESO_KEY_VALUE_VARIANTS = {
  layout: {
    columns: { classes: "grid grid-cols-kv gap-x-24", description: "Label and value side by side (default)" },
    stacked: { classes: "flex flex-col", description: "Label above value, for narrow sheets" },
    grid: { classes: "grid gap-x-32", description: "Label above value, in columns: a wide sheet's details, two or more to a row" },
  },
  columns: {
    2: { classes: "grid-cols-1 @kv:grid-cols-2", description: "Two to a row from 480px; one below (default)" },
    3: { classes: "grid-cols-2 @kv:grid-cols-3", description: "Three to a row from 480px; two below" },
    4: { classes: "grid-cols-2 @kv:grid-cols-4", description: "Four to a row from 480px, for short values (counts, sizes); two below" },
  },
} as const;
export const KAESO_KEY_VALUE_DEFAULT_VARIANTS = { layout: "columns", columns: 2 } as const;

export interface KeyValueItem {
  label: ReactNode;
  value: ReactNode;
  copy?: string | undefined;
  mono?: boolean | undefined;
  wrap?: boolean | undefined;
  span?: "row" | undefined;
}

export interface KeyValueProps {
  items: KeyValueItem[];
  layout?: keyof typeof KAESO_KEY_VALUE_VARIANTS.layout | undefined;
  columns?: keyof typeof KAESO_KEY_VALUE_VARIANTS.columns | undefined;
  className?: string | undefined;
}

function Cut({ children }: { children: string }) {
  const ref = useRef<HTMLSpanElement>(null);
  const cut = useTruncated(ref);
  const el = (
    <span ref={ref} className="block min-w-0 truncate" {...(cut ? { tabIndex: 0 } : {})}>
      {children}
    </span>
  );
  return cut ? (
    <Tooltip content={children} shape="multiline">
      {el}
    </Tooltip>
  ) : (
    el
  );
}

export const KeyValue = forwardRef<HTMLDListElement, KeyValueProps>(function KeyValue({ items, layout = "columns", columns = 2, className }, ref) {
  const grid = layout === "grid";
  const list = (
    <dl ref={ref} className={cn(KAESO_KEY_VALUE_VARIANTS.layout[layout].classes, grid && KAESO_KEY_VALUE_VARIANTS.columns[columns].classes, className)}>
      {items.map((it, i) => (
        <div key={i} className={cn(layout === "columns" ? "contents" : "flex min-w-0 flex-col gap-2 py-8", grid && it.span === "row" && "col-span-full")}>
          <dt
            className={cn(
              "text-small text-fg-secondary",
              layout === "columns" && "flex min-h-36 items-center",
              layout === "columns" && it.wrap && "items-start pt-8",
              grid && "truncate",
            )}
          >
            {it.label}
          </dt>
          <dd
            className={cn(
              "group/kv flex min-w-0 items-center gap-4 text-body text-fg",
              grid ? "min-h-24" : "min-h-36",
              it.mono && "mono text-small",
              it.wrap && (grid ? "items-start" : "items-start py-8"),
            )}
          >
            {it.wrap ? (
              <span className="min-w-0 break-words">{it.value}</span>
            ) : typeof it.value === "string" ? (
              <Cut>{it.value}</Cut>
            ) : (
              <span className="min-w-0 truncate">{it.value}</span>
            )}
            {it.copy && (
              <span className="shrink-0 opacity-0 transition-control group-hover/kv:opacity-100 coarse:opacity-100 focus-within:opacity-100">
                <CopyButton value={it.copy} size="xs" />
              </span>
            )}
          </dd>
        </div>
      ))}
    </dl>
  );
  return grid ? <div className="@container min-w-0">{list}</div> : list;
});
KeyValue.displayName = "KeyValue";
