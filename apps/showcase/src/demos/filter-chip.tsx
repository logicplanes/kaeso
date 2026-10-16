import { useState } from "react";
import { FilterChip, Button, icons } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "FilterChip",
  description: "A filter as one pill, “Status is 2xx”: property, operator and value, each opening a menu.",
  group: "Components",
  entry: "filter-chip",
  import: "FilterChip",
  keyboard: [
    ["Tab", "Moves between the parts"],
    ["Enter", "Opens the part's menu"],
  ],
  a11y: ["The chip is a group named after its property (“Filter: Status”); the remove circle says which filter it removes (“Remove the Status filter”)."],
};
export const examples = [
  { id: "basic", title: "Filters in a toolbar", fn: "Basic" },
  { id: "search", title: "A long list of values, searched", fn: "Searched" },
];
export function Basic() {
  const [status, setStatus] = useState(["2xx"]);
  const [op, setOp] = useState("is");
  return (
    <div className="flex items-center gap-8">
      <FilterChip
        property="Status"
        propertyIcon={icons.chart}
        operator={op}
        operators={["is", "is not"]}
        onOperatorChange={setOp}
        value={status.length > 1 ? `${status.length} classes` : (status[0] ?? "any")}
        multiple
        values={[
          { value: "2xx", label: "2xx" },
          { value: "4xx", label: "4xx" },
          { value: "5xx", label: "5xx" },
        ]}
        selected={status}
        onSelectedChange={setStatus}
        onRemove={() => setStatus([])}
      />
      <Button variant="ghost" size="sm" leadingIcon={icons.add}>
        Add filter
      </Button>
    </div>
  );
}
const MODELS = [
  ["claude-haiku-5-5", "Claude Haiku 5.5"],
  ["claude-opus-5-5", "Claude Opus 5.5"],
  ["claude-sonnet-5-5", "Claude Sonnet 5.5"],
  ["gpt-5.4-mini", "GPT-5.4 mini"],
  ["gpt-5.4-nano", "GPT-5.4 nano"],
  ["gpt-5.6", "GPT-5.6"],
  ["gpt-6-luna", "GPT-6 Luna"],
  ["gpt-6-sol", "GPT-6 Sol"],
  ["text-embedding-3-small", "text-embedding-3-small"],
] as const;
export function Searched() {
  const [model, setModel] = useState<string | null>(null);
  const name = MODELS.find(([id]) => id === model)?.[1];
  return (
    <FilterChip
      property="Model"
      operator="is"
      value={name ?? "any"}
      values={MODELS.map(([id, label]) => ({ value: id, label, textValue: `${label} ${id}` }))}
      selected={model ? [model] : []}
      onSelectedChange={(v) => setModel(v[0] ?? null)}
      onRemove={() => setModel(null)}
    />
  );
}
export function States() {
  return (
    <StateSection title="Chips">
      <div className="flex flex-col items-start gap-12">
        <FilterChip property="Model" operator="is" value="gpt-5" onRemove={() => undefined} />
        <FilterChip property="Key" operator="is any of" value="2 keys" propertyIcon={icons.key} onRemove={() => undefined} />
        <FilterChip property="Latency" operator="over" value="2 s" data-force="hover" />
      </div>
    </StateSection>
  );
}
