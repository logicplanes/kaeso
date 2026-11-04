import { useState, type ReactNode } from "react";
import { Menu, ProviderBadge, icons } from "@logicplanes/kaeso-ui";
import { FlowCanvas, type FlowCanvasNode } from "@logicplanes/kaeso-ui/flow-canvas";
import { providerNames } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "FlowCanvas",
  description: "An editable route drawn as a tree: conditions and splits branch downward, and each model's fallback continues the row to its right.",
  group: "Components",
  entry: "flow-canvas",
  import: "FlowCanvas",
  barrel: false,
  keyboard: [
    ["Tab", "Reaches the route as one stop, on the selected step or the start"],
    ["↑ ↓", "Moves to the step's parent or its first branch"],
    ["← →", "Moves between branches of one step"],
    ["Home, End", "Moves to the start or the last step"],
    ["Enter, Space", "Selects the step, or opens an open slot's menu"],
    ["Escape", "Clears the selection"],
    ["Delete", "Asks to remove the step"],
    ["Shift F10", "Opens the step's actions"],
    ["+ − 0", "Zooms in, out, and fits the route to the view"],
  ],
  a11y: [
    "The route is a named tree: each step is a tree item with its level and position among its branches, and the tree's description lists the keys.",
    "Every connection is also written out for screen readers, such as “gpt-5 to claude-sonnet-4-5, Fallback” or “x-plan is paid to gpt-5, Then”.",
    "A traced request is told in words too: each model on the path carries its try as a badge, and the steps off the path stay in the tree.",
    "A step's status is a word, never a colour alone; a removed model also draws a red ring around its step.",
    "The + between steps is for the pointer; from the keyboard, a step's actions (Shift F10) carry the same choices.",
  ],
};

export const examples = [
  {
    id: "editor",
    title: "Edit a route",
    description: "Select a step, add a fallback in the open slot beside a model, or insert a condition, a split or a model with the + between steps.",
    fn: "Editor",
    minHeight: 560,
  },
  {
    id: "fallbacks",
    title: "A fallback chain",
    description: "Each fallback continues the row to its right, in the order the models are tried.",
    fn: "Fallbacks",
    minHeight: 400,
  },
  {
    id: "trace",
    title: "A traced request",
    description: "The path one request takes stays strong and each model on it says which try it is; the rest fades.",
    fn: "Traced",
    minHeight: 560,
  },
  { id: "branches", title: "A condition with a split", fn: "Branches", minHeight: 560 },
  { id: "removed", title: "A removed model", description: "A step whose model is gone says so in words, with a red ring.", fn: "Removed", minHeight: 420 },
  {
    id: "large",
    title: "A large route",
    description: "Larger than its view, it fits itself once; drag, scroll or zoom to move around.",
    fn: "Large",
    minHeight: 560,
  },
];

type Step =
  | { id: string; type: "model"; provider: string; model: string; fallback?: Step | undefined; removed?: boolean | undefined }
  | { id: string; type: "condition"; rule: string; matched: Step; otherwise: Step }
  | { id: string; type: "split"; branches: { weight: number; step: Step }[] };

let next = 100;
type ModelStep = Extract<Step, { type: "model" }>;
const fresh = (provider = "anthropic", model = "claude-haiku-4-5"): ModelStep => ({ id: `s${next++}`, type: "model", provider, model });

function toNode(step: Step): FlowCanvasNode {
  if (step.type === "model")
    return {
      id: step.id,
      kind: "Model",
      title: step.model,
      subtitle: providerNames[step.provider],
      leading: <ProviderBadge provider={step.provider} name={providerNames[step.provider]} size="sm" />,
      terminal: true,
      ...(step.removed ? { status: { tone: "danger" as const, label: "Model removed" } } : {}),
      children: [
        {
          id: `fallback:${step.id}`,
          label: "Fallback",
          variant: "fallback",
          addLabel: `Add a fallback to ${step.model}`,
          ...(step.fallback ? { node: toNode(step.fallback), insertLabel: `Insert a step before ${step.fallback.id}` } : {}),
        },
      ],
    };
  if (step.type === "condition")
    return {
      id: step.id,
      kind: "Condition",
      title: step.rule,
      subtitle: "1 rule",
      children: [
        { id: `then:${step.id}`, label: "Then", node: toNode(step.matched) },
        { id: `else:${step.id}`, label: "Else", node: toNode(step.otherwise) },
      ],
    };
  return {
    id: step.id,
    kind: "Split",
    title: "By caller",
    subtitle: `${step.branches.length} branches`,
    children: step.branches.map((b, i) => ({ id: `branch:${step.id}:${i}`, label: `${b.weight}%`, node: toNode(b.step) })),
  };
}

const startNode = (alias: string, step: Step): FlowCanvasNode => ({
  id: "start",
  kind: "Start",
  title: alias,
  children: [{ id: "root", node: toNode(step) }],
});

const basic: Step = {
  id: "s1",
  type: "condition",
  rule: "x-plan is paid",
  matched: { id: "s2", type: "model", provider: "anthropic", model: "claude-opus-4-1" },
  otherwise: { id: "s3", type: "model", provider: "openai", model: "gpt-5-mini" },
};

