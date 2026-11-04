import { describe, expect, it } from "vitest";
import type { RouteStep } from "~/api/generated/types";
import {
  add,
  defaultRemoval,
  edgeId,
  evenWeights,
  find,
  fromRoute,
  modelsOf,
  move,
  moves,
  parentEdge,
  parseEdge,
  problems,
  removals,
  remove,
  ruleProblem,
  sameRoute,
  stepAt,
  swapBranches,
  template,
  TEMPLATES,
  toRoute,
  traced,
  walk,
  type DraftModel,
  type DraftStep,
} from "./draft";

const model = (id: string, fallback: RouteStep | null = null): RouteStep => ({
  type: "model",
  model: id,
  fallback,
  fall_back_on: ["unavailable", "limited", "unsupported"],
  timeout_seconds: null,
});

const branchy: RouteStep = {
  type: "condition",
  match: "all",
  rules: [{ field: "header", name: "x-plan", op: "equals", value: "paid" }],
  then: model("m1", model("m2")),
  else: {
    type: "split",
    by: "caller",
    branches: [
      { weight: 90, step: model("m3") },
      { weight: 10, step: model("m4") },
    ],
  },
};

const ids = (root: DraftStep) => [...walk(root)].map(({ step }) => (step.type === "model" ? step.model : step.type));
const known = (m: string) => m.startsWith("m");
const stepWhere = (root: DraftStep, test: (s: DraftStep) => boolean): DraftStep => {
  const found = [...walk(root)].find(({ step }) => test(step));
  if (!found) throw new Error("no such step");
  return found.step;
};
const modelStep = (root: DraftStep, id: string) => stepWhere(root, (s) => s.type === "model" && s.model === id);

function idAt(root: DraftStep, path: string): string {
  const found = [...walk(root)].find((w) => w.path === path);
  if (!found) throw new Error(`no step at ${path}`);
  return found.step.id;
}

