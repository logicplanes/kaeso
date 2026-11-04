import type { RouteFailure, RouteRule, RouteStep, TraceStep } from "~/api/generated/types";

export const MAX_STEPS = 32;
export const MAX_DEPTH = 12;
export const MAX_RULES = 10;
export const MIN_BRANCHES = 2;
export const MAX_BRANCHES = 8;
export const MAX_TIMEOUT = 3600;
export const MAX_TOKENS = 10_000_000;
export const MAX_IDS = 100;
export const MAX_HEADER_VALUE = 200;
export const FAILURES: readonly RouteFailure[] = ["unavailable", "limited", "unsupported", "rejected"];
export const DEFAULT_FALL_BACK_ON: readonly RouteFailure[] = ["unavailable", "limited", "unsupported"];
export const SECRET_HEADERS: readonly string[] = ["authorization", "proxy-authorization", "cookie", "x-api-key", "x-goog-api-key", "api-key"];

export type DraftModel = {
  id: string;
  type: "model";
  model: string | null;
  fallback: DraftStep | null;
  fall_back_on: RouteFailure[];
  timeout_seconds: number | null;
};
export type DraftCondition = { id: string; type: "condition"; match: "all" | "any"; rules: DraftRule[]; then: DraftStep; else: DraftStep };
export type DraftBranch = { id: string; weight: number; step: DraftStep };
export type DraftSplit = { id: string; type: "split"; by: "request" | "caller" | "header"; header: string; branches: DraftBranch[] };
export type DraftStep = DraftModel | DraftCondition | DraftSplit;
export type DraftRule = RouteRule & { id: string };
export type StepKind = DraftStep["type"];

let counter = 0;
export const freshId = (prefix = "s") => `${prefix}${(++counter).toString(36)}`;

export const emptyModel = (model: string | null = null): DraftModel => ({
  id: freshId(),
  type: "model",
  model,
  fallback: null,
  fall_back_on: [...DEFAULT_FALL_BACK_ON],
  timeout_seconds: null,
});

export function defaultRule(): DraftRule {
  return { id: freshId("r"), field: "header", name: "", op: "equals", value: "" };
}

export function fromRoute(step: RouteStep): DraftStep {
  switch (step.type) {
    case "model":
      return {
        id: freshId(),
        type: "model",
        model: step.model,
        fallback: step.fallback ? fromRoute(step.fallback) : null,
        fall_back_on: [...(step.fall_back_on ?? DEFAULT_FALL_BACK_ON)],
        timeout_seconds: step.timeout_seconds ?? null,
      };
    case "condition":
      return {
        id: freshId(),
        type: "condition",
        match: step.match,
        rules: step.rules.map((r) => ({ ...headerValue(r), id: freshId("r") })),
        then: fromRoute(step.then),
        else: fromRoute(step.else),
      };
    case "split":
      return {
        id: freshId(),
        type: "split",
        by: step.by,
        header: step.header ?? "",
        branches: step.branches.map((b) => ({ id: freshId("b"), weight: b.weight, step: fromRoute(b.step) })),
      };
  }
}

function headerValue(rule: RouteRule): RouteRule {
  if (rule.field !== "header") return rule;
  const presence = rule.op === "present" || rule.op === "absent";
  return { ...rule, value: presence ? null : (rule.value ?? "") };
}

const ruleOut = ({ id: _id, ...rule }: DraftRule): RouteRule => {
  if (rule.field === "header") {
    const presence = rule.op === "present" || rule.op === "absent";
    return { field: "header", name: rule.name.trim(), op: rule.op, value: presence ? null : (rule.value ?? "") };
  }
  return rule;
};

export function toRoute(step: DraftStep): RouteStep {
  switch (step.type) {
    case "model":
      return {
        type: "model",
        model: step.model ?? "",
        fallback: step.fallback ? toRoute(step.fallback) : null,
        fall_back_on: FAILURES.filter((f) => step.fall_back_on.includes(f)),
        timeout_seconds: step.timeout_seconds,
      };
    case "condition":
      return { type: "condition", match: step.match, rules: step.rules.map(ruleOut), then: toRoute(step.then), else: toRoute(step.else) };
    case "split":
      return {
        type: "split",
        by: step.by,
        ...(step.by === "header" ? { header: step.header.trim() } : {}),
        branches: step.branches.map((b) => ({ weight: b.weight, step: toRoute(b.step) })),
      };
  }
}

