import { usePortalContainer } from "../../utils/portal-context.js";
import { forwardRef, useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode, type Ref } from "react";
import { Dialog as BaseDialog } from "@base-ui/react/dialog";
import { cn } from "../../utils/cn.js";
import { useRestoreFocus } from "../../utils/restore-focus.js";
import { useOpenSession, useScopedDialog } from "../../utils/dialog-scope.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon, type IconData } from "../icon/icon.js";
import { KbdSequence, Kbd } from "../kbd/kbd.js";
import { useUiText } from "../../i18n/context.js";
import { useResizeMotion } from "../../utils/resize-motion.js";
import { CommandList, type CommandListGroup, type CommandListItem, type CommandListRowState } from "../command-list/command-list.js";

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
  query?: string | undefined;
  counts?: boolean | undefined;
}

function matches(item: CommandListItem, q: string) {
  const hay = [item.label, ...(item.keywords ?? [])].join(" ").toLowerCase();
  return q
    .toLowerCase()
    .split(/\s+/)
    .every((w) => hay.includes(w))
    ? 1
    : 0;
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
  query: controlledQuery,
  counts,
  rootRef,
}: CommandPaletteProps & { close: () => void; rootRef?: Ref<HTMLDivElement> | undefined }) {
  const t = useUiText();
  const placeholder = placeholderProp ?? t("commandPalette.placeholder", "Search or run a command");
  const [ownQuery, setOwnQuery] = useState(defaultQuery);
  const query = controlledQuery ?? ownQuery;
  const setQuery = (next: string) => {
    setOwnQuery(next);
    onQueryChange?.(next);
  };
  const [pages, setPages] = useState<{ title: string; groups: CommandGroup[] }[]>([]);
  const input = useRef<HTMLInputElement>(null);
  const [box, setBox] = useState<HTMLElement | null>(null);
  const boxRef = useCallback(
    (el: HTMLDivElement | null) => {
      setBox(inline ? null : (el?.parentElement ?? null));
      if (typeof rootRef === "function") rootRef(el);
      else if (rootRef) rootRef.current = el;
    },
    [inline, rootRef],
  );
  useResizeMotion(box);
  const page = pages[pages.length - 1];
  const source = page ? page.groups : groups;
  const loading = source.some((g) => g.loading);
  const byId = useMemo(() => new Map(source.flatMap((g) => g.items.map((i) => [i.id, i] as const))), [source]);
  const listGroups = useMemo<CommandListGroup[]>(
    () =>
      source.map((g, gi) => ({
        id: `${gi}:${g.label ?? ""}`,
        ...(g.label ? { label: g.label } : {}),
        ...(g.loading ? { loading: true } : {}),
        items: g.items.map((item) => ({
          value: item.id,
          label: item.label,
          keywords: [item.hint ?? "", ...(item.keywords ?? [])],
          render: ({ active }: CommandListRowState) => (
            <>
              {item.icon && <Icon icon={item.icon} size="sm" className={active ? "text-fg" : "text-fg-secondary"} />}
              <span className="min-w-0 truncate">{item.label}</span>
              {item.hint && (
                <span className={cn("min-w-0 truncate text-body font-regular", active ? "text-fg-secondary" : "text-fg-tertiary")}>{item.hint}</span>
              )}
              <span className="ml-auto flex shrink-0 items-center gap-8">
                {item.shortcut && <KbdSequence keys={item.shortcut} />}
                {item.children && <Icon icon={icons.chevronRight} size="sm" className="text-fg-tertiary" />}
              </span>
            </>
          ),
        })),
      })),
    [source],
  );

  const select = (id: string) => {
    const item = byId.get(id);
    if (!item) return;
    const children = item.children;
    if (children) {
      setPages((p) => [...p, { title: item.label, groups: children }]);
      setQuery("");
      input.current?.focus();
      return;
    }
    item.onSelect?.();
    close();
  };

  return (
    <div
      ref={boxRef}
      className={cn("relative flex palette-limits min-h-0 flex-col overflow-hidden", inline ? "w-640 rounded-palette bg-raised shadow-dialog" : "flex-1")}
    >
      <CommandList
        ref={input}
        groups={listGroups}
        aria-label={t("commandPalette.results", "Results")}
        size="lg"
        height="fill"
        escapeClears={false}
        match={matches}
        counts={counts}
        inputProps={{ "data-palette-input": "" }}
        autoFocus
        query={query}
        onQueryChange={setQuery}
        onSelect={select}
        loading={loading}
        placeholder={placeholder}
        emptyText={emptyText ?? t("commandPalette.noResults", "No results")}
        searchPrefix={
          page && <span className="inline-flex h-24 shrink-0 items-center rounded-pill bg-fill-2 px-10 text-caption font-medium text-fg">{page.title}</span>
        }
        onInputKeyDown={(e: KeyboardEvent<HTMLInputElement>) => {
          if (e.key === "Backspace" && !query && pages.length) {
            e.preventDefault();
            setPages((p) => p.slice(0, -1));
          }
        }}
        footer={
          keyHints && (
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
          )
        }
      />
    </div>
  );
}

export const CommandPalette = forwardRef<HTMLDivElement, CommandPaletteProps>(function CommandPalette(props, ref) {
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
  if (inline) return <Palette {...props} close={() => undefined} rootRef={ref} />;
  return (
    <BaseDialog.Root open={isOpen} onOpenChange={(o: boolean) => setOpen(o)} onOpenChangeComplete={settled}>
      <BaseDialog.Portal container={container}>
        <BaseDialog.Backdrop className="fixed inset-0 z-50 bg-overlay backdrop-motion" />
        <BaseDialog.Viewport className="fixed inset-0 z-50 flex items-start justify-center px-16 pt-palette">
          <BaseDialog.Popup
            ref={ref}
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
});
CommandPalette.displayName = "CommandPalette";
