import { forwardRef, useEffect, useLayoutEffect, useRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { Skeleton } from "../skeleton/skeleton.js";
import { SwapValue, textOf } from "./swap-value.js";

export interface RankItem {
  id: string;
  label: ReactNode;
  value?: ReactNode | undefined;
  /** Draws the row's share of the largest row behind it; leave it out for a plain row. */
  share?: number | undefined;
  secondary?: ReactNode | undefined;
}

const SKELETON_WIDTHS = ["w-160", "w-120", "w-140", "w-96", "w-112"];

export const RankList = forwardRef<
  HTMLOListElement,
  {
    items: RankItem[];
    loading?: boolean | undefined;
    rows?: number | undefined;
    empty?: ReactNode | undefined;
    onOpen?: ((id: string) => void) | undefined;
    "aria-label": string;
    className?: string | undefined;
  }
>(function RankList({ items, loading, rows = 5, empty, onOpen, "aria-label": ariaLabel, className }, ref) {
  const list = useRef<HTMLOListElement | null>(null);
  const tops = useRef(new Map<string, number>());
  const seen = useRef<Set<string> | null>(null);
  const arrived = useRef(new Set<string>());
  if (seen.current) for (const i of items) if (!seen.current.has(i.id)) arrived.current.add(i.id);
  const order = items.map((i) => i.id).join("|");
  useEffect(() => {
    if (!loading) seen.current = new Set(items.map((i) => i.id));
  });
  useLayoutEffect(() => {
    const el = list.current;
    if (!el) return;
    const rowsNow = [...el.querySelectorAll<HTMLElement>(":scope > li[data-id]")];
    const before = tops.current;
    const next = new Map(rowsNow.map((r) => [r.dataset.id!, r.offsetTop]));
    tops.current = next;
    if (before.size === 0 || typeof matchMedia === "undefined" || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const moved = rowsNow.filter((r) => before.has(r.dataset.id!) && before.get(r.dataset.id!) !== r.offsetTop);
    for (const r of moved) {
      r.style.transition = "none";
      r.style.translate = `0 ${before.get(r.dataset.id!)! - r.offsetTop}px`;
    }
    if (!moved.length) return;
    void el.offsetHeight;
    for (const r of moved) {
      r.style.transition = "translate var(--k-duration-chart) var(--k-ease-standard)";
      r.style.translate = "";
    }
  }, [order]);
  if (loading)
    return (
      <ol ref={ref} aria-label={ariaLabel} aria-busy className={cn("flex flex-col gap-2", className)}>
        {Array.from({ length: rows }, (_, i) => (
          <li key={i} className="flex h-36 items-center justify-between gap-12 px-12">
            <Skeleton shape="line" className={SKELETON_WIDTHS[i % SKELETON_WIDTHS.length]} />
            <Skeleton shape="line" className="w-48" />
          </li>
        ))}
      </ol>
    );
  if (items.length === 0) return <p className="flex min-h-120 flex-1 items-center justify-center px-12 text-small text-fg-tertiary">{empty}</p>;
  const max = Math.max(...items.map((i) => i.share ?? 0), 0);
  return (
    <ol
      ref={(el) => {
        list.current = el;
        if (typeof ref === "function") ref(el);
        else if (ref) ref.current = el;
      }}
      aria-label={ariaLabel}
      className={cn("flex flex-col gap-2", className)}
    >
      {items.map((item) => {
        const body = (
          <>
            {item.share !== undefined && max > 0 && (
              <span
                aria-hidden
                className="width-motion absolute inset-y-0 left-0 rounded-cell bg-fill-2"
                style={{ width: `${Math.max(1, (item.share / max) * 100)}%` }}
              />
            )}
            <span className="relative flex min-w-0 flex-1 items-center gap-8 text-body text-fg">{item.label}</span>
            {item.secondary !== undefined && (
              <span className="relative min-w-56 shrink-0 text-right text-small text-fg-tertiary tabular">{item.secondary}</span>
            )}
            {item.value !== undefined && (
              <SwapValue text={textOf(item.value)} className="relative min-w-64 shrink-0 text-right text-body font-medium text-fg tabular">
                {item.value}
              </SwapValue>
            )}
          </>
        );
        const row = "relative flex h-36 w-full min-w-0 items-center gap-12 rounded-cell px-12";
        return (
          <li key={item.id} data-id={item.id} className={cn(arrived.current.has(item.id) && "value-swap-in")}>
            {onOpen ? (
              <button
                type="button"
                onClick={() => onOpen(item.id)}
                className={cn(row, "cursor-pointer text-left transition-control hover:bg-item-hover focus-ring-inset")}
              >
                {body}
              </button>
            ) : (
              <div className={row}>{body}</div>
            )}
          </li>
        );
      })}
    </ol>
  );
});
RankList.displayName = "RankList";
