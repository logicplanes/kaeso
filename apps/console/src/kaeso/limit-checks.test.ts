import { describe, expect, it } from "vitest";
import type { LimitView } from "~/api/generated/types";
import { parentConflict } from "./limit-checks";
import { limitBody, limitProblem, newLimit } from "./limit-editor";

const limit = (p: Partial<LimitView>): LimitView =>
  ({
    id: "lim_1",
    kind: "spend_budget",
    period: "month",
    amount: "50",
    model: null,
    hard: true,
    alert_thresholds: [],
    level: "project",
    description: "monthly spend budget of 50 USD",
    subject: { type: "project", id: "proj_1" },
    created_at: "",
    updated_at: "",
    ...p,
  }) as LimitView;

describe("parentConflict", () => {
  it("finds the limit above that a higher amount would exceed", () => {
    expect(parentConflict({ ...newLimit(), amount: "80" }, [limit({})])?.id).toBe("lim_1");
    expect(parentConflict({ ...newLimit(), amount: "40" }, [limit({})])).toBeNull();
  });
  it("compares only the same kind, period and model", () => {
    const draft = { ...newLimit(), amount: "80" };
    expect(parentConflict({ ...draft, period: "day" }, [limit({})])).toBeNull();
    expect(parentConflict({ ...draft, kind: "token_budget" }, [limit({})])).toBeNull();
    expect(parentConflict({ ...draft, model: "cm_a" }, [limit({ model: "cm_b" })])).toBeNull();
    expect(parentConflict({ ...draft, model: "cm_a" }, [limit({ model: null })])?.id).toBe("lim_1");
  });
  it("treats rates as per minute", () => {
    expect(
      parentConflict({ ...newLimit(), kind: "request_rate", period: "month", amount: "100" }, [limit({ kind: "request_rate", period: "minute", amount: "60" })])
        ?.id,
    ).toBe("lim_1");
  });
  it("finds nothing for an amount that isn't a positive number", () => {
    for (const amount of ["", "0", "-5", "lots"]) expect(parentConflict({ ...newLimit(), amount }, [limit({ amount: "1" })])).toBeNull();
  });
  it("names the lowest of several limits above", () => {
    const parents = [limit({ id: "lim_high", amount: "60" }), limit({ id: "lim_low", amount: "20" }), limit({ id: "lim_mid", amount: "40" })];
    expect(parentConflict({ ...newLimit(), amount: "80" }, parents)?.id).toBe("lim_low");
  });
});

describe("limitProblem", () => {
  it("accepts up to 10 alert thresholds, as the gateway does", () => {
    const ten = Array.from({ length: 10 }, (_, i) => (i + 1) * 10);
    expect(limitProblem({ ...newLimit(), amount: "5", alert_thresholds: ten })).toBeNull();
    expect(limitProblem({ ...newLimit(), amount: "5", alert_thresholds: [5, ...ten] })).toBe("Set up to 10 alert thresholds.");
  });

  it("accepts a spend budget of 1,000,000,000 USD and refuses a nano-dollar more", () => {
    expect(limitProblem({ ...newLimit(), amount: "1000000000" })).toBeNull();
    expect(limitProblem({ ...newLimit(), amount: "1000000000.000000000" })).toBeNull();
    expect(limitProblem({ ...newLimit(), amount: "999999999.999999999" })).toBeNull();
    for (const amount of ["1000000000.000000001", "1000000001", "1e10", "00000001000000000.5"]) {
      expect(limitProblem({ ...newLimit(), amount })).toBe("Enter an amount up to 1,000,000,000 USD.");
    }
  });

  it("lets tokens and requests go beyond the spend cap", () => {
    for (const kind of ["token_budget", "request_rate", "token_rate"] as const) {
      expect(limitProblem({ ...newLimit(), kind, period: kind === "token_budget" ? "month" : "minute", amount: "1000000000000" })).toBeNull();
    }
  });

  it("ignores thresholds on a rate, which has none", () => {
    expect(limitProblem({ ...newLimit(), kind: "request_rate", period: "minute", amount: "60", alert_thresholds: [50] })).toBeNull();
    expect(limitBody({ ...newLimit(), kind: "request_rate", period: "minute", amount: "60", alert_thresholds: [50] }).alert_thresholds).toEqual([]);
  });
});
