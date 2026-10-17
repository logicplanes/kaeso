import { forwardRef, useCallback, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { useInset } from "../../utils/inset.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon, type IconData } from "../icon/icon.js";
import { IconButton } from "../icon-button/icon-button.js";
import { Text } from "../text/text.js";
import { Skeleton } from "../skeleton/skeleton.js";
import { scrollParent } from "../../hooks/scroll-parent.js";
import { useLayoutWidth } from "../../hooks/use-layout-width.js";
import { createPortal } from "react-dom";
import { usePortalContainer } from "../../utils/portal-context.js";
import { lazyModule, useLazyModuleRetried } from "../../utils/lazy-module.js";
import { RowCheckbox } from "./row-checkbox.js";
import { useUiText } from "../../i18n/context.js";

const virtualRows = lazyModule(() => import("./virtual-rows.js"), { base: () => import.meta.url });

export const KAESO_LIST_VARIANTS = {
  density: {
    default: { classes: "h-44", description: "44px rows (default)" },
    compact: { classes: "h-36", description: "36px rows" },
  },
} as const;
export const KAESO_LIST_DEFAULT_VARIANTS = { density: "default" } as const;

export interface ListItemData {
  id: string;
  title: ReactNode;
  textValue?: string | undefined;
  subtitle?: ReactNode | undefined;
  glyph?: ReactNode | undefined;
  leading?: ReactNode | undefined;
  displayId?: string | undefined;
  meta?: ReactNode | undefined;
  date?: ReactNode | undefined;
  avatar?: ReactNode | undefined;
  actions?: ReactNode | undefined;
  children?: ListItemData[] | undefined;
  disabled?: boolean | undefined;
}

export interface ListGroup {
  id: string;
  title: ReactNode;
  glyph?: ReactNode | undefined;
  count?: number | undefined;
  items: ListItemData[];
  onAdd?: (() => void) | undefined;
  addLabel?: string | undefined;
  defaultCollapsed?: boolean | undefined;
}

type Flat = { kind: "header"; group: ListGroup; key: string } | { kind: "row"; item: ListItemData; depth: 0 | 1; lastChild: boolean; key: string };

export interface ListProps {
  items?: ListItemData[] | undefined;
  groups?: ListGroup[] | undefined;
  onOpen?: ((item: ListItemData) => void) | undefined;
  selectable?: boolean | undefined;
  selected?: Set<string> | undefined;
  onSelectedChange?: ((selected: Set<string>) => void) | undefined;
  idWidth?: 48 | 64 | 80 | 96 | 120 | undefined;
  density?: keyof typeof KAESO_LIST_VARIANTS.density | undefined;
  virtualizeAbove?: number | undefined;
  selectionActions?: ReactNode | undefined;
  selectionBar?: boolean | undefined;
  loading?: boolean | undefined;
  empty?: ReactNode | undefined;
  "aria-label": string;
  className?: string | undefined;
  "data-force-row"?: string | undefined;
}

const ID_W = { 48: "w-48", 64: "w-64", 80: "w-80", 96: "w-96", 120: "w-120" } as const;

const NARROW = 520;
const FOCUSABLE = "a[href], button, input, select, textarea, [tabindex], [contenteditable=true]";
const TEXT_ENTRY = "input:not([type=checkbox]):not([type=radio]), textarea, select, [contenteditable=true]";

function removeWidgetsFromTabOrder(root: HTMLElement) {
  for (const line of root.querySelectorAll<HTMLElement>("[role=row]")) {
    const primary = line.querySelector("[data-index]");
    if (!primary) continue;
    for (const widget of line.querySelectorAll<HTMLElement>(FOCUSABLE)) {
      if (widget !== primary && widget.getAttribute("tabindex") !== "-1") widget.setAttribute("tabindex", "-1");
    }
  }
}

export const List = forwardRef<HTMLDivElement, ListProps>(function List(
  {
    items,
    groups,
    onOpen,
    selectable,
    selected: selectedProp,
    onSelectedChange,
    idWidth,
    density = "default",
    virtualizeAbove = 100,
    selectionActions,
    selectionBar = true,
    loading,
    empty,
    className,
    "aria-label": ariaLabel,
    "data-force-row": forceRow,
  },
  ref,
) {
  const inset = useInset();
  const [selectedInner, setSelectedInner] = useState<Set<string>>(new Set());
  const selected = selectedProp ?? selectedInner;
  const setSelected = useCallback(
    (s: Set<string>) => {
      if (!selectedProp) setSelectedInner(s);
      onSelectedChange?.(s);
    },
    [selectedProp, onSelectedChange],
  );
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set((groups ?? []).filter((g) => g.defaultCollapsed).map((g) => g.id)));
  const [active, setActive] = useState(-1);
  const [anchor, setAnchor] = useState<number | null>(null);
  const root = useRef<HTMLDivElement | null>(null);
  const width = useLayoutWidth(root);
  const narrow = !!width && width < NARROW;

  const flat = useMemo<Flat[]>(() => {
    const rows = (list: ListItemData[]): Flat[] =>
      list.flatMap((item) => [
        { kind: "row" as const, item, depth: 0 as const, lastChild: false, key: item.id },
        ...(item.children ?? []).map((c, i, a) => ({ kind: "row" as const, item: c, depth: 1 as const, lastChild: i === a.length - 1, key: c.id })),
      ]);
    if (groups) return groups.flatMap((g) => [{ kind: "header" as const, group: g, key: `h-${g.id}` }, ...(collapsed.has(g.id) ? [] : rows(g.items))]);
    return rows(items ?? []);
  }, [items, groups, collapsed]);
  const firstRow = flat.findIndex((f) => f.kind === "row");
  const none = groups ? groups.every((g) => g.items.length === 0) : (items ?? []).length === 0;

  const virtual = flat.length > virtualizeAbove;
  const estimateSize = (i: number) => (flat[i]?.kind === "header" ? (i === 0 ? 36 : 52) : narrow ? 64 : density === "compact" ? 36 : 44);
  const [rowsModule, retryRows] = useLazyModuleRetried(virtualRows, virtual);
  const VirtualRows = rowsModule?.VirtualRows;
  const scrollTo = useRef<((index: number) => void) | null>(null);

  if (active >= flat.length) setActive(-1);
  const tabStop = active >= 0 ? active : flat.length ? 0 : -1;
  const focusLine = (i: number) => {
    setActive(i);
    if (virtual) scrollTo.current?.(i);
    const line = () => root.current?.querySelector<HTMLElement>(`[data-index="${i}"]`);
    const now = line();
    if (now) now.focus();
    else requestAnimationFrame(() => line()?.focus());
  };
  const hadVirtualRows = useRef(!!VirtualRows);
  useLayoutEffect(() => {
    if (VirtualRows && !hadVirtualRows.current && active >= 0 && document.activeElement === document.body) {
      root.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.focus();
    }
    hadVirtualRows.current = !!VirtualRows;
  });
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    removeWidgetsFromTabOrder(el);
    const observer = new MutationObserver(() => removeWidgetsFromTabOrder(el));
    observer.observe(el, { subtree: true, childList: true, attributes: true, attributeFilter: ["tabindex"] });
    return () => observer.disconnect();
  }, []);
  const toggle = (id: string) => {
    const s = new Set(selected);
    if (s.has(id)) s.delete(id);
    else s.add(id);
    setSelected(s);
  };
  const enabledIds = (lines: Flat[]) => lines.flatMap((f) => (f.kind === "row" && !f.item.disabled ? [f.item.id] : []));
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    if (e.defaultPrevented || !e.currentTarget.contains(target) || target.closest(TEXT_ENTRY)) return;
    const line = target.closest<HTMLElement>("[role=row]");
    const primary = line?.querySelector<HTMLElement>("[data-index]");
    if (!line || !primary) return;
    const index = Number(primary.dataset.index);
    const onPrimary = target === primary;
    const f = flat[index];
    const row = f?.kind === "row" ? f.item : undefined;
    if (e.altKey) return;
    if (e.metaKey || e.ctrlKey) {
      if (e.key.toLowerCase() === "a" && selectable && !e.shiftKey) {
        e.preventDefault();
        setSelected(new Set(enabledIds(flat)));
      }
      return;
    }
    const moveTo = (next: number) => {
      e.preventDefault();
      if (e.shiftKey && selectable) {
        const from = anchor ?? index;
        setAnchor(from);
        const s = new Set(selected);
        enabledIds(flat.slice(Math.min(from, next), Math.max(from, next) + 1)).forEach((id) => s.add(id));
        setSelected(s);
      } else setAnchor(null);
      focusLine(next);
    };
    const widgets = () =>
      [...line.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
        (w) => w !== primary && !w.closest("[role=checkbox], [aria-hidden=true], [hidden]") && !w.matches(":disabled, input[type=checkbox]"),
      );
    if (e.key === "ArrowDown" || (e.key === "j" && !e.shiftKey)) moveTo(Math.min(flat.length - 1, index + 1));
    else if (e.key === "ArrowUp" || (e.key === "k" && !e.shiftKey)) moveTo(Math.max(0, index - 1));
    else if (e.key === "Home") moveTo(0);
    else if (e.key === "End") moveTo(flat.length - 1);
    else if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
      const list = widgets();
      const at = onPrimary ? -1 : list.indexOf(target);
      const next = e.key === "ArrowRight" ? list[at + 1] : at > 0 ? list[at - 1] : at === 0 ? primary : undefined;
      if (!next) return;
      e.preventDefault();
      next.focus();
    } else if (e.key === "Escape" && !onPrimary) {
      e.preventDefault();
      primary.focus();
    } else if (!onPrimary || !row || e.shiftKey) {
      return;
    } else if ((e.key === " " || e.key === "x") && selectable) {
      e.preventDefault();
      if (!row.disabled) toggle(row.id);
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (!row.disabled) onOpen?.(row);
    } else if (e.key === "Escape" && selectable && selected.size) {
      setSelected(new Set());
    }
  };
  const selecting = selected.size > 0;
  const render = (f: Flat, i: number) =>
    f.kind === "header" ? (
      <GroupHeader
        key={f.key}
        title={f.group.title}
        glyph={f.group.glyph}
        count={f.group.count ?? f.group.items.length}
        collapsed={collapsed.has(f.group.id)}
        onToggle={() => {
          const s = new Set(collapsed);
          if (s.has(f.group.id)) s.delete(f.group.id);
          else s.add(f.group.id);
          setCollapsed(s);
        }}
        onAdd={f.group.onAdd}
        addLabel={f.group.addLabel}
        indent={!!selectable}
        first={i === 0}
        index={i}
        active={i === tabStop}
        onFocus={() => setActive(i)}
      />
    ) : (
      <ListRow
        key={f.key}
        index={i}
        item={f.item}
        depth={f.depth}
        lastChild={f.lastChild}
        idWidth={idWidth}
        density={density}
        narrow={narrow}
        active={i === tabStop}
        selected={selected.has(f.item.id)}
        selectable={!!selectable}
        selecting={selecting}
        onSelect={() => toggle(f.item.id)}
        onOpen={() => onOpen?.(f.item)}
        onFocus={() => setActive(i)}
        {...(forceRow && i === firstRow ? { "data-force": forceRow } : {})}
      />
    );

  return (
    <div
      ref={(e) => {
        root.current = e;
        if (typeof ref === "function") ref(e);
        else if (ref) ref.current = e;
      }}
      role="grid"
      aria-label={ariaLabel}
      aria-multiselectable={selectable || undefined}
      aria-busy={loading || undefined}
      onKeyDown={(e) => {
        retryRows();
        onKeyDown(e);
      }}
      onPointerEnter={retryRows}
      onPointerDown={retryRows}
      onFocus={retryRows}
      onWheel={retryRows}
      className={cn("relative flex flex-col outline-none", inset && "overflow-hidden rounded-group bg-raised shadow-xs", className)}
    >
      {loading && (none || flat.length === 0) ? (
        <LoadingRows density={density} />
      ) : none && empty ? (
        <div role="row">
          <div role="gridcell">{empty}</div>
        </div>
      ) : virtual && VirtualRows ? (
        <VirtualRows
          count={flat.length}
          estimateSize={estimateSize}
          getScrollElement={() => scrollParent(root.current)}
          scrollTo={scrollTo}
          row={(i) => render(flat[i]!, i)}
          measure={narrow}
        />
      ) : virtual ? (
        <FirstRows count={flat.length} estimateSize={estimateSize} row={(i) => render(flat[i]!, i)} />
      ) : (
        flat.map(render)
      )}
      {selectable && selecting && selectionBar && (
        <SelectionBar count={selected.size} onClear={() => setSelected(new Set())}>
          {selectionActions}
        </SelectionBar>
      )}
    </div>
  );
});
List.displayName = "List";

