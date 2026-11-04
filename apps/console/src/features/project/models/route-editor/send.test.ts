import { describe, expect, it } from "vitest";
import { fromRoute } from "./draft";
import { parseRoute, passed, pathIds, toneOf } from "./send";
import { modelRoute } from "~/test/fixtures/project";

describe("parseRoute", () => {
  it("reads every step of the header, with its model, status and time", () => {
    expect(
      parseRoute(
        '"route";skipped, "route.fallback";model="anthropic/claude-sonnet-4-5";status=529;dur=1204, "route.fallback.fallback";model="custom/a \\"quoted\\" \\\\ model";status=200',
      ),
    ).toEqual([
      { path: "route", model: null, status: null, ms: null },
      { path: "route.fallback", model: "anthropic/claude-sonnet-4-5", status: 529, ms: 1204 },
      { path: "route.fallback.fallback", model: 'custom/a "quoted" \\ model', status: 200, ms: null },
    ]);
  });

  it("is empty without the header and stops at what it can't read", () => {
    expect(parseRoute(null)).toEqual([]);
    expect(parseRoute("")).toEqual([]);
    const ok = { path: "route", model: null, status: 200, ms: null };
    expect(parseRoute('"route";note="kept";;status=200;weight=?1, nonsense')).toEqual([ok]);
    expect(parseRoute('"route";status=200, "route.then')).toEqual([ok]);
    expect(parseRoute('"route";status=200, "route.then\\')).toEqual([ok]);
  });
});

describe("passed", () => {
  const root = fromRoute({
    type: "condition",
    match: "all",
    rules: [{ field: "header", name: "x-plan", op: "equals", value: "paid" }],
    then: {
      type: "split",
      by: "request",
      header: null,
      branches: [
        { weight: 30, step: modelRoute("cm_sonnet") },
        { weight: 70, step: modelRoute("cm_haiku", modelRoute("cm_sonnet")) },
      ],
    },
    else: modelRoute("cm_haiku"),
  });

  it("lists each decision once, before the models it leads to", () => {
    const steps = passed(root, [
      { path: "route.then.branches.1.step", model: "anthropic/claude-haiku-4-5", status: 503, ms: 20 },
      { path: "route.then.branches.1.step.fallback", model: "anthropic/claude-sonnet-4-5", status: 200, ms: null },
      { path: "route.gone", model: null, status: null, ms: null },
    ]);
    expect(steps.map((p) => [p.kind, p.path, p.kind === "decision" ? p.taken : p.last])).toEqual([
      ["decision", "route", "then.branches.1.step"],
      ["decision", "route.then", "branches.1.step"],
      ["tried", "route.then.branches.1.step", false],
      ["tried", "route.then.branches.1.step.fallback", false],
    ]);
    expect(pathIds(root, [{ path: "route.else", model: null, status: 200, ms: null }])).toHaveLength(2);
  });

  it("tones a status by its class", () => {
    expect([toneOf(200), toneOf(429), toneOf(529), toneOf(0)]).toEqual(["success", "warning", "danger", "danger"]);
  });
});
