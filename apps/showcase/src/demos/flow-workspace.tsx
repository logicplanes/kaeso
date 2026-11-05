import { useState } from "react";
import { Button, Checkbox, CheckboxGroup, Field, IconButton, Input, PageHeader, ProviderBadge, Select, icons } from "@logicplanes/kaeso-ui";
import { FlowCanvas, FlowWorkspace, type FlowCanvasNode } from "@logicplanes/kaeso-ui/flow-canvas";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "FlowWorkspace",
  description: "The body of a route editor: the canvas fills the panel under the page header, with an inspector on the right that edits what is selected.",
  group: "Layout",
  entry: "flow-canvas",
  import: "FlowWorkspace",
  barrel: false,
  a11y: [
    "The inspector is a complementary region named by `panelLabel`; each panel inside it is a region named by its heading.",
    "From 1024px the inspector scrolls on its own beside the canvas; below, it follows a 400px canvas and the page scrolls.",
  ],
};

export const examples = [
  {
    id: "editor",
    title: "A route editor",
    description: "Select a step on the canvas to edit it in the inspector; with nothing selected, the inspector shows the alias.",
    fn: "Editor",
    minHeight: 640,
  },
];

const models = [
  { value: "claude-sonnet-4-5", label: "claude-sonnet-4-5", provider: "anthropic", name: "Anthropic" },
  { value: "gpt-5", label: "gpt-5", provider: "openai", name: "OpenAI" },
];

function route(primary: string): FlowCanvasNode {
  const first = models.find((m) => m.value === primary)!;
  const second = models.find((m) => m.value !== primary)!;
  const node = (m: typeof first, id: string, fallback?: FlowCanvasNode): FlowCanvasNode => ({
    id,
    kind: "Model",
    title: m.label,
    subtitle: m.name,
    leading: <ProviderBadge provider={m.provider} name={m.name} size="sm" />,
    terminal: true,
    children: [{ id: `${id}-fallback`, label: "Fallback", variant: "fallback", ...(fallback ? { node: fallback } : {}) }],
  });
  return { id: "start", kind: "Start", title: "fast", children: [{ id: "root", node: node(first, "primary", node(second, "fallback")) }] };
}

function Page({
  selected,
  onSelect,
  primary,
  setPrimary,
}: {
  selected: string | null;
  onSelect: (id: string | null) => void;
  primary: string;
  setPrimary: (m: string) => void;
}) {
  const model = selected === "primary" ? primary : models.find((m) => m.value !== primary)!.value;
  return (
    <div className="flex h-640 w-full flex-col overflow-hidden rounded-frame bg-panel shadow-frame">
      <PageHeader
        title="fast"
        start={<IconButton icon={icons.chevronLeft} aria-label="Back to aliases" />}
        actions={
          <Button variant="primary" size="sm">
            Save alias
          </Button>
        }
      />
      <FlowWorkspace
        panelLabel="Step settings"
        panel={
          selected && selected !== "start" ? (
            <FlowWorkspace.Panel
              title="Model step"
              description={selected === "primary" ? "Requests go to this model first." : "Requests come here when the model above fails."}
              actions={<IconButton icon={icons.delete} aria-label="Remove step" />}
            >
              <Select
                label="Model"
                value={model}
                onValueChange={(v) => selected === "primary" && setPrimary(v)}
                options={models.map((m) => ({ value: m.value, label: m.label }))}
              />
              <Field label="Fall back when">
                <CheckboxGroup aria-label="Fall back when" defaultValue={["unavailable", "limited", "unsupported"]}>
                  <Checkbox value="unavailable" label="The provider is unavailable" />
                  <Checkbox value="limited" label="A limit refuses the request" />
                  <Checkbox value="unsupported" label="The model can't take the request" />
                  <Checkbox value="rejected" label="The provider rejects the request" />
                </CheckboxGroup>
              </Field>
              <Input label="Time to first token" placeholder="The model's own" type="number" />
            </FlowWorkspace.Panel>
          ) : (
            <FlowWorkspace.Panel title="Alias" description="Apps send this name as model.">
              <Input label="Name" defaultValue="fast" />
            </FlowWorkspace.Panel>
          )
        }
      >
        <FlowCanvas aria-label="Route of fast" root={route(primary)} selected={selected} onSelect={onSelect} />
      </FlowWorkspace>
    </div>
  );
}

export function Editor() {
  const [selected, setSelected] = useState<string | null>("primary");
  const [primary, setPrimary] = useState("claude-sonnet-4-5");
  return <Page selected={selected} onSelect={setSelected} primary={primary} setPrimary={setPrimary} />;
}

export function States() {
  return (
    <>
      <StateSection title="A step selected">
        <Page selected="fallback" onSelect={() => undefined} primary="claude-sonnet-4-5" setPrimary={() => undefined} />
      </StateSection>
      <StateSection title="Nothing selected">
        <Page selected={null} onSelect={() => undefined} primary="gpt-5" setPrimary={() => undefined} />
      </StateSection>
    </>
  );
}