function FirstRows({ count, estimateSize, row }: { count: number; estimateSize: (index: number) => number; row: (index: number) => ReactNode }) {
  const view = typeof window === "undefined" ? 1024 : window.innerHeight;
  const rows: ReactNode[] = [];
  let total = 0;
  let extra = 12;
  for (let i = 0; i < count; i++) {
    if (total >= view && extra-- <= 0) {
      for (let j = i; j < count; j++) total += estimateSize(j);
      break;
    }
    rows.push(
      <div key={i} role="presentation" className="absolute top-0 left-0 w-full" style={{ transform: `translateY(${Math.round(total)}px)` }}>
        {row(i)}
      </div>,
    );
    total += estimateSize(i);
  }
  return (
    <div role="presentation" className="relative w-full" style={{ height: total }}>
      {rows}
    </div>
  );
}

function LoadingRows({ density }: { density: keyof typeof KAESO_LIST_VARIANTS.density }) {
  return (
    <div role="presentation">
      {[62, 48, 70, 54, 40, 58].map((w, i) => (
        <div key={i} role="presentation" className="px-8">
          <div className={cn("flex items-center gap-12 px-16", KAESO_LIST_VARIANTS.density[density].classes)}>
            <Skeleton shape="line" className="min-w-0" style={{ width: `${w}%` }} />
            <span className="flex-1" />
            <Skeleton shape="line" className="w-64 shrink-0" />
          </div>
        </div>
      ))}
    </div>
  );
}

