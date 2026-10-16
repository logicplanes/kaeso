import { describe, expect, it } from "vitest";
import { groupSmall, shareLabel, wholeShares } from "./donut.js";
import { assignSeries, readableTicks, seriesIndex, tickLabel, tokenKindColor, wholeTicks } from "./core.js";
import { scaleLinear } from "d3-scale";

describe("series colors", () => {
  it("keeps an entity's slot for the session", () => {
    expect(seriesIndex("model-a")).toBe(seriesIndex("model-a"));
  });
  it("is the same wherever it is computed (server and client)", () => {
    expect([...assignSeries(["a", "b", "c"])]).toEqual([...assignSeries(["a", "b", "c"])]);
  });
  it("never puts near twins in one chart", () => {
    const near: Record<number, number> = { 1: 6, 6: 1, 2: 5, 5: 2, 3: 7, 7: 3 };
    const slots = [...assignSeries(["gpt-5", "gpt-5-mini", "claude-sonnet-4-5", "claude-haiku-4-5"]).values()];
    for (const s of slots) if (near[s] !== undefined) expect(slots).not.toContain(near[s]);
    expect(new Set(slots).size).toBe(slots.length);
  });
  it("draws cache read as the soft variant of input's blue", () => {
    expect(tokenKindColor("input")).toBe("var(--k-viz-1)");
    expect(tokenKindColor("cacheRead")).toBe("var(--k-viz-1-soft)");
    expect(tokenKindColor("output")).toBe("var(--k-viz-3)");
  });
});

describe("chart helpers", () => {
  it("groups slices under 2% into Other", () => {
    const out = groupSmall([
      { id: "a", label: "A", value: 90 },
      { id: "b", label: "B", value: 8 },
      { id: "c", label: "C", value: 1 },
      { id: "d", label: "D", value: 1 },
    ]);
    expect(out.map((s) => s.id)).toEqual(["a", "b", "__other"]);
    expect(out[2]?.value).toBe(2);
  });
  it("keeps a lone small slice under its own name", () => {
    const out = groupSmall([
      { id: "a", label: "A", value: 96 },
      { id: "b", label: "B", value: 3.6 },
      { id: "c", label: "C", value: 0.4 },
    ]);
    expect(out.map((s) => s.label)).toEqual(["A", "B", "C"]);
  });
  it("reads a share that rounds to nothing as under one percent", () => {
    expect(shareLabel(0.4, 100)).toBe("<1%");
    expect(shareLabel(0, 100)).toBe("0%");
    expect(shareLabel(96, 100)).toBe("96%");
  });
  it("keeps value ticks whose labels differ", () => {
    const whole = (v: number) => String(Math.round(v));
    expect(readableTicks(scaleLinear().domain([0, 1]).nice(4), whole)).toEqual([0, 1]);
    expect(readableTicks(scaleLinear().domain([0, 400]).nice(4), whole)).toEqual([0, 100, 200, 300, 400]);
  });
  it("drops zero cents from axis labels", () => {
    const label = tickLabel((v) => `$${v.toFixed(2)}`);
    expect(label(20)).toBe("$20");
    expect(label(2.5)).toBe("$2.50");
  });
});

describe("whole ticks", () => {
  it("labels only whole ticks for whole counts", () => {
    expect(wholeTicks([0, 0.2, 0.4, 0.6, 0.8, 1], [0, 1, 0])).toEqual([0, 1]);
    expect(wholeTicks([0, 0.2, 0.4, 0.6, 0.8, 1], [])).toEqual([0, 1]);
  });
  it("keeps fractional ticks for fractional values (dollars)", () => {
    expect(wholeTicks([0, 0.02, 0.04], [0.01, 0.03])).toEqual([0, 0.02, 0.04]);
  });
});

describe("whole shares", () => {
  it("adds up to 100", () => {
    expect(wholeShares([0.0003, 0.03, 0.06, 0.0000003])).toEqual(["<1%", "33%", "67%", "<1%"]);
    expect(wholeShares([1, 1, 1])).toEqual(["34%", "33%", "33%"]);
  });
  it("is 0% for nothing", () => {
    expect(wholeShares([0, 0])).toEqual(["0%", "0%"]);
  });
});
