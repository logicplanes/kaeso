import type * as React from "react";
import { createContext, forwardRef, useContext, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";
import { useCopy } from "../../hooks/use-copy.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_JSON_VIEW_VARIANTS = {
  surface: {
    well: { classes: "rounded-well bg-fill-1 p-16", description: "On a well (default)" },
    plain: { classes: "", description: "No surface" },
  },
} as const;
export const KAESO_JSON_VIEW_DEFAULT_VARIANTS = { surface: "well" } as const;

type Json = string | number | boolean | null | Json[] | { [k: string]: Json };

export interface JsonViewProps {
  value: unknown;
  defaultDepth?: number | undefined;
  query?: string | undefined;
  surface?: keyof typeof KAESO_JSON_VIEW_VARIANTS.surface | undefined;
  className?: string | undefined;
}

interface Tree {
  active: string;
  setActive: (path: string) => void;
  announce: (text: string) => void;
}
const TreeContext = createContext<Tree>({ active: "", setActive: () => undefined, announce: () => undefined });

function Mark({ text, query }: { text: string; query?: string | undefined }) {
  if (!query) return <>{text}</>;
  const i = text.toLowerCase().indexOf(query.toLowerCase());
  if (i < 0) return <>{text}</>;
  return (
    <>
      {text.slice(0, i)}
      <mark className="rounded-cell bg-warning-tint text-inherit ring-mark">{text.slice(i, i + query.length)}</mark>
      {text.slice(i + query.length)}
    </>
  );
}

function Scalar({ v, query }: { v: Json; query?: string | undefined }) {
  if (v === null) return <span className="font-semibold text-fg">null</span>;
  if (typeof v === "boolean") return <span className="font-semibold text-fg">{String(v)}</span>;
  if (typeof v === "number") return <span className="text-warning-text tabular">{v}</span>;
  return (
    <span className="text-info-text wrap-anywhere">
      "<Mark text={String(v)} query={query} />"
    </span>
  );
}

function useCopyPath(path: string) {
  const { copied, copy } = useCopy();
  const { announce } = useContext(TreeContext);
  const t = useUiText();
  return {
    copied,
    copyPath: () => {
      void copy(path).then((ok) => ok && announce(t("jsonView.pathCopied", "Path copied")));
    },
  };
}

function CopyPath({ path, copied, onCopy }: { path: string; copied: boolean; onCopy: () => void }) {
  const t = useUiText();
  return (
    <button
      type="button"
      tabIndex={-1}
      aria-hidden
      title={copied ? t("jsonView.pathCopied", "Path copied") : t("jsonView.copyPath", "Copy path {path}", { path })}
      onClick={(e) => {
        e.stopPropagation();
        onCopy();
      }}
      className={cn(
        "relative ml-8 inline-flex size-20 shrink-0 cursor-pointer items-center justify-center rounded-pill text-fg-tertiary tap-target transition-control hover:bg-fill-2 hover:text-fg group-hover/row:opacity-100 group-focus-within/row:opacity-100",
        copied ? "text-fg opacity-100" : "opacity-0",
      )}
    >
      <Icon icon={copied ? icons.check : icons.copy} size="xs" />
    </button>
  );
}

const isInside = (path: string, branch: string) => path !== branch && (branch === "" || path.startsWith(`${branch}.`) || path.startsWith(`${branch}[`));

function moveFocus(from: HTMLElement, key: string): boolean {
  const tree = from.closest("[role=tree]");
  if (!tree) return false;
  const items = [...tree.querySelectorAll<HTMLElement>("[role=treeitem]")];
  const i = items.indexOf(from);
  const next =
    key === "ArrowDown" ? items[i + 1] : key === "ArrowUp" ? items[i - 1] : key === "Home" ? items[0] : key === "End" ? items[items.length - 1] : undefined;
  if (!next) return key === "ArrowDown" || key === "ArrowUp" || key === "Home" || key === "End";
  next.focus();
  return true;
}

