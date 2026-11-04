import type { ReactNode } from "react";

export interface FlowCanvasStatus {
  tone: "danger" | "warning" | "success" | "neutral";
  label: string;
}

export interface FlowCanvasNode {
  id: string;
  kind: ReactNode;
  title: ReactNode;
  label?: string | undefined;
  subtitle?: ReactNode | undefined;
  leading?: ReactNode | undefined;
  status?: FlowCanvasStatus | undefined;
  badge?: ReactNode | undefined;
  terminal?: boolean | undefined;
  children?: FlowCanvasEdge[] | undefined;
  "data-force"?: string | undefined;
}

export interface FlowCanvasEdge {
  id: string;
  label?: ReactNode | undefined;
  variant?: "default" | "fallback" | undefined;
  tone?: "success" | "warning" | "danger" | undefined;
  node?: FlowCanvasNode | undefined;
  addLabel?: string | undefined;
  insertLabel?: string | undefined;
}

export const NODE_W = 240;
export const NODE_H = 80;
export const SLOT = 24;
export const STUB = 32;
const V_GAP = 96;
const LEVEL = NODE_H + V_GAP;
const H_GAP = 40;
const FORK = 24;
const RADIUS = 12;

interface Box {
  l: number;
  r: number;
}

interface Rel {
  key: string;
  x: number;
  level: number;
}

interface Shape {
  items: Rel[];
  contour: Box[];
  tail: Box | null;
}

export type PlacedKind = "node" | "slot";

export interface Placed {
  key: string;
  kind: PlacedKind;
  x: number;
  y: number;
  w: number;
  h: number;
  level: number;
  node: FlowCanvasNode | undefined;
  edge: FlowCanvasEdge | undefined;
  parent: string | null;
  children: string[];
}

export interface Wire {
  key: string;
  from: string;
  to: string;
  d: string;
  variant: "default" | "fallback";
  label: ReactNode | undefined;
  labelAt: { x: number; y: number } | null;
  insertAt: { x: number; y: number } | null;
  edge: FlowCanvasEdge | null;
}

export interface FlowLayout {
  items: Map<string, Placed>;
  order: string[];
  wires: Wire[];
  bounds: { minX: number; maxX: number; minY: number; maxY: number };
}

const union = (a: Box | null | undefined, b: Box | null | undefined): Box | null => {
  if (!a) return b ?? null;
  if (!b) return a;
  return { l: Math.min(a.l, b.l), r: Math.max(a.r, b.r) };
};

const at = (s: Shape, level: number): Box | null => (level < s.contour.length ? (s.contour[level] ?? null) : s.tail);

const shift = (s: Shape, dx: number): Shape =>
  dx === 0
    ? s
    : {
        items: s.items.map((i) => ({ ...i, x: i.x + dx })),
        contour: s.contour.map((b) => ({ l: b.l + dx, r: b.r + dx })),
        tail: s.tail && { l: s.tail.l + dx, r: s.tail.r + dx },
      };

function gapBetween(left: Shape, right: Shape): number {
  const depth = Math.max(left.contour.length, right.contour.length) + 1;
  let need = -Infinity;
  for (let level = 0; level < depth; level++) {
    const a = at(left, level);
    const b = at(right, level);
    if (a && b) need = Math.max(need, a.r - b.l + H_GAP);
  }
  return need === -Infinity ? 0 : need;
}

function merge(shapes: Shape[]): Shape {
  const contour: Box[] = [];
  let tail: Box | null = null;
  const items: Rel[] = [];
  for (const s of shapes) {
    items.push(...s.items);
    tail = union(tail, s.tail);
  }
  const depth = Math.max(0, ...shapes.map((s) => s.contour.length));
  for (let level = 0; level < depth; level++) {
    let box: Box | null = null;
    for (const s of shapes) box = union(box, at(s, level));
    contour.push(box ?? { l: 0, r: 0 });
  }
  return { items, contour, tail };
}

interface Built {
  placed: Map<string, Placed>;
  order: string[];
}

function edgeKey(parent: string, edge: FlowCanvasEdge) {
  return edge.node ? edge.node.id : `${parent}::${edge.id}`;
}

function slot(built: Built, parent: string, edge: FlowCanvasEdge, depth: number): string {
  const key = edgeKey(parent, edge);
  built.order.push(key);
  built.placed.set(key, { key, kind: "slot", x: 0, y: 0, w: SLOT, h: SLOT, level: depth, node: undefined, edge, parent, children: [] });
  return key;
}

