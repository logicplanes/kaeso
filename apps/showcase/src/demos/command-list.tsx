import { useState } from "react";
import { CommandList, PickerPopover, PickerTrigger, ProviderBadge, type CommandListGroup } from "@logicplanes/kaeso-ui";
import { models, providerNames } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

const groups: CommandListGroup[] = [...new Set(models.map((m) => m.provider))].map((provider) => ({
  id: provider,
  label: providerNames[provider],
  leading: <ProviderBadge provider={provider} name={providerNames[provider]} size="sm" />,
  count: models.filter((m) => m.provider === provider).length,
  items: models
    .filter((m) => m.provider === provider)
    .map((m) => ({ value: m.model, label: m.name, keywords: [m.model, providerNames[provider] ?? provider] })),
}));

const thousands: CommandListGroup[] = Array.from({ length: 40 }, (_, g) => ({
  id: `provider-${g}`,
  label: `Provider ${g + 1}`,
  count: 250,
  items: Array.from({ length: 250 }, (_, i) => ({ value: `p${g}-m${i}`, label: `model-${g + 1}-${i + 1}` })),
}));

export const meta = {
  title: "CommandList",
  description: "A search field over a long list that stays fast with thousands of rows, grouped under sticky captions.",
  group: "Components",
  entry: "command-list",
  import: "CommandList",
  keyboard: [
    ["Up Down", "Moves through rows, skipping disabled ones"],
    ["Home End", "Goes to the first or last row"],
    ["PageUp PageDown", "Moves a page at a time"],
    ["Enter", "Chooses the active row"],
    ["Escape", "Clears the search, then closes"],
  ],
  a11y: [
    "The search field is a combobox that points at the active row; the list is a listbox with one group per caption.",
    "Each row says its position in the whole list, so a screen reader counts every row, not just those drawn.",
  ],
};
export const examples = [
  { id: "inline", title: "In a dialog step", fn: "Inline" },
  { id: "popover", title: "In a picker", fn: "Picker", minHeight: 520 },
  { id: "thousands", title: "Ten thousand rows", fn: "Thousands" },
];
export function Inline() {
  const [value, setValue] = useState("gpt-5");
  return (
    <div className="w-400 max-w-full">
      <CommandList height="inline" aria-label="Models" groups={groups} value={value} onSelect={setValue} placeholder="Search models" />
    </div>
  );
}
export function Picker() {
  const [open, setOpen] = useState(false);
  const [value, setValue] = useState<string | null>(null);
  const chosen = models.find((m) => m.model === value);
  return (
    <PickerPopover
      title="Model"
      open={open}
      onOpenChange={setOpen}
      trigger={
        <PickerTrigger aria-label="Model" placeholder="Choose a model">
          {chosen?.name}
        </PickerTrigger>
      }
    >
      <CommandList
        aria-label="Models"
        groups={groups}
        value={value}
        autoFocus
        placeholder="Search models"
        onSelect={(v) => {
          setValue(v);
          setOpen(false);
        }}
      />
    </PickerPopover>
  );
}
export function Thousands() {
  const [value, setValue] = useState<string | null>(null);
  return (
    <div className="w-400 max-w-full">
      <CommandList height="inline" aria-label="Models" groups={thousands} value={value} onSelect={setValue} placeholder="Search 10,000 models" />
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Nothing matches">
        <div className="w-400 max-w-full">
          <CommandList height="inline" aria-label="Models" groups={groups} defaultQuery="llama" emptyText="No models match" />
        </div>
      </StateSection>
      <StateSection title="Several">
        <div className="w-400 max-w-full">
          <CommandList height="inline" aria-label="Models" groups={groups} multiple value={["gpt-5", "claude-sonnet-4-5"]} />
        </div>
      </StateSection>
    </>
  );
}