export interface ListRowProps {
  item: ListItemData;
  index?: number | undefined;
  depth?: 0 | 1 | undefined;
  lastChild?: boolean | undefined;
  idWidth?: keyof typeof ID_W | undefined;
  density?: keyof typeof KAESO_LIST_VARIANTS.density | undefined;
  narrow?: boolean | undefined;
  active?: boolean | undefined;
  selected?: boolean | undefined;
  selectable?: boolean | undefined;
  selecting?: boolean | undefined;
  onSelect?: (() => void) | undefined;
  onOpen?: (() => void) | undefined;
  onFocus?: (() => void) | undefined;
  "data-force"?: string | undefined;
}

export const ListRow = forwardRef<HTMLDivElement, ListRowProps>(function ListRow(
  {
    item,
    index,
    depth = 0,
    lastChild,
    idWidth,
    density = "default",
    narrow = false,
    active,
    selected,
    selectable,
    selecting,
    onSelect,
    onOpen,
    onFocus,
    "data-force": force,
  },
  ref,
) {
  const card = useInset();
  const t = useUiText();
  const lines = narrow && !!(item.meta || item.date);
  const stacked = (card || lines) && !!item.subtitle;
  return (
    <div role="row" className={card ? cn("px-0", index !== undefined && index > 0 && "hairline-t") : "px-8"}>
      <div
        ref={ref}
        role="gridcell"
        aria-selected={selectable ? !!selected : undefined}
        aria-disabled={item.disabled || undefined}
        data-index={index}
        data-row=""
        tabIndex={active === false ? -1 : 0}
        onFocus={onFocus}
        onClick={(e: MouseEvent<HTMLElement>) => {
          if (!e.currentTarget.contains(e.target as Node) || (e.target as HTMLElement).closest("button, a, [role=checkbox]")) return;
          if (selecting && selectable) onSelect?.();
          else onOpen?.();
        }}
        {...(force ? { "data-force": force } : {})}
        className={cn(
          "group/row relative flex cursor-pointer items-center gap-12 px-16 text-body transition-control focus-ring-inset select-none",
          card ? "rounded-none" : "rounded-item",
          lines ? "min-h-56 py-8" : stacked ? "h-56" : card && density === "default" ? "h-48" : KAESO_LIST_VARIANTS.density[density].classes,
          "hover:bg-fill-1 hover:controls-on-fill",
          selected && "bg-fill-2 controls-on-fill hover:bg-fill-2",
          item.disabled && "pointer-events-none text-fg-disabled",
        )}
      >
        {selectable && (
          <span
            className={cn(
              "flex shrink-0 transition-control",
              selecting || selected ? "opacity-100" : "opacity-0 group-hover/row:opacity-100 group-focus-visible/row:opacity-100 coarse:opacity-100",
            )}
            onClick={(e) => e.stopPropagation()}
          >
            <RowCheckbox
              checked={!!selected}
              onToggle={() => onSelect?.()}
              aria-label={t("list.selectItem", "Select {name}", { name: item.textValue ?? (typeof item.title === "string" ? item.title : item.id) })}
            />
          </span>
        )}
        {depth === 1 && (
          <span aria-hidden className="relative -ml-4 h-full w-16 shrink-0">
            <span className={cn("absolute left-6 w-1 bg-line-strong", lastChild ? "top-0 h-1/2" : "inset-y-0")} />
            <span className="absolute top-1/2 left-6 h-1 w-10 bg-line-strong" />
          </span>
        )}
        {item.glyph && <span className="flex size-16 shrink-0 items-center justify-center">{item.glyph}</span>}
        {item.leading && <span className="-mr-4 flex shrink-0">{item.leading}</span>}
        {item.displayId && <span className={cn("shrink-0 truncate mono text-small text-fg-tertiary tabular", idWidth && ID_W[idWidth])}>{item.displayId}</span>}
        <span className={cn("flex min-w-0 flex-1", lines ? "flex-col items-start gap-4" : "contents")}>
          <span className={cn("flex min-w-0", lines ? "w-full" : "flex-1", stacked ? "flex-col items-start gap-2" : "items-baseline gap-8")}>
            {typeof item.title === "string" ? (
              <Text truncate weight="medium" className={cn("min-w-0 shrink", stacked && "max-w-full")}>
                {item.title}
              </Text>
            ) : (
              <span className={cn("min-w-0 truncate font-medium text-fg", stacked && "max-w-full")}>{item.title}</span>
            )}
            {item.subtitle && (
              <span className={cn("min-w-0 truncate text-fg-secondary", stacked ? "max-w-full text-small" : "flex-1 basis-0")}>{item.subtitle}</span>
            )}
          </span>
          {lines && (
            <span className="flex min-w-0 flex-wrap items-center gap-x-8 gap-y-4">
              {item.meta && <span className="flex min-w-0 flex-wrap items-center gap-6">{item.meta}</span>}
              {item.date && <span className="text-small whitespace-nowrap text-fg-secondary tabular">{item.date}</span>}
            </span>
          )}
        </span>
        {lines && item.actions && <span className="flex shrink-0 items-center gap-2">{item.actions}</span>}
        {!lines && (item.meta || item.actions) && (
          <span className="relative flex shrink-0 items-center gap-6">
            <span className={cn("flex items-center gap-6", item.actions && "transition-control group-hover/row:opacity-0 group-focus-within/row:opacity-0")}>
              {item.meta}
            </span>
            {item.actions && (
              <span
                className={cn(
                  "flex items-center gap-2 opacity-0 transition-control group-hover/row:opacity-100 group-focus-within/row:opacity-100 coarse:opacity-100",
                  item.meta && "absolute inset-y-0 right-0 coarse:static",
                )}
              >
                {item.actions}
              </span>
            )}
          </span>
        )}
        {!lines && item.date && <span className="min-w-80 shrink-0 text-right text-small whitespace-nowrap text-fg-secondary tabular">{item.date}</span>}
        {item.avatar && <span className="flex shrink-0">{item.avatar}</span>}
      </div>
    </div>
  );
});
ListRow.displayName = "ListRow";

