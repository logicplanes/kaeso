import { forwardRef, useState, type ReactNode } from "react";
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
    <span className="break-all text-info-text">
      "<Mark text={String(v)} query={query} />"
    </span>
  );
}

function CopyPath({ path }: { path: string }) {
  const { copied, copy } = useCopy();
  const t = useUiText();
  return (
    <button
      type="button"
      aria-label={copied ? t("jsonView.pathCopied", "Path copied") : t("jsonView.copyPath", "Copy path {path}", { path })}
      onClick={(e) => {
        e.stopPropagation();
        void copy(path);
      }}
      className="relative ml-8 inline-flex size-20 items-center justify-center rounded-pill text-fg-tertiary tap-target opacity-0 transition-control group-hover/row:opacity-100 coarse:opacity-100 hover:bg-fill-2 hover:text-fg focus-visible:opacity-100"
    >
      <Icon icon={copied ? icons.check : icons.copy} size="xs" />
    </button>
  );
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
  if (!isObj) {
    return (
      <div className="group/row flex min-h-20 items-start pl-20" style={{ paddingLeft: depth * 16 + 20 }}>
        <span>
          {keyLabel}
          <Scalar v={value} query={query} />
          <span className="text-fg-tertiary">{comma}</span>
        </span>
        {path && <CopyPath path={path} />}
      </div>
    );
  }
  const [o, c] = Array.isArray(value) ? ["[", "]"] : ["{", "}"];
  return (
    <div>
      <div className="group/row flex min-h-24 items-center" style={{ paddingLeft: depth * 16 }}>
        <button
          type="button"
          aria-expanded={open}
          aria-label={
            path
              ? open
                ? t("jsonView.collapsePath", "Collapse {path}", { path })
                : t("jsonView.expandPath", "Expand {path}", { path })
              : open
                ? t("jsonView.collapse", "Collapse")
                : t("jsonView.expand", "Expand")
          }
          onClick={() => setOpen(!open)}
          className="-ml-4 inline-flex size-24 cursor-pointer items-center justify-center rounded-pill text-fg-tertiary hover:bg-fill-2 hover:text-fg focus-ring"
        >
          <Icon icon={icons.chevronRight} size="xs" className={cn("transition-transform duration-180", open && "rotate-90")} />
        </button>
        <span>
          {keyLabel}
          <span className="text-fg-tertiary">{o}</span>
          {!open && (
            <button type="button" onClick={() => setOpen(true)} className="mx-4 cursor-pointer rounded-code px-4 text-fg-tertiary hover:bg-fill-2">
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
        {path && <CopyPath path={path} />}
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
          <div className="min-h-20 text-fg-tertiary" style={{ paddingLeft: depth * 16 + 20 }}>
            {c}
            {comma}
          </div>
        </>
      )}
    </div>
  );
}

export const JsonView = forwardRef<HTMLDivElement, JsonViewProps>(function JsonView({ value, defaultDepth = 2, query, surface = "well", className }, ref) {
  return (
    <div
      ref={ref}
      tabIndex={0}
      role="region"
      aria-label="JSON"
      className={cn(
        "mono overflow-x-auto text-small focus-ring-inset leading-code text-fg scrollbar-quiet",
        KAESO_JSON_VIEW_VARIANTS.surface[surface].classes,
        className,
      )}
    >
      <Node value={value as Json} path="" depth={0} open={defaultDepth} query={query} last />
    </div>
  );
});
JsonView.displayName = "JsonView";
