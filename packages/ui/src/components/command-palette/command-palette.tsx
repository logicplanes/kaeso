import { usePortalContainer } from "../../utils/portal-context.js";
import { Fragment, useCallback, useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Dialog as BaseDialog } from "@base-ui/react/dialog";
import { cn } from "../../utils/cn.js";
import { focusInputOnPress } from "../../utils/field-press.js";
import { useRestoreFocus } from "../../utils/restore-focus.js";
import { useOpenSession, useScopedDialog } from "../../utils/dialog-scope.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon, type IconData } from "../icon/icon.js";
import { KbdSequence, Kbd } from "../kbd/kbd.js";
import { useUiText } from "../../i18n/context.js";
import { useResizeMotion } from "../../utils/resize-motion.js";

export const KAESO_COMMAND_PALETTE_VARIANTS = {
  size: {
    md: { classes: "w-640", description: "640px, 20vh from the top (default)" },
  },
} as const;
export const KAESO_COMMAND_PALETTE_DEFAULT_VARIANTS = { size: "md" } as const;

export interface CommandItem {
  id: string;
  label: string;
  icon?: IconData | undefined;
  hint?: string | undefined;
  shortcut?: string[] | string[][] | undefined;
  keywords?: string[] | undefined;
  onSelect?: (() => void) | undefined;
  children?: CommandGroup[] | undefined;
}

export interface CommandGroup {
  label?: string | undefined;
  items: CommandItem[];
  loading?: boolean | undefined;
}

export interface CommandPaletteProps {
  open?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  groups: CommandGroup[];
  placeholder?: string | undefined;
  emptyText?: ReactNode | undefined;
  onQueryChange?: ((query: string) => void) | undefined;
  hotkey?: boolean | undefined;
  keyHints?: boolean | undefined;
  inline?: boolean | undefined;
  defaultQuery?: string | undefined;
}

function matches(item: CommandItem, q: string) {
  if (!q) return true;
  const hay = [item.label, item.hint ?? "", ...(item.keywords ?? [])].join(" ").toLowerCase();
  return q
    .toLowerCase()
    .split(/\s+/)
    .every((w) => hay.includes(w));
}