export function sameRoute(a: RouteStep, b: RouteStep): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export function* walk(step: DraftStep, path = "route", depth = 1): Generator<{ step: DraftStep; path: string; depth: number }> {
  yield { step, path, depth };
  if (step.type === "model" && step.fallback) yield* walk(step.fallback, `${path}.fallback`, depth + 1);
  if (step.type === "condition") {
    yield* walk(step.then, `${path}.then`, depth + 1);
    yield* walk(step.else, `${path}.else`, depth + 1);
  }
  if (step.type === "split") for (const [i, b] of step.branches.entries()) yield* walk(b.step, `${path}.branches.${i}.step`, depth + 1);
}

export function find(root: DraftStep, id: string): DraftStep | undefined {
  for (const { step } of walk(root)) if (step.id === id) return step;
  return undefined;
}

export function stepAt(root: DraftStep, param: string): { step: DraftStep; field: string } {
  let best = { step: root, path: "route" };
  for (const { step, path } of walk(root)) if (param.startsWith(`${path}.`) && path.length > best.path.length) best = { step, path };
  return { step: best.step, field: param.slice(best.path.length).replace(/^\./, "") };
}

export function modelsOf(root: DraftStep): string[] {
  const out: string[] = [];
  for (const { step } of walk(root)) if (step.type === "model" && step.model && !out.includes(step.model)) out.push(step.model);
  return out;
}

export function map(root: DraftStep, id: string, change: (step: DraftStep) => DraftStep): DraftStep {
  if (root.id === id) return change(root);
  switch (root.type) {
    case "model":
      return root.fallback ? { ...root, fallback: map(root.fallback, id, change) } : root;
    case "condition":
      return { ...root, then: map(root.then, id, change), else: map(root.else, id, change) };
    case "split":
      return { ...root, branches: root.branches.map((b) => ({ ...b, step: map(b.step, id, change) })) };
  }
}

export function fresh(kind: StepKind, keep?: DraftStep): DraftStep {
  if (kind === "model") return keep ? { ...emptyModel(), fallback: keep } : emptyModel();
  if (kind === "condition") return { id: freshId(), type: "condition", match: "all", rules: [defaultRule()], then: keep ?? emptyModel(), else: emptyModel() };
  return {
    id: freshId(),
    type: "split",
    by: "request",
    header: "",
    branches: [
      { id: freshId("b"), weight: 50, step: keep ?? emptyModel() },
      { id: freshId("b"), weight: 50, step: emptyModel() },
    ],
  };
}

export type Edge =
  | { kind: "root" }
  | { kind: "fallback"; of: string }
  | { kind: "then"; of: string }
  | { kind: "else"; of: string }
  | { kind: "branch"; of: string; index: number };

export function edgeId(edge: Edge): string {
  switch (edge.kind) {
    case "root":
      return "root";
    case "branch":
      return `branch:${edge.of}:${edge.index}`;
    default:
      return `${edge.kind}:${edge.of}`;
  }
}

export function parseEdge(id: string): Edge {
  const [kind, of = "", index = "0"] = id.split(":");
  if (kind === "branch") return { kind, of, index: Number(index) };
  if (kind === "fallback" || kind === "then" || kind === "else") return { kind, of };
  return { kind: "root" };
}

function childAt(root: DraftStep, edge: Edge): DraftStep | null | undefined {
  if (edge.kind === "root") return root;
  const parent = find(root, edge.of);
  if (!parent) return undefined;
  if (edge.kind === "fallback") return parent.type === "model" ? parent.fallback : undefined;
  if (edge.kind === "then" || edge.kind === "else") return parent.type === "condition" ? parent[edge.kind] : undefined;
  return parent.type === "split" ? parent.branches[edge.index]?.step : undefined;
}

function setChild(root: DraftStep, edge: Edge, next: DraftStep | null): DraftStep {
  if (edge.kind === "root") return next || root;
  return map(root, edge.of, (parent) => {
    if (edge.kind === "fallback" && parent.type === "model") return { ...parent, fallback: next };
    if ((edge.kind === "then" || edge.kind === "else") && parent.type === "condition" && next) return { ...parent, [edge.kind]: next };
    if (edge.kind === "branch" && parent.type === "split" && next)
      return { ...parent, branches: parent.branches.map((b, i) => (i === edge.index ? { ...b, step: next } : b)) };
    return parent;
  });
}

export function add(root: DraftStep, edge: Edge, kind: StepKind, insert: boolean): { root: DraftStep; added: string } {
  const current = childAt(root, edge);
  const step = insert && current ? fresh(kind, current) : fresh(kind);
  return { root: setChild(root, edge, step), added: step.id };
}

