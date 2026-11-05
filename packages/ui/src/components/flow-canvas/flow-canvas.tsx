import {
  forwardRef,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent,
  type ReactNode,
} from "react";
import { cn } from "../../utils/cn.js";
import { observeResize } from "../../utils/observe-resize.js";
import { useUiText } from "../../i18n/context.js";
import { icons } from "../../icons/vocabulary.js";
import { IconButton } from "../icon-button/icon-button.js";
import { Menu } from "../menu/menu.js";
import { Status } from "../status/status.js";
import { layoutFlow, type FlowCanvasEdge, type FlowCanvasNode, type FlowLayout, type Placed, type Wire } from "./layout.js";

const WIRE_TONE: Record<NonNullable<FlowCanvasEdge["tone"]>, string> = {
  success: "var(--k-success-text)",
  warning: "var(--k-warning-text)",
  danger: "var(--k-danger-text)",
};

const NODE_RING = { danger: "ring-invalid", warning: "ring-warning", success: "ring-success", neutral: undefined } as const;

export type { FlowCanvasEdge, FlowCanvasNode, FlowCanvasStatus } from "./layout.js";

export const KAESO_FLOW_CANVAS_VARIANTS = {
  background: {
    dots: { classes: "flow-canvas-dots", description: "A dotted grid that moves and scales with the view (default)" },
    plain: { classes: "", description: "The panel alone, for a canvas inside a framed preview" },
  },
} as const;
export const KAESO_FLOW_CANVAS_DEFAULT_VARIANTS = { background: "dots" } as const;

export interface FlowCanvasSlot {
  edge: string;
  insert: boolean;
}

export interface FlowCanvasProps {
  root: FlowCanvasNode;
  "aria-label": string;
  selected?: string | null | undefined;
  onSelect?: ((id: string | null) => void) | undefined;
  addMenu?: ((slot: FlowCanvasSlot) => ReactNode) | undefined;
  nodeActions?: ((id: string) => ReactNode) | undefined;
  onDelete?: ((id: string) => void) | undefined;
  trace?: readonly string[] | undefined;
  busy?: boolean | undefined;
  background?: keyof typeof KAESO_FLOW_CANVAS_VARIANTS.background | undefined;
  className?: string | undefined;
}

interface View {
  x: number;
  y: number;
  k: number;
}

const LEVELS = [0.25, 0.33, 0.5, 0.67, 0.8, 1, 1.2, 1.5] as const;
const MIN_K = 0.25;
const MAX_K = 1.5;
const FIT_MIN_K = 0.67;
const PAD = 48;
const RAIL = 64;
const MARGIN = 24;
const DOT = 16;
const DRAG = 4;

const clampK = (k: number) => Math.min(MAX_K, Math.max(MIN_K, k));
const stepped = (k: number, dir: 1 | -1): number =>
  dir > 0 ? (LEVELS.find((l) => l > k + 0.001) ?? MAX_K) : ([...LEVELS].reverse().find((l) => l < k - 0.001) ?? MIN_K);
const round = (n: number) => Math.round(n * 100) / 100;

function fitView(bounds: FlowLayout["bounds"], width: number, height: number): View {
  const rail = width > RAIL * 4 ? RAIL : 0;
  const room = width - rail;
  const w = bounds.maxX - bounds.minX + PAD * 2;
  const h = bounds.maxY - bounds.minY + PAD * 2;
  const whole = room > 0 && height > 0 ? Math.min(1, room / w, height / h) : 1;
  const k = clampK(Math.max(FIT_MIN_K, whole));
  // A route wider than the view shows its start, at the top centre, as far as the content reaches.
  const x =
    k * w <= room
      ? rail + room / 2 - (k * (bounds.minX + bounds.maxX)) / 2
      : Math.min(rail + PAD * k - k * bounds.minX, Math.max(rail + room - PAD * k - k * bounds.maxX, rail + room / 2));
  // Whole pixels, so a step's edges stay crisp at 100%.
  return { k, x: Math.round(x), y: Math.round(PAD * k - bounds.minY * k) };
}