function edit(step: Step, edge: string, insert: boolean, kind: "model" | "condition" | "split"): Step {
  const wrap = (s: Step): Step =>
    kind === "model"
      ? { ...fresh("openai", "gpt-5"), fallback: s }
      : kind === "condition"
        ? { id: `s${next++}`, type: "condition", rule: "Uses images", matched: s, otherwise: fresh() }
        : {
            id: `s${next++}`,
            type: "split",
            branches: [
              { weight: 50, step: s },
              { weight: 50, step: fresh() },
            ],
          };
  const visit = (s: Step): Step => {
    if (s.type === "model") {
      if (edge === `fallback:${s.id}`)
        return insert && s.fallback ? { ...s, fallback: wrap(s.fallback) } : { ...s, fallback: kind === "model" ? fresh() : wrap(fresh()) };
      return s.fallback ? { ...s, fallback: visit(s.fallback) } : s;
    }
    if (s.type === "condition") {
      if (edge === `then:${s.id}`) return { ...s, matched: wrap(s.matched) };
      if (edge === `else:${s.id}`) return { ...s, otherwise: wrap(s.otherwise) };
      return { ...s, matched: visit(s.matched), otherwise: visit(s.otherwise) };
    }
    return { ...s, branches: s.branches.map((b, i) => (edge === `branch:${s.id}:${i}` ? { ...b, step: wrap(b.step) } : { ...b, step: visit(b.step) })) };
  };
  return edge === "root" ? wrap(step) : visit(step);
}

function remove(step: Step, id: string): Step {
  if (step.type === "model") {
    if (step.fallback?.id === id) return { ...step, fallback: step.fallback.type === "model" ? step.fallback.fallback : undefined };
    return step.fallback ? { ...step, fallback: remove(step.fallback, id) } : step;
  }
  if (step.type === "condition") return { ...step, matched: remove(step.matched, id), otherwise: remove(step.otherwise, id) };
  return { ...step, branches: step.branches.map((b) => ({ ...b, step: remove(b.step, id) })) };
}

function Frame({ children, height = "h-480" }: { children: ReactNode; height?: "h-400" | "h-480" }) {
  return <div className={`${height} w-full overflow-hidden rounded-frame shadow-frame`}>{children}</div>;
}

export function Editor() {
  const [route, setRoute] = useState<Step>(basic);
  const [selected, setSelected] = useState<string | null>("s2");
  const add = (edge: string, insert: boolean) => (kind: "model" | "condition" | "split") => setRoute((r) => edit(r, edge, insert, kind));
  return (
    <Frame>
      <FlowCanvas
        aria-label="Route of fast"
        root={startNode("fast", route)}
        selected={selected}
        onSelect={setSelected}
        onDelete={(id) => setRoute((r) => remove(r, id))}
        addMenu={({ edge, insert }) => (
          <>
            <Menu.Item icon={icons.model} onClick={() => add(edge, insert)("model")}>
              Model
            </Menu.Item>
            <Menu.Item icon={icons.condition} onClick={() => add(edge, insert)("condition")}>
              Condition
            </Menu.Item>
            <Menu.Item icon={icons.split} onClick={() => add(edge, insert)("split")}>
              Split
            </Menu.Item>
          </>
        )}
        nodeActions={(id) =>
          id === "start" ? null : (
            <Menu.Item icon={icons.delete} variant="danger" onClick={() => setRoute((r) => remove(r, id))}>
              Remove step
            </Menu.Item>
          )
        }
      />
    </Frame>
  );
}

export function Fallbacks() {
  const chain: Step = {
    id: "f1",
    type: "model",
    provider: "openai",
    model: "gpt-5",
    fallback: {
      id: "f2",
      type: "model",
      provider: "anthropic",
      model: "claude-sonnet-4-5",
      fallback: { id: "f3", type: "model", provider: "groq", model: "llama-3.3-70b-versatile" },
    },
  };
  return (
    <Frame height="h-400">
      <FlowCanvas aria-label="Route of chat-default" root={startNode("chat-default", chain)} />
    </Frame>
  );
}

const tries = (node: FlowCanvasNode, order: Record<string, number>): FlowCanvasNode => ({
  ...node,
  ...(order[node.id] ? { badge: `Try ${order[node.id]}` } : {}),
  ...(node.children ? { children: node.children.map((e) => (e.node ? { ...e, node: tries(e.node, order) } : e)) } : {}),
});

export function Traced() {
  const route: Step = {
    id: "t1",
    type: "condition",
    rule: "x-plan is enterprise",
    matched: {
      id: "t2",
      type: "model",
      provider: "anthropic",
      model: "claude-sonnet-4-5",
      fallback: { id: "t3", type: "model", provider: "openai", model: "gpt-5" },
    },
    otherwise: { id: "t4", type: "model", provider: "openai", model: "gpt-5-mini" },
  };
  return (
    <Frame>
      <FlowCanvas
        aria-label="Route of support"
        root={tries(startNode("support", route), { t2: 1, t3: 2 })}
        trace={["start", "t1", "t2", "t3"]}
        background="plain"
      />
    </Frame>
  );
}

