import { useState } from "react";
import { Combobox, type ComboboxOption } from "@logicplanes/kaeso-ui";
import { models } from "~/fixtures/kaeso";
import { Matrix, StateSection } from "~/lib/states";

const options: ComboboxOption[] = models.map((m) => ({ value: m.model, label: m.model, description: m.name }));

export const meta = {
  title: "Combobox",
  description: "A pill input with a filtered list: matches highlighted, async results, chips for many, create.",
  group: "Components",
  entry: "combobox",
  import: "Combobox",
  keyboard: [
    ["Up Down", "Moves through results"],
    ["Enter", "Chooses"],
    ["Escape", "Closes"],
    ["Backspace", "Removes the last chip"],
  ],
  a11y: ["role=combobox with a listbox; the result count is announced through a status region."],
};
export const examples = [
  { id: "single", title: "Single", fn: "Single" },
  { id: "multiple", title: "Multiple with chips", fn: "Multiple" },
  { id: "create", title: "Create", fn: "Create" },
];
export function Single() {
  return (
    <div className="w-320">
      <Combobox label="Model" options={options} placeholder="Search models" emptyText="No models match" defaultValue="gpt-5" />
    </div>
  );
}
export function Multiple() {
  return (
    <div className="w-400">
      <Combobox multiple label="Models this key may use" options={options} placeholder="Add a model" defaultValue={["gpt-5-mini", "claude-haiku-4-5"]} />
    </div>
  );
}
export function Create() {
  const [opts, setOpts] = useState<ComboboxOption[]>([
    { value: "checkout", label: "checkout" },
    { value: "support", label: "support" },
  ]);
  return (
    <div className="w-320">
      <Combobox label="Alias" options={opts} placeholder="Find or create an alias" onCreate={(l) => setOpts((o) => [...o, { value: l, label: l }])} />
    </div>
  );
}
function Field(props: Record<string, unknown>) {
  return (
    <div className="w-140">
      <Combobox aria-label="Model" options={options} placeholder="Search models" defaultValue="gpt-5" {...props} />
    </div>
  );
}

export function States() {
  return (
    <>
      <Matrix
        title="Field"
        rows={[{ label: "value" }]}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Disabled", props: { disabled: true } },
        ]}
        render={(_, p) => <Field {...p} />}
      />
      <Matrix
        title="Focus and errors"
        rows={[{ label: "value" }]}
        columns={[
          { label: "Focus", props: { "data-force": "focus" } },
          { label: "Invalid", props: { invalid: true } },
        ]}
        render={(_, p) => <Field {...p} />}
      />
      <StateSection title="Sizes">
        <div className="flex w-400 flex-col gap-12">
          {(["sm", "md", "lg", "xl"] as const).map((size) => (
            <Combobox key={size} aria-label={`Model, ${size}`} size={size} options={options} defaultValue="claude-sonnet-4-5" />
          ))}
        </div>
      </StateSection>
      <StateSection title="Several values">
        <div className="flex w-400 flex-col gap-12">
          <Combobox multiple aria-label="Models" options={options} placeholder="Add models" />
          <Combobox multiple aria-label="Models, three" options={options} defaultValue={["gpt-5", "gpt-5-mini", "claude-haiku-4-5"]} />
          <Combobox multiple size="sm" aria-label="Models, small" options={options} defaultValue={["gpt-5", "claude-sonnet-4-5"]} />
          <Combobox multiple size="xl" aria-label="Models, extra large" options={options} defaultValue={["gpt-5", "claude-sonnet-4-5"]} />
          <Combobox multiple aria-label="Models, disabled" options={options} defaultValue={["gpt-5"]} disabled />
          <Combobox label="Fallback models" options={options} placeholder="Search models" error="Choose a model that is enabled in this project." />
        </div>
      </StateSection>
    </>
  );
}
