import { describe, expect, it } from "vitest";
import { scaleLinear as d3Linear } from "d3-scale";
import { scaleLinear } from "./linear-scale.js";

const domains: [number, number][] = [
  [0, 1],
  [0, 1.08],
  [0, 7],
  [0, 13.5],
  [0, 97.2],
  [0, 1234],
  [0, 0.0042],
  [0, 86_400_000 * 30],
  [1_760_000_000_000, 1_762_592_000_000],
  [-3, 17],
  [5, 5],
  [12, 0],
  [0, 1e-12],
];
const ranges: [number, number][] = [
  [180, 0],
  [0, 640],
  [24, 24],
];

describe("the charts' linear scale", () => {
  it.each(domains)("nices, ticks and maps [%d, %d] exactly as d3 does", (lo, hi) => {
    for (const range of ranges) {
      for (const count of [undefined, 2, 4, 10]) {
        const ours = scaleLinear().domain([lo, hi]).nice(count).range(range);
        const theirs = d3Linear().domain([lo, hi]).nice(count).range(range);
        expect(ours.domain()).toEqual(theirs.domain());
        expect(ours.range()).toEqual(theirs.range());
        expect(ours.ticks(count)).toEqual(theirs.ticks(count));
        for (const v of [lo, hi, (lo + hi) / 2, lo - 1, hi * 1.5]) expect(ours(v)).toBe(theirs(v));
        for (const y of [range[0], range[1], (range[0] + range[1]) / 3]) expect(ours.invert(y)).toBe(theirs.invert(y));
      }
    }
  });

  it("maps dates by their time, as the time axis passes them", () => {
    const from = new Date("2026-10-01T00:00:00Z");
    const to = new Date("2026-10-31T00:00:00Z");
    const x = scaleLinear().domain([from.getTime(), to.getTime()]).range([0, 600]);
    expect(x(new Date("2026-10-16T00:00:00Z"))).toBe(300);
    expect(x.invert(150)).toBe(Date.parse("2026-10-08T12:00:00Z"));
  });

  it("copies keep their own domain while the original changes", () => {
    const axis = scaleLinear().domain([0, 100]).range([200, 0]);
    const copy = axis.copy().domain([0, 50]);
    expect(axis(50)).toBe(100);
    expect(copy(50)).toBe(0);
    expect(axis.domain()).toEqual([0, 100]);
  });

  it("leaves a domain it cannot make nice as it was, as d3 does", () => {
    for (const domain of [
      [Number.NaN, 4],
      [0, Number.POSITIVE_INFINITY],
    ] as [number, number][]) {
      expect(scaleLinear().domain(domain).nice(4).domain()).toEqual(d3Linear().domain(domain).nice(4).domain());
    }
  });

  it("starts at [0, 1] on both sides, as d3 does", () => {
    expect(scaleLinear().domain()).toEqual([0, 1]);
    expect(scaleLinear().range()).toEqual([0, 1]);
    expect(scaleLinear()(0.25)).toBe(d3Linear()(0.25));
  });
});