function Node({
  name,
  value,
  path,
  depth,
  open: initialOpen,
  query,
  last,
}: {
  name?: string | number | undefined;
  value: Json;
  path: string;
  depth: number;
  open: number;
  query?: string | undefined;
  last: boolean;
}) {
  const [open, setOpen] = useState(depth < initialOpen);
  const t = useUiText();
  const tree = useContext(TreeContext);
  const { copied, copyPath } = useCopyPath(path);
  const isObj = value !== null && typeof value === "object";
  const entries: [string | number, Json][] = isObj ? (Array.isArray(value) ? value.map((v, i) => [i, v]) : Object.entries(value)) : [];
  const keyLabel: ReactNode =
    name === undefined ? null : typeof name === "number" ? (
      <span className="text-fg-tertiary tabular">{name}: </span>
    ) : (
      <span className="text-fg">
        "<Mark text={name} query={query} />"<span className="text-fg-tertiary">: </span>
      </span>
    );
  const comma = last ? "" : ",";
  const toggle = (next: boolean) => {
    setOpen(next);
    if (!next && isInside(tree.active, path)) tree.setActive(path);
  };
  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.target !== e.currentTarget) return;
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "c" && path) {
      e.preventDefault();
      copyPath();
      return;
    }
    if (isObj && (e.key === "Enter" || e.key === " ")) {
      e.preventDefault();
      toggle(!open);
      return;
    }
    if (isObj && e.key === "ArrowRight") {
      e.preventDefault();
      if (!open) setOpen(true);
      else if (entries.length) moveFocus(e.currentTarget, "ArrowDown");
      return;
    }
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      if (isObj && open) toggle(false);
      else e.currentTarget.parentElement?.parentElement?.closest<HTMLElement>("[data-json-node]")?.querySelector<HTMLElement>("[role=treeitem]")?.focus();
      return;
    }
    if (moveFocus(e.currentTarget, e.key)) e.preventDefault();
  };
  const item = {
    role: "treeitem",
    "aria-level": depth + 1,
    tabIndex: tree.active === path ? 0 : -1,
    onKeyDown,
    onFocus: (e: React.FocusEvent<HTMLDivElement>) => {
      if (e.target === e.currentTarget) tree.setActive(path);
    },
  } as const;
  if (!isObj) {
    return (
      <div data-json-node="">
        <div {...item} className="group/row flex min-h-20 items-start rounded-cell focus-ring-inset" style={{ paddingLeft: depth * 16 + 20 }}>
          <span className="min-w-0">
            {keyLabel}
            <Scalar v={value} query={query} />
            <span className="text-fg-tertiary">{comma}</span>
          </span>
          {path && <CopyPath path={path} copied={copied} onCopy={copyPath} />}
        </div>
      </div>
    );
  }
  const [o, c] = Array.isArray(value) ? ["[", "]"] : ["{", "}"];
  return (
    <div data-json-node="">
      <div {...item} aria-expanded={open} className="group/row flex min-h-24 items-center rounded-cell focus-ring-inset" style={{ paddingLeft: depth * 16 }}>
        <button
          type="button"
          tabIndex={-1}
          aria-hidden
          onClick={() => toggle(!open)}
          className="relative -ml-4 inline-flex size-24 shrink-0 cursor-pointer items-center justify-center rounded-pill text-fg-tertiary tap-target hover:bg-fill-2 hover:text-fg"
        >
          <Icon icon={icons.chevronRight} size="xs" className={cn("transition-transform duration-180", open && "rotate-90")} />
        </button>
        <span className="min-w-0">
          {keyLabel}
          <span className="text-fg-tertiary">{o}</span>
          {!open && (
            <button
              type="button"
              tabIndex={-1}
              onClick={() => setOpen(true)}
              className="relative mx-4 cursor-pointer rounded-code px-4 text-fg-tertiary tap-target hover:bg-fill-2 hover:text-fg"
            >
              {Array.isArray(value)
                ? t("jsonView.items", "{count, plural, one {{n} item} other {{n} items}}", { count: entries.length, n: String(entries.length) })
                : t("jsonView.keys", "{count, plural, one {{n} key} other {{n} keys}}", { count: entries.length, n: String(entries.length) })}
            </button>
          )}
          {!open && (
            <span className="text-fg-tertiary">
              {c}
              {comma}
            </span>
          )}
        </span>
        {path && <CopyPath path={path} copied={copied} onCopy={copyPath} />}
      </div>
      {open && (
        <>
          {entries.map(([k, v], i) => (
            <Node
              key={k}
              name={k}
              value={v}
              path={typeof k === "number" ? `${path}[${k}]` : path ? `${path}.${k}` : String(k)}
              depth={depth + 1}
              open={initialOpen}
              query={query}
              last={i === entries.length - 1}
            />
          ))}
          <div aria-hidden className="min-h-20 text-fg-tertiary" style={{ paddingLeft: depth * 16 + 20 }}>
            {c}
            {comma}
          </div>
        </>
      )}
    </div>
  );
}

export const JsonView = forwardRef<HTMLDivElement, JsonViewProps>(function JsonView({ value, defaultDepth = 2, query, surface = "well", className }, ref) {
  const t = useUiText();
  const [active, setActive] = useState("");
  const [said, setSaid] = useState("");
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const root = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (!root.current?.querySelector('[role=treeitem][tabindex="0"]')) setActive("");
  }, [value]);
  const tree = useMemo<Tree>(
    () => ({
      active,
      setActive,
      announce: (text) => {
        setSaid("");
        clearTimeout(timer.current);
        timer.current = setTimeout(() => setSaid(text), 50);
      },
    }),
    [active],
  );
  return (
    <div
      ref={ref}
      className={cn("mono overflow-x-auto text-small leading-code text-fg scrollbar-quiet", KAESO_JSON_VIEW_VARIANTS.surface[surface].classes, className)}
    >
      <TreeContext.Provider value={tree}>
        <div ref={root} role="tree" aria-label={t("jsonView.label", "JSON")}>
          <Node value={value as Json} path="" depth={0} open={defaultDepth} query={query} last />
        </div>
      </TreeContext.Provider>
      <div role="status" className="sr-only">
        {said}
      </div>
    </div>
  );
});
JsonView.displayName = "JsonView";
