import type * as React from "react";
import { forwardRef, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type ReactNode } from "react";
import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
  type Row,
  type RowSelectionState,
  type SortingState,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import { cn } from "../../utils/cn.js";
import { observeResize } from "../../utils/observe-resize.js";
import { useGutterPadded, useInset, useRaised } from "../../utils/inset.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";
import { CheckboxBox } from "../checkbox/checkbox.js";
import { Skeleton } from "../skeleton/skeleton.js";
import { Empty } from "../empty/empty.js";
import { Button } from "../button/button.js";
import { Pagination } from "../pagination/pagination.js";
import { ContextMenu } from "../menu/menu.js";
import { Text } from "../text/text.js";
import { TruncatedText } from "../text/truncated.js";
import { STATUS_TONE_TEXT, type StatusTone } from "../../utils/tone.js";
import { scrollParent } from "../../hooks/scroll-parent.js";
import { useScrollMargin } from "../../hooks/use-scroll-margin.js";
import { useUiText } from "../../i18n/context.js";
import { useScrollFadeX } from "../../hooks/use-scroll-fade.js";
import { useCoarsePointer } from "../../hooks/use-coarse-pointer.js";
import { TIME_COLUMNS } from "./time-columns.js";

export const KAESO_DATA_TABLE_VARIANTS = {
  density: {
    compact: { classes: "h-36 coarse:h-44", description: "36px rows; 44px with a coarse pointer" },
    default: { classes: "h-40 coarse:h-44", description: "40px rows (default); 44px with a coarse pointer" },
    comfortable: { classes: "h-48", description: "48px rows" },
  },
} as const;
export const KAESO_DATA_TABLE_DEFAULT_VARIANTS = { density: "default" } as const;

declare module "@tanstack/react-table" {
  interface ColumnMeta<TData, TValue> {
    align?: "start" | "end" | undefined;
    priority?: number | undefined;
    grow?: boolean | undefined;
    minWidth?: number | undefined;
    mono?: boolean | undefined;
    text?: ((row: TData) => string) | undefined;
    tone?: ((row: TData) => StatusTone | undefined) | undefined;
  }
}

export interface DataTableProps<T> {
  data: T[];
  columns: ColumnDef<T, any>[];
  getRowId?: ((row: T) => string) | undefined;
  density?: keyof typeof KAESO_DATA_TABLE_VARIANTS.density | undefined;
  onRowClick?: ((row: T) => void) | undefined;
  rowMenu?: ((row: T) => ReactNode) | undefined;
  selectable?: boolean | undefined;
  canSelectRow?: ((row: T) => boolean) | undefined;
  selection?: RowSelectionState | undefined;
  onSelectionChange?: ((selection: RowSelectionState) => void) | undefined;
  sorting?: SortingState | undefined;
  onSortingChange?: ((sorting: SortingState) => void) | undefined;
  manualSorting?: boolean | undefined;
  loading?: boolean | undefined;
  error?: ReactNode | undefined;
  onRetry?: (() => void) | undefined;
  empty?: ReactNode | undefined;
  hasMore?: boolean | undefined;
  loadingMore?: boolean | undefined;
  onLoadMore?: (() => void) | undefined;
  stickyFirstColumn?: boolean | undefined;
  virtualizeAbove?: number | undefined;
  "aria-label": string;
  className?: string | undefined;
  activeRowId?: string | undefined;
  freshRowIds?: ReadonlySet<string> | undefined;
  onHiddenColumnsChange?: ((ids: string[]) => void) | undefined;
  "data-force-row"?: string | undefined;
}

const ROW_H = { compact: 36, default: 40, comfortable: 48 } as const;

export const DATA_TABLE_GROW_MIN = 136;

type FitColumn = { id: string; size: number; priority: number; grow: boolean; minWidth: number | undefined; fixed?: boolean | undefined };

