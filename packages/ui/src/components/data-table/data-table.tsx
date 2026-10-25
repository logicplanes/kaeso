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
import { STATUS_TONE_TEXT, type StatusTone } from "../../utils/tone.js";
import { scrollParent } from "../../hooks/scroll-parent.js";
import { useScrollMargin } from "../../hooks/use-scroll-margin.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_DATA_TABLE_VARIANTS = {
  density: {
    compact: { classes: "h-36", description: "36px rows" },
    default: { classes: "h-40", description: "40px rows (default)" },
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

function rowName<T>(column: ColumnDef<T, any> | undefined, row: Row<T>): string | undefined {
  if (!column?.id) return undefined;
  const text = column.meta?.text?.(row.original);
  if (text) return text;
  const raw = row.getValue(column.id);
  return typeof raw === "string" && raw ? raw : undefined;
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
          const name = rowName(userColumns[0], row);
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
  const getScrollElement = () => scrollParent(root.current);
  const scrollMargin = useScrollMargin(body, getScrollElement, rows.length, virtual);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement,
    estimateSize: () => ROW_H[density],
    overscan: 16,
    enabled: virtual,
    scrollMargin,
  });

  const cellPad = (i: number) => cn("flex min-w-0 items-center px-12", inset && i === 0 && "pl-20", inset && i === cols.length - 1 && "pr-20");
  const stickyCol = (i: number, header = false) =>
    ((stickyFirstColumn && i === 0) || (pinEnd && i === cols.length - 1)) &&
    cn(
      "sticky z-10 group-hover/tr:bg-fill-1 group-aria-selected/tr:bg-fill-2 group-data-active/tr:bg-fill-2",
      i === 0 ? "left-0" : "right-0",
      inset ? "bg-raised" : cn(raised ? "bg-raised" : "bg-panel", !header && (i === 0 ? "rounded-l-item" : "rounded-r-item")),
    );

  const focusable = !!onRowClick || !!selectable;
  const stop = Math.min(focusRow, Math.max(rows.length - 1, 0));
  const focusAt = (i: number) => {
    const next = Math.max(0, Math.min(rows.length - 1, i));
    setFocusRow(next);
    if (virtual) virtualizer.scrollToIndex(next, { align: "auto" });
    const line = () => body.current?.querySelector<HTMLElement>(`[data-row-index="${next}"]`);
    const now = line();
    if (now) now.focus();
    else requestAnimationFrame(() => line()?.focus());
  };
  const page = () => Math.max(1, Math.floor((getScrollElement()?.clientHeight ?? 400) / ROW_H[density]) - 1);
  const onRowKeyDown = (e: KeyboardEvent<HTMLDivElement>, row: Row<T>, i: number) => {
    if (e.target !== e.currentTarget || e.altKey || e.metaKey || e.ctrlKey) return;
    const to = { ArrowDown: i + 1, ArrowUp: i - 1, Home: 0, End: rows.length - 1, PageDown: i + page(), PageUp: i - page() }[e.key];
    if (to !== undefined) {
      e.preventDefault();
      focusAt(to);
    } else if (e.key === "Enter") {
      e.preventDefault();
      onRowClick?.(row.original);
    } else if (e.key === " " && selectable) {
      e.preventDefault();
      row.toggleSelected();
    }
  };

  const renderRow = (row: Row<T>, i: number, style?: CSSProperties) => {
    const cells = row.getVisibleCells().filter((c) => !hidden.has(c.column.id));
    const el = (
      <div
        role="row"
        aria-rowindex={i + 2}
        aria-selected={selectable ? row.getIsSelected() : undefined}
        data-active={row.id === activeRowId || undefined}
        data-fresh={freshRowIds?.has(row.id) || undefined}
        data-row-index={i}
        tabIndex={focusable ? (i === stop ? 0 : -1) : undefined}
        onFocus={(e) => {
          if (e.target === e.currentTarget && i !== focusRow) setFocusRow(i);
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
          "hover:bg-fill-1 aria-selected:bg-fill-2 data-active:bg-fill-2 data-fresh:animate-row-fresh",
          "hover:controls-on-fill aria-selected:controls-on-fill data-active:controls-on-fill",
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
                stickyCol(i),
              )}
            >
              {text !== undefined && typeof content !== "object" ? (
                <Text truncate tone={tone ?? "default"} className={cn("min-w-0", meta?.mono && "text-small")} mono={!!meta?.mono}>
                  {text}
                </Text>
              ) : cell.column.id === "__select" ? (
                <span className="flex">{content}</span>
              ) : (
                <span className={cn("min-w-0 truncate cell-fit", meta?.grow && "flex-1")}>{content}</span>
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
        className={cn("w-full overflow-clip", inset && "rounded-group bg-raised shadow-xs")}
      >
        <div
          ref={head}
          role="rowgroup"
          className={cn("sticky top-0 z-20 overflow-hidden", inset || raised ? "bg-raised" : "bg-panel")}
          onScroll={(e) => {
            if (body.current && body.current.scrollLeft !== e.currentTarget.scrollLeft) body.current.scrollLeft = e.currentTarget.scrollLeft;
          }}
        >
          {table.getHeaderGroups().map((hg) => (
            <div key={hg.id} role="row" aria-rowindex={1} className="grid h-36 hairline-b-strong" style={gridStyle}>
              {hg.headers
                .filter((h) => !hidden.has(h.column.id))
                .map((h, i) => {
                  const meta = h.column.columnDef.meta;
                  const sortable = h.column.getCanSort();
                  const dir = h.column.getIsSorted();
                  return (
                    <div
                      key={h.id}
                      role="columnheader"
                      aria-sort={dir === "asc" ? "ascending" : dir === "desc" ? "descending" : sortable ? "none" : undefined}
                      className={cn(
                        "group/th relative",
                        cellPad(i),
                        meta?.align === "end" && "justify-end",
                        stickyCol(i, true),
                        stickyCol(i, true) && "hairline-b-strong",
                      )}
                    >
                      {sortable ? (
                        <button
                          type="button"
                          onClick={h.column.getToggleSortingHandler()}
                          className={cn(
                            "relative -mx-6 inline-flex h-28 min-w-0 cursor-pointer items-center gap-4 rounded-pill px-6 text-caption font-medium transition-control hover:bg-fill-1 hover:text-fg focus-ring tap-target",
                            dir ? "text-fg" : "text-fg-secondary",
                            meta?.align === "end" && "flex-row-reverse",
                          )}
                        >
                          <span className="truncate">{flexRender(h.column.columnDef.header, h.getContext())}</span>
                          <Icon
                            icon={dir === "desc" ? icons.chevronDown : icons.chevronUp}
                            size="xs"
                            className={cn("shrink-0 transition-control", dir ? "opacity-100" : "opacity-0 group-hover/th:opacity-60")}
                          />
                        </button>
                      ) : (
                        <span className="truncate text-caption font-medium text-fg-secondary">{flexRender(h.column.columnDef.header, h.getContext())}</span>
                      )}
                      {h.column.getCanResize() && (
                        <div
                          role="separator"
                          aria-orientation="vertical"
                          aria-label={t("dataTable.resizeColumn", "Resize column")}
                          aria-valuenow={h.column.getSize()}
                          aria-valuemin={h.column.columnDef.minSize ?? 64}
                          aria-valuemax={h.column.columnDef.maxSize ?? 640}
                          tabIndex={0}
                          onKeyDown={(e) => {
                            const step = { ArrowLeft: -16, ArrowRight: 16 }[e.key];
                            if (step === undefined) return;
                            e.preventDefault();
                            const id = h.column.id;
                            const min = h.column.columnDef.minSize ?? 64;
                            const max = h.column.columnDef.maxSize ?? 640;
                            table.setColumnSizing((old) => ({ ...old, [id]: Math.max(min, Math.min(max, h.column.getSize() + step)) }));
                          }}
                          onMouseDown={h.getResizeHandler()}
                          onTouchStart={h.getResizeHandler()}
                          onDoubleClick={() => h.column.resetSize()}
                          className="absolute top-8 right-0 bottom-8 w-8 cursor-col-resize rounded-pill after:absolute after:inset-y-0 after:right-0 after:w-1 after:bg-transparent after:transition-control group-hover/th:after:bg-line-strong focus-ring-inset"
                        />
                      )}
                    </div>
                  );
                })}
            </div>
          ))}
        </div>
        <div
          ref={body}
          role="rowgroup"
          className="overflow-x-auto scrollbar-quiet"
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
                {empty ?? (
                  <Empty size="compact" title={t("dataTable.noRows", "No rows")} description={t("dataTable.noMatches", "Nothing matches these filters.")} />
                )}
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