export function parentEdge(root: DraftStep, id: string): Edge | undefined {
  for (const { step } of walk(root)) {
    if (step.type === "model" && step.fallback?.id === id) return { kind: "fallback", of: step.id };
    if (step.type === "condition") {
      if (step.then.id === id) return { kind: "then", of: step.id };
      if (step.else.id === id) return { kind: "else", of: step.id };
    }
    if (step.type === "split") {
      const index = step.branches.findIndex((b) => b.step.id === id);
      if (index >= 0) return { kind: "branch", of: step.id, index };
    }
  }
  return root.id === id ? { kind: "root" } : undefined;
}

export type Removal = { kind: "step" } | { kind: "keep"; edge: "then" | "else" } | { kind: "keepBranch"; branch: DraftBranch } | { kind: "branch" };

export function removals(root: DraftStep, id: string): Removal[] {
  const step = find(root, id);
  const edge = parentEdge(root, id);
  if (!step || !edge) return [];
  const out: Removal[] = [];
  if (step.type === "model" && (step.fallback || edge.kind === "fallback")) out.push({ kind: "step" });
  if (step.type === "condition") out.push({ kind: "keep", edge: "then" }, { kind: "keep", edge: "else" });
  if (step.type === "split") for (const branch of step.branches) out.push({ kind: "keepBranch", branch });
  if (edge.kind === "branch") {
    const parent = find(root, edge.of);
    if (parent?.type === "split" && parent.branches.length > MIN_BRANCHES) out.push({ kind: "branch" });
  }
  return out;
}

export function remove(root: DraftStep, id: string, how: Removal): DraftStep {
  const step = find(root, id);
  const edge = parentEdge(root, id);
  if (!step || !edge) return root;
  if (how.kind === "branch" && edge.kind === "branch")
    return map(root, edge.of, (parent) => {
      const split = parent as DraftSplit;
      const gone = split.branches.filter((_, i) => i === edge.index).reduce((n, b) => n + b.weight, 0);
      const rest = split.branches.filter((_, i) => i !== edge.index).map((b, i) => (i === 0 ? { ...b, weight: b.weight + gone } : b));
      return { ...split, branches: rest };
    });
  if (how.kind === "keepBranch") return setChild(root, edge, how.branch.step);
  if (how.kind === "keep") return step.type === "condition" ? setChild(root, edge, step[how.edge]) : root;
  return step.type === "model" ? setChild(root, edge, step.fallback) : root;
}

export function defaultRemoval(root: DraftStep, id: string): Removal | undefined {
  return removals(root, id)[0];
}

export function evenWeights(count: number): number[] {
  const base = Math.floor(100 / count);
  return Array.from({ length: count }, (_, i) => base + (i < 100 - base * count ? 1 : 0));
}

export function shareEvenly(branches: DraftBranch[]): DraftBranch[] {
  const base = Math.floor(100 / branches.length);
  const extra = 100 - base * branches.length;
  return branches.map((b, i) => ({ ...b, weight: base + (i < extra ? 1 : 0) }));
}

export type Problem =
  | { kind: "noModel" }
  | { kind: "removedModel" }
  | { kind: "weights"; total: number }
  | { kind: "headerName" }
  | { kind: "secretHeader" }
  | { kind: "headerValue" }
  | { kind: "noFormats" }
  | { kind: "tokens" }
  | { kind: "noIds" }
  | { kind: "timeout" }
  | { kind: "fallBackOn" }
  | { kind: "tooMany" }
  | { kind: "tooDeep" };