export interface GroupHeaderProps {
  title: ReactNode;
  glyph?: ReactNode | undefined;
  count?: number | undefined;
  collapsed?: boolean | undefined;
  onToggle?: (() => void) | undefined;
  onAdd?: (() => void) | undefined;
  addLabel?: string | undefined;
  indent?: boolean | undefined;
  first?: boolean | undefined;
  index?: number | undefined;
  active?: boolean | undefined;
  onFocus?: (() => void) | undefined;
}

export const GroupHeader = forwardRef<HTMLDivElement, GroupHeaderProps>(function GroupHeader(
  { title, glyph, count, collapsed, onToggle, onAdd, addLabel, indent, first, index, active, onFocus },
  ref,
) {
  const card = useInset();
  const t = useUiText();
  return (
    <div ref={ref} role="row" onFocus={onFocus} className={card ? cn("group/gh bg-fill-1", !first && "hairline-t") : "group/gh px-8 pt-16 first:pt-0"}>
      <div role="gridcell" className={cn("flex items-center gap-4 pr-8", card ? "h-36 pl-6" : "h-36", !card && (indent ? "pl-36" : "pl-6"))}>
        <button
          type="button"
          aria-expanded={!collapsed}
          data-index={index}
          tabIndex={active === false ? -1 : 0}
          onClick={onToggle}
          className="relative flex h-28 min-w-0 cursor-pointer items-center gap-8 rounded-pill px-10 text-small transition-control hover:bg-fill-1 focus-ring tap-target"
        >
          {glyph && <span className="flex size-16 items-center justify-center">{glyph}</span>}
          <span className="truncate font-medium text-fg-secondary">{title}</span>
          {count !== undefined && <span className="text-fg-tertiary tabular">{count}</span>}
          <svg viewBox="0 0 8 8" aria-hidden className={cn("size-8 text-fg-tertiary transition-transform duration-180", !collapsed && "rotate-90")}>
            <path d="M2.5 1.5 5.5 4l-3 2.5z" fill="currentColor" />
          </svg>
        </button>
        <span className="flex-1" />
        {onAdd && (
          <span className="flex opacity-0 transition-control group-hover/gh:opacity-100 group-focus-within/gh:opacity-100 coarse:opacity-100">
            <IconButton icon={icons.add} aria-label={addLabel ?? t("list.add", "Add")} size="xs" onClick={onAdd} />
          </span>
        )}
      </div>
    </div>
  );
});
GroupHeader.displayName = "GroupHeader";

