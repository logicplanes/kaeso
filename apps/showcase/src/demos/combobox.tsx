import { useState } from "react";
import { Combobox, ModelLabel, type ComboboxOption } from "@logicplanes/kaeso-ui";
import { models } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

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
export function States() {
  return (
    <StateSection title="Fields">
      <div className="flex w-400 flex-col gap-16">
        <Combobox aria-label="Empty" options={options} placeholder="Search models" />
        <Combobox aria-label="Focused" options={options} placeholder="Search models" data-force="focus" />
        <Combobox aria-label="Value" options={options} defaultValue="claude-sonnet-4-5" />
        <Combobox multiple aria-label="Chips" options={options} defaultValue={["gpt-5", "gpt-5-mini", "claude-haiku-4-5"]} />
        <Combobox aria-label="Disabled" options={options} disabled defaultValue="gpt-5" />
        <ModelLabel provider="openai" model="gpt-5" name="GPT-5" />
      </div>
    </StateSection>
  );
}
