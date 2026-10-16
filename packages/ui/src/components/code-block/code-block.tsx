import { forwardRef, useEffect, useMemo, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn";
import { CopyButton } from "../copy-button/copy-button";
import { highlight, type CodeLang } from "./highlighter";
import { useUiText } from "../../i18n/context";

export const KAESO_CODE_BLOCK_VARIANTS = {
  surface: {
    well: { classes: "rounded-well bg-fill-1", description: "A well: radius 14, fill, no ring (default)" },
    plain: { classes: "bg-transparent", description: "No surface, inside another well" },
  },
} as const;
export const KAESO_CODE_BLOCK_DEFAULT_VARIANTS = { surface: "well" } as const;

export interface CodeTab {
  label: string;
  code: string;
  lang?: CodeLang | undefined;
}

export interface CodeBlockProps {
  code?: string | undefined;
  lang?: CodeLang | undefined;
  tabs?: CodeTab[] | undefined;
  title?: ReactNode | undefined;
  lineNumbers?: boolean | undefined;
  highlightLines?: number[] | undefined;
  collapseAfter?: number | undefined;
  surface?: keyof typeof KAESO_CODE_BLOCK_VARIANTS.surface | undefined;
  copy?: boolean | undefined;
  wrap?: boolean | undefined;
  className?: string | undefined;
}

export const CodeBlock = forwardRef<HTMLDivElement, CodeBlockProps>(function CodeBlock(
  { code = "", lang = "ts", tabs, title, lineNumbers, highlightLines = [], collapseAfter = 24, surface = "well", copy = true, wrap = false, className },
  ref,
) {
  const [tab, setTab] = useState(0);
  const t = useUiText();
  const current = tabs?.[tab] ?? { label: "", code, lang };
  const text = current.code.replace(/\n$/, "");
  const lines = useMemo(() => text.split("\n"), [text]);
  const [highlighted, setHighlighted] = useState<{ text: string; lang: CodeLang; html: string | null } | null>(null);
  const [expanded, setExpanded] = useState(false);
  useEffect(() => {
    let alive = true;
    const lang = current.lang ?? "ts";
    highlight(text, lang)
      .then((h) => alive && setHighlighted({ text, lang, html: h }))
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [text, current.lang]);
  const html = highlighted && highlighted.lang === (current.lang ?? "ts") ? highlighted.html : null;

  const htmlLines = useMemo(() => {
    if (!html || !highlighted) return null;
    const inner = /<code[^>]*>([\s\S]*)<\/code>/.exec(html)?.[1] ?? "";
    const source = highlighted.text.split("\n");
    return inner.split("\n").map((l, i) => (source[i] === lines[i] ? l.replace(/^<span class="line">|<\/span>$/g, "") : undefined));
  }, [html, highlighted, lines]);
  const collapsed = collapseAfter > 0 && lines.length > collapseAfter && !expanded;
  const visible = collapsed ? collapseAfter : lines.length;

  return (
    <div ref={ref} className={cn("group/code relative min-w-0", KAESO_CODE_BLOCK_VARIANTS.surface[surface].classes, className)}>
      {(title || tabs) && (
        <div className="flex h-44 items-center gap-4 px-8 hairline-b">
          {title && <span className="px-8 mono text-small text-fg-secondary">{title}</span>}
          {tabs && (
            <div role="tablist" aria-label={t("codeBlock.language", "Language")} className="flex items-center gap-2">
              {tabs.map((item, i) => (
                <button
                  key={item.label}
                  role="tab"
                  type="button"
                  aria-selected={i === tab}
                  onClick={() => setTab(i)}
                  className={cn(
                    "inline-flex h-28 cursor-pointer items-center rounded-pill px-10 text-small font-medium transition-control focus-ring",
                    i === tab ? "bg-fill-2 text-fg" : "text-fg-secondary hover:bg-fill-2 hover:text-fg",
                  )}
                >
                  {item.label}
                </button>
              ))}
            </div>
          )}
        </div>
      )}
      <div
        tabIndex={0}
        role="region"
        aria-label={typeof title === "string" ? t("codeBlock.titledCode", "{title} code", { title }) : t("codeBlock.code", "Code")}
        className={cn("relative overflow-x-auto py-16 scrollbar-quiet focus-ring-inset", collapsed && "overflow-y-hidden")}
      >
        <pre className="mono text-small text-fg">
          <code className={cn("grid", !wrap && "min-w-max")}>
            {lines.slice(0, visible).map((line, i) => (
              <span
                key={i}
                className={cn("flex px-16 leading-code", wrap && "whitespace-pre-wrap wrap-anywhere", highlightLines.includes(i + 1) && "bg-fill-2")}
              >
                {lineNumbers && <span className="mr-16 inline-block w-24 shrink-0 text-right text-fg-tertiary select-none tabular">{i + 1}</span>}
                {htmlLines?.[i] !== undefined ? (
                  <span className="min-w-0" dangerouslySetInnerHTML={{ __html: htmlLines[i] || " " }} />
                ) : (
                  <span className="min-w-0">{line || " "}</span>
                )}
              </span>
            ))}
          </code>
        </pre>
      </div>
      {collapseAfter > 0 && lines.length > collapseAfter && (
        <div className="flex justify-center pb-12">
          <button
            type="button"
            onClick={() => setExpanded((e) => !e)}
            className="inline-flex h-28 cursor-pointer items-center rounded-pill px-12 text-small font-medium text-fg-secondary transition-control hover:bg-fill-2 hover:text-fg focus-ring"
          >
            {expanded ? t("codeBlock.showLess", "Show less") : t("codeBlock.showAllLines", "Show all {count} lines", { count: String(lines.length) })}
          </button>
        </div>
      )}
      {copy && (
        <div
          className={cn(
            "absolute right-8 opacity-0 transition-control group-hover/code:opacity-100 coarse:opacity-100 focus-within:opacity-100",
            title || tabs ? "top-52" : "top-8",
          )}
        >
          <CopyButton value={text} label={t("codeBlock.copy", "Copy code")} />
        </div>
      )}
    </div>
  );
});
CodeBlock.displayName = "CodeBlock";

export const CodeHighlighted = CodeBlock;
