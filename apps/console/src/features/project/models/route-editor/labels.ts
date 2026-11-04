import type { RouteFailure, RouteFeature, RouteRule } from "~/api/generated/types";
import { formatNumber } from "@logicplanes/kaeso-ui/format";
import { dialectLabel } from "~/format/labels";
import { locale, t } from "~/i18n";
import type { DraftCondition, DraftSplit, Move, Problem, Template } from "./draft";

export const kindLabel = {
  start: () => t("models.route.kind.start", "Start"),
  model: () => t("models.route.kind.model", "Model"),
  condition: () => t("models.route.kind.condition", "Condition"),
  split: () => t("models.route.kind.split", "Split"),
};

export const failureLabel: Record<RouteFailure, () => string> = {
  unavailable: () => t("models.route.failure.unavailable", "The provider is unavailable"),
  limited: () => t("models.route.failure.limited", "A rate limit or budget is reached"),
  unsupported: () => t("models.route.failure.unsupported", "The model can't take the request"),
  rejected: () => t("models.route.failure.rejected", "The provider refuses the request"),
};

export const failureHint: Record<RouteFailure, () => string> = {
  unavailable: () => t("models.route.failureHint.unavailable", "Unreachable, overloaded, 5xx, a timeout or a refused provider key."),
  limited: () => t("models.route.failureHint.limited", "The provider's 429, or a Kaeso limit or budget for this model."),
  unsupported: () => t("models.route.failureHint.unsupported", "It lacks images, PDFs, tools or another feature the request uses."),
  rejected: () => t("models.route.failureHint.rejected", "A 400 such as a context that is too long; only for a better fallback."),
};

export const featureLabel: Record<RouteFeature, () => string> = {
  images: () => t("models.route.feature.images", "Images"),
  pdfs: () => t("models.route.feature.pdfs", "PDFs"),
  tools: () => t("models.route.feature.tools", "Tools"),
  reasoning: () => t("models.route.feature.reasoning", "Reasoning"),
  structured_output: () => t("models.route.feature.structuredOutput", "A JSON schema"),
  streaming: () => t("models.route.feature.streaming", "Streaming"),
};

export const fieldLabel: Record<RouteRule["field"], () => string> = {
  header: () => t("models.route.field.header", "Header"),
  format: () => t("models.route.field.format", "API format"),
  feature: () => t("models.route.field.feature", "Feature"),
  input_tokens: () => t("models.route.field.inputTokens", "Input tokens"),
  key: () => t("models.route.field.key", "API key"),
  member: () => t("models.route.field.member", "Member"),
  service_account: () => t("models.route.field.serviceAccount", "Service account"),
};

export type Names = { key: (id: string) => string; member: (id: string) => string; serviceAccount: (id: string) => string };

const list = (items: string[]) =>
  items.length <= 3
    ? new Intl.ListFormat(locale(), { type: "disjunction" }).format(items)
    : t("models.route.listMore", "{first} or {count} others", { first: items.slice(0, 1).join(""), count: formatNumber(items.length - 1) });

function headerSummary(rule: Extract<RouteRule, { field: "header" }>): string {
  const name = rule.name.trim() || t("models.route.unnamedHeader", "A header");
  const value = rule.value ?? "";
  switch (rule.op) {
    case "equals":
      return t("models.route.rule.equals", "{name} is {value}", { name, value });
    case "not_equals":
      return t("models.route.rule.notEquals", "{name} isn't {value}", { name, value });
    case "contains":
      return t("models.route.rule.contains", "{name} contains {value}", { name, value });
    case "present":
      return t("models.route.rule.present", "{name} is sent", { name });
    case "absent":
      return t("models.route.rule.absent", "{name} isn't sent", { name });
  }
}

export function ruleSummary(rule: RouteRule, names: Names): string {
  switch (rule.field) {
    case "header":
      return headerSummary(rule);
    case "format": {
      const formats = list(rule.value.map(dialectLabel));
      return rule.op === "in"
        ? t("models.route.rule.formatIn", "Sent as {formats}", { formats })
        : t("models.route.rule.formatNotIn", "Not sent as {formats}", { formats });
    }
    case "feature": {
      const feature = featureLabel[rule.value]();
      return rule.op === "used"
        ? t("models.route.rule.uses", "Uses {feature}", { feature })
        : t("models.route.rule.doesntUse", "Doesn't use {feature}", { feature });
    }
    case "input_tokens": {
      const count = formatNumber(rule.value);
      return rule.op === "at_least"
        ? t("models.route.rule.atLeast", "{count} input tokens or more", { count })
        : t("models.route.rule.below", "Under {count} input tokens", { count });
    }
    case "key": {
      const who = list(rule.value.map(names.key));
      return rule.op === "in" ? t("models.route.rule.keyIn", "Sent with {who}", { who }) : t("models.route.rule.keyNotIn", "Not sent with {who}", { who });
    }
    case "member": {
      const who = list(rule.value.map(names.member));
      return rule.op === "in" ? t("models.route.rule.memberIn", "Sent by {who}", { who }) : t("models.route.rule.memberNotIn", "Not sent by {who}", { who });
    }
    case "service_account": {
      const who = list(rule.value.map(names.serviceAccount));
      return rule.op === "in" ? t("models.route.rule.accountIn", "Sent by {who}", { who }) : t("models.route.rule.accountNotIn", "Not sent by {who}", { who });
    }
  }
}

