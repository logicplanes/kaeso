import { forwardRef, useEffect, useId, useRef, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { useUiText, type UiText } from "../../i18n/context.js";
import { useFormatters } from "../../i18n/formatters.js";
import { Button } from "../button/button.js";
import { Markdown } from "../markdown/markdown.js";
import { ProbabilityBar, probabilityText } from "./probability-bar.js";
import { kindLabel, levelLabel, optionLabel, questionName, type DecisionAnswer, type DecisionQuestion } from "./decision-types.js";

export const KAESO_DECISION_RESPONSE_VARIANTS = {
  status: {
    pending: {
      classes: "",
      description: "Sent, no answers yet: every question's row is drawn with quiet placeholders of the height its answer will take",
    },
    done: { classes: "", description: "Answered: each question's answer, its probabilities and the confidence; a refusal in its own row (default)" },
    error: { classes: "", description: "Failed: the error in place of the answers, and Retry" },
  },
} as const;
export const KAESO_DECISION_RESPONSE_DEFAULT_VARIANTS = { status: "done" } as const;

export type DecisionStatus = keyof typeof KAESO_DECISION_RESPONSE_VARIANTS.status;

/** The image a box question was asked about, to draw the box on. */
export interface DecisionImage {
  src: string;
  alt?: string | undefined;
}

export interface DecisionResponseProps {
  questions: readonly DecisionQuestion[];
  answers?: readonly (DecisionAnswer | null | undefined)[] | undefined;
  status?: DecisionStatus | undefined;
  image?: DecisionImage | undefined;
  maxBars?: number | undefined;
  error?: string | undefined;
  onRetry?: (() => void) | undefined;
  footer?: ReactNode | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
}

const WAITING = "animate-pulse-quiet reduced:animate-none";

interface Bar {
  label: string;
  value: number | null | undefined;
  highlight: boolean;
}

function shownBars(bars: readonly Bar[], max: number): number[] {
  const all = bars.map((_, i) => i);
  if (bars.length <= max) return all;
  const rank = (i: number) => (bars[i]?.highlight ? 2 : (bars[i]?.value ?? -1));
  return [...all]
    .sort((a, b) => rank(b) - rank(a) || a - b)
    .slice(0, max)
    .sort((a, b) => a - b);
}

function Bars({ bars, pending, max, unit, t }: { bars: readonly Bar[]; pending: boolean; max: number; unit: "options" | "levels"; t: UiText }) {
  const [all, setAll] = useState(false);
  const listId = useId();
  const cut = bars.length > max;
  const shown = all || !cut ? bars.map((_, i) => i) : pending ? bars.slice(0, max).map((_, i) => i) : shownBars(bars, max);
  return (
    <div className="flex flex-col gap-4">
      <div id={listId} className="flex flex-col gap-4">
        {shown.map((i) => {
          const bar = bars[i];
          return bar ? <ProbabilityBar key={i} label={bar.label} value={bar.value} highlight={!pending && bar.highlight} pending={pending} /> : null;
        })}
      </div>
      {cut && (
        <span className="flex h-32 items-center">
          <Button
            size="sm"
            variant="ghost"
            disabled={pending}
            aria-expanded={all}
            aria-controls={listId}
            leadingIcon={all ? icons.collapse : icons.expand}
            onClick={() => setAll((v) => !v)}
          >
            {all
              ? t("decisionResponse.showFewer", "Show fewer")
              : unit === "options"
                ? t("decisionResponse.showAllOptions", "Show all {count} options", { count: bars.length })
                : t("decisionResponse.showAllLevels", "Show all {count} levels", { count: bars.length })}
          </Button>
        </span>
      )}
    </div>
  );
}

function clamp01(n: number): number {
  return Number.isFinite(n) ? Math.max(0, Math.min(1, n)) : 0;
}

function BoxAnswer({ answer, image, t }: { answer: Extract<DecisionAnswer, { kind: "box" }> | undefined; image: DecisionImage | undefined; t: UiText }) {
  const { formatPercent } = useFormatters();
  const edges = answer
    ? {
        top: clamp01(Math.min(answer.top, answer.bottom)),
        bottom: clamp01(Math.max(answer.top, answer.bottom)),
        left: clamp01(Math.min(answer.left, answer.right)),
        right: clamp01(Math.max(answer.left, answer.right)),
      }
    : undefined;
  const terms = [
    ["top", t("decisionResponse.boxTop", "Top")],
    ["left", t("decisionResponse.boxLeft", "Left")],
    ["bottom", t("decisionResponse.boxBottom", "Bottom")],
    ["right", t("decisionResponse.boxRight", "Right")],
  ] as const;
  return (
    <div className="flex flex-col gap-8">
      {image && (
        <div className="relative max-w-480 min-w-0 overflow-hidden rounded-frame bg-fill-1 image-outline">
          <img src={image.src} alt={image.alt ?? ""} decoding="async" className="block h-auto w-full" />
          {edges && (
            <span
              aria-hidden
              data-box=""
              className="pointer-events-none absolute rounded-code ring-box"
              style={{
                top: `${edges.top * 100}%`,
                left: `${edges.left * 100}%`,
                width: `${(edges.right - edges.left) * 100}%`,
                height: `${(edges.bottom - edges.top) * 100}%`,
              }}
            />
          )}
        </div>
      )}
      <dl className="flex flex-wrap gap-x-24 gap-y-4">
        {terms.map(([edge, term]) => (
          <div key={edge} className="flex min-h-22 items-baseline gap-8">
            <dt className="text-small text-fg-tertiary">{term}</dt>
            <dd className="text-small text-fg tabular">
              {edges ? formatPercent(edges[edge], 1) : <span aria-hidden className={cn("inline-block h-12 w-40 rounded-pill bg-fill-1", WAITING)} />}
            </dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

function QuestionAnswer({
  question,
  index,
  answer,
  pending,
  image,
  maxBars,
}: {
  question: DecisionQuestion;
  index: number;
  answer: DecisionAnswer | null | undefined;
  pending: boolean;
  image: DecisionImage | undefined;
  maxBars: number;
}) {
  const t = useUiText();
  const formatters = useFormatters();
  const { formatNumber } = formatters;
  const nameId = useId();
  const kind = question.kind;
  const own = answer && (answer.kind === kind || answer.kind === "refusal") ? answer : undefined;
  const refused = own?.kind === "refusal" ? own : undefined;
  const confidence = own && own.kind !== "refusal" && typeof own.confidence === "number" && Number.isFinite(own.confidence) ? own.confidence : undefined;

  let text: string | undefined;
  let bars: Bar[] | undefined;
  let unit: "options" | "levels" = "options";
  if (kind === "predicate") {
    const p = own?.kind === "predicate" && Number.isFinite(own.probability) ? clamp01(own.probability) : undefined;
    const yes = t("decisionResponse.yes", "Yes");
    const no = t("decisionResponse.no", "No");
    if (p !== undefined) text = p >= 0.5 ? yes : no;
    bars = [
      { label: yes, value: p, highlight: p !== undefined && p >= 0.5 },
      { label: no, value: p === undefined ? undefined : 1 - p, highlight: p !== undefined && p < 0.5 },
    ];
  } else if (kind === "choice") {
    const options = question.options ?? [];
    const chosen = own?.kind === "choice" ? own.option : undefined;
    const probabilities = own?.kind === "choice" ? (own.probabilities ?? []) : [];
    if (chosen !== undefined) text = optionLabel(t, options[chosen], chosen);
    bars = options.map((o, i) => ({ label: optionLabel(t, o, i), value: probabilities[i], highlight: i === chosen }));
  } else if (kind === "score") {
    unit = "levels";
    const levels = question.levels ?? [];
    const score = own?.kind === "score" && Number.isFinite(own.score) ? own.score : undefined;
    const probabilities = own?.kind === "score" ? (own.probabilities ?? []) : [];
    const nearest = score === undefined ? undefined : Math.max(0, Math.min(levels.length - 1, Math.round(score)));
    if (score !== undefined) {
      const values = { score: formatNumber(score, { maximumFractionDigits: 2 }), max: formatNumber(Math.max(0, levels.length - 1)) };
      const near = nearest !== undefined ? levels[nearest]?.label?.trim() : undefined;
      text = near
        ? t("decisionResponse.scoreNear", "{score} of {max}, nearest {level}", { ...values, level: near })
        : t("decisionResponse.score", "{score} of {max}", values);
    }
    bars = levels.map((l, i) => ({ label: levelLabel(t, l, i), value: probabilities[i], highlight: i === nearest }));
  }

  return (
    <li className="flex min-w-0 flex-col gap-8 px-12" aria-labelledby={nameId} data-kind={kind} data-refused={refused ? "" : undefined}>
      <div className="flex min-w-0 items-baseline justify-between gap-12">
        <span id={nameId} className="min-w-0 text-body font-medium text-fg wrap-anywhere">
          {questionName(t, question, index)}
        </span>
        <span className="shrink-0 text-small text-fg-tertiary">{kindLabel(t, kind)}</span>
      </div>
      {refused ? (
        <div className="flex min-h-22 flex-col gap-4">
          <span className="text-body text-fg-secondary">{t("decisionResponse.refused", "Refused: the model declined to answer this question")}</span>
          {refused.reason && <span className="text-small text-fg-tertiary wrap-anywhere">{refused.reason}</span>}
        </div>
      ) : (
        <>
          {kind !== "box" && (
            <div className="flex min-h-22 flex-wrap items-baseline gap-x-12 gap-y-4">
              {pending ? (
                <span aria-hidden className={cn("h-12 w-120 self-center rounded-pill bg-fill-1", WAITING)} />
              ) : text !== undefined ? (
                <span className="min-w-0 text-body text-fg wrap-anywhere">{text}</span>
              ) : (
                <span className="text-body text-fg-tertiary">{t("decisionResponse.noAnswer", "No answer")}</span>
              )}
              {confidence !== undefined && (
                <span className="text-small text-fg-tertiary tabular">
                  {t("decisionResponse.confidence", "Confidence {percent}", { percent: probabilityText(clamp01(confidence), t, formatters) })}
                </span>
              )}
            </div>
          )}
          {kind === "box" && (
            <>
              {!pending && !own && <span className="text-body text-fg-tertiary">{t("decisionResponse.noAnswer", "No answer")}</span>}
              <BoxAnswer answer={own?.kind === "box" ? own : undefined} image={image} t={t} />
              {confidence !== undefined && (
                <span className="text-small text-fg-tertiary tabular">
                  {t("decisionResponse.confidence", "Confidence {percent}", { percent: probabilityText(clamp01(confidence), t, formatters) })}
                </span>
              )}
            </>
          )}
          {bars && bars.length > 0 && <Bars bars={bars} pending={pending} max={maxBars} unit={unit} t={t} />}
        </>
      )}
    </li>
  );
}

export const DecisionResponse = forwardRef<HTMLElement, DecisionResponseProps>(function DecisionResponse(
  { questions, answers = [], status = "done", image, maxBars = 8, error, onRetry, footer, "aria-label": ariaLabelProp, className },
  ref,
) {
  const t = useUiText();
  const ariaLabel = ariaLabelProp ?? t("responseView.response", "Response");
  const pending = status === "pending";
  const said = useId();
  const before = useRef(status);
  const [announced, setAnnounced] = useState("");
  useEffect(() => {
    const was = before.current;
    before.current = status;
    if (pending) setAnnounced("");
    else if (was === "pending" && status === "done") setAnnounced(t("responseView.endedDone", "{label} finished", { label: ariaLabel }));
  }, [status, pending, ariaLabel, t]);
  const hasFooter = footer !== undefined && footer !== null;
  const bars = Math.max(1, Math.floor(maxBars));
  return (
    <article
      ref={ref}
      aria-label={ariaLabel}
      aria-busy={pending || undefined}
      {...(pending ? { "aria-describedby": said } : {})}
      className={cn("flex min-w-0 flex-col gap-16", className)}
    >
      {status === "error" ? (
        <div className="flex flex-col items-start gap-8 px-12">
          <div role="alert" className="flex min-w-0 flex-col gap-4">
            <span className="text-small font-medium text-fg">{t("decisionResponse.failed", "No answers")}</span>
            {error && <Markdown text={error} size="small" tone="secondary" className="max-w-prose min-w-0" />}
          </div>
          {onRetry && (
            <Button size="sm" variant="secondary" onClick={onRetry}>
              {t("responseView.retry", "Retry")}
            </Button>
          )}
        </div>
      ) : (
        <ol className="flex min-w-0 flex-col gap-24">
          {questions.map((q, i) => (
            <QuestionAnswer key={q.id ?? i} question={q} index={i} answer={pending ? undefined : answers[i]} pending={pending} image={image} maxBars={bars} />
          ))}
        </ol>
      )}
      {pending && (
        <p id={said} className="sr-only">
          {t("decisionResponse.waiting", "Waiting for the answers")}
        </p>
      )}
      {(hasFooter || (status === "done" && onRetry)) && (
        <div className="flex min-h-32 min-w-0 flex-wrap items-center gap-x-4 gap-y-4">
          {status === "done" && onRetry && (
            <span data-reply-actions="" className="flex items-center gap-x-4 transition-arrive reduced:transition-none starting:-ml-36 starting:opacity-0">
              <Button size="sm" variant="quiet" icon={icons.retry} aria-label={t("responseView.retry", "Retry")} onClick={onRetry} />
            </span>
          )}
          {hasFooter && <div className={cn("min-w-0", status === "done" && onRetry ? "px-8" : "px-12")}>{footer}</div>}
        </div>
      )}
      <span role="status" className="sr-only">
        {announced}
      </span>
    </article>
  );
});
DecisionResponse.displayName = "DecisionResponse";
