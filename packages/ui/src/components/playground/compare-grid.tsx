import {
  Children,
  createContext,
  forwardRef,
  isValidElement,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactElement,
  type ReactNode,
} from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { IconButton } from "../icon-button/icon-button.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_COMPARE_GRID_VARIANTS = {
  divider: {
    hairline: { classes: "hairline-l", description: "Columns separated by a hairline, edge to edge (default)" },
    none: { classes: "", description: "No line between columns: columns that sit in panels of their own" },
  },
} as const;
export const KAESO_COMPARE_GRID_DEFAULT_VARIANTS = { divider: "hairline" } as const;

interface GridContext {
  index: number;
  count: number;
  divider: keyof typeof KAESO_COMPARE_GRID_VARIANTS.divider;
  add: ReactNode;
}
const ColumnContext = createContext<GridContext>({ index: 0, count: 1, divider: "hairline", add: null });

export interface CompareGridProps {
  children: ReactNode;
  "aria-label": string;
  onAdd?: (() => void) | undefined;
  addLabel?: string | undefined;
  max?: number | undefined;
  active?: number | undefined;
  onActiveChange?: ((index: number) => void) | undefined;
  divider?: keyof typeof KAESO_COMPARE_GRID_VARIANTS.divider | undefined;
  className?: string | undefined;
}

const Root = forwardRef<HTMLDivElement, CompareGridProps>(function CompareGrid(
  { children, "aria-label": ariaLabel, onAdd, addLabel, max = 4, active, onActiveChange, divider = "hairline", className },
  ref,
) {
  const t = useUiText();
  const columns = Children.toArray(children).filter(isValidElement) as ReactElement<CompareColumnProps>[];
  const count = columns.length;
  const scroller = useRef<HTMLDivElement | null>(null);
  const [current, setCurrent] = useState(0);
  const shown = Math.min(active ?? current, Math.max(0, count - 1));
  const target = useRef<number | null>(null);
  const changed = useRef(onActiveChange);
  useLayoutEffect(() => {
    changed.current = onActiveChange;
  });

  const scrollTo = useCallback((i: number) => {
    const el = scroller.current?.children[i] as HTMLElement | undefined;
    const box = scroller.current;
    if (!el || !box) return;
    target.current = box.scrollLeft === el.offsetLeft ? null : i;
    const reduce = typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
    box.scrollTo({ left: el.offsetLeft, behavior: reduce ? "auto" : "smooth" });
  }, []);
  const go = (i: number) => {
    const next = Math.max(0, Math.min(count - 1, i));
    setCurrent(next);
    onActiveChange?.(next);
    scrollTo(next);
  };
  useEffect(() => {
    if (active !== undefined) scrollTo(active);
  }, [active, scrollTo]);
  const before = useRef(count);
  useLayoutEffect(() => {
    if (count > before.current) {
      setCurrent(count - 1);
      changed.current?.(count - 1);
      scrollTo(count - 1);
    }
    before.current = count;
  }, [count, scrollTo]);

  const fade = useCallback(() => {
    const box = scroller.current;
    if (!box) return;
    const end = box.scrollWidth - box.clientWidth - box.scrollLeft;
    box.style.setProperty("--k-fade-start", box.scrollLeft > 1 ? "24px" : "0px");
    box.style.setProperty("--k-fade-end", end > 1 ? "24px" : "0px");
  }, []);
  useLayoutEffect(() => {
    const box = scroller.current;
    if (!box || typeof ResizeObserver === "undefined") return;
    fade();
    const ro = new ResizeObserver(fade);
    ro.observe(box);
    return () => ro.disconnect();
  }, [fade, count]);

  const onScroll = () => {
    fade();
    const box = scroller.current;
    const first = box?.children[0] as HTMLElement | undefined;
    if (!box || !first || !first.offsetWidth) return;
    const i = Math.max(0, Math.min(count - 1, Math.round(box.scrollLeft / first.offsetWidth)));
    if (target.current !== null) {
      if (i !== target.current) return;
      target.current = null;
    }
    if (i !== shown) {
      setCurrent(i);
      onActiveChange?.(i);
    }
  };

  const release = () => {
    target.current = null;
  };

  const full = count >= max;
  const add = onAdd ? (
    <IconButton
      icon={icons.add}
      aria-label={addLabel ?? t("compareGrid.addModel", "Add model")}
      size="sm"
      onClick={onAdd}
      disabled={full}
      {...(full ? { disabledReason: t("compareGrid.compareUpTo", "Compare up to {max} at once", { max }) } : {})}
    />
  ) : null;
  const label = columns[shown]?.props.label ?? "";

  return (
    <div ref={ref} role="group" aria-label={ariaLabel} className={cn("flex h-full min-h-0 min-w-0 flex-col", className)}>
      {count > 1 || onAdd ? (
        <div className="flex h-44 shrink-0 items-center gap-4 px-24 hairline-b md:hidden">
          <IconButton
            icon={icons.chevronLeft}
            aria-label={t("compareGrid.previousColumn", "Previous column")}
            size="sm"
            disabled={shown === 0}
            onClick={() => go(shown - 1)}
          />
          <span aria-live="polite" className="min-w-0 flex-1 truncate text-center text-small text-fg-secondary tabular">
            <span className="sr-only">{t("compareGrid.columnName", "{label}, ", { label })}</span>
            {t("compareGrid.position", "{index} of {count}", { index: shown + 1, count })}
          </span>
          <IconButton
            icon={icons.chevronRight}
            aria-label={t("compareGrid.nextColumn", "Next column")}
            size="sm"
            disabled={shown >= count - 1}
            onClick={() => go(shown + 1)}
          />
          {add}
        </div>
      ) : null}
      <div
        ref={scroller}
        onScroll={onScroll}
        onPointerDown={release}
        onWheel={release}
        onTouchStart={release}
        onKeyDown={release}
        className="relative min-h-0 min-w-0 flex-1 compare-grid"
      >
        {columns.map((col, i) => (
          <ColumnContext.Provider key={col.key ?? i} value={{ index: i, count, divider, add: i === count - 1 ? add : null }}>
            {col}
          </ColumnContext.Provider>
        ))}
      </div>
    </div>
  );
});
Root.displayName = "CompareGrid";