export function conditionTitle(step: DraftCondition, names: Names): string {
  const [first] = step.rules;
  return first ? ruleSummary(first, names) : t("models.route.noRules", "No rules yet");
}

export function conditionSubtitle(step: DraftCondition): string {
  const count = step.rules.length;
  if (count === 1) return t("models.route.oneRule", "1 rule");
  return step.match === "all"
    ? t("models.route.allRules", "All of {count} rules", { count: formatNumber(count) })
    : t("models.route.anyRule", "Any of {count} rules", { count: formatNumber(count) });
}

export function splitTitle(step: DraftSplit): string {
  switch (step.by) {
    case "caller":
      return t("models.route.byCaller", "By caller");
    case "header": {
      const header = step.header.trim();
      return header ? t("models.route.byHeaderName", "By {header}", { header }) : t("models.route.byHeader", "By header");
    }
    case "request":
      return t("models.route.byRequest", "Per request");
  }
}

export function splitSubtitle(step: DraftSplit): string {
  return t("models.route.branches", "{count} branches", { count: formatNumber(step.branches.length) });
}

export function weightLabel(weight: number): string {
  return t("models.route.weight", "{weight}%", { weight: formatNumber(weight) });
}

export function problemLabel(p: Problem): string {
  switch (p.kind) {
    case "noModel":
      return t("models.route.problem.noModel", "Choose a model");
    case "removedModel":
      return t("models.route.problem.removedModel", "Model removed");
    case "weights":
      return t("models.route.problem.weights", "Weights add up to {total}%", { total: formatNumber(p.total) });
    case "headerName":
      return t("models.route.problem.headerName", "Name the header");
    case "secretHeader":
      return t("models.route.problem.secretHeader", "Header carries credentials");
    case "headerValue":
      return t("models.route.problem.headerValue", "Give the header's value");
    case "noFormats":
      return t("models.route.problem.noFormats", "Choose a format");
    case "tokens":
      return t("models.route.problem.tokens", "Give a token count");
    case "noIds":
      return t("models.route.problem.noIds", "Choose at least one");
    case "timeout":
      return t("models.route.problem.timeout", "Timeout out of range");
    case "fallBackOn":
      return t("models.route.problem.fallBackOn", "Choose when it falls back");
    case "tooMany":
      return t("models.route.problem.tooMany", "Over 32 steps");
    case "tooDeep":
      return t("models.route.problem.tooDeep", "Over 12 steps deep");
  }
}

export const templateLabel: Record<Template, () => string> = {
  single: () => t("models.route.template.single", "One model"),
  chain: () => t("models.route.template.chain", "A fallback chain"),
  header: () => t("models.route.template.header", "By a header"),
  long: () => t("models.route.template.long", "Long prompts elsewhere"),
  ab: () => t("models.route.template.ab", "An A/B test"),
};

export const templateHint: Record<Template, () => string> = {
  single: () => t("models.route.templateHint.single", "Every request goes to one model."),
  chain: () => t("models.route.templateHint.chain", "Tries a model, then the next one when it fails."),
  header: () => t("models.route.templateHint.header", "Requests with x-plan: enterprise get their own models."),
  long: () => t("models.route.templateHint.long", "Requests of 100,000 input tokens or more go to another model."),
  ab: () => t("models.route.templateHint.ab", "Callers split 50/50 between two models, each staying on one."),
};

export const moveLabel: Record<Move, () => string> = {
  earlier: () => t("models.route.moveEarlier", "Try earlier"),
  later: () => t("models.route.moveLater", "Try later"),
};

export function headerHint(p: Problem): string {
  return p.kind === "secretHeader"
    ? t("models.route.secretHeaderHint", "Routes can't read headers that carry credentials.")
    : t("models.route.headerNameHint", "Enter a lowercase header name, such as x-user-id.");
}
