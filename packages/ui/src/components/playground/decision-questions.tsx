import { forwardRef, useId } from "react";
import { cn } from "../../utils/cn.js";
import { useUiText, type UiText } from "../../i18n/context.js";
import { useFormatters } from "../../i18n/formatters.js";
import { kindLabel, optionLabel, questionName, type DecisionQuestion } from "./decision-types.js";

export const KAESO_DECISION_QUESTIONS_VARIANTS = {
  layout: {
    list: {
      classes: "",
      description: "Each question with its type, instructions and what its answers mean, a hairline between questions (default)",
    },
  },
} as const;
export const KAESO_DECISION_QUESTIONS_DEFAULT_VARIANTS = { layout: "list" } as const;

export interface DecisionQuestionsProps {
  questions: readonly DecisionQuestion[];
  layout?: keyof typeof KAESO_DECISION_QUESTIONS_VARIANTS.layout | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
}

interface Term {
  key: string;
  term: string;
  mono: boolean;
  description: string | undefined;
}

function termsOf(question: DecisionQuestion, t: UiText, formatNumber: (n: number) => string): Term[] {
  switch (question.kind) {
    case "predicate": {
      const yes = question.whenTrue?.trim();
      const no = question.whenFalse?.trim();
      if (!yes && !no) return [];
      return [
        { key: "yes", term: t("decisionResponse.yes", "Yes"), mono: false, description: yes },
        { key: "no", term: t("decisionResponse.no", "No"), mono: false, description: no },
      ];
    }
    case "choice":
      return (question.options ?? []).map((o, i) => ({
        key: `o${i}`,
        term: optionLabel(t, o, i),
        mono: !!o.value.trim(),
        description: o.description?.trim() || undefined,
      }));
    case "score":
      return (question.levels ?? []).map((l, i) => {
        const label = l.label?.trim();
        const description = l.description?.trim();
        return {
          key: `l${i}`,
          term: formatNumber(i),
          mono: false,
          description: [label, description].filter(Boolean).join(": ") || undefined,
        };
      });
    case "box":
      return [];
  }
}

function Question({ question, index }: { question: DecisionQuestion; index: number }) {
  const t = useUiText();
  const { formatNumber } = useFormatters();
  const nameId = useId();
  const instructions = question.instructions?.trim();
  const terms = termsOf(question, t, (n) => formatNumber(n));
  const caption =
    question.kind === "choice"
      ? t("decisionQuestions.options", "{count, plural, one {# option} other {# options}}", { count: terms.length })
      : question.kind === "score"
        ? t("decisionQuestions.levels", "{count, plural, one {# level} other {# levels}}", { count: terms.length })
        : undefined;
  return (
    <li className="flex min-w-0 flex-col gap-12 px-12 py-16 not-first:hairline-t first:pt-0 last:pb-0" aria-labelledby={nameId} data-kind={question.kind}>
      <div className="flex min-w-0 flex-col gap-4">
        <div className="flex min-w-0 flex-wrap items-baseline justify-between gap-x-12 gap-y-4">
          <span id={nameId} className="min-w-0 text-body font-medium text-fg wrap-anywhere">
            {questionName(t, question, index)}
          </span>
          <span className="flex shrink-0 items-baseline gap-12 text-small text-fg-tertiary">
            {caption && <span className="tabular">{caption}</span>}
            <span>{kindLabel(t, question.kind)}</span>
          </span>
        </div>
        {instructions ? (
          <span className="max-w-prose text-body text-fg-secondary wrap-break-word whitespace-pre-line">{instructions}</span>
        ) : (
          <span className="text-body text-fg-tertiary">{t("decisionQuestions.noInstructions", "No instructions")}</span>
        )}
      </div>
      {terms.length > 0 && (
        <dl className="grid min-w-0 grid-cols-term gap-x-16 gap-y-6">
          {terms.map((term) => (
            <div key={term.key} className="col-span-full grid min-h-22 grid-cols-subgrid items-baseline">
              <dt className={cn("min-w-0 text-small text-fg wrap-anywhere", term.mono ? "font-mono" : "tabular font-medium")}>{term.term}</dt>
              <dd className={cn("min-w-0 text-small wrap-break-word", term.description ? "text-fg-secondary" : "text-fg-tertiary")}>
                {term.description ?? t("decisionQuestions.noDescription", "No description")}
              </dd>
            </div>
          ))}
        </dl>
      )}
    </li>
  );
}

/** The questions a decision model was asked, as an app sent them: each with its type, instructions, and what yes and no, each option or each level means. */
export const DecisionQuestions = forwardRef<HTMLOListElement, DecisionQuestionsProps>(function DecisionQuestions(
  { questions, layout = "list", "aria-label": ariaLabel, className },
  ref,
) {
  const t = useUiText();
  return (
    <ol
      ref={ref}
      aria-label={ariaLabel ?? t("decisionQuestions.label", "Questions")}
      className={cn("flex min-w-0 flex-col", KAESO_DECISION_QUESTIONS_VARIANTS.layout[layout].classes, className)}
    >
      {questions.map((q, i) => (
        <Question key={q.id ?? i} question={q} index={i} />
      ))}
    </ol>
  );
});
DecisionQuestions.displayName = "DecisionQuestions";
