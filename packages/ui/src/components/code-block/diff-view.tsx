import { forwardRef, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";
import { CopyButton } from "../copy-button/copy-button.js";
import { useUiText } from "../../i18n/context.js";
import { useScrollFadeX } from "../../hooks/use-scroll-fade.js";
import { Text } from "../text/text.js";
import { highlight, type CodeLang, type CodeToken } from "./highlighter.js";
import { INDENT } from "./indent.js";
import { numberWidth } from "./numbers.js";
import { diffLines, diffStats, foldUnchanged, largestLineNumber, unifiedDiff, type DiffKind, type DiffLine } from "./diff.js";

export const KAESO_DIFF_VIEW_VARIANTS = {
  surface: {
    well: { classes: "rounded-well bg-fill-1", description: "A well: radius 14, fill, no ring (default)" },
    plain: { classes: "bg-transparent", description: "No surface, inside another well" },
  },
} as const;
export const KAESO_DIFF_VIEW_DEFAULT_VARIANTS = { surface: "well" } as const;

const NONE_OPEN: ReadonlySet<number> = new Set();
const LINE: Record<DiffKind, string> = { same: "", removed: "bg-diff-removed", added: "bg-diff-added" };
const WORD: Record<DiffKind, string> = { same: "", removed: "bg-diff-removed-word", added: "bg-diff-added-word" };
const MARK: Record<DiffKind, { sign: string; className: string }> = {
  same: { sign: " ", className: "" },
  removed: { sign: "−", className: "text-danger-text" },
  added: { sign: "+", className: "text-success-text" },
};

export interface DiffViewProps {
  before: string;
  after: string;
  lang?: CodeLang | undefined;
  title?: ReactNode | undefined;
  context?: number | undefined;
  lineNumbers?: boolean | undefined;
  surface?: keyof typeof KAESO_DIFF_VIEW_VARIANTS.surface | undefined;
  copy?: boolean | undefined;
  wrap?: boolean | undefined;
  className?: string | undefined;
}

interface Segment {
  text: string;
  className: string;
  changed: boolean;
}

function segments(text: string, tokens: CodeToken[] | undefined, changes: readonly [number, number][] = []): Segment[] {
  const source = tokens?.length ? tokens : [{ content: text, className: "" }];
  const out: Segment[] = [];
  let at = 0;
  for (const token of source) {
    let start = 0;
    while (start < token.content.length) {
      const pos = at + start;
      const inside = changes.find(([s, e]) => pos >= s && pos < e);
      const next = inside ? inside[1] : Math.min(Infinity, ...changes.map(([s]) => s).filter((s) => s > pos));
      const cut = Math.min(token.content.length, next - at);
      out.push({ text: token.content.slice(start, cut), className: token.className, changed: !!inside });
      start = cut;
    }
    at += token.content.length;
  }
  return out;
}

function dropChars(parts: Segment[], n: number): Segment[] {
  let left = n;
  return parts.flatMap((p) => {
    if (left === 0) return [p];
    const cut = Math.min(left, p.text.length);
    left -= cut;
    return cut === p.text.length ? [] : [{ ...p, text: p.text.slice(cut) }];
  });
}

function Code({ parts, kind }: { parts: Segment[]; kind: DiffKind }) {
  const groups: Segment[][] = [];
  for (const part of parts) {
    const last = groups[groups.length - 1];
    if (last && last[0]?.changed === part.changed) last.push(part);
    else groups.push([part]);
  }
  return (
    <>
      {groups.map((group, g) => {
        const spans = group.map((p, k) => (
          <span key={k} className={(group[0]?.changed ? p.className.replace(/text-fg-(tertiary|secondary)/, "text-fg") : p.className) || undefined}>
            {p.text}
          </span>
        ));
        return group[0]?.changed ? (
          <span key={g} data-diff-change="" className={cn("rounded-cell forced-colors:underline", WORD[kind])}>
            {spans}
          </span>
        ) : (
          spans
        );
      })}
    </>
  );
}

export const DiffView = forwardRef<HTMLDivElement, DiffViewProps>(function DiffView(
  { before, after, lang = "text", title, context = 3, lineNumbers, surface = "well", copy = true, wrap = false, className },
  ref,
) {
  const t = useUiText();
  const lines = useMemo(() => diffLines(before, after), [before, after]);
  const stats = useMemo(() => diffStats(lines), [lines]);
  const [tokens, setTokens] = useState<{ before: string; after: string; lang: CodeLang; old: CodeToken[][] | null; new: CodeToken[][] | null } | null>(null);
  useEffect(() => {
    let alive = true;
    Promise.all([highlight(before.replace(/\n$/, ""), lang), highlight(after.replace(/\n$/, ""), lang)])
      .then(([o, n]) => alive && setTokens({ before, after, lang, old: o, new: n }))
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [before, after, lang]);
  const current = tokens && tokens.before === before && tokens.after === after && tokens.lang === lang ? tokens : null;
  const [open, setOpen] = useState<{ lines: DiffLine[]; at: ReadonlySet<number> }>({ lines, at: new Set() });
  const opened = open.lines === lines ? open.at : NONE_OPEN;
  const unchanged = stats.added === 0 && stats.removed === 0;
  const rows = useMemo(
    () => (unchanged ? lines.map((line, index) => ({ type: "line" as const, line, index })) : foldUnchanged(lines, context, opened)),
    [lines, context, opened, unchanged],
  );
  const scroller = useRef<HTMLDivElement | null>(null);
  useScrollFadeX(scroller, "code");
  const width = lineNumbers ? numberWidth(largestLineNumber(lines)) : "";

  const lineTokens = (line: DiffLine) => (line.kind === "added" ? current?.new?.[(line.newNumber ?? 0) - 1] : current?.old?.[(line.oldNumber ?? 0) - 1]);

  return (
    <div ref={ref} className={cn("relative min-w-0", KAESO_DIFF_VIEW_VARIANTS.surface[surface].classes, className)}>
      <div className="flex h-44 items-center gap-4 px-8 hairline-b">
        {title && (
          <Text as="span" variant="small" tone="secondary" mono truncate className="px-8">
            {title}
          </Text>
        )}
        <span className="ml-auto flex shrink-0 items-center gap-8 px-8 mono text-small tabular">
          {unchanged ? (
            <span className="text-fg-secondary">{t("diffView.noChanges", "No changes")}</span>
          ) : (
            <>
              {stats.added > 0 && (
                <span aria-hidden="true" className="text-success-text">
                  +{stats.added}
                </span>
              )}
              {stats.removed > 0 && (
                <span aria-hidden="true" className="text-danger-text">
                  −{stats.removed}
                </span>
              )}
              <span className="sr-only">
                {t("diffView.summary", "{added, plural, one {# line added} other {# lines added}}, {removed, plural, one {# removed} other {# removed}}", {
                  added: stats.added,
                  removed: stats.removed,
                })}
              </span>
            </>
          )}
        </span>
        {copy && !unchanged && <CopyButton value={unifiedDiff(lines)} label={t("diffView.copy", "Copy diff")} />}
      </div>
      <div
        ref={scroller}
        tabIndex={0}
        role="group"
        aria-label={typeof title === "string" ? title : t("diffView.label", "Changes")}
        className="relative overflow-x-auto py-8 scrollbar-quiet scroll-fade-x focus-ring-inset"
      >
        <pre className="mono text-small text-fg">
          <code className={cn("grid", !wrap && "min-w-max")}>
            {rows.map((row) => {
              if (row.type === "fold") {
                const count = row.to - row.from;
                return (
                  <button
                    key={`fold-${row.from}`}
                    type="button"
                    onClick={() => {
                      scroller.current?.focus({ preventScroll: true });
                      setOpen({ lines, at: new Set([...opened, row.from]) });
                    }}
                    className="flex h-32 cursor-pointer items-center bg-fill-2 font-sans text-small text-fg-secondary transition-control hover:bg-fill-3 hover:text-fg focus-ring-inset"
                  >
                    <span className="sticky left-0 flex items-center gap-8 px-16">
                      <Icon icon={icons.unfold} size="xs" />
                      {t("diffView.showUnchanged", "{count, plural, one {Show # unchanged line} other {Show # unchanged lines}}", { count })}
                    </span>
                  </button>
                );
              }
              const { line } = row;
              const mark = MARK[line.kind];
              const indent = wrap ? (INDENT.exec(line.text)?.[0] ?? "") : "";
              const parts = dropChars(segments(line.text, lineTokens(line), line.changes), indent.length);
              return (
                <span key={row.index} data-diff={line.kind} className={cn("flex leading-code", LINE[line.kind])}>
                  {lineNumbers && (
                    <>
                      <span aria-hidden="true" className={cn("ml-8 shrink-0 text-right text-fg-tertiary select-none tabular", width)}>
                        {line.oldNumber && line.kind !== "added" ? line.oldNumber : ""}
                      </span>
                      <span aria-hidden="true" className={cn("ml-8 shrink-0 text-right text-fg-tertiary select-none tabular", width)}>
                        {line.newNumber && line.kind !== "removed" ? line.newNumber : ""}
                      </span>
                    </>
                  )}
                  <span aria-hidden="true" className={cn("w-32 shrink-0 text-center select-none", mark.className)}>
                    {mark.sign}
                  </span>
                  {line.kind !== "same" && (
                    <span className="sr-only">{line.kind === "added" ? t("diffView.added", "Added") : t("diffView.removed", "Removed")} </span>
                  )}
                  {indent && <span className="shrink-0 whitespace-pre">{indent}</span>}
                  <span className={cn("min-w-0 pr-16", wrap && "pl-16 -indent-16 whitespace-pre-wrap wrap-anywhere")}>
                    {parts.length ? <Code parts={parts} kind={line.kind} /> : " "}
                  </span>
                </span>
              );
            })}
          </code>
        </pre>
      </div>
    </div>
  );
});
DiffView.displayName = "DiffView";
