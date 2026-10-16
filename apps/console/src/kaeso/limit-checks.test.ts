import { describe, expect, it } from "vitest";
import type { LimitView } from "~/api/generated/types";
import { parentConflict } from "./limit-checks";
import { newLimit } from "./limit-editor";

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
