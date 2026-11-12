import type { UiText } from "../../i18n/context.js";

/** The four kinds of question a decision model answers: yes or no, one of several options, a level on a scale, a box on an image. */
export type DecisionKind = "predicate" | "choice" | "score" | "box";

export function kindLabel(t: UiText, kind: DecisionKind): string {
  switch (kind) {
    case "predicate":
      return t("decision.kindPredicate", "Yes/no");
    case "choice":
      return t("decision.kindChoice", "Choice");
    case "score":
      return t("decision.kindScore", "Score");
    case "box":
      return t("decision.kindBox", "Box");
  }
}

export function questionName(t: UiText, question: { name?: string | undefined }, index: number): string {
  return question.name?.trim() || t("decision.question", "Question {n}", { n: index + 1 });
}

export function optionLabel(t: UiText, option: { value: string } | undefined, index: number): string {
  return option?.value.trim() || t("decision.option", "Option {n}", { n: index + 1 });
}

export function levelLabel(t: UiText, level: { label?: string | undefined } | undefined, index: number): string {
  return level?.label?.trim() || t("decision.level", "Level {n}", { n: index });
}

/** One option of a choice question: the value the model picks, and what it means. */
export interface DecisionOption {
  value: string;
  description?: string | undefined;
}

/** One level of a score question, counted from 0: a short label and what it means. */
export interface DecisionLevel {
  label?: string | undefined;
  description?: string | undefined;
}

/** A typed question for a decision model, as QuestionEditor edits it and DecisionResponse shows its answer. */
export interface DecisionQuestion {
  /** A stable id for the editor's rows and focus; QuestionEditor gives every question it adds one. */
  id?: string | undefined;
  /** The name the answer comes back under, such as `is_invoice`. */
  name?: string | undefined;
  kind: DecisionKind;
  instructions?: string | undefined;
  /** For a yes/no question: what yes means. */
  whenTrue?: string | undefined;
  /** For a yes/no question: what no means. */
  whenFalse?: string | undefined;
  /** For a choice question: the options, in order. */
  options?: readonly DecisionOption[] | undefined;
  /** For a score question: the levels, from level 0 up. */
  levels?: readonly DecisionLevel[] | undefined;
}

/**
 * A decision model's answer to one question. Probabilities run from 0 to 1, indexed by option or level, and are
 * `null` where the provider left one out; nothing is renormalised.
 */
export type DecisionAnswer =
  | { kind: "predicate"; probability: number; confidence?: number | null | undefined }
  | { kind: "choice"; option: number; probabilities?: readonly (number | null | undefined)[] | undefined; confidence?: number | null | undefined }
  | { kind: "score"; score: number; probabilities?: readonly (number | null | undefined)[] | undefined; confidence?: number | null | undefined }
  | { kind: "box"; top: number; bottom: number; left: number; right: number; confidence?: number | null | undefined }
  | { kind: "refusal"; reason?: string | undefined };