export function Branches() {
  const route: Step = {
    id: "b1",
    type: "condition",
    rule: "Uses images",
    matched: {
      id: "b2",
      type: "model",
      provider: "anthropic",
      model: "claude-sonnet-4-5",
      fallback: { id: "b3", type: "model", provider: "openai", model: "gpt-5" },
    },
    otherwise: {
      id: "b4",
      type: "split",
      branches: [
        { weight: 90, step: { id: "b5", type: "model", provider: "openai", model: "gpt-5-mini" } },
        { weight: 10, step: { id: "b6", type: "model", provider: "deepseek", model: "deepseek-chat" } },
      ],
    },
  };
  return (
    <Frame>
      <FlowCanvas aria-label="Route of support" root={startNode("support", route)} selected="b4" />
    </Frame>
  );
}

export function Removed() {
  const route: Step = {
    id: "r1",
    type: "model",
    provider: "openai",
    model: "gpt-4o",
    removed: true,
    fallback: { id: "r2", type: "model", provider: "openai", model: "gpt-5-mini" },
  };
  return (
    <Frame height="h-400">
      <FlowCanvas aria-label="Route of legacy" root={startNode("legacy", route)} selected="r1" />
    </Frame>
  );
}

export function Large() {
  const chain = (id: string, models: [string, string][]): Step => {
    const [first, ...rest] = models;
    return { id, type: "model", provider: first![0], model: first![1], ...(rest.length ? { fallback: chain(`${id}x`, rest) } : {}) };
  };
  const route: Step = {
    id: "l1",
    type: "condition",
    rule: "Input over 100,000 tokens",
    matched: chain("l2", [
      ["anthropic", "claude-sonnet-4-5"],
      ["openai", "gpt-5"],
    ]),
    otherwise: {
      id: "l3",
      type: "split",
      branches: [
        {
          weight: 70,
          step: chain("l4", [
            ["openai", "gpt-5-mini"],
            ["anthropic", "claude-haiku-4-5"],
            ["groq", "llama-3.3-70b-versatile"],
          ]),
        },
        {
          weight: 20,
          step: {
            id: "l5",
            type: "condition",
            rule: "Uses tools",
            matched: chain("l6", [["deepseek", "deepseek-chat"]]),
            otherwise: chain("l7", [
              ["mistral", "mistral-large-latest"],
              ["cerebras", "gpt-oss-120b"],
            ]),
          },
        },
        { weight: 10, step: chain("l8", [["moonshotai", "kimi-k2"]]) },
      ],
    },
  };
  return (
    <Frame>
      <FlowCanvas aria-label="Route of everything" root={startNode("everything", route)} />
    </Frame>
  );
}

const forced = (id: string, force: string | undefined, extra?: Partial<FlowCanvasNode>): FlowCanvasNode => ({
  id: "start",
  kind: "Start",
  title: `${id}`,
  children: [
    {
      id: "root",
      node: {
        id,
        kind: "Model",
        title: "claude-sonnet-4-5",
        subtitle: "Anthropic",
        leading: <ProviderBadge provider="anthropic" name="Anthropic" size="sm" />,
        terminal: true,
        ...(force ? { "data-force": force } : {}),
        ...(id === "removed" ? { status: { tone: "danger" as const, label: "Model removed" } } : {}),
        ...(id === "warning" ? { status: { tone: "warning" as const, label: "Unchecked" } } : {}),
        children: [{ id: "fb", label: "Fallback", variant: "fallback", addLabel: "Add a fallback" }],
        ...extra,
      },
    },
  ],
});

export function States() {
  const menu = () => <Menu.Item onClick={() => undefined}>Model</Menu.Item>;
  return (
    <>
      {(
        [
          ["Rest", "rest", undefined],
          ["Hover", "hover", "hover"],
          ["Focus", "focus", "focus"],
          ["Selected", "selected", "selected"],
          ["Model removed", "removed", undefined],
          ["Needs a check", "warning", undefined],
        ] as const
      ).map(([title, id, force]) => (
        <StateSection key={id} title={title}>
          <Frame height="h-400">
            <FlowCanvas aria-label={`${title} step`} root={forced(id, force)} addMenu={menu} />
          </Frame>
        </StateSection>
      ))}
      <StateSection title="Traced: the path stays strong, each try has a badge, the rest fades">
        <Frame height="h-400">
          <FlowCanvas aria-label="Traced step" root={forced("traced", undefined, { badge: "Try 1" })} trace={["start", "traced"]} />
        </Frame>
      </StateSection>
      <StateSection title="Off a traced path">
        <Frame height="h-400">
          <FlowCanvas aria-label="Step off the path" root={forced("off-path", undefined)} trace={["start"]} />
        </Frame>
      </StateSection>
      <StateSection title="Read only: no open slots and no + between steps">
        <Frame height="h-400">
          <FlowCanvas aria-label="Read-only step" root={forced("read-only", undefined)} background="plain" />
        </Frame>
      </StateSection>
    </>
  );
}
