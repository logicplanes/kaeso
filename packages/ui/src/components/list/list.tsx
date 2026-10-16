import { forwardRef, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";
import { cn } from "../../utils/cn";
import { useInset } from "../../utils/inset";
import { icons } from "../../icons/vocabulary";
import { Icon, type IconData } from "../icon/icon";
import { IconButton } from "../icon-button/icon-button";
import { Text } from "../text/text";
import { Skeleton } from "../skeleton/skeleton";
import { scrollParent } from "../../hooks/scroll-parent";
import { useLayoutWidth } from "../../hooks/use-layout-width";
import { createPortal } from "react-dom";
import { usePortalContainer } from "../../utils/portal-context";
import { lazyModule, useLazyModule } from "../../utils/lazy-module";
import { RowCheckbox } from "./row-checkbox";
import { useUiText } from "../../i18n/context";

const virtualRows = lazyModule(() => import("./virtual-rows"));

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
  const rowIndexes = useMemo(() => flat.map((f, i) => (f.kind === "row" ? i : -1)).filter((i) => i >= 0), [flat]);
  const none = groups ? groups.every((g) => g.items.length === 0) : (items ?? []).length === 0;

  const virtual = flat.length > virtualizeAbove;
  const estimateSize = (i: number) => (flat[i]?.kind === "header" ? (i === 0 ? 36 : 52) : narrow ? 64 : density === "compact" ? 36 : 44);
  const VirtualRows = useLazyModule(virtualRows, virtual)?.VirtualRows;
  const scrollTo = useRef<((index: number) => void) | null>(null);

  const focusRow = (i: number) => {
    setActive(i);
    if (virtual) scrollTo.current?.(i);
    requestAnimationFrame(() => root.current?.querySelector<HTMLElement>(`[data-index="${i}"]`)?.focus());
  };
  const hadVirtualRows = useRef(!!VirtualRows);
  useLayoutEffect(() => {
    if (VirtualRows && !hadVirtualRows.current && active >= 0 && document.activeElement === document.body) {
      root.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.focus();
    }
    hadVirtualRows.current = !!VirtualRows;
  });
  const toggle = (id: string) => {
    const s = new Set(selected);
    if (s.has(id)) s.delete(id);
    else s.add(id);
    setSelected(s);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (!e.currentTarget.contains(e.target as Node) || (e.target as HTMLElement).closest("input, button:not([data-row])")) return;
    const pos = rowIndexes.indexOf(active);
    const move = (d: number) => {
      const next = rowIndexes[Math.max(0, Math.min(rowIndexes.length - 1, (pos < 0 ? (d > 0 ? -1 : rowIndexes.length) : pos) + d))];
      if (next === undefined) return;
      if (e.shiftKey && selectable) {
        const from = anchor ?? active;
        setAnchor(from);
        const [a, b] = [Math.min(from, next), Math.max(from, next)];
        const s = new Set(selected);
        flat.slice(a, b + 1).forEach((f) => f.kind === "row" && s.add(f.item.id));
        setSelected(s);
      } else setAnchor(null);
      focusRow(next);
    };
    if (e.key === "ArrowDown" || e.key === "j") {
      e.preventDefault();
      move(1);
    } else if (e.key === "ArrowUp" || e.key === "k") {
      e.preventDefault();
      move(-1);
    } else if (e.key === "Enter" && active >= 0) {
      const f = flat[active];
      if (f?.kind === "row") onOpen?.(f.item);
    } else if (e.key === "x" && selectable && active >= 0) {
      const f = flat[active];
      if (f?.kind === "row") toggle(f.item.id);
    } else if ((e.metaKey || e.ctrlKey) && e.key === "a" && selectable) {
      e.preventDefault();
      setSelected(new Set(flat.flatMap((f) => (f.kind === "row" ? [f.item.id] : []))));
    } else if (e.key === "Escape" && selected.size) {
      setSelected(new Set());
    }
  };
  useEffect(() => {
    if (active >= flat.length) setActive(-1);
  }, [flat.length, active]);

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
        active={i === active}
        selected={selected.has(f.item.id)}
        selectable={!!selectable}
        selecting={selecting}
        onSelect={() => toggle(f.item.id)}
        onOpen={() => onOpen?.(f.item)}
        onFocus={() => setActive(i)}
        {...(forceRow && i === rowIndexes[0] ? { "data-force": forceRow } : {})}
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
      onKeyDown={onKeyDown}
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
        aria-selected={selected ?? false}
        aria-disabled={item.disabled || undefined}
        data-index={index}
        data-row=""
        tabIndex={active || (index === 0 && !active) ? 0 : -1}
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
}

export const GroupHeader = forwardRef<HTMLDivElement, GroupHeaderProps>(function GroupHeader(
  { title, glyph, count, collapsed, onToggle, onAdd, addLabel, indent, first },
  ref,
) {
  const card = useInset();
  const t = useUiText();
  return (
    <div ref={ref} role="row" className={card ? cn("group/gh bg-fill-1", !first && "hairline-t") : "group/gh px-8 pt-16 first:pt-0"}>
      <div role="gridcell" className={cn("flex items-center gap-4 pr-8", card ? "h-36 pl-6" : "h-36", !card && (indent ? "pl-36" : "pl-6"))}>
        <button
          type="button"
          aria-expanded={!collapsed}
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

export function SelectionBar({ count, onClear, children }: { count: number; onClear: () => void; children?: ReactNode }) {
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
  onClick?: () => void;
  icon?: IconData | undefined;
  danger?: boolean;
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
