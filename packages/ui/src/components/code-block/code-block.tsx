import { forwardRef, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Tabs as BaseTabs } from "@base-ui/react/tabs";
import { cn } from "../../utils/cn.js";
import { CopyButton } from "../copy-button/copy-button.js";
import { highlight, type CodeLang, type CodeToken } from "./highlighter.js";
import { INDENT, dropIndent } from "./indent.js";
import { useUiText } from "../../i18n/context.js";
import { useScrollFadeX } from "../../hooks/use-scroll-fade.js";

export const KAESO_CODE_BLOCK_VARIANTS = {
  surface: {
    well: { classes: "rounded-well bg-fill-1", description: "A well: radius 14, fill, no ring (default)" },
    plain: { classes: "bg-transparent", description: "No surface, inside another well" },
  },
} as const;
export const KAESO_CODE_BLOCK_DEFAULT_VARIANTS = { surface: "well" } as const;

function diffLineClass(line: string): string | false {
  if (line.startsWith("+") && !line.startsWith("+++")) return "bg-diff-added";
  if (line.startsWith("-") && !line.startsWith("---")) return "bg-diff-removed";
  return false;
}

export interface CodeTab {
  label: string;
  code: string;
  lang?: CodeLang | undefined;
}

export interface CodeBlockProps {
  code?: string | undefined;
  lang?: CodeLang | undefined;
  tabs?: CodeTab[] | undefined;
  tab?: string | undefined;
  onTabChange?: ((label: string) => void) | undefined;
  title?: ReactNode | undefined;
  lineNumbers?: boolean | undefined;
  highlightLines?: number[] | undefined;
  collapseAfter?: number | undefined;
  surface?: keyof typeof KAESO_CODE_BLOCK_VARIANTS.surface | undefined;
  copy?: boolean | undefined;
  wrap?: boolean | "phone" | undefined;
  className?: string | undefined;
}

export const CodeBlock = forwardRef<HTMLDivElement, CodeBlockProps>(function CodeBlock(
  {
    code = "",
    lang = "ts",
    tabs,
    tab: chosen,
    onTabChange,
    title,
    lineNumbers,
    highlightLines = [],
    collapseAfter = 24,
    surface = "well",
    copy = true,
    wrap = false,
    className,
  },
  ref,
) {
  const [own, setOwn] = useState(0);
  const match = chosen === undefined ? -1 : (tabs?.findIndex((item) => item.label === chosen) ?? -1);
  const tab = match >= 0 ? match : own;
  const choose = (i: number) => {
    setOwn(i);
    const label = tabs?.[i]?.label;
    if (label !== undefined) onTabChange?.(label);
  };
  const t = useUiText();
  const current = tabs?.[tab] ?? { label: "", code, lang };
  const text = current.code.replace(/\n$/, "");
  const lines = useMemo(() => text.split("\n"), [text]);
  const [highlighted, setHighlighted] = useState<{ text: string; lang: CodeLang; tokens: CodeToken[][] | null } | null>(null);
  const [expanded, setExpanded] = useState(false);
  useEffect(() => {
    let alive = true;
    const lang = current.lang ?? "ts";
    highlight(text, lang)
      .then((tokens) => alive && setHighlighted({ text, lang, tokens }))
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [text, current.lang]);
  const tokenLines = highlighted && highlighted.lang === (current.lang ?? "ts") && highlighted.text === text ? highlighted.tokens : null;
  const collapsed = collapseAfter > 0 && lines.length > collapseAfter && !expanded;
  const visible = collapsed ? collapseAfter : lines.length;
  const scroller = useRef<HTMLDivElement | null>(null);
  useScrollFadeX(scroller, "code", !!tabs);
  const codeClass = cn(
    "relative overflow-x-auto py-16 scrollbar-quiet scroll-fade-x focus-ring-inset",
    collapsed && "overflow-y-hidden",
    copy && "coarse:mr-40",
  );
  const source = (
    <pre className="mono text-small text-fg">
      <code className={cn("grid", !wrap && "min-w-max", wrap === "phone" && "sm:min-w-max")}>
        {lines.slice(0, visible).map((line, i) => (
          <span
            key={i}
            className={cn(
              "flex px-16 leading-code",
              wrap === true && "whitespace-pre-wrap wrap-anywhere",
              wrap === "phone" && "max-sm:whitespace-pre-wrap max-sm:wrap-anywhere",
              current.lang === "diff" && diffLineClass(line),
              highlightLines.includes(i + 1) && "bg-fill-2",
            )}
          >
            {lineNumbers && <span className="mr-16 inline-block w-24 shrink-0 text-right text-fg-tertiary select-none tabular">{i + 1}</span>}
            {wrap && INDENT.test(line) && <span className="shrink-0 whitespace-pre">{INDENT.exec(line)?.[0]}</span>}
            <span className={cn("min-w-0", wrap === true && "pl-16 -indent-16", wrap === "phone" && "max-sm:pl-16 max-sm:-indent-16")}>
              {tokenLines?.[i]?.length
                ? dropIndent(tokenLines[i], wrap ? (INDENT.exec(line)?.[0].length ?? 0) : 0).map((token, j) => (
                    <span key={j} className={token.className || undefined}>
                      {token.content}
                    </span>
                  ))
                : (wrap ? line.replace(INDENT, "") : line) || " "}
            </span>
          </span>
        ))}
      </code>
    </pre>
  );

  return (
    <div ref={ref} className={cn("group/code relative min-w-0", KAESO_CODE_BLOCK_VARIANTS.surface[surface].classes, className)}>
      <BaseTabs.Root value={tab} onValueChange={(v: unknown) => choose(Number(v))}>
        {(title || tabs) && (
          <div className="flex h-44 items-center gap-4 px-8 hairline-b">
            {title && <span className="px-8 mono text-small text-fg-secondary">{title}</span>}
            {tabs && (
              <BaseTabs.List aria-label={t("codeBlock.language", "Language")} className="flex items-center gap-2">
                {tabs.map((item, i) => (
                  <BaseTabs.Tab
                    key={item.label}
                    value={i}
                    className={cn(
                      "inline-flex h-28 cursor-pointer items-center rounded-pill px-10 text-small font-medium transition-control focus-ring",
                      i === tab ? "bg-fill-2 text-fg" : "text-fg-secondary hover:bg-fill-2 hover:text-fg",
                    )}
                  >
                    {item.label}
                  </BaseTabs.Tab>
                ))}
              </BaseTabs.List>
            )}
          </div>
        )}
        {tabs ? (
          <BaseTabs.Panel ref={scroller} value={tab} className={codeClass}>
            {source}
          </BaseTabs.Panel>
        ) : (
          <div
            ref={scroller}
            tabIndex={0}
            role="region"
            aria-label={typeof title === "string" ? t("codeBlock.titledCode", "{title} code", { title }) : t("codeBlock.code", "Code")}
            className={codeClass}
          >
            {source}
          </div>
        )}
      </BaseTabs.Root>
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
