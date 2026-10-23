import {
  createContext,
  forwardRef,
  useCallback,
  useContext,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { defaultRangeExtractor, observeElementRect, useVirtualizer, type Range, type Virtualizer } from "@tanstack/react-virtual";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";
import { Highlight } from "../combobox/combobox.js";
import { MENU_HINT, MENU_ITEM, MENU_LABEL } from "../menu/styles.js";
import { OptionCheckbox } from "../select/option-checkbox.js";
import { useUiText } from "../../i18n/context.js";
import { commandMatch } from "./match.js";

export const KAESO_COMMAND_LIST_VARIANTS = {
  height: {
    popover: { classes: "picker-limits", description: "At most 480px or 70% of the window, shrinking to its rows: a popover (default)" },
    inline: {
      classes: "max-h-360 rounded-group bg-raised shadow-xs",
      description: "Its rows' height up to 360px in a raised frame, kept while searching: a dialog step",
    },
    fill: { classes: "min-h-0 flex-1", description: "Fills its parent: a phone sheet or a command palette" },
  },
  size: {
    md: { classes: "", description: "A 44px search over 34px rows (default)" },
    lg: { classes: "", description: "A 56px search over 40px rows: the command palette" },
  },
} as const;
export const KAESO_COMMAND_LIST_DEFAULT_VARIANTS = { height: "popover", size: "md" } as const;

const SIZES = {
  md: { search: "h-44 gap-10 px-16", input: "text-body", row: 34, item: "" },
  lg: { search: "h-56 gap-12 px-20", input: "text-body-lg font-regular", row: 40, item: "rounded-palette-item gap-12 px-12" },
} as const;

export type CommandListHeight = keyof typeof KAESO_COMMAND_LIST_VARIANTS.height;

export interface CommandListRowState {
  query: string;
  active: boolean;
  selected: boolean;
}

export interface CommandListItem {
  value: string;
  label: string;
  keywords?: readonly string[] | undefined;
  leading?: ReactNode | undefined;
  meta?: ReactNode | undefined;
  render?: ((state: CommandListRowState) => ReactNode) | undefined;
  disabled?: boolean | undefined;
}

export interface CommandListGroup {
  id: string;
  label?: string | undefined;
  leading?: ReactNode | undefined;
  count?: ReactNode | undefined;
  loading?: boolean | undefined;
  items: CommandListItem[];
}

export interface CommandListProps {
  groups: CommandListGroup[];
  "aria-label": string;
  value?: string | readonly string[] | null | undefined;
  multiple?: boolean | undefined;
  onSelect?: ((value: string) => void) | undefined;
  query?: string | undefined;
  defaultQuery?: string | undefined;
  onQueryChange?: ((query: string) => void) | undefined;
  placeholder?: string | undefined;
  emptyText?: ReactNode | undefined;
  toolbar?: ReactNode | undefined;
  footer?: ReactNode | undefined;
  searchPrefix?: ReactNode | undefined;
  loading?: boolean | undefined;
  onInputKeyDown?: ((event: KeyboardEvent<HTMLInputElement>) => void) | undefined;
  size?: keyof typeof KAESO_COMMAND_LIST_VARIANTS.size | undefined;
  height?: CommandListHeight | undefined;
  autoFocus?: boolean | undefined;
  onEscape?: (() => void) | undefined;
  escapeClears?: boolean | undefined;
  match?: ((item: CommandListItem, query: string) => number) | undefined;
  inputProps?: { [attribute: `data-${string}`]: string } | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

const COARSE = "(pointer: coarse)";
const subscribeCoarse = (onChange: () => void) => {
  if (typeof window === "undefined" || !window.matchMedia) return () => {};
  const q = window.matchMedia(COARSE);
  q.addEventListener("change", onChange);
  return () => q.removeEventListener("change", onChange);
};
const isCoarse = () => typeof window !== "undefined" && !!window.matchMedia?.(COARSE).matches;

const HEADER = 28;
const EDGE = 6;
const SEARCH = { md: 44, lg: 56 } as const;
const EMPTY = 80;
const FALLBACK_VIEWPORT = 480;
const VIRTUALIZE_ABOVE = 200;

export const CommandListHeightContext = createContext<CommandListHeight | null>(null);

type Row = { kind: "header"; group: CommandListGroup; gi: number } | { kind: "item"; item: CommandListItem; gi: number; option: number; score: number };

const fuzzy = (item: CommandListItem, query: string) => commandMatch(query, [item.label, ...(item.keywords ?? [])]);

function visibleGroups(groups: CommandListGroup[], query: string, match: (item: CommandListItem, query: string) => number) {
  const q = query.trim();
  const kept = (g: { group: CommandListGroup; items: unknown[] }) => g.items.length > 0 || !!g.group.loading;
  if (!q) return groups.map((g) => ({ group: g, items: g.items.map((item) => ({ item, score: 1 })) })).filter(kept);
  return groups
    .map((g, order) => {
      const items = g.items
        .map((item) => ({ item, score: match(item, q) }))
        .filter((m) => m.score > 0)
        .sort((a, b) => b.score - a.score);
      return { group: g, items, order, best: items[0]?.score ?? 0 };
    })
    .filter(kept)
    .sort((a, b) => b.best - a.best || a.order - b.order);
}

function observeWithFallback(instance: Virtualizer<HTMLDivElement, Element>, cb: (rect: { width: number; height: number }) => void) {
  return observeElementRect(instance, (rect) => cb(rect.height > 0 ? rect : { width: rect.width, height: FALLBACK_VIEWPORT }));
}

export const CommandList = forwardRef<HTMLInputElement, CommandListProps>(function CommandList(
  {
    groups,
    "aria-label": ariaLabel,
    value,
    multiple,
    onSelect,
    query: controlledQuery,
    defaultQuery = "",
    onQueryChange,
    placeholder,
    emptyText,
    toolbar,
    footer,
    searchPrefix,
    loading,
    onInputKeyDown,
    size = "md",
    height: heightProp,
    autoFocus,
    onEscape,
    escapeClears = true,
    match = fuzzy,
    inputProps,
    className,
    "data-force": force,
  },
  ref,
) {
  const t = useUiText();
  const fromFrame = useContext(CommandListHeightContext);
  const height = heightProp ?? fromFrame ?? "popover";
  const uid = useId();
  const listId = `${uid}-list`;
  const [ownQuery, setOwnQuery] = useState(defaultQuery);
  const query = controlledQuery ?? ownQuery;
  const coarse = useSyncExternalStore(subscribeCoarse, isCoarse, () => false);
  const look = SIZES[size];
  const rowHeight = coarse ? Math.max(44, look.row) : look.row;
  const commands = value === undefined && !multiple;
  const checks = !multiple && !commands;
  const selected = useMemo(() => new Set(value == null ? [] : typeof value === "string" ? [value] : value), [value]);

  const shown = useMemo(() => visibleGroups(groups, query, match), [groups, query, match]);
  const { rows, options, spans } = useMemo(() => {
    const rows: Row[] = [];
    const options: { row: number; item: CommandListItem; gi: number }[] = [];
    const spans: { first: number; last: number }[] = [];
    shown.forEach(({ group, items }, gi) => {
      const first = rows.length;
      if (group.label) rows.push({ kind: "header", group, gi });
      for (const { item, score } of items) {
        options.push({ row: rows.length, item, gi });
        rows.push({ kind: "item", item, gi, option: options.length - 1, score });
      }
      spans.push({ first, last: rows.length - 1 });
    });
    return { rows, options, spans };
  }, [shown]);
  const labelled = shown.some((g) => g.group.label);
  const fits = height === "inline" && !toolbar && !footer;
  const unfiltered = useMemo(
    () => (fits ? visibleGroups(groups, "", match).reduce((sum, { group, items }) => sum + (group.label ? HEADER : 0) + items.length * rowHeight, 0) : 0),
    [fits, groups, match, rowHeight],
  );
  const fitted = fits ? { height: SEARCH[size] + 2 * EDGE + Math.max(unfiltered, EMPTY) } : undefined;
  const offsets = useMemo(() => {
    const out = [EDGE];
    for (const row of rows) out.push((out[out.length - 1] ?? EDGE) + (row.kind === "header" ? HEADER : rowHeight));
    return out;
  }, [rows, rowHeight]);

  const firstEnabled = useCallback(
    (from: number, step: 1 | -1) => {
      for (let i = from; i >= 0 && i < options.length; i += step) if (!options[i]?.item.disabled) return i;
      return -1;
    },
    [options],
  );
  const initialActive = () => {
    const chosen = options.findIndex((o) => selected.has(o.item.value) && !o.item.disabled);
    return chosen >= 0 ? chosen : firstEnabled(0, 1);
  };
  const [active, setActive] = useState(initialActive);
  const signature = useMemo(() => `${query.trim()}\u0000${options.map((o) => o.item.value).join("\u0000")}`, [options, query]);
  const [shownFor, setShownFor] = useState(signature);
  if (shownFor !== signature) {
    setShownFor(signature);
    setActive(query.trim() ? firstEnabled(0, 1) : initialActive());
  }
  const activeRef = useRef(active);
  activeRef.current = active;

  const scroller = useRef<HTMLDivElement | null>(null);
  const virtual = rows.length > VIRTUALIZE_ABOVE;
  const virtualizer = useVirtualizer<HTMLDivElement, Element>({
    enabled: virtual,
    count: rows.length,
    getScrollElement: () => scroller.current,
    estimateSize: (i) => (rows[i]?.kind === "header" ? HEADER : rowHeight),
    overscan: 8,
    useFlushSync: false,
    paddingStart: EDGE,
    paddingEnd: EDGE,
    scrollPaddingStart: labelled ? HEADER : 0,
    observeElementRect: observeWithFallback,
    initialRect: { width: 0, height: FALLBACK_VIEWPORT },
    rangeExtractor: (range: Range) => {
      const activeRow = options[activeRef.current]?.row;
      const keep = defaultRangeExtractor(range);
      return activeRow === undefined || keep.includes(activeRow) ? keep : [...keep, activeRow].sort((a, b) => a - b);
    },
  });
  const keyboardMove = useRef(false);
  useLayoutEffect(() => {
    if (virtual) virtualizer.measure();
  }, [rowHeight, virtualizer, virtual]);
  const reveal = (row: number, align: "auto" | "center") => {
    if (virtual) return virtualizer.scrollToIndex(row, { align });
    const option = options.find((o) => o.row === row);
    const el = option ? document.getElementById(optionId(options.indexOf(option))) : null;
    el?.scrollIntoView?.({ block: align === "auto" ? "nearest" : "center" });
  };
  useEffect(() => {
    const row = options[active]?.row;
    if (row === undefined) return;
    if (keyboardMove.current) {
      keyboardMove.current = false;
      reveal(row, "auto");
    }
  });
  const opened = useRef(false);
  useLayoutEffect(() => {
    if (opened.current) return;
    opened.current = true;
    const row = options[active]?.row;
    if (row !== undefined && row > 0) reveal(row, "center");
  });

  const setQuery = (next: string) => {
    if (controlledQuery === undefined) setOwnQuery(next);
    onQueryChange?.(next);
  };
  const move = (to: number) => {
    if (to < 0) return;
    keyboardMove.current = true;
    setActive(to);
  };
  const page = Math.max(1, Math.floor((scroller.current?.clientHeight || FALLBACK_VIEWPORT) / rowHeight) - 1);
  const choose = (i: number) => {
    const o = options[i];
    if (!o || o.item.disabled) return;
    setActive(i);
    onSelect?.(o.item.value);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    onInputKeyDown?.(e);
    if (e.defaultPrevented || e.altKey || e.metaKey || e.ctrlKey) return;
    const last = options.length - 1;
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        move(active < 0 ? firstEnabled(0, 1) : firstEnabled(active + 1, 1) >= 0 ? firstEnabled(active + 1, 1) : firstEnabled(0, 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        move(active < 0 ? firstEnabled(last, -1) : firstEnabled(active - 1, -1) >= 0 ? firstEnabled(active - 1, -1) : firstEnabled(last, -1));
        break;
      case "Home":
        if (e.shiftKey) return;
        e.preventDefault();
        move(firstEnabled(0, 1));
        break;
      case "End":
        if (e.shiftKey) return;
        e.preventDefault();
        move(firstEnabled(last, -1));
        break;
      case "PageDown":
        e.preventDefault();
        move(firstEnabled(Math.min(last, Math.max(active, 0) + page), -1));
        break;
      case "PageUp":
        e.preventDefault();
        move(firstEnabled(Math.max(0, active - page), 1));
        break;
      case "Enter":
        if (active < 0) return;
        e.preventDefault();
        e.stopPropagation();
        choose(active);
        break;
      case "Escape":
        if (query && escapeClears) {
          e.preventDefault();
          e.stopPropagation();
          setQuery("");
        } else if (onEscape) {
          e.preventDefault();
          e.stopPropagation();
          onEscape();
        }
        break;
    }
  };

  const optionId = (i: number) => `${uid}-${i}-${(options[i]?.item.value ?? "").replace(/\s/g, "_")}`;
  const groupLabelId = (gi: number) => `${uid}-g-${gi}`;
  const header = (group: CommandListGroup, gi: number) => {
    const count = query.trim() ? (shown[gi]?.items.length ?? 0) : group.count;
    return (
      <div aria-hidden className={cn(MENU_LABEL, "sticky top-0 z-10 gap-10 bg-raised px-16")}>
        {group.leading}
        <span id={groupLabelId(gi)} className="min-w-0 truncate">
          {group.label}
        </span>
        {count !== undefined && <span className="ml-auto shrink-0 font-regular text-fg-tertiary tabular">{count}</span>}
        {count !== undefined && checks && <span className="size-16 shrink-0" />}
      </div>
    );
  };
  const option = (row: Extract<Row, { kind: "item" }>) => {
    const { item, option: i } = row;
    const isActive = i === active;
    const isSelected = commands ? isActive : selected.has(item.value);
    const state = { query, active: isActive, selected: isSelected };
    return (
      <div
        id={optionId(i)}
        role="option"
        aria-selected={isSelected}
        aria-setsize={options.length}
        aria-posinset={i + 1}
        {...(item.disabled ? { "aria-disabled": true, "data-disabled": "" } : {})}
        {...(isActive ? { "data-highlighted": "" } : {})}
        {...(isSelected ? { "data-selected": "" } : {})}
        onMouseMove={() => !isActive && !item.disabled && setActive(i)}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => choose(i)}
        className={cn(MENU_ITEM, "group/opt mx-6 w-auto", look.item)}
      >
        {multiple && <OptionCheckbox />}
        {item.leading}
        {item.render ? (
          <span className="flex min-w-0 flex-1 items-center gap-8">{item.render(state)}</span>
        ) : (
          <span className="min-w-0 flex-1 truncate">
            <Highlight text={item.label} query={query} />
          </span>
        )}
        {item.meta !== undefined && <span className={cn(MENU_HINT, "pl-8 font-regular")}>{item.meta}</span>}
        {checks && (
          <span aria-hidden className={cn("flex size-16 shrink-0 items-center justify-center text-fg", !isSelected && "opacity-0")}>
            <Icon icon={icons.check} size="sm" />
          </span>
        )}
      </div>
    );
  };

  const indexes = virtual ? virtualizer.getVirtualItems().map((v) => v.index) : rows.map((_, i) => i);
  const rendered = new Map<number, number[]>();
  for (const index of indexes) {
    const row = rows[index];
    if (!row) continue;
    const list = rendered.get(row.gi) ?? [];
    if (row.kind === "item") list.push(index);
    rendered.set(row.gi, list);
  }
  const empty = rows.length === 0;
  return (
    <div
      {...(force ? { "data-force": force } : {})}
      className={cn("relative flex min-w-0 flex-col overflow-hidden", KAESO_COMMAND_LIST_VARIANTS.height[height].classes, className)}
      {...(fitted ? { style: fitted } : {})}
    >
      {loading && (
        <span aria-hidden className="absolute inset-x-0 top-0 h-2 overflow-hidden">
          <span className="block h-full w-1/3 animate-indeterminate bg-fg-tertiary" />
        </span>
      )}
      <div
        onMouseDown={(e) => {
          if (e.target !== e.currentTarget) return;
          e.preventDefault();
          e.currentTarget.querySelector("input")?.focus();
        }}
        className={cn("flex shrink-0 cursor-text items-center hairline-b", look.search)}
      >
        <Icon icon={icons.search} size={size === "lg" ? "md" : "sm"} className="text-fg-tertiary" />
        {searchPrefix}
        <input
          ref={ref}
          {...inputProps}
          role="combobox"
          aria-expanded="true"
          aria-controls={listId}
          aria-autocomplete="list"
          aria-label={placeholder ?? t("commandList.search", "Search")}
          {...(active >= 0 && options.length > 0 ? { "aria-activedescendant": optionId(active) } : {})}
          {...(autoFocus ? { autoFocus: true, "data-autofocus": "" } : {})}
          type="text"
          autoComplete="off"
          spellCheck={false}
          placeholder={placeholder ?? t("commandList.search", "Search")}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
          className={cn("min-w-0 flex-1 bg-transparent text-fg outline-none placeholder:text-fg-tertiary", look.input)}
        />
      </div>
      {toolbar && <div className="flex shrink-0 flex-wrap items-center gap-8 px-12 py-8 hairline-b">{toolbar}</div>}
      <div
        ref={scroller}
        id={listId}
        {...(empty
          ? { role: "status" }
          : { role: "listbox", "aria-label": ariaLabel, ...(multiple ? { "aria-multiselectable": true } : {}), ...(loading ? { "aria-busy": true } : {}) })}
        data-dialog-body=""
        {...(labelled ? { style: { scrollPaddingTop: HEADER } } : {})}
        className={cn("min-h-0 overflow-y-auto overscroll-contain scrollbar-quiet", height === "popover" ? "flex-initial" : "flex-1")}
      >
        {empty ? (
          <div className="flex h-80 items-center justify-center px-16 text-center text-body text-fg-secondary">
            {loading ? t("commandList.loading", "Loading…") : (emptyText ?? t("commandList.noResults", "No results"))}
          </div>
        ) : (
          <div role="presentation" className="relative w-full" style={{ height: (offsets[rows.length] ?? 0) + EDGE }}>
            {[...rendered].map(([gi, indexes]) => {
              const span = spans[gi];
              const group = shown[gi]?.group;
              if (!span || !group) return null;
              const top = offsets[span.first] ?? 0;
              const bottom = offsets[span.last + 1] ?? top;
              return (
                <div
                  key={group.id}
                  {...(group.label ? { role: "group", "aria-labelledby": groupLabelId(gi) } : { role: "presentation" })}
                  className="absolute left-0 w-full"
                  style={{ top, height: bottom - top }}
                >
                  {group.label && header(group, gi)}
                  {indexes.map((index) => {
                    const row = rows[index];
                    return row?.kind === "item" ? (
                      <div
                        key={index}
                        role="presentation"
                        className="absolute left-0 w-full"
                        style={{ top: (offsets[index] ?? 0) - top, height: (offsets[index + 1] ?? 0) - (offsets[index] ?? 0) }}
                      >
                        {option(row)}
                      </div>
                    ) : null;
                  })}
                </div>
              );
            })}
          </div>
        )}
      </div>
      {footer}
    </div>
  );
});
CommandList.displayName = "CommandList";
