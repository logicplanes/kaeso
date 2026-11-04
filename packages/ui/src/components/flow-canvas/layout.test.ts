import { describe, expect, it } from "vitest";
import { CHAIN, NODE_H, NODE_W, SLOT, layoutFlow, type FlowCanvasEdge, type FlowCanvasNode, type Placed } from "./layout.js";

const model = (id: string, children: FlowCanvasEdge[] = []): FlowCanvasNode => ({ id, kind: "Model", title: id, terminal: true, children });
const fallback = (node?: FlowCanvasNode): FlowCanvasEdge => ({
  id: `fb-${node?.id ?? "open"}`,
  label: "Fallback",
  variant: "fallback",
  ...(node ? { node } : {}),
});
const start = (child: FlowCanvasNode): FlowCanvasNode => ({ id: "start", kind: "Start", title: "fast", children: [{ id: "root", node: child }] });
const condition = (id: string, then: FlowCanvasNode, otherwise: FlowCanvasNode): FlowCanvasNode => ({
  id,
  kind: "Condition",
  title: id,
  children: [
    { id: `${id}-then`, label: "Then", node: then },
    { id: `${id}-else`, label: "Else", node: otherwise },
  ],
});
const split = (id: string, branches: FlowCanvasNode[]): FlowCanvasNode => ({
  id,
  kind: "Split",
  title: id,
  children: branches.map((b, i) => ({ id: `${id}-${i}`, label: `${Math.round(100 / branches.length)}%`, node: b })),
});

const item = (l: ReturnType<typeof layoutFlow>, key: string) => l.items.get(key)!;
const left = (p: Placed) => p.x - p.w / 2;
const right = (p: Placed) => p.x + p.w / 2;

function invariants(root: FlowCanvasNode) {
  const l = layoutFlow(root);
  const all = [...l.items.values()];
  for (const a of all)
    for (const b of all) {
      if (a === b) continue;
      const apart = right(a) <= left(b) || right(b) <= left(a) || a.y + a.h <= b.y || b.y + b.h <= a.y;
      expect(apart, `${a.key} and ${b.key} overlap`).toBe(true);
    }
  for (const w of l.wires) {
    const from = l.items.get(w.from)!;
    const to = l.items.get(w.to)!;
    if (!from.node?.terminal) continue;
    const y = from.y + NODE_H / 2;
    expect(to.y + to.h / 2, `${to.key} sits on the row of ${from.key}`).toBe(y);
    for (const n of all) {
      if (n === from || n === to || n.y > y || n.y + n.h < y) continue;
      expect(right(n) <= left(from) || left(n) >= right(to), `the fallback line from ${from.key} crosses ${n.key}`).toBe(true);
    }
  }
  for (const p of all) {
    if (!p.parent) continue;
    const parent = l.items.get(p.parent)!;
    if (parent.node?.terminal) continue;
    expect(p.y, `${p.key} sits a row under ${parent.key}`).toBe(parent.y + NODE_H + 96 + (p.kind === "slot" ? (NODE_H - SLOT) / 2 : 0));
  }
  return l;
}