function layoutTree(node: FlowCanvasNode, built: Built, parent: string | null, edge: FlowCanvasEdge | undefined, depth: number): Shape {
  const key = node.id;
  const children = node.children ?? [];
  built.order.push(key);
  built.placed.set(key, {
    key,
    kind: "node",
    x: 0,
    y: 0,
    w: NODE_W,
    h: NODE_H,
    level: depth,
    node,
    edge,
    parent,
    children: children.map((e) => edgeKey(key, e)),
  });
  const self: Box = { l: -NODE_W / 2, r: NODE_W / 2 };
  const shapes: Shape[] = children.map((e) =>
    e.node
      ? layoutTree(e.node, built, key, e, depth + 1)
      : { items: [{ key: slot(built, key, e, depth + 1), x: 0, level: 0 }], contour: [{ l: -SLOT / 2, r: SLOT / 2 }], tail: null },
  );
  let placedKids: Shape[] = [];
  let cursor: Shape | null = null;
  for (const s of shapes) {
    const next: Shape = cursor ? shift(s, gapBetween(cursor, s)) : s;
    placedKids.push(next);
    cursor = cursor ? merge([cursor, next]) : next;
  }
  if (placedKids.length > 0) {
    const roots = placedKids.map((s) => s.items[0]!.x);
    const dx = -(roots[0]! + roots[roots.length - 1]!) / 2;
    placedKids = placedKids.map((s) => shift(s, dx));
  }
  const group = placedKids.length > 0 ? merge(placedKids) : null;
  return {
    items: [{ key, x: 0, level: 0 }, ...(group ? group.items.map((i) => ({ ...i, level: i.level + 1 })) : [])],
    contour: [self, ...(group ? group.contour : [])],
    tail: group?.tail ?? null,
  };
}

const round = (n: number) => Math.round(n * 2) / 2;

function elbow(x1: number, y1: number, x2: number, y2: number, turnY: number): string {
  if (Math.abs(x2 - x1) < 1) return `M${x1} ${y1}V${y2}`;
  const dir = x2 > x1 ? 1 : -1;
  const r = Math.min(RADIUS, Math.abs(x2 - x1) / 2, Math.abs(turnY - y1), Math.abs(y2 - turnY));
  return `M${x1} ${y1}V${turnY - r}Q${x1} ${turnY} ${x1 + dir * r} ${turnY}H${x2 - dir * r}Q${x2} ${turnY} ${x2} ${turnY + r}V${y2}`;
}

export function layoutFlow(root: FlowCanvasNode): FlowLayout {
  const built: Built = { placed: new Map(), order: [] };
  const shape = layoutTree(root, built, null, undefined, 0);
  for (const rel of shape.items) {
    const p = built.placed.get(rel.key)!;
    p.x = round(rel.x);
    p.y = p.kind === "slot" ? rel.level * LEVEL + (NODE_H - SLOT) / 2 : rel.level * LEVEL;
  }
  const items = built.placed;
  for (const key of built.order) {
    const p = items.get(key)!;
    const parent = p.parent === null ? undefined : items.get(p.parent);
    if (p.kind === "slot" && parent?.node?.terminal) p.y = parent.y + NODE_H + STUB;
  }
  const wires: Wire[] = [];
  let maxBottom = 0;
  let minX = Infinity;
  let maxX = -Infinity;
  for (const p of items.values()) {
    maxBottom = Math.max(maxBottom, p.y + p.h);
    minX = Math.min(minX, p.x - p.w / 2);
    maxX = Math.max(maxX, p.x + p.w / 2);
  }
  const labelled = (edge: FlowCanvasEdge) => edge.label !== undefined && edge.label !== null && edge.label !== "";
  for (const key of built.order) {
    const p = items.get(key)!;
    if (p.parent === null) continue;
    const parent = items.get(p.parent)!;
    const edge = p.edge!;
    const variant = edge.variant ?? "default";
    const top = p.y;
    const turnY = parent.y + NODE_H + FORK;
    const d = elbow(parent.x, parent.y + NODE_H, p.x, top, turnY);
    const firstY = Math.abs(p.x - parent.x) < 1 ? parent.y + NODE_H : turnY;
    const run = top - firstY;
    const labelAt = p.kind === "node" && labelled(edge) ? { x: p.x, y: round(firstY + Math.min(22, run / 3)) } : null;
    const insertAt = p.kind === "node" ? { x: p.x, y: round(labelAt ? top - 20 : firstY + run / 2) } : null;
    wires.push({ key: `wire:${key}`, from: parent.key, to: key, d, variant, label: edge.label, labelAt, insertAt, edge });
  }
  return { items, order: built.order, wires, bounds: { minX, maxX, minY: 0, maxY: maxBottom } };
}