function prune(node: FlowCanvasNode): FlowCanvasNode {
  const children = node.children?.filter((e) => e.node).map((e) => ({ ...e, node: prune(e.node!) }));
  return children ? { ...node, children } : node;
}

function rich(template: string, values: Record<string, ReactNode>): ReactNode[] {
  return template.split(/(\{\w+\})/).map((part, i) => {
    const name = /^\{(\w+)\}$/.exec(part)?.[1];
    return name && name in values ? <span key={i}>{values[name]}</span> : part;
  });
}

const plainName = (node: FlowCanvasNode | undefined): string | undefined =>
  node ? (node.label ?? (typeof node.title === "string" ? node.title : typeof node.kind === "string" ? node.kind : undefined)) : undefined;

export const FlowCanvas = forwardRef<HTMLDivElement, FlowCanvasProps>(function FlowCanvas(
  { root, selected = null, onSelect, addMenu, nodeActions, onDelete, trace, busy, background = "dots", className, "aria-label": ariaLabel },
  ref,
) {
  const t = useUiText();
  const hintId = `k-flow-hint-${useId().replace(/[^\w-]/g, "")}`;
  const editing = !!addMenu;
  const tree = useMemo(() => (editing ? root : prune(root)), [root, editing]);
  const layout = useMemo(() => layoutFlow(tree), [tree]);
  const traced = useMemo(() => (trace ? new Set(trace) : null), [trace]);
  const viewport = useRef<HTMLDivElement | null>(null);
  const els = useRef(new Map<string, HTMLElement>());
  const [size, setSize] = useState({ w: 0, h: 0 });
  const sizeRef = useRef(size);
  sizeRef.current = size;
  const [view, setView] = useState<View>({ x: 0, y: PAD, k: 1 });
  const fitted = useRef(false);
  const [focusKey, setFocusKey] = useState<string | null>(null);
  const [actionsOpen, setActionsOpen] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const pointers = useRef(new Map<number, { x: number; y: number }>());
  const gesture = useRef<{ x: number; y: number; view: View; moved: boolean; pinch: { d: number; k: number } | null } | null>(null);

  useLayoutEffect(() => {
    const el = viewport.current;
    if (!el) return;
    const measure = () => setSize({ w: el.clientWidth, h: el.clientHeight });
    measure();
    return observeResize([el], measure);
  }, []);

  useLayoutEffect(() => {
    if (fitted.current || size.w === 0 || size.h === 0) return;
    fitted.current = true;
    setView(fitView(layout.bounds, size.w, size.h));
  }, [layout, size]);

  const zoomAt = useCallback((factor: number, cx: number, cy: number) => {
    setView((v) => {
      const k = clampK(v.k * factor);
      const ratio = k / v.k;
      return { k, x: round(cx - (cx - v.x) * ratio), y: round(cy - (cy - v.y) * ratio) };
    });
  }, []);
  const zoomStep = useCallback(
    (dir: 1 | -1) =>
      setView((v) => {
        const k = stepped(v.k, dir);
        const cx = sizeRef.current.w / 2;
        const cy = sizeRef.current.h / 2;
        return { k, x: round(cx - ((cx - v.x) * k) / v.k), y: round(cy - ((cy - v.y) * k) / v.k) };
      }),
    [],
  );
  const fit = useCallback(() => setView(fitView(layout.bounds, sizeRef.current.w, sizeRef.current.h)), [layout]);

  const reveal = useCallback(
    (key: string) => {
      const p = layout.items.get(key);
      if (!p) return;
      setView((v) => {
        const { w, h } = sizeRef.current;
        if (w === 0 || h === 0) return v;
        const left = v.x + (p.x - p.w / 2) * v.k;
        const right = v.x + (p.x + p.w / 2) * v.k;
        const top = v.y + p.y * v.k;
        const bottom = v.y + (p.y + p.h) * v.k;
        let dx = 0;
        let dy = 0;
        if (left < MARGIN) dx = MARGIN - left;
        else if (right > w - MARGIN) dx = Math.max(w - MARGIN - right, MARGIN - left);
        if (top < MARGIN) dy = MARGIN - top;
        else if (bottom > h - MARGIN) dy = Math.max(h - MARGIN - bottom, MARGIN - top);
        return dx === 0 && dy === 0 ? v : { ...v, x: round(v.x + dx), y: round(v.y + dy) };
      });
    },
    [layout],
  );

  const revealLatest = useRef(reveal);
  revealLatest.current = reveal;
  useEffect(() => {
    if (selected) revealLatest.current(selected);
  }, [selected]);
  const traceEnd = trace?.[trace.length - 1];
  useEffect(() => {
    if (traceEnd) revealLatest.current(traceEnd);
  }, [traceEnd]);

  useEffect(() => {
    const el = viewport.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const unit = e.deltaMode === 1 ? DOT : e.deltaMode === 2 ? el.clientHeight : 1;
      if (e.ctrlKey || e.metaKey) {
        const box = el.getBoundingClientRect();
        zoomAt(Math.exp(-e.deltaY * unit * 0.01), e.clientX - box.left, e.clientY - box.top);
      } else {
        setView((v) => ({ ...v, x: round(v.x - e.deltaX * unit), y: round(v.y - e.deltaY * unit) }));
      }
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [zoomAt]);

  const startGesture = () => {
    const list = [...pointers.current.values()];
    const [a, b] = list;
    if (!a) {
      gesture.current = null;
      return;
    }
    const pinch = b ? { d: Math.hypot(b.x - a.x, b.y - a.y) || 1, k: view.k } : null;
    const x = b ? (a.x + b.x) / 2 : a.x;
    const y = b ? (a.y + b.y) / 2 : a.y;
    gesture.current = { x, y, view, moved: gesture.current?.moved ?? false, pinch };
  };

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    // React bubbles events from portalled menus up the component tree; only the canvas itself pans.
    if (!e.currentTarget.contains(e.target as Node)) return;
    if ((e.target as Element).closest("[data-flow-interactive]")) return;
    if (e.pointerType === "mouse" && e.button !== 0) return;
    e.currentTarget.setPointerCapture?.(e.pointerId);
    pointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY });
    startGesture();
  };

  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!pointers.current.has(e.pointerId)) return;
    pointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY });
    const g = gesture.current;
    if (!g) return;
    const list = [...pointers.current.values()];
    const [a, b] = list;
    if (g.pinch && a && b) {
      const box = e.currentTarget.getBoundingClientRect();
      const k = clampK(g.pinch.k * (Math.hypot(b.x - a.x, b.y - a.y) / g.pinch.d));
      const cx = (a.x + b.x) / 2 - box.left;
      const cy = (a.y + b.y) / 2 - box.top;
      const ratio = k / g.view.k;
      const ox = g.x - box.left;
      const oy = g.y - box.top;
      g.moved = true;
      setView({ k, x: round(cx - (ox - g.view.x) * ratio), y: round(cy - (oy - g.view.y) * ratio) });
      return;
    }
    if (!a) return;
    const dx = a.x - g.x;
    const dy = a.y - g.y;
    if (!g.moved && Math.abs(dx) + Math.abs(dy) < DRAG) return;
    if (!g.moved) setDragging(true);
    g.moved = true;
    setView({ ...g.view, x: round(g.view.x + dx), y: round(g.view.y + dy) });
  };

  const onPointerEnd = (e: PointerEvent<HTMLDivElement>) => {
    if (!pointers.current.delete(e.pointerId)) return;
    const moved = gesture.current?.moved ?? false;
    if (pointers.current.size > 0) {
      startGesture();
      return;
    }
    gesture.current = null;
    setDragging(false);
    if (!moved && e.type === "pointerup") onSelect?.(null);
  };

  const current = focusKey && layout.items.has(focusKey) ? focusKey : selected && layout.items.has(selected) ? selected : tree.id;

  const move = (key: string | null | undefined) => {
    if (!key) return;
    setFocusKey(key);
    els.current.get(key)?.focus({ preventScroll: true });
    reveal(key);
  };

  const onItemKey = (e: KeyboardEvent<HTMLElement>, p: Placed) => {
    const siblings = p.parent ? layout.items.get(p.parent)!.children : [p.key];
    const at = siblings.indexOf(p.key);
    let target: string | null | undefined;
    switch (e.key) {
      case "ArrowDown":
        target = p.children[0];
        break;
      case "ArrowUp":
        target = p.parent;
        break;
      case "ArrowLeft":
        target = siblings[at - 1];
        break;
      case "ArrowRight":
        target = siblings[at + 1];
        break;
      case "Home":
        target = tree.id;
        break;
      case "End":
        target = layout.order[layout.order.length - 1];
        break;
      case "Escape":
        if (selected === null) return;
        e.preventDefault();
        onSelect?.(null);
        return;
      case "+":
      case "=":
        e.preventDefault();
        zoomStep(1);
        return;
      case "-":
        e.preventDefault();
        zoomStep(-1);
        return;
      case "0":
        e.preventDefault();
        fit();
        return;
      default:
        if (p.kind !== "node") return;
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect?.(p.key);
        } else if ((e.key === "Delete" || e.key === "Backspace") && onDelete) {
          e.preventDefault();
          onDelete(p.key);
        } else if ((e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey)) && nodeActions?.(p.key)) {
          e.preventDefault();
          setActionsOpen(p.key);
        }
        return;
    }
    e.preventDefault();
    move(target);
  };

  const register = (key: string) => (el: HTMLElement | null) => {
    if (el) els.current.set(key, el);
    else els.current.delete(key);
  };

  const nameOf = (key: string): ReactNode => {
    const p = layout.items.get(key);
    if (p?.kind === "slot") return p.edge?.addLabel ?? t("flowCanvas.add", "Add a step");
    return p?.node?.title;
  };

  const positioned = (p: Placed, extra?: CSSProperties): CSSProperties => ({ left: p.x - p.w / 2, top: p.y, width: p.w, height: p.h, ...extra });
  const touches = (from: string, to: string) => selected !== null && (from === selected || to === selected);
  const onTrace = (from: string, to: string) => !traced || (traced.has(from) && traced.has(to));
  const toneOf = (w: Wire) => (w.edge?.tone && onTrace(w.from, w.to) ? w.edge.tone : undefined);
  const stroke = (w: Wire) => {
    const tone = toneOf(w);
    if (tone) return WIRE_TONE[tone];
    if (traced) return onTrace(w.from, w.to) ? "var(--k-text)" : "var(--k-line)";
    return touches(w.from, w.to) ? "var(--k-text-secondary)" : "var(--k-text-tertiary)";
  };
  const strong = (w: Wire) => !!traced && onTrace(w.from, w.to);
  const wires = [...layout.wires].sort((a, b) => Number(strong(a)) + Number(!!toneOf(a)) - (Number(strong(b)) + Number(!!toneOf(b))));
  const dots = { "--k-flow-dot-gap": `${round(DOT * view.k)}px`, "--k-flow-dot-x": `${view.x}px`, "--k-flow-dot-y": `${view.y}px` } as CSSProperties;
  const layer = { transform: `translate(${view.x}px, ${view.y}px) scale(${view.k})` };
  const connection = t("flowCanvas.connection", "{from} to {to}");
  const labelled = t("flowCanvas.connectionLabelled", "{from} to {to}, {label}");

  return (
    <div
      ref={(el) => {
        viewport.current = el;
        if (typeof ref === "function") ref(el);
        else if (ref) ref.current = el;
      }}
      data-flow-canvas=""
      {...(dragging ? { "data-dragging": "" } : {})}
      className={cn(
        "relative h-full min-h-0 w-full touch-none overflow-hidden bg-panel select-none cursor-grab dragging:cursor-grabbing",
        KAESO_FLOW_CANVAS_VARIANTS.background[background].classes,
        className,
      )}
      style={dots}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerEnd}
      onPointerCancel={onPointerEnd}
    >
      <div aria-hidden className="pointer-events-none absolute top-0 left-0 origin-top-left" style={layer}>
        <svg className="absolute top-0 left-0 overflow-visible" width={1} height={1}>
          {wires.map((w) => {
            const tone = toneOf(w);
            const flowing = !!busy && strong(w) && !tone;
            return (
              <path
                key={w.key}
                data-flow-wire={w.key}
                {...(tone ? { "data-tone": tone } : {})}
                {...(flowing ? { "data-flowing": "" } : {})}
                d={w.d}
                fill="none"
                stroke={stroke(w)}
                strokeWidth={strong(w) ? 2 : 1.5}
                strokeLinecap="round"
                strokeLinejoin="round"
                className={flowing ? "animate-flow-wire" : undefined}
                {...(w.variant === "fallback" || flowing ? { strokeDasharray: "4 5" } : {})}
              />
            );
          })}
        </svg>
        {layout.wires.map(
          (w) =>
            w.labelAt && (
              <span
                key={`label:${w.key}`}
                data-flow-label={w.to}
                className={cn(
                  "absolute flex h-20 -translate-x-1/2 -translate-y-1/2 items-center rounded-pill bg-panel px-8 text-caption font-medium whitespace-nowrap text-fg-secondary shadow-ring",
                  !onTrace(w.from, w.to) && "text-fg-tertiary",
                )}
                style={{ left: w.labelAt.x, top: w.labelAt.y }}
              >
                {w.label}
              </span>
            ),
        )}
      </div>
      {addMenu && (
        <div className="pointer-events-none absolute top-0 left-0 origin-top-left" style={layer}>
          {layout.wires.map(
            (w) =>
              w.insertAt &&
              w.edge && (
                <span
                  key={`insert:${w.key}`}
                  data-flow-interactive=""
                  className="pointer-events-auto absolute -translate-x-1/2 -translate-y-1/2"
                  style={{ left: w.insertAt.x, top: w.insertAt.y }}
                >
                  <Menu>
                    <Menu.Trigger>
                      <IconButton
                        icon={icons.add}
                        variant="secondary"
                        size="xs"
                        tabIndex={-1}
                        aria-label={w.edge.insertLabel ?? t("flowCanvas.insert", "Insert a step here")}
                      />
                    </Menu.Trigger>
                    <Menu.Content align="center">{addMenu({ edge: w.edge.id, insert: true })}</Menu.Content>
                  </Menu>
                </span>
              ),
          )}
        </div>
      )}
      <div
        role="tree"
        aria-label={ariaLabel}
        aria-describedby={hintId}
        aria-busy={busy || undefined}
        className="absolute top-0 left-0 origin-top-left"
        style={layer}
      >
        {layout.order.map((key) => {
          const p = layout.items.get(key)!;
          const level = p.level + 1;
          const siblings = p.parent ? layout.items.get(p.parent)!.children : [key];
          const common = {
            role: "treeitem",
            "aria-level": level,
            "aria-setsize": siblings.length,
            "aria-posinset": siblings.indexOf(key) + 1,
            tabIndex: current === key ? 0 : -1,
            "data-flow-item": key,
            "data-flow-interactive": "",
            onFocus: () => setFocusKey(key),
            onKeyDown: (e: KeyboardEvent<HTMLElement>) => onItemKey(e, p),
          } as const;
          if (p.kind === "slot") {
            const edge = p.edge!;
            return (
              <span key={key} className="absolute flex items-center justify-center" style={positioned(p)}>
                <Menu>
                  <Menu.Trigger>
                    <IconButton
                      ref={register(key)}
                      {...common}
                      aria-selected={false}
                      icon={icons.add}
                      variant="secondary"
                      size="xs"
                      aria-label={edge.addLabel ?? t("flowCanvas.add", "Add a step")}
                    />
                  </Menu.Trigger>
                  <Menu.Content align="start">{addMenu!({ edge: edge.id, insert: false })}</Menu.Content>
                </Menu>
              </span>
            );
          }
          const node = p.node!;
          const off = !!traced && !traced.has(key);
          const actions = nodeActions?.(key);
          const name = plainName(node) ?? "";
          return (
            <div
              key={key}
              ref={register(key)}
              {...common}
              aria-selected={selected === key}
              {...(actions ? { "aria-haspopup": "menu" as const } : {})}
              {...(node["data-force"] ? { "data-force": node["data-force"] } : {})}
              {...(traced ? { "data-trace": traced.has(key) ? "on" : "off" } : {})}
              onClick={() => onSelect?.(key)}
              className={cn(
                "absolute flex cursor-pointer flex-col justify-center gap-2 rounded-well bg-panel px-12 py-10 text-left shadow-control transition-control outline-none focus-ring",
                "hover:shadow-control-hover selected:bg-item-hover selected:shadow-control-selected selected:hover:shadow-control-selected forced-colors:outline-1 forced-colors:outline-solid",
                node.status && NODE_RING[node.status.tone],
                off && "shadow-ring",
                actions && "pr-40",
              )}
              style={positioned(p)}
            >
              <span className="flex h-16 min-w-0 items-center justify-between gap-8 text-caption font-medium text-fg-tertiary">
                <span className="truncate">{node.kind}</span>
                {node.status ? (
                  <Status tone={node.status.tone} size="sm" className="shrink-0 text-caption">
                    {node.status.label}
                  </Status>
                ) : (
                  node.badge !== undefined && node.badge !== null && <span className="shrink-0 text-fg-secondary">{node.badge}</span>
                )}
              </span>
              <span className="flex h-20 min-w-0 items-center gap-8">
                {node.leading && <span className="flex shrink-0">{node.leading}</span>}
                <span className={cn("truncate text-body font-medium", off ? "text-fg-secondary" : "text-fg")}>{node.title}</span>
              </span>
              {node.subtitle !== undefined && node.subtitle !== null && node.subtitle !== "" && (
                <span className={cn("h-18 truncate text-small", off ? "text-fg-tertiary" : "text-fg-secondary")}>{node.subtitle}</span>
              )}
              {actions && (
                <span
                  aria-hidden
                  data-flow-interactive=""
                  className="absolute top-1/2 right-8 -translate-y-1/2"
                  onClick={(e) => e.stopPropagation()}
                  onFocus={(e) => {
                    e.stopPropagation();
                    els.current.get(key)?.focus({ preventScroll: true });
                  }}
                >
                  <Menu open={actionsOpen === key} onOpenChange={(o) => setActionsOpen(o ? key : null)}>
                    <Menu.Trigger>
                      <IconButton icon={icons.more} size="xs" tabIndex={-1} aria-label={t("flowCanvas.actionsFor", "Actions for {name}", { name })} />
                    </Menu.Trigger>
                    <Menu.Content align="start">{actions}</Menu.Content>
                  </Menu>
                </span>
              )}
            </div>
          );
        })}
      </div>
      <div data-flow-interactive="" className="absolute bottom-24 left-24 flex flex-col gap-8">
        <IconButton
          icon={icons.add}
          variant="secondary"
          tooltipSide="right"
          aria-label={t("flowCanvas.zoomIn", "Zoom in")}
          disabled={view.k >= MAX_K}
          onClick={() => zoomStep(1)}
        />
        <IconButton
          icon={icons.minus}
          variant="secondary"
          tooltipSide="right"
          aria-label={t("flowCanvas.zoomOut", "Zoom out")}
          disabled={view.k <= MIN_K}
          onClick={() => zoomStep(-1)}
        />
        <IconButton icon={icons.expand} variant="secondary" tooltipSide="right" aria-label={t("flowCanvas.fit", "Fit to view")} onClick={fit} />
      </div>
      <p id={hintId} className="sr-only">
        {t("flowCanvas.keys", "Arrow keys move between steps, Enter selects one, plus and minus zoom, and 0 fits the view.")}
      </p>
      <ul className="sr-only">
        {layout.wires.map((w) => (
          <li key={w.key}>{rich(w.label ? labelled : connection, { from: nameOf(w.from), to: nameOf(w.to), label: w.label })}</li>
        ))}
      </ul>
    </div>
  );
});
FlowCanvas.displayName = "FlowCanvas";