function Palette({
  groups,
  keyHints,
  placeholder: placeholderProp,
  emptyText,
  onQueryChange,
  close,
  inline,
  defaultQuery = "",
}: CommandPaletteProps & { close: () => void }) {
  const t = useUiText();
  const placeholder = placeholderProp ?? t("commandPalette.placeholder", "Search or run a command");
  const [query, setQuery] = useState(defaultQuery);
  const [pages, setPages] = useState<{ title: string; groups: CommandGroup[] }[]>([]);
  const [active, setActive] = useState(0);
  const listId = useId();
  const input = useRef<HTMLInputElement>(null);
  const [box, setBox] = useState<HTMLElement | null>(null);
  const boxRef = useCallback((el: HTMLElement | null) => setBox(inline ? null : (el?.parentElement ?? null)), [inline]);
  useResizeMotion(box);
  const page = pages[pages.length - 1];
  const source = page ? page.groups : groups;
  const visible = useMemo(
    () => source.map((g) => ({ ...g, items: g.items.filter((i) => matches(i, query)) })).filter((g) => g.items.length > 0 || g.loading),
    [source, query],
  );
  const flat = visible.flatMap((g) => g.items);
  const loading = source.some((g) => g.loading);
  useEffect(() => {
    document.getElementById(`${listId}-${flat[active]?.id}`)?.scrollIntoView({ block: "nearest" });
  }, [active, flat, listId]);

  const select = (item: CommandItem | undefined) => {
    if (!item) return;
    const children = item.children;
    if (children) {
      setPages((p) => [...p, { title: item.label, groups: children }]);
      setQuery("");
      setActive(0);
      input.current?.focus();
      return;
    }
    item.onSelect?.();
    close();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => (flat.length ? (a + 1) % flat.length : 0));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => (flat.length ? (a - 1 + flat.length) % flat.length : 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      select(flat[active]);
    } else if (e.key === "Backspace" && !query && pages.length) {
      e.preventDefault();
      setPages((p) => p.slice(0, -1));
      setActive(0);
    } else if (e.key === "Home") {
      setActive(0);
    } else if (e.key === "End") {
      setActive(Math.max(0, flat.length - 1));
    }
  };

  let index = -1;
  return (
    <div
      ref={boxRef}
      className={cn("relative flex palette-limits min-h-0 flex-col overflow-hidden", inline ? "w-640 rounded-palette bg-raised shadow-dialog" : "flex-1")}
    >
      {loading && (
        <span aria-hidden className="absolute inset-x-0 top-0 h-2 overflow-hidden">
          <span className="block h-full w-1/3 animate-indeterminate bg-fg-tertiary" />
        </span>
      )}
      <div onMouseDown={focusInputOnPress} className="flex h-56 shrink-0 cursor-text items-center gap-12 px-20 hairline-b">
        <Icon icon={icons.search} size="md" className="text-fg-tertiary" />
        {page && <span className="inline-flex h-24 shrink-0 items-center rounded-pill bg-fill-2 px-10 text-caption font-medium text-fg">{page.title}</span>}
        <input
          ref={input}
          autoFocus
          role="combobox"
          aria-expanded="true"
          aria-controls={listId}
          aria-autocomplete="list"
          aria-activedescendant={flat[active] ? `${listId}-${flat[active].id}` : undefined}
          aria-label={placeholder}
          placeholder={placeholder}
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
            onQueryChange?.(e.target.value);
          }}
          onKeyDown={onKeyDown}
          className="min-w-0 flex-1 bg-transparent text-body-lg font-regular text-fg outline-none placeholder:text-fg-tertiary"
        />
      </div>
      <div
        id={listId}
        data-dialog-body=""
        {...(flat.length ? { role: "listbox", "aria-label": t("commandPalette.results", "Results") } : { role: "status" })}
        className="min-h-0 flex-1 overflow-y-auto overscroll-contain p-8 scrollbar-quiet"
      >
        {flat.length === 0 && !loading && (
          <div className="flex h-80 items-center justify-center text-body text-fg-secondary">{emptyText ?? t("commandPalette.noResults", "No results")}</div>
        )}
        {visible.map((g, gi) => (
          <Fragment key={g.label ?? gi}>
            {g.label && <div className="flex h-28 items-center px-12 pt-4 text-caption font-medium text-fg-secondary">{g.label}</div>}
            {g.items.map((item) => {
              index += 1;
              const i = index;
              const isActive = i === active;
              return (
                <div
                  key={item.id}
                  id={`${listId}-${item.id}`}
                  role="option"
                  aria-selected={isActive}
                  onMouseMove={() => setActive(i)}
                  onClick={() => select(item)}
                  className={cn(
                    "flex h-40 cursor-pointer items-center gap-12 rounded-palette-item px-12 text-body font-medium text-fg select-none",
                    isActive && "bg-item-hover",
                  )}
                >
                  {item.icon && <Icon icon={item.icon} size="sm" className={isActive ? "text-fg" : "text-fg-secondary"} />}
                  <span className="min-w-0 truncate">{item.label}</span>
                  {item.hint && (
                    <span className={cn("min-w-0 truncate text-body font-regular", isActive ? "text-fg-secondary" : "text-fg-tertiary")}>{item.hint}</span>
                  )}
                  <span className="ml-auto flex shrink-0 items-center gap-8">
                    {item.shortcut && <KbdSequence keys={item.shortcut} />}
                    {item.children && <Icon icon={icons.chevronRight} size="sm" className="text-fg-tertiary" />}
                  </span>
                </div>
              );
            })}
          </Fragment>
        ))}
      </div>
      {keyHints && (
        <div className="flex h-40 shrink-0 items-center gap-16 px-16 text-caption font-medium text-fg-tertiary hairline-t">
          <span className="inline-flex items-center gap-6">
            <Kbd>up</Kbd>
            <Kbd>down</Kbd> {t("commandPalette.toNavigate", "to navigate")}
          </span>
          <span className="inline-flex items-center gap-6">
            <Kbd>enter</Kbd> {t("commandPalette.toOpen", "to open")}
          </span>
          {pages.length > 0 && (
            <span className="inline-flex items-center gap-6">
              <Kbd>backspace</Kbd> {t("commandPalette.toGoBack", "to go back")}
            </span>
          )}
          <span className="ml-auto inline-flex items-center gap-6">
            <Kbd>esc</Kbd> {t("commandPalette.toClose", "to close")}
          </span>
        </div>
      )}
    </div>
  );
}

export function CommandPalette(props: CommandPaletteProps) {
  const { open, onOpenChange, hotkey = true, inline } = props;
  const [inner, setInner] = useState(false);
  const isOpen = open ?? inner;
  const setOpen = (o: boolean) => {
    if (open === undefined) setInner(o);
    onOpenChange?.(o);
  };
  const toggle = useRef(() => setOpen(!isOpen));
  toggle.current = () => setOpen(!isOpen);
  useEffect(() => {
    if (!hotkey || inline) return;
    const onKey = (e: globalThis.KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        toggle.current();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [hotkey, inline]);
  const focus = useRestoreFocus(isOpen);
  const t = useUiText();
  const container = usePortalContainer();
  const settled = useScopedDialog(inline ? false : isOpen);
  const session = useOpenSession(isOpen);
  if (inline) return <Palette {...props} close={() => undefined} />;
  return (
    <BaseDialog.Root open={isOpen} onOpenChange={(o: boolean) => setOpen(o)} onOpenChangeComplete={settled}>
      <BaseDialog.Portal container={container}>
        <BaseDialog.Backdrop className="fixed inset-0 z-50 bg-overlay backdrop-motion" />
        <BaseDialog.Viewport className="fixed inset-0 z-50 flex items-start justify-center px-16 pt-palette">
          <BaseDialog.Popup
            aria-label={t("commandPalette.label", "Command palette")}
            initialFocus={focus.initialFocus}
            finalFocus={focus.finalFocus}
            className="flex w-640 max-w-full flex-col overflow-hidden rounded-palette bg-raised text-fg shadow-dialog outline-none dialog-motion"
          >
            <Palette key={session} {...props} close={() => setOpen(false)} />
          </BaseDialog.Popup>
        </BaseDialog.Viewport>
      </BaseDialog.Portal>
    </BaseDialog.Root>
  );
}
CommandPalette.displayName = "CommandPalette";