export interface CompareColumnProps {
  label: string;
  header: ReactNode;
  actions?: ReactNode | undefined;
  children?: ReactNode | undefined;
  footer?: ReactNode | undefined;
  onRemove?: (() => void) | undefined;
  removeLabel?: string | undefined;
  stickToEnd?: boolean | undefined;
  className?: string | undefined;
}

function useStickToEnd(enabled: boolean) {
  const body = useRef<HTMLDivElement | null>(null);
  const content = useRef<HTMLDivElement | null>(null);
  const pinned = useRef(true);
  useEffect(() => {
    const el = body.current;
    const inner = content.current;
    if (!enabled || !el || !inner || typeof ResizeObserver === "undefined") return;
    let last = el.scrollTop;
    const fade = () => {
      el.style.setProperty("--scroll-area-overflow-y-start", `${Math.max(0, el.scrollTop)}px`);
      el.style.setProperty("--scroll-area-overflow-y-end", `${Math.max(0, el.scrollHeight - el.clientHeight - el.scrollTop)}px`);
    };
    const onScroll = () => {
      fade();
      const near = el.scrollHeight - el.scrollTop - el.clientHeight <= 24;
      pinned.current = el.scrollTop < last - 1 ? near : pinned.current || near;
      last = el.scrollTop;
    };
    const ro = new ResizeObserver(() => {
      if (pinned.current) {
        el.scrollTop = el.scrollHeight;
        last = el.scrollTop;
      }
      fade();
    });
    el.addEventListener("scroll", onScroll, { passive: true });
    ro.observe(inner);
    ro.observe(el);
    return () => {
      el.removeEventListener("scroll", onScroll);
      ro.disconnect();
    };
  }, [enabled]);
  return { body, content };
}

const Column = forwardRef<HTMLElement, CompareColumnProps>(function CompareGridColumn(
  { label, header, actions, children, footer, onRemove, removeLabel, stickToEnd = true, className },
  ref,
) {
  const t = useUiText();
  const { index, count, divider, add } = useContext(ColumnContext);
  const { body, content } = useStickToEnd(stickToEnd);
  return (
    <section
      ref={ref}
      aria-label={label}
      className={cn(
        "row-span-3 grid min-h-0 min-w-0 snap-start snap-always grid-rows-subgrid",
        index > 0 && KAESO_COMPARE_GRID_VARIANTS.divider[divider].classes,
        className,
      )}
    >
      <header className="flex h-52 min-w-0 items-center gap-4 px-24 hairline-b">
        <div className="flex min-w-0 flex-1 items-center">{header}</div>
        {actions}
        {onRemove && count > 1 && (
          <IconButton icon={icons.close} aria-label={removeLabel ?? t("compareGrid.remove", "Remove {label}", { label })} size="sm" onClick={onRemove} />
        )}
        {add && <span className="hidden md:contents">{add}</span>}
      </header>
      <div
        ref={body}
        tabIndex={0}
        aria-label={t("compareGrid.thread", "{label} thread", { label })}
        role="region"
        className="relative min-h-0 overflow-y-auto overscroll-contain scrollbar-quiet scroll-fade-y focus-ring-inset"
      >
        <div ref={content} className="flex min-h-full flex-col px-24 pt-16 pb-24">
          {children}
        </div>
      </div>
      {footer !== undefined && footer !== null && <footer className="flex min-h-44 min-w-0 items-start px-24 pt-12 pb-12">{footer}</footer>}
    </section>
  );
});
Column.displayName = "CompareGrid.Column";

export const CompareGrid = Object.assign(Root, { Column });
