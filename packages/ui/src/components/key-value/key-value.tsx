import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { CopyButton } from "../copy-button/copy-button.js";
import { TruncatedText } from "../text/truncated.js";

export const KAESO_KEY_VALUE_VARIANTS = {
  layout: {
    columns: {
      classes: "grid h-fit grid-cols-kv content-start",
      description: "Label and value side by side, each pair a row with a hairline at its foot (default)",
    },
    stacked: { classes: "flex flex-col", description: "Label above value, each pair over a hairline, for narrow sheets" },
    grid: {
      classes: "grid h-fit content-start gap-x-32",
      description: "Label above value, in columns, each pair over its own hairline: a wide sheet's details, two or more to a row",
    },
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

export const KeyValue = forwardRef<HTMLDListElement, KeyValueProps>(function KeyValue({ items, layout = "columns", columns = 2, className }, ref) {
  const grid = layout === "grid";
  const list = (
    <dl ref={ref} className={cn(KAESO_KEY_VALUE_VARIANTS.layout[layout].classes, grid && KAESO_KEY_VALUE_VARIANTS.columns[columns].classes, className)}>
      {items.map((it, i) => (
        <div
          key={i}
          className={cn(layout === "columns" ? "contents" : "flex min-w-0 flex-col gap-2 py-8 hairline-b", grid && it.span === "row" && "col-span-full")}
        >
          <dt
            className={cn(
              "text-small font-medium text-fg-secondary",
              layout === "columns" && "flex min-h-36 min-w-120 items-start pt-10 pb-8 pe-24 hairline-b",
              grid && "flex min-w-0",
            )}
          >
            {grid ? <TruncatedText>{it.label}</TruncatedText> : it.label}
          </dt>
          <dd
            className={cn(
              "group/kv flex min-w-0 gap-4 text-body text-fg",
              grid ? "min-h-24" : "min-h-36",
              layout === "columns" ? cn("items-start pb-8 hairline-b", it.mono ? "pt-10" : "pt-8") : it.wrap ? "items-start" : "items-center",
              layout === "stacked" && it.wrap && "py-8",
              it.mono && "mono text-small",
            )}
          >
            {it.wrap ? <span className="min-w-0 break-words">{it.value}</span> : <TruncatedText className="block min-w-0">{it.value}</TruncatedText>}
            {it.copy && (
              <span
                className={cn(
                  "shrink-0 opacity-0 transition-control group-hover/kv:opacity-100 coarse:opacity-100 focus-within:opacity-100",
                  layout === "columns" && "-my-2",
                )}
              >
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