export function fitColumns(columns: FitColumn[], width: number, reserve = 48): { hidden: string[]; tight: boolean; overflow: boolean } {
  if (!width) return { hidden: [], tight: false, overflow: false };
  const byPriority = [...columns].sort((a, b) => a.priority - b.priority);
  let used = reserve;
  const keep = new Set<string>();
  for (const c of byPriority) {
    used += c.size;
    if (used <= width || c.priority <= 1 || c.fixed) keep.add(c.id);
  }
  const kept = columns.filter((c) => keep.has(c.id));
  const full = kept.reduce((a, c) => a + c.size, reserve);
  let least = kept.reduce((a, c) => a + (c.grow ? growMin(c) : c.size), 0);
  if (least > width && kept.some((c) => c.grow)) {
    const last = kept[kept.length - 1];
    for (const c of [...kept].reverse()) {
      if (least <= width) break;
      if (c.grow || c.fixed || c === last) continue;
      keep.delete(c.id);
      least -= c.size;
    }
  }
  return { hidden: columns.filter((c) => !keep.has(c.id)).map((c) => c.id), tight: full > width, overflow: least > width };
}

function contentWidth(el: HTMLElement): number {
  const style = getComputedStyle(el);
  return el.getBoundingClientRect().width - (parseFloat(style.paddingLeft) || 0) - (parseFloat(style.paddingRight) || 0);
}

function nameColumn<T>(columns: ColumnDef<T, any>[]): ColumnDef<T, any> | undefined {
  const labelled = columns.filter((c) => c.id && c.meta?.text && c.meta.align !== "end");
  return labelled.find((c) => !TIME_COLUMNS.has(c)) ?? labelled[0] ?? columns[0];
}

function rowName<T>(column: ColumnDef<T, any> | undefined, row: Row<T>): string | undefined {
  if (!column?.id) return undefined;
  const text = column.meta?.text?.(row.original);
  if (text) return text;
  const raw = row.getValue(column.id);
  return typeof raw === "string" && raw ? raw : undefined;
}

const FOCUSABLE = "a[href], button, input, select, textarea, [tabindex], [contenteditable=true]";
const TEXT_ENTRY = "input:not([type=checkbox]):not([type=radio]), textarea, select, [contenteditable=true]";

function cellWidgets(row: HTMLElement): HTMLElement[] {
  return [...row.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (w) => !w.closest("[role=checkbox], [aria-hidden=true], [hidden]") && !w.matches(":disabled, input[type=checkbox]"),
  );
}

function headTargets(head: HTMLElement | null): HTMLElement[] {
  if (!head) return [];
  return [...head.querySelectorAll<HTMLElement>("[role=columnheader]")].flatMap((th) => {
    const target = th.querySelector<HTMLElement>("[role=checkbox], button") ?? th.querySelector<HTMLElement>("[role=separator]");
    return target ? [target] : [];
  });
}

function rowTone(raised: boolean, on: boolean) {
  if (on)
    return cn(
      "controls-on-fill",
      raised ? "bg-raised-pressed hover:raised-selected-hover active:raised-selected-hover" : "bg-fill-2 hover:bg-fill-3 active:bg-fill-3",
    );
  return raised ? "hover:bg-raised-hover active:bg-raised-pressed hover:controls-on-fill" : "hover:bg-fill-1 active:bg-fill-2 hover:controls-on-fill";
}

const growMin = (c: { size: number; minWidth: number | undefined }) => Math.min(c.size, c.minWidth ?? DATA_TABLE_GROW_MIN);

