import { Icon, ProviderBadge, icons } from "@logicplanes/kaeso-ui";
import type { ReactNode } from "react";
import type { FlowCanvasEdge, FlowCanvasNode } from "@logicplanes/kaeso-ui/flow-canvas";
import { formatNumber } from "@logicplanes/kaeso-ui/format";
import type { CatalogueModelView } from "~/api/generated/types";
import { modelName } from "~/format/models";
import { t } from "~/i18n";
import { edgeId, type DraftStep, type Problem } from "./draft";
import { conditionSubtitle, conditionTitle, kindLabel, problemLabel, splitSubtitle, splitTitle, weightLabel, type Names } from "./labels";

export const START = "start";

export type TreeInput = {
  alias: string;
  root: DraftStep;
  models: ReadonlyMap<string, CatalogueModelView>;
  providerName: (id: string) => string;
  problems: ReadonlyMap<string, Problem[]>;
  names: Names;
  tries?: ReadonlyMap<string, number> | undefined;
  outcomes?: ReadonlyMap<string, Outcome> | undefined;
};

export type Outcome = { tone: "success" | "warning" | "danger" | "neutral"; label: string };

const into = (step: DraftStep, input: TreeInput): Pick<FlowCanvasEdge, "tone"> => {
  const tone = input.outcomes?.get(step.id)?.tone;
  return tone && tone !== "neutral" ? { tone } : {};
};

export function stepName(step: DraftStep, input: Pick<TreeInput, "models" | "names">): string {
  if (step.type === "model") {
    const m = step.model ? input.models.get(step.model) : undefined;
    if (m) return modelName(m);
    return step.model ? t("models.route.removedModel", "Removed model") : t("models.route.noModel", "No model yet");
  }
  return step.type === "condition" ? conditionTitle(step, input.names) : splitTitle(step);
}

export function glyph(step: DraftStep, models: TreeInput["models"], providerName: (id: string) => string): ReactNode {
  if (step.type === "condition") return <Icon icon={icons.condition} size="sm" />;
  if (step.type === "split") return <Icon icon={icons.split} size="sm" />;
  const m = step.model ? models.get(step.model) : undefined;
  return m ? <ProviderBadge provider={m.provider} name={providerName(m.provider)} size="sm" /> : <Icon icon={icons.model} size="sm" />;
}

function status(problems: Problem[] | undefined): FlowCanvasNode["status"] {
  const [first] = problems ?? [];
  if (!first) return undefined;
  return { tone: first.kind === "noModel" ? "warning" : "danger", label: problemLabel(first) };
}

function node(step: DraftStep, input: TreeInput): FlowCanvasNode {
  const name = stepName(step, input);
  const problem = status(input.problems.get(step.id)) ?? input.outcomes?.get(step.id);
  const attempt = input.tries?.get(step.id);
  const base = {
    id: step.id,
    title: name,
    label: name,
    ...(problem ? { status: problem } : {}),
    ...(attempt ? { badge: t("models.route.try", "Try {n}", { n: attempt }) } : {}),
  };
  if (step.type === "model") {
    const m = step.model ? input.models.get(step.model) : undefined;
    const fallback: FlowCanvasEdge = {
      id: edgeId({ kind: "fallback", of: step.id }),
      label: t("models.route.fallback", "Fallback"),
      variant: "fallback",
      addLabel: t("models.route.addFallback", "Add a fallback to {name}", { name }),
      ...(step.fallback
        ? {
            ...into(step.fallback, input),
            node: node(step.fallback, input),
            insertLabel: t("models.route.insertBefore", "Insert a step before {name}", { name: stepName(step.fallback, input) }),
          }
        : {}),
    };
    return {
      ...base,
      kind: kindLabel.model(),
      ...(m
        ? {
            subtitle:
              step.timeout_seconds === null
                ? input.providerName(m.provider)
                : t("models.route.withTimeout", "{provider} · {seconds} s", {
                    provider: input.providerName(m.provider),
                    seconds: formatNumber(step.timeout_seconds),
                  }),
          }
        : {}),
      leading: glyph(step, input.models, input.providerName),
      terminal: true,
      children: [fallback],
    };
  }
  if (step.type === "condition") {
    return {
      ...base,
      kind: kindLabel.condition(),
      leading: glyph(step, input.models, input.providerName),
      subtitle: conditionSubtitle(step),
      children: (["then", "else"] as const).map((side) => ({
        id: edgeId({ kind: side, of: step.id }),
        label: side === "then" ? t("models.route.then", "Then") : t("models.route.else", "Else"),
        ...into(step[side], input),
        node: node(step[side], input),
        insertLabel: t("models.route.insertBefore", "Insert a step before {name}", { name: stepName(step[side], input) }),
      })),
    };
  }
  return {
    ...base,
    kind: kindLabel.split(),
    leading: glyph(step, input.models, input.providerName),
    subtitle: splitSubtitle(step),
    children: step.branches.map((b, index) => ({
      id: edgeId({ kind: "branch", of: step.id, index }),
      label: weightLabel(b.weight),
      ...into(b.step, input),
      node: node(b.step, input),
      insertLabel: t("models.route.insertBefore", "Insert a step before {name}", { name: stepName(b.step, input) }),
    })),
  };
}

export function canvasTree(input: TreeInput): FlowCanvasNode {
  const alias = input.alias.trim() || t("models.route.newAlias", "New alias");
  return {
    id: START,
    kind: kindLabel.start(),
    leading: <Icon icon={icons.alias} size="sm" />,
    title: alias,
    label: alias,
    children: [
      {
        id: edgeId({ kind: "root" }),
        ...into(input.root, input),
        node: node(input.root, input),
        insertLabel: t("models.route.insertBefore", "Insert a step before {name}", { name: stepName(input.root, input) }),
      },
    ],
  };
}