const HEADER_NAME = /^[a-z0-9!#$%&'*+\-.^_`|~]{1,100}$/;

export function headerProblem(name: string): Problem | undefined {
  const trimmed = name.trim();
  if (!HEADER_NAME.test(trimmed)) return { kind: "headerName" };
  if (SECRET_HEADERS.includes(trimmed)) return { kind: "secretHeader" };
  return undefined;
}

export function ruleProblem(rule: RouteRule): Problem | undefined {
  switch (rule.field) {
    case "header": {
      const named = headerProblem(rule.name);
      if (named) return named;
      const presence = rule.op === "present" || rule.op === "absent";
      const value = rule.value ?? "";
      if (!presence && (value === "" || [...value].length > MAX_HEADER_VALUE)) return { kind: "headerValue" };
      return undefined;
    }
    case "format":
      return rule.value.length === 0 ? { kind: "noFormats" } : undefined;
    case "input_tokens":
      return !Number.isInteger(rule.value) || rule.value < 1 || rule.value > MAX_TOKENS ? { kind: "tokens" } : undefined;
    case "key":
    case "member":
    case "service_account":
      return rule.value.length === 0 || rule.value.length > MAX_IDS ? { kind: "noIds" } : undefined;
    case "feature":
      return undefined;
  }
}

export function stepProblems(step: DraftStep, known: (model: string) => boolean): Problem[] {
  const out: Problem[] = [];
  if (step.type === "model") {
    if (!step.model) out.push({ kind: "noModel" });
    else if (!known(step.model)) out.push({ kind: "removedModel" });
    if (step.timeout_seconds !== null && (!Number.isInteger(step.timeout_seconds) || step.timeout_seconds < 1 || step.timeout_seconds > MAX_TIMEOUT))
      out.push({ kind: "timeout" });
    if (step.fallback && step.fall_back_on.length === 0) out.push({ kind: "fallBackOn" });
  }
  if (step.type === "condition") for (const r of step.rules) out.push(...[ruleProblem(r)].filter((p): p is Problem => !!p));
  if (step.type === "split") {
    const named = step.by === "header" ? headerProblem(step.header) : undefined;
    if (named) out.push(named);
    const total = step.branches.reduce((n, b) => n + b.weight, 0);
    if (total !== 100 || step.branches.some((b) => !Number.isInteger(b.weight) || b.weight < 1 || b.weight > 99)) out.push({ kind: "weights", total });
  }
  return out;
}

export function problems(root: DraftStep, known: (model: string) => boolean): Map<string, Problem[]> {
  const out = new Map<string, Problem[]>();
  let count = 0;
  for (const { step, depth } of walk(root)) {
    count++;
    const found = stepProblems(step, known);
    if (count > MAX_STEPS) found.push({ kind: "tooMany" });
    if (depth > MAX_DEPTH) found.push({ kind: "tooDeep" });
    if (found.length) out.set(step.id, found);
  }
  return out;
}

export type Move = "earlier" | "later";

export function moves(root: DraftStep, id: string): Move[] {
  const step = find(root, id);
  if (step?.type !== "model") return [];
  const out: Move[] = [];
  if (parentEdge(root, id)?.kind === "fallback") out.push("earlier");
  if (step.fallback?.type === "model") out.push("later");
  return out;
}

function swapWithFallback(root: DraftStep, id: string): DraftStep {
  return map(root, id, (outer) => {
    if (outer.type !== "model" || outer.fallback?.type !== "model") return outer;
    const { fallback: inner, ...first } = outer;
    const { fallback: rest, ...second } = inner;
    return { ...second, fallback: { ...first, fallback: rest } };
  });
}

export function move(root: DraftStep, id: string, to: Move): DraftStep {
  if (to === "later") return swapWithFallback(root, id);
  const edge = parentEdge(root, id);
  return edge?.kind === "fallback" ? swapWithFallback(root, edge.of) : root;
}

export function swapBranches(root: DraftStep, id: string): DraftStep {
  return map(root, id, (s) => (s.type === "condition" ? { ...s, then: s.else, else: s.then } : s));
}

export type Template = "single" | "chain" | "header" | "long" | "ab";
export const TEMPLATES: readonly Template[] = ["single", "chain", "header", "long", "ab"];

export function template(kind: Template, models: readonly string[]): DraftStep {
  const pick = (i: number) => emptyModel(models[i] ?? null);
  const branch = (step: DraftStep): DraftBranch => ({ id: freshId("b"), weight: 50, step });
  const condition = (rule: RouteRule, then: DraftStep, otherwise: DraftStep): DraftCondition => ({
    id: freshId(),
    type: "condition",
    match: "all",
    rules: [{ ...rule, id: freshId("r") }],
    then,
    else: otherwise,
  });
  switch (kind) {
    case "single":
      return pick(0);
    case "chain":
      return { ...pick(0), fallback: { ...pick(1), fallback: pick(2) } };
    case "header":
      return condition({ field: "header", name: "x-plan", op: "equals", value: "enterprise" }, { ...pick(0), fallback: pick(1) }, pick(2));
    case "long":
      return condition({ field: "input_tokens", op: "at_least", value: 100_000 }, pick(0), pick(1));
    case "ab":
      return { id: freshId(), type: "split", by: "caller", header: "", branches: [branch(pick(0)), branch(pick(1))] };
  }
}

export type Trace = { ids: string[]; tries: ReadonlyMap<string, number>; steps: { step: DraftStep; at: TraceStep }[] };

export function traced(root: DraftStep, steps: readonly TraceStep[]): Trace {
  const byPath = new Map([...walk(root)].map(({ step, path }) => [path, step]));
  const found = steps.flatMap((at) => {
    const step = byPath.get(at.path);
    return step ? [{ step, at }] : [];
  });
  return {
    ids: found.map((f) => f.step.id),
    tries: new Map(found.flatMap(({ step, at }) => (at.type === "model" ? [[step.id, at.attempt] as const] : []))),
    steps: found,
  };
}