function DataTableInner<T>(
  {
    data,
    columns: userColumns,
    getRowId,
    density = "default",
    onRowClick,
    rowMenu,
    selectable,
    canSelectRow,
    selection,
    onSelectionChange,
    sorting: sortingProp,
    onSortingChange,
    manualSorting,
    loading,
    error,
    onRetry,
    empty,
    hasMore,
    loadingMore,
    onLoadMore,
    stickyFirstColumn,
    virtualizeAbove = 100,
    className,
    activeRowId,
    freshRowIds,
    onHiddenColumnsChange,
    "aria-label": ariaLabel,
    "data-force-row": forceRow,
  }: DataTableProps<T>,
  ref: React.Ref<HTMLDivElement>,
) {
  const inset = useInset();
  const raised = useRaised();
  const padded = useGutterPadded();
  const t = useUiText();
  const [sortingInner, setSortingInner] = useState<SortingState>([]);
  const [selectionInner, setSelectionInner] = useState<RowSelectionState>({});
  const [focusRow, setFocusRow] = useState(0);
  const [zone, setZone] = useState<"head" | "body">("body");
  const [headStop, setHeadStop] = useState(0);
  const [anchor, setAnchor] = useState<number | null>(null);
  const sorting = sortingProp ?? sortingInner;
  const rowSelection = selection ?? selectionInner;
  const columns = useMemo<ColumnDef<T, any>[]>(() => {
    if (!selectable) return userColumns;
    return [
      {
        id: "__select",
        size: 44,
        enableSorting: false,
        enableResizing: false,
        meta: { priority: 1 },
        header: ({ table }) => (
          <CheckboxBox
            aria-label={t("dataTable.selectAllRows", "Select all rows")}
            checked={table.getIsAllRowsSelected()}
            indeterminate={table.getIsSomeRowsSelected() && !table.getIsAllRowsSelected()}
            onCheckedChange={(c) => table.toggleAllRowsSelected(c)}
          />
        ),
        cell: ({ row }) => {
          const name = rowName(nameColumn(userColumns), row);
          return (
            <CheckboxBox
              aria-label={name ? t("dataTable.selectNamed", "Select {name}", { name }) : t("dataTable.selectRow", "Select row")}
              checked={row.getIsSelected()}
              disabled={!row.getCanSelect()}
              onCheckedChange={(c) => row.toggleSelected(c)}
            />
          );
        },
      },
      ...userColumns,
    ];
  }, [userColumns, selectable, t]);

  const table = useReactTable({
    data,
    columns,
    state: { sorting, rowSelection },
    onSortingChange: (u) => {
      const next = typeof u === "function" ? u(sorting) : u;
      if (!sortingProp) setSortingInner(next);
      onSortingChange?.(next);
    },
    onRowSelectionChange: (u) => {
      const next = typeof u === "function" ? u(rowSelection) : u;
      if (!selection) setSelectionInner(next);
      onSelectionChange?.(next);
    },
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    manualSorting: !!manualSorting,
    enableRowSelection: selectable ? (canSelectRow ? (row) => canSelectRow(row.original) : true) : false,
    columnResizeMode: "onChange",
    ...(getRowId ? { getRowId: (r: T) => getRowId(r) } : {}),
    defaultColumn: { size: 140, minSize: 64, maxSize: 640 },
  });

  const root = useRef<HTMLDivElement | null>(null);
  const head = useRef<HTMLDivElement | null>(null);
  const body = useRef<HTMLDivElement | null>(null);
  const leaf = table.getVisibleLeafColumns();
  const fitInput = leaf.map((c) => ({
    id: c.id,
    size: c.getSize(),
    priority: c.columnDef.meta?.priority ?? 2,
    grow: !!c.columnDef.meta?.grow,
    minWidth: c.columnDef.meta?.minWidth,
    fixed: c.id === "__select",
  }));
  const fitKey = JSON.stringify(fitInput);
  const hiddenFor = (width: number) => {
    const fit = fitColumns(fitInput, width, inset ? 48 : 0);
    return `${fit.tight ? "!" : ""}${fit.overflow ? ">" : ""}|${fit.hidden.join(" ")}`;
  };
  const hiddenForRef = useRef(hiddenFor);
  hiddenForRef.current = hiddenFor;
  const [hiddenKey, setHiddenKey] = useState("|");
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    return observeResize([el], ([e]) => setHiddenKey(hiddenForRef.current(Math.round(e?.contentRect.width ?? 0))));
  }, []);
  useLayoutEffect(() => {
    if (root.current) setHiddenKey(hiddenForRef.current(Math.round(contentWidth(root.current))));
  }, [fitKey]);
  const [mode = "", hiddenIds = ""] = hiddenKey.split("|");
  const tight = mode.includes("!");
  const overflow = mode.includes(">");
  const hidden = useMemo(() => new Set(hiddenIds.split(" ").filter(Boolean)), [hiddenIds]);
  const onHiddenRef = useRef(onHiddenColumnsChange);
  onHiddenRef.current = onHiddenColumnsChange;
  useLayoutEffect(() => {
    onHiddenRef.current?.([...hidden]);
  }, [hidden]);
  const cols = leaf.filter((c) => !hidden.has(c.id));
  const trackMin = (c: (typeof cols)[number]) =>
    c.columnDef.meta?.grow ? (tight ? growMin({ size: c.getSize(), minWidth: c.columnDef.meta.minWidth }) : c.getSize()) : c.getSize();
  const template = cols.map((c) => (c.columnDef.meta?.grow ? `minmax(${trackMin(c)}px, 1fr)` : `${c.getSize()}px`)).join(" ");
  const gridStyle: CSSProperties = { gridTemplateColumns: template, minWidth: cols.reduce((a, c) => a + trackMin(c), 0) };
  const pinEnd = overflow && cols.length > 1 && !cols[cols.length - 1]?.columnDef.meta?.grow;

  const rows = table.getRowModel().rows;
  const virtual = rows.length > virtualizeAbove;
  const rowHeight = Math.max(ROW_H[density], useCoarsePointer() ? 44 : 0);
  const getScrollElement = () => scrollParent(root.current);
  const scrollMargin = useScrollMargin(body, getScrollElement, rows.length, virtual);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement,
    estimateSize: () => rowHeight,
    overscan: 16,
    enabled: virtual,
    scrollMargin,
  });

  const cellPad = (i: number) => cn("flex min-w-0 items-center px-12", inset && i === 0 && "pl-20", inset && i === cols.length - 1 && "pr-20");
  const onRaised = inset || raised;
  const stickyCol = (i: number, header = false, on = false) =>
    ((stickyFirstColumn && i === 0) || (pinEnd && i === cols.length - 1)) &&
    cn(
      "sticky z-10",
      header
        ? onRaised
          ? "bg-raised"
          : "bg-panel"
        : on
          ? "bg-inherit"
          : onRaised
            ? "bg-raised group-hover/tr:bg-raised-hover group-active/tr:bg-raised-pressed"
            : "bg-panel group-hover/tr:bg-fill-1 group-active/tr:bg-fill-2",
      i === 0 ? "left-0" : "right-0",
      !inset && !header && (i === 0 ? "rounded-l-item" : "rounded-r-item"),
    );

  const focusable = !!onRowClick || !!selectable;
  const bodyZone = focusable && rows.length > 0 && zone === "body";
  const stop = Math.min(focusRow, Math.max(rows.length - 1, 0));
  const focusAt = (i: number) => {
    const next = Math.max(0, Math.min(rows.length - 1, i));
    setZone("body");
    setFocusRow(next);
    if (virtual) virtualizer.scrollToIndex(next, { align: "auto" });
    const line = () => body.current?.querySelector<HTMLElement>(`[data-row-index="${next}"]`);
    const now = line();
    if (now) now.focus();
    else requestAnimationFrame(() => line()?.focus());
  };
  const focusHead = (i: number) => {
    const targets = headTargets(head.current);
    const target = targets[Math.max(0, Math.min(targets.length - 1, i))];
    if (!target) return false;
    setZone("head");
    setHeadStop(targets.indexOf(target));
    target.focus();
    return true;
  };
  const rovingHead = useRef(() => {});
  rovingHead.current = () => {
    const targets = headTargets(head.current);
    const own = Math.min(headStop, targets.length - 1);
    for (const el of head.current?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? []) {
      const tabindex = targets.indexOf(el) === own && !bodyZone ? "0" : "-1";
      if (el.getAttribute("tabindex") !== tabindex) el.setAttribute("tabindex", tabindex);
    }
  };
  useLayoutEffect(() => rovingHead.current());
  useLayoutEffect(() => {
    const el = head.current;
    if (!el) return;
    const observer = new MutationObserver(() => rovingHead.current());
    observer.observe(el, { subtree: true, childList: true, attributes: true, attributeFilter: ["tabindex"] });
    return () => observer.disconnect();
  }, []);
  useLayoutEffect(() => {
    const el = body.current;
    if (!el || !focusable) return;
    const quiet = () => {
      for (const line of el.querySelectorAll<HTMLElement>("[data-row-index]"))
        for (const w of line.querySelectorAll<HTMLElement>(FOCUSABLE)) if (w.getAttribute("tabindex") !== "-1") w.setAttribute("tabindex", "-1");
    };
    quiet();
    const observer = new MutationObserver(quiet);
    observer.observe(el, { subtree: true, childList: true, attributes: true, attributeFilter: ["tabindex"] });
    return () => observer.disconnect();
  }, [focusable]);
  useScrollFadeX(body, "[role=row]");

  const resizeBy = (columnId: string | undefined, step: number) => {
    const column = columnId ? table.getColumn(columnId) : undefined;
    if (!column?.getCanResize()) return false;
    const min = column.columnDef.minSize ?? 64;
    const max = column.columnDef.maxSize ?? 640;
    table.setColumnSizing((old) => ({ ...old, [column.id]: Math.max(min, Math.min(max, column.getSize() + step)) }));
    return true;
  };
  const onHeadKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.altKey || e.metaKey || e.ctrlKey) return;
    const targets = headTargets(head.current);
    const at = targets.indexOf(e.target as HTMLElement);
    if (at < 0) return;
    const step = { ArrowLeft: -16, ArrowRight: 16 }[e.key];
    if (e.shiftKey) {
      if (step !== undefined && resizeBy((e.target as HTMLElement).closest<HTMLElement>("[role=columnheader]")?.dataset.col, step)) e.preventDefault();
      return;
    }
    const to = { ArrowLeft: at - 1, ArrowRight: at + 1, Home: 0, End: targets.length - 1 }[e.key];
    if (to !== undefined) {
      e.preventDefault();
      if (to >= 0 && to < targets.length) focusHead(to);
    } else if (e.key === "ArrowDown" && focusable && rows.length) {
      e.preventDefault();
      focusAt(stop);
    }
  };

  const page = () => Math.max(1, Math.floor((getScrollElement()?.clientHeight ?? 400) / rowHeight) - 1);
  const selectRange = (from: number, to: number) => {
    const next = { ...rowSelection };
    for (const r of rows.slice(Math.min(from, to), Math.max(from, to) + 1)) if (r.getCanSelect()) next[r.id] = true;
    table.setRowSelection(next);
  };
  const onRowKeyDown = (e: KeyboardEvent<HTMLDivElement>, row: Row<T>, i: number) => {
    const line = e.currentTarget;
    const target = e.target as HTMLElement;
    if (target !== line) {
      if (e.altKey || e.metaKey || e.ctrlKey || target.closest(TEXT_ENTRY)) return;
      const widgets = cellWidgets(line);
      const at = widgets.indexOf(target);
      if (at < 0) return;
      // A tooltip trigger prevents Escape's default even while its tooltip is closed; only an open popup keeps the key.
      if (e.key === "Escape" && !target.closest("[data-popup-open], [aria-expanded=true]")) {
        e.preventDefault();
        line.focus();
      } else if (e.defaultPrevented) {
        return;
      } else if (e.key === "ArrowRight" && widgets[at + 1]) {
        e.preventDefault();
        widgets[at + 1]?.focus();
      } else if (e.key === "ArrowLeft") {
        e.preventDefault();
        (widgets[at - 1] ?? line).focus();
      }
      return;
    }
    if (e.altKey) return;
    if (e.metaKey || e.ctrlKey) {
      if (e.key.toLowerCase() === "a" && selectable && !e.shiftKey) {
        e.preventDefault();
        table.toggleAllRowsSelected(true);
      }
      return;
    }
    const to = { ArrowDown: i + 1, ArrowUp: i - 1, Home: 0, End: rows.length - 1, PageDown: i + page(), PageUp: i - page() }[e.key];
    if (to !== undefined) {
      e.preventDefault();
      if (to < 0 && focusHead(headStop)) return;
      const next = Math.max(0, Math.min(rows.length - 1, to));
      if (e.shiftKey && selectable) {
        const from = anchor ?? i;
        setAnchor(from);
        selectRange(from, next);
      } else setAnchor(null);
      focusAt(next);
    } else if (e.shiftKey) {
      return;
    } else if (e.key === "ArrowRight") {
      const first = cellWidgets(line)[0];
      if (!first) return;
      e.preventDefault();
      first.focus();
    } else if (e.key === "Enter") {
      e.preventDefault();
      onRowClick?.(row.original);
    } else if ((e.key === " " || e.key === "x") && selectable) {
      e.preventDefault();
      if (row.getCanSelect()) row.toggleSelected();
    } else if (e.key === "Escape" && selectable && Object.keys(rowSelection).length) {
      e.preventDefault();
      table.toggleAllRowsSelected(false);
    }
  };

  const renderRow = (row: Row<T>, i: number, style?: CSSProperties) => {
    const cells = row.getVisibleCells().filter((c) => !hidden.has(c.column.id));
    const active = row.id === activeRowId;
    const on = active || (!!selectable && row.getIsSelected());
    const el = (
      <div
        role="row"
        aria-rowindex={i + 2}
        aria-selected={selectable ? row.getIsSelected() : undefined}
        aria-current={active || undefined}
        data-active={active || undefined}
        data-fresh={freshRowIds?.has(row.id) || undefined}
        data-row-index={i}
        {...(forceRow && i === 0 ? { "data-force": forceRow } : {})}
        tabIndex={focusable ? (bodyZone && i === stop ? 0 : -1) : undefined}
        onFocus={(e) => {
          if (e.target !== e.currentTarget) return;
          setZone("body");
          if (i !== focusRow) setFocusRow(i);
        }}
        onClick={(e) => {
          if ((e.target as HTMLElement).closest("button, a, [role=checkbox]")) return;
          onRowClick?.(row.original);
        }}
        onKeyDown={(e) => onRowKeyDown(e, row, i)}
        style={{ ...gridStyle, ...style }}
        className={cn(
          "group/tr grid w-full text-body text-fg transition-control focus-ring-inset",
          !inset && "rounded-item",
          KAESO_DATA_TABLE_VARIANTS.density[density].classes,
          onRowClick && "cursor-pointer",
          rowTone(onRaised, on),
          "data-fresh:animate-row-fresh",
          "forced-colors:data-active:outline-2 forced-colors:data-active:-outline-offset-2 forced-colors:data-active:outline-solid",
        )}
      >
        {cells.map((cell, i) => {
          const meta = cell.column.columnDef.meta;
          const content = flexRender(cell.column.columnDef.cell, cell.getContext());
          const raw = cell.getValue();
          const text = meta?.text ? meta.text(row.original) : typeof raw === "string" ? raw : undefined;
          const tone = meta?.tone?.(row.original);
          return (
            <div
              key={cell.id}
              role="cell"
              className={cn(
                cellPad(i),
                meta?.align === "end" && "justify-end text-right tabular",
                meta?.mono && "mono text-small",
                tone && STATUS_TONE_TEXT[tone],
                stickyCol(i, false, on),
              )}
            >
              {text !== undefined && typeof content !== "object" ? (
                <Text truncate tone={tone ?? "default"} className={cn("min-w-0", meta?.mono && "text-small")} mono={!!meta?.mono}>
                  {text}
                </Text>
              ) : cell.column.id === "__select" ? (
                <span className="flex">{content}</span>
              ) : (
                <TruncatedText className={cn("min-w-0 cell-fit", meta?.grow && "flex-1")}>{content}</TruncatedText>
              )}
            </div>
          );
        })}
      </div>
    );
    if (!rowMenu) return <div key={row.id}>{el}</div>;
    return (
      <ContextMenu key={row.id}>
        <ContextMenu.Trigger>{el}</ContextMenu.Trigger>
        <ContextMenu.Content>{rowMenu(row.original)}</ContextMenu.Content>
      </ContextMenu>
    );
  };

  const header = (h: ReturnType<typeof table.getHeaderGroups>[number]["headers"][number], i: number) => {
    const meta = h.column.columnDef.meta;
    const sortable = h.column.getCanSort();
    const resizable = h.column.getCanResize();
    const dir = h.column.getIsSorted();
    const label = flexRender(h.column.columnDef.header, h.getContext());
    const content = (
      <>
        {sortable ? (
          <button
            type="button"
            onClick={h.column.getToggleSortingHandler()}
            className={cn(
              "relative -mx-6 inline-flex h-28 min-w-0 cursor-pointer items-center gap-4 rounded-pill px-6 text-caption font-medium transition-control focus-ring tap-target",
              onRaised ? "hover:bg-raised-hover active:bg-raised-pressed" : "hover:bg-fill-1 active:bg-fill-2",
              "hover:text-fg",
              dir ? "text-fg" : "text-fg-secondary",
              meta?.align === "end" && "flex-row-reverse",
            )}
          >
            <TruncatedText>{label}</TruncatedText>
            <Icon
              icon={dir === "desc" ? icons.chevronDown : icons.chevronUp}
              size="xs"
              className={cn("shrink-0 transition-control", dir ? "opacity-100" : "opacity-0 group-hover/th:opacity-60")}
            />
          </button>
        ) : (
          <TruncatedText className="text-caption font-medium text-fg-secondary">{label}</TruncatedText>
        )}
        {resizable && (
          <div
            role="separator"
            aria-orientation="vertical"
            aria-label={t("dataTable.resizeColumn", "Resize column")}
            aria-valuenow={h.column.getSize()}
            aria-valuemin={h.column.columnDef.minSize ?? 64}
            aria-valuemax={h.column.columnDef.maxSize ?? 640}
            tabIndex={-1}
            onMouseDown={h.getResizeHandler()}
            onTouchStart={h.getResizeHandler()}
            onDoubleClick={() => h.column.resetSize()}
            className="absolute inset-y-0 -right-8 z-10 w-16 cursor-col-resize after:absolute after:inset-y-8 after:right-8 after:w-1 after:bg-transparent after:transition-control group-hover/th:after:bg-line-strong coarse:after:bg-line focus-ring-inset"
          />
        )}
      </>
    );
    return (
      <div
        key={h.id}
        role="columnheader"
        data-col={h.column.id}
        aria-sort={dir === "asc" ? "ascending" : dir === "desc" ? "descending" : sortable ? "none" : undefined}
        className={cn("group/th relative", cellPad(i), meta?.align === "end" && "justify-end", stickyCol(i, true), stickyCol(i, true) && "hairline-b-strong")}
      >
        {resizable ? (
          <ContextMenu>
            <ContextMenu.Trigger className={cn("flex min-w-0 flex-1 items-center self-stretch", meta?.align === "end" && "justify-end")}>
              {content}
            </ContextMenu.Trigger>
            <ContextMenu.Content>
              <ContextMenu.Item onClick={() => resizeBy(h.column.id, 32)}>{t("dataTable.widenColumn", "Widen column")}</ContextMenu.Item>
              <ContextMenu.Item onClick={() => resizeBy(h.column.id, -32)}>{t("dataTable.narrowColumn", "Narrow column")}</ContextMenu.Item>
              <ContextMenu.Item onClick={() => h.column.resetSize()}>{t("dataTable.resetWidth", "Reset width")}</ContextMenu.Item>
            </ContextMenu.Content>
          </ContextMenu>
        ) : (
          content
        )}
      </div>
    );
  };

  return (
    <div
      ref={(e) => {
        root.current = e;
        if (typeof ref === "function") ref(e);
        else if (ref) (ref as { current: HTMLDivElement | null }).current = e;
      }}
      className={cn("relative w-full min-w-0", !inset && !padded && "px-24", className)}
    >
      <div
        role="table"
        aria-label={ariaLabel}
        aria-rowcount={rows.length + 1}
        aria-busy={loading || undefined}
        aria-multiselectable={selectable || undefined}
        className={cn("w-full overflow-clip", inset && "rounded-group bg-raised shadow-xs")}
      >
        <div
          ref={head}
          role="rowgroup"
          className={cn("sticky top-0 z-20 overflow-hidden", onRaised ? "bg-raised" : "bg-panel")}
          onKeyDown={onHeadKeyDown}
          onFocus={(e) => {
            const at = headTargets(head.current).indexOf(e.target as HTMLElement);
            if (at < 0) return;
            setZone("head");
            setHeadStop(at);
          }}
          onScroll={(e) => {
            if (body.current && body.current.scrollLeft !== e.currentTarget.scrollLeft) body.current.scrollLeft = e.currentTarget.scrollLeft;
          }}
        >
          {table.getHeaderGroups().map((hg) => (
            <div key={hg.id} role="row" aria-rowindex={1} className="grid h-36 hairline-b-strong" style={gridStyle}>
              {hg.headers.filter((h) => !hidden.has(h.column.id)).map(header)}
            </div>
          ))}
        </div>
        <div
          ref={body}
          role="rowgroup"
          className="overflow-x-auto scrollbar-quiet scroll-fade-x"
          onScroll={(e) => {
            if (head.current && head.current.scrollLeft !== e.currentTarget.scrollLeft) head.current.scrollLeft = e.currentTarget.scrollLeft;
          }}
        >
          {error ? (
            <div role="row">
              <div role="cell" aria-colspan={cols.length} className="block">
                <div role="alert">
                  <Empty
                    size="compact"
                    title={t("dataTable.loadFailed", "Couldn't load these rows")}
                    {...(onRetry ? {} : { description: t("dataTable.reloadToRetry", "Reload the page to try again.") })}
                    actions={
                      onRetry && (
                        <Button size="sm" onClick={onRetry}>
                          {t("dataTable.tryAgain", "Try again")}
                        </Button>
                      )
                    }
                  />
                </div>
              </div>
            </div>
          ) : loading && rows.length === 0 ? (
            Array.from({ length: 8 }, (_, r) => (
              <div key={r} role="row" className={cn("grid", KAESO_DATA_TABLE_VARIANTS.density[density].classes)} style={gridStyle}>
                {cols.map((c, i) => (
                  <div key={c.id} role="cell" className={cn(cellPad(i), c.columnDef.meta?.align === "end" && "justify-end")}>
                    <Skeleton shape="line" style={{ width: `${40 + ((r * 7 + i * 13) % 45)}%` }} />
                  </div>
                ))}
              </div>
            ))
          ) : rows.length === 0 ? (
            <div role="row">
              <div role="cell" aria-colspan={cols.length} className="block">
                {empty ?? <Empty size="compact" title={t("dataTable.noRows", "No rows")} />}
              </div>
            </div>
          ) : virtual ? (
            <div className="relative" style={{ height: virtualizer.getTotalSize() }}>
              {virtualizer.getVirtualItems().map((v) =>
                renderRow(rows[v.index]!, v.index, {
                  position: "absolute",
                  top: 0,
                  left: 0,
                  transform: `translateY(${Math.round(v.start - scrollMargin)}px)`,
                }),
              )}
            </div>
          ) : (
            rows.map((r, i) => renderRow(r, i))
          )}
        </div>
      </div>
      {hasMore && <Pagination variant="load-more" hasMore loading={loadingMore} onLoadMore={onLoadMore} />}
    </div>
  );
}

const DataTableBase = forwardRef(DataTableInner);
DataTableBase.displayName = "DataTable";
export const DataTable = DataTableBase as unknown as <T>(
  p: DataTableProps<T> & { ref?: React.Ref<HTMLDivElement> | undefined },
) => ReturnType<typeof DataTableInner>;