export function SelectionBar({ count, onClear, children }: { count: number; onClear: () => void; children?: ReactNode | undefined }) {
  const container = usePortalContainer();
  const t = useUiText();
  if (typeof document === "undefined") return null;
  return createPortal(
    <div
      role="region"
      aria-label={t("list.selection", "Selection")}
      className="pointer-events-none fixed right-0 bottom-24 left-0 z-50 flex justify-center px-16"
    >
      <div className="pointer-events-auto flex min-h-44 items-center gap-4 rounded-pill bg-toast py-6 pr-6 pl-16 text-body font-medium text-toast-fg shadow-toast popup-motion">
        <span className="pr-8 tabular">{t("list.selected", "{count} selected", { count: String(count) })}</span>
        <span className="mr-4 h-20 w-1 bg-toast-action" aria-hidden />
        {children}
        <button
          type="button"
          onClick={onClear}
          aria-label={t("list.clearSelection", "Clear selection")}
          className="inline-flex size-32 cursor-pointer items-center justify-center rounded-pill text-toast-fg-secondary transition-control hover:bg-toast-action hover:text-toast-fg focus-ring"
        >
          <Icon icon={icons.close} size="sm" />
        </button>
      </div>
    </div>,
    container ?? document.body,
  );
}
SelectionBar.displayName = "SelectionBar";

export function SelectionAction({
  children,
  onClick,
  icon,
  danger,
}: {
  children: ReactNode;
  onClick?: (() => void) | undefined;
  icon?: IconData | undefined;
  danger?: boolean | undefined;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "inline-flex h-32 cursor-pointer items-center gap-6 rounded-pill px-12 text-body font-medium transition-control hover:bg-toast-action focus-ring",
        danger ? "text-toast-danger" : "text-toast-fg",
        icon && "pl-10",
      )}
    >
      {icon && <Icon icon={icon} size="sm" className={danger ? undefined : "text-toast-fg-secondary"} />}
      {children}
    </button>
  );
}
SelectionAction.displayName = "SelectionAction";