describe("layoutFlow", () => {
  it("puts a single model under the start and ends there", () => {
    const l = invariants(start(model("gpt-5")));
    expect(item(l, "start")).toMatchObject({ x: 0, y: 0, w: NODE_W, h: NODE_H, level: 0 });
    expect(item(l, "gpt-5")).toMatchObject({ x: 0, y: 176, level: 1, parent: "start" });
    expect(l.wires.map((w) => [w.from, w.to, w.d])).toEqual([["start", "gpt-5", "M0 80V176"]]);
    expect(l.wires[0]!.insertAt).toEqual({ x: 0, y: 128 });
    expect(l.bounds).toEqual({ minX: -120, maxX: 120, minY: 0, maxY: 256 });
  });

  it("places an open fallback slot beside its model, joined by a straight dashed line", () => {
    const l = invariants(start(model("gpt-5", [fallback()])));
    const slot = item(l, "gpt-5::fb-open");
    expect(slot).toMatchObject({ kind: "slot", x: 176, w: SLOT, level: 2 });
    expect(slot.y).toBe(176 + (NODE_H - SLOT) / 2);
    const wire = l.wires.find((w) => w.to === slot.key)!;
    expect(wire).toMatchObject({ d: "M120 216H164", variant: "fallback", labelAt: null, insertAt: null });
  });

  it("continues a fallback chain to the right on the model's row, in the order the models are tried", () => {
    const l = invariants(start(model("gpt-5", [fallback(model("claude-sonnet-4-5", [fallback(model("llama-3-3-70b"))]))])));
    const a = item(l, "gpt-5");
    const b = item(l, "claude-sonnet-4-5");
    const c = item(l, "llama-3-3-70b");
    expect([a.x, b.x, c.x]).toEqual([0, NODE_W + CHAIN, 2 * (NODE_W + CHAIN)]);
    expect([a.y, b.y, c.y]).toEqual([176, 176, 176]);
    expect([a.level, b.level, c.level]).toEqual([1, 2, 3]);
    const wire = l.wires.find((w) => w.to === b.key)!;
    expect(wire).toMatchObject({ d: "M120 216H248", variant: "fallback", labelAt: { x: 164, y: 216 }, insertAt: { x: 224, y: 216 } });
    expect(layoutFlow(start(model("gpt-5", [fallback(model("claude-sonnet-4-5"))])), { editing: false }).wires[1]!.labelAt).toEqual({ x: 184, y: 216 });
  });

  it("keeps the branches of a fallback under it, clear of the model before it", () => {
    const l = invariants(start(model("gpt-5", [fallback(condition("images", model("claude-sonnet-4-5"), split("ab", [model("a"), model("b"), model("c")])))])));
    const first = item(l, "gpt-5");
    const branching = item(l, "images");
    expect(branching.y).toBe(first.y);
    for (const key of ["claude-sonnet-4-5", "ab"]) {
      expect(item(l, key).y).toBe(first.y + NODE_H + 96);
      expect(left(item(l, key))).toBeGreaterThanOrEqual(left(first));
    }
  });

  it("centres a condition's branches under it with Then on the left", () => {
    const l = invariants(start(condition("plan", model("gpt-5"), model("gpt-5-mini"))));
    const then = item(l, "gpt-5");
    const otherwise = item(l, "gpt-5-mini");
    expect(then.x).toBe(-otherwise.x);
    expect(otherwise.x - then.x).toBe(NODE_W + 40);
    const wire = l.wires.find((w) => w.to === "gpt-5")!;
    expect(wire.d).toBe("M0 256V268Q0 280 -12 280H-128Q-140 280 -140 292V352");
    expect(wire.labelAt).toEqual({ x: -140, y: 302 });
    expect(wire.insertAt).toEqual({ x: -140, y: 332 });
  });

  it("spreads a split's branches and keeps every route clear of every step", () => {
    invariants(
      start(
        split("ab", [
          model("gpt-5", [fallback(condition("images", model("claude-sonnet-4-5"), model("gpt-5-mini", [fallback()])))]),
          model("claude-opus-4-1"),
          condition("long", model("gemini-2-5-pro", [fallback(model("gpt-4-1"))]), split("canary", [model("grok-4"), model("kimi-k2")])),
        ]),
      ),
    );
  });

  it("keeps every arrangement clear: no step overlaps another and no fallback line crosses a step", () => {
    let seed = 7;
    const random = () => {
      seed = (seed * 48271) % 2147483647;
      return seed / 2147483647;
    };
    let n = 0;
    const grow = (depth: number): FlowCanvasNode => {
      const id = `n${n++}`;
      const pick = depth > 4 ? 0 : random();
      if (pick < 0.45) return model(id, random() < 0.6 ? [fallback(depth < 5 && random() < 0.6 ? grow(depth + 1) : undefined)] : []);
      if (pick < 0.75) return condition(id, grow(depth + 1), grow(depth + 1));
      return split(
        id,
        Array.from({ length: 2 + Math.floor(random() * 3) }, () => grow(depth + 1)),
      );
    };
    for (let i = 0; i < 40; i++) invariants(start(grow(0)));
  });

  it("shows an empty route as the start with an open slot under it", () => {
    const l = layoutFlow({ id: "start", kind: "Start", title: "fast", children: [{ id: "root", addLabel: "Add the first step" }] });
    expect(item(l, "start::root")).toMatchObject({ kind: "slot", x: 0, level: 1 });
    expect(l.wires.map((w) => w.d)).toEqual(["M0 80V204"]);
  });
});