describe("route drafts", () => {
  it("read a route and write the same route back", () => {
    const draft = fromRoute(branchy);
    expect(sameRoute(toRoute(draft), branchy)).toBe(true);
    expect(ids(draft)).toEqual(["condition", "m1", "m2", "split", "m3", "m4"]);
    expect(modelsOf(draft)).toEqual(["m1", "m2", "m3", "m4"]);
  });

  it("send no value for a header rule that only checks presence", () => {
    const draft = fromRoute({ ...branchy, rules: [{ field: "header", name: " x-beta ", op: "present", value: "left over" }] } as RouteStep);
    const out = toRoute(draft);
    expect(out.type === "condition" && out.rules[0]).toEqual({ field: "header", name: "x-beta", op: "present", value: null });
  });

  it("add a fallback in an open slot and insert a step before another", () => {
    const draft = fromRoute(model("m1"));
    const withFallback = add(draft, { kind: "fallback", of: draft.id }, "model", false);
    expect(ids(withFallback.root)).toEqual(["m1", null]);
    expect(find(withFallback.root, withFallback.added)).toMatchObject({ type: "model", model: null });

    const inserted = add(withFallback.root, { kind: "root" }, "condition", true);
    const out = toRoute(inserted.root);
    expect(out.type).toBe("condition");
    expect(out.type === "condition" && out.then.type === "model" && out.then.model).toBe("m1");

    const split = add(inserted.root, { kind: "root" }, "split", true);
    const top = find(split.root, split.added);
    expect(top?.type === "split" && top.branches.map((b) => b.weight)).toEqual([50, 50]);
  });

  it("remove a model by closing up its fallback chain, and offer what a condition or split can keep", () => {
    const draft = fromRoute(model("m1", model("m2", model("m3"))));
    const middle = modelStep(draft, "m2");
    expect(removals(draft, middle.id)).toEqual([{ kind: "step" }]);
    expect(ids(remove(draft, middle.id, { kind: "step" }))).toEqual(["m1", "m3"]);
    expect(removals(draft, draft.id)).toEqual([{ kind: "step" }]);
    expect(ids(remove(draft, draft.id, { kind: "step" }))).toEqual(["m2", "m3"]);

    const tree = fromRoute(branchy);
    expect(removals(tree, tree.id)).toEqual([
      { kind: "keep", edge: "then" },
      { kind: "keep", edge: "else" },
    ]);
    const splitStep = stepWhere(tree, (st) => st.type === "split");
    expect(removals(tree, splitStep.id).map((r) => r.kind)).toEqual(["keepBranch", "keepBranch"]);
    expect(ids(remove(tree, tree.id, { kind: "keep", edge: "else" }))).toEqual(["split", "m3", "m4"]);
    const lone = modelStep(tree, "m4");
    expect(removals(tree, lone.id)).toEqual([]);
  });

  it("remove a split's branch and give its weight to the first", () => {
    const draft = fromRoute({
      type: "split",
      by: "request",
      branches: [
        { weight: 50, step: model("m1") },
        { weight: 30, step: model("m2") },
        { weight: 20, step: model("m3") },
      ],
    });
    const third = modelStep(draft, "m3");
    expect(removals(draft, third.id)).toContainEqual({ kind: "branch" });
    const out = toRoute(remove(draft, third.id, { kind: "branch" }));
    expect(out.type === "split" && out.branches.map((b) => b.weight)).toEqual([70, 30]);
  });

  it("find what needs a fix before saving", () => {
    const draft = fromRoute({
      type: "split",
      by: "request",
      branches: [
        { weight: 60, step: model("gone") },
        {
          weight: 30,
          step: {
            type: "condition",
            match: "all",
            rules: [{ field: "header", name: "Authorization", op: "contains", value: "" }],
            then: model("m1"),
            else: model("m2"),
          },
        },
      ],
    });
    const found = [...problems(draft, known).values()].flat().map((p) => p.kind);
    expect(found).toEqual(["weights", "removedModel", "headerName"]);
    const added = add(fromRoute(model("m1")), { kind: "root" }, "model", true);
    expect([...problems(added.root, known).values()].flat()).toEqual([{ kind: "noModel" }]);
  });

  it("place a refused field on its step", () => {
    const draft = fromRoute(branchy);
    const at = stepAt(draft, "route.else.branches.1.step.model");
    expect(at?.step.type === "model" && at.step.model).toBe("m4");
    expect(at?.field).toBe("model");
    expect(stepAt(draft, "route.rules.0.value")).toMatchObject({ step: { type: "condition" }, field: "rules.0.value" });
  });

  it("read edges and share weights evenly", () => {
    expect(parseEdge("root")).toEqual({ kind: "root" });
    expect(parseEdge("branch:s1:2")).toEqual({ kind: "branch", of: "s1", index: 2 });
    expect(parseEdge("else:s9")).toEqual({ kind: "else", of: "s9" });
    expect(parseEdge("loop:s1")).toEqual({ kind: "root" });
    expect(parseEdge("then")).toEqual({ kind: "then", of: "" });
    expect(evenWeights(3)).toEqual([34, 33, 33]);
    expect(evenWeights(8).reduce((a, b) => a + b)).toBe(100);
  });

  it("fill in what an older route leaves out", () => {
    const draft = fromRoute({ type: "model", model: "m1" } as RouteStep);
    expect(toRoute(draft)).toEqual({
      type: "model",
      model: "m1",
      fallback: null,
      fall_back_on: ["unavailable", "limited", "unsupported"],
      timeout_seconds: null,
    });
    const header = fromRoute({ ...branchy, rules: [{ field: "header", name: "x-plan", op: "equals" }] } as RouteStep);
    const out = toRoute(header);
    expect(out.type === "condition" && out.rules[0]).toEqual({ field: "header", name: "x-plan", op: "equals", value: "" });
  });

  it("add on every kind of edge and leave a route alone for an edge that doesn't fit", () => {
    const draft = fromRoute(branchy);
    const condition = draft.id;
    const split = stepWhere(draft, (st) => st.type === "split").id;
    const m1 = modelStep(draft, "m1").id;
    expect(ids(add(draft, { kind: "then", of: condition }, "model", true).root)).toEqual(["condition", null, "m1", "m2", "split", "m3", "m4"]);
    expect(ids(add(draft, { kind: "else", of: condition }, "condition", false).root).slice(0, 5)).toEqual(["condition", "m1", "m2", "condition", null]);
    expect(ids(add(draft, { kind: "branch", of: split, index: 1 }, "split", true).root)).toEqual(["condition", "m1", "m2", "split", "m3", "split", "m4", null]);
    expect(ids(add(draft, { kind: "fallback", of: m1 }, "split", false).root)).toEqual(["condition", "m1", "split", null, null, "split", "m3", "m4"]);
    const presence = fromRoute({ ...branchy, rules: [{ field: "header", name: "x-beta", op: "present", value: "stale" }] } as RouteStep);
    expect(presence.type === "condition" && presence.rules[0]).toMatchObject({ value: null });
    for (const wrong of [
      { kind: "fallback", of: condition },
      { kind: "then", of: m1 },
      { kind: "branch", of: m1, index: 0 },
      { kind: "branch", of: split, index: 9 },
      { kind: "fallback", of: "gone" },
    ] as const) {
      const r = add(draft, wrong, "model", true);
      expect(sameRoute(toRoute(r.root), toRoute(draft))).toBe(true);
    }
  });

  it("know which edge leads to a step", () => {
    const draft = fromRoute(branchy);
    expect(parentEdge(draft, draft.id)).toEqual({ kind: "root" });
    expect(parentEdge(draft, modelStep(draft, "m2").id)).toMatchObject({ kind: "fallback" });
    expect(parentEdge(draft, stepWhere(draft, (st) => st.type === "split").id)).toMatchObject({ kind: "else" });
    expect(parentEdge(draft, modelStep(draft, "m4").id)).toMatchObject({ kind: "branch", index: 1 });
    expect(parentEdge(draft, "gone")).toBeUndefined();
    expect([edgeId({ kind: "root" }), edgeId({ kind: "then", of: "a" }), edgeId({ kind: "branch", of: "b", index: 2 })]).toEqual([
      "root",
      "then:a",
      "branch:b:2",
    ]);
  });

  it("remove nothing it can't, and keep a split's branch", () => {
    const draft = fromRoute(branchy);
    const split = stepWhere(draft, (st) => st.type === "split");
    expect(removals(draft, "gone")).toEqual([]);
    expect(remove(draft, "gone", { kind: "step" })).toBe(draft);
    expect(remove(draft, split.id, { kind: "keep", edge: "then" })).toBe(draft);
    expect(remove(draft, split.id, { kind: "step" })).toBe(draft);
    const second = split.type === "split" ? split.branches[1] : undefined;
    expect(second && ids(remove(draft, split.id, { kind: "keepBranch", branch: second }))).toEqual(["condition", "m1", "m2", "m4"]);
    const lone = fromRoute(model("m1"));
    expect(remove(lone, lone.id, { kind: "step" })).toEqual(lone);
    expect(defaultRemoval(draft, draft.id)).toEqual({ kind: "keep", edge: "then" });
    expect(stepAt(draft, "route")).toMatchObject({ step: { id: draft.id }, field: "" });
  });

  it("check every kind of rule, timeouts, fallbacks and the size of a route", () => {
    const rule = (r: Parameters<typeof ruleProblem>[0]) => ruleProblem(r)?.kind;
    expect(rule({ field: "header", name: "x-plan", op: "absent" })).toBeUndefined();
    expect(rule({ field: "header", name: "x-plan", op: "equals", value: "a".repeat(201) })).toBe("headerValue");
    expect(rule({ field: "header", name: "x-plan", op: "equals" })).toBe("headerValue");
    expect(rule({ field: "format", op: "in", value: [] })).toBe("noFormats");
    expect(rule({ field: "format", op: "in", value: ["openai_chat"] })).toBeUndefined();
    expect(rule({ field: "feature", op: "used", value: "images" })).toBeUndefined();
    expect(rule({ field: "input_tokens", op: "below", value: 1.5 })).toBe("tokens");
    expect(rule({ field: "input_tokens", op: "below", value: 10_000_001 })).toBe("tokens");
    expect(rule({ field: "input_tokens", op: "below", value: 5 })).toBeUndefined();
    expect(rule({ field: "member", op: "in", value: Array.from({ length: 101 }, (_, i) => `usr_${i}`) })).toBe("noIds");
    expect(rule({ field: "key", op: "in", value: ["key_web"] })).toBeUndefined();

    const slow = fromRoute({ type: "model", model: "m1", fallback: model("m2"), timeout_seconds: 9000, fall_back_on: [] });
    expect([...problems(slow, known).values()].flat().map((p) => p.kind)).toEqual(["timeout", "fallBackOn"]);
    let deep: RouteStep = model("m1");
    for (let i = 0; i < 40; i++) deep = model("m1", deep);
    const kinds = [...problems(fromRoute(deep), known).values()].flat().map((p) => p.kind);
    expect(kinds).toContain("tooDeep");
    expect(kinds).toContain("tooMany");
  });

  it("split by a header: send the header, refuse a missing or secret one, and leave it out otherwise", () => {
    const split = (by: "header" | "caller", header?: string): RouteStep => ({
      type: "split",
      by,
      ...(header === undefined ? {} : { header }),
      branches: [
        { weight: 50, step: model("m1") },
        { weight: 50, step: model("m2") },
      ],
    });
    const draft = fromRoute(split("header", "x-user-id"));
    expect(toRoute(draft)).toEqual(split("header", "x-user-id"));
    expect(toRoute(fromRoute(split("caller")))).toEqual(split("caller"));
    expect(toRoute({ ...draft, by: "caller" } as DraftStep)).toEqual(split("caller"));
    const known = () => true;
    const blank = fromRoute(split("header", ""));
    expect(problems(blank, known).get(blank.id)).toEqual([{ kind: "headerName" }]);
    const secret = fromRoute(split("header", "cookie"));
    expect(problems(secret, known).get(secret.id)).toEqual([{ kind: "secretHeader" }]);
  });

  it("move a model earlier or later in its fallback chain, taking its settings along", () => {
    const middle: RouteStep = { type: "model", model: "m2", fallback: model("m3"), fall_back_on: ["unavailable"], timeout_seconds: 30 };
    const root = fromRoute(model("m1", middle));
    const [first, second, third] = [...walk(root)].map((w) => w.step.id);
    expect(moves(root, String(first))).toEqual(["later"]);
    expect(moves(root, String(second))).toEqual(["earlier", "later"]);
    expect(moves(root, String(third))).toEqual(["earlier"]);
    const earlier = move(root, String(second), "earlier");
    expect(modelsOf(earlier)).toEqual(["m2", "m1", "m3"]);
    expect(earlier.id).toBe(second);
    expect((earlier as DraftModel).timeout_seconds).toBe(30);
    expect(modelsOf(move(root, String(second), "later"))).toEqual(["m1", "m3", "m2"]);
    expect(move(root, String(first), "earlier")).toBe(root);
    const branched = fromRoute(branchy);
    expect(moves(branched, branched.id)).toEqual([]);
    expect(moves(branched, idAt(branched, "route.then"))).toEqual(["later"]);
  });

  it("swap a condition's Then and Else", () => {
    const root = fromRoute(branchy);
    const swapped = toRoute(swapBranches(root, root.id));
    expect(swapped).toMatchObject({ type: "condition", then: { type: "split" }, else: { type: "model", model: "m1" } });
    expect(swapBranches(root, "missing")).toEqual(root);
    const then = idAt(root, "route.then");
    expect(swapBranches(root, then)).toEqual(root);
    expect(move(root, root.id, "later")).toEqual(root);
  });

  it("start from a template with the catalogue's models, or with models still to choose", () => {
    const shapes = Object.fromEntries(TEMPLATES.map((kind) => [kind, toRoute(template(kind, ["a", "b", "c"]))]));
    expect(shapes.single).toMatchObject({ type: "model", model: "a", fallback: null });
    expect(shapes.chain).toMatchObject({ model: "a", fallback: { model: "b", fallback: { model: "c" } } });
    expect(shapes.header).toMatchObject({
      type: "condition",
      rules: [{ field: "header", name: "x-plan", op: "equals", value: "enterprise" }],
      then: { model: "a", fallback: { model: "b" } },
      else: { model: "c" },
    });
    expect(shapes.long).toMatchObject({ rules: [{ field: "input_tokens", op: "at_least", value: 100000 }], then: { model: "a" }, else: { model: "b" } });
    expect(shapes.ab).toMatchObject({ type: "split", by: "caller", branches: [{ weight: 50 }, { weight: 50 }] });
    const empty = template("chain", []);
    expect(problems(empty, () => true).size).toBe(3);
  });

  it("place a traced request's steps on the draft and number its tries", () => {
    const root = fromRoute(branchy);
    const at = (path: string) => idAt(root, path);
    const trace = traced(root, [
      { type: "condition", path: "route", holds: true, rules: [true] },
      { type: "model", path: "route.then", catalogue_model_id: "m1", attempt: 1 },
      { type: "model", path: "route.then.fallback", catalogue_model_id: "m2", attempt: 2 },
      { type: "model", path: "route.gone", catalogue_model_id: "m9", attempt: 3 },
    ]);
    expect(trace.ids).toEqual([root.id, at("route.then"), at("route.then.fallback")]);
    expect([...trace.tries]).toEqual([
      [at("route.then"), 1],
      [at("route.then.fallback"), 2],
    ]);
    expect(trace.steps.map((s) => s.at.type)).toEqual(["condition", "model", "model"]);
  });
});
