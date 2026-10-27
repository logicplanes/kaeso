import { useState } from "react";
import { Button, icons } from "@logicplanes/kaeso-ui";
import { ParameterPanel, ParameterPopover, type PlaygroundParameters } from "@logicplanes/kaeso-ui/playground";
import { instructions } from "~/fixtures/playground";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "ParameterPanel",
  description:
    "A request's parameters: system instructions, temperature, top P, max output tokens, and reasoning for models that reason. Unset is the model's default.",
  group: "Playground",
  entry: "playground",
  import: "ParameterPanel, ParameterPopover",
  barrel: false,
  keyboard: [
    ["Left Right", "Moves a slider by a step"],
    ["Escape", "Closes the popover"],
  ],
  a11y: [
    "Every control has a visible label that names it; the popover holds a form, keeps the focus inside until it closes and has a Close button; Escape closes it too.",
  ],
};
export const examples = [
  { id: "popover", title: "In a popover", fn: "Basic", minHeight: 120 },
  { id: "panel", title: "As a panel", description: "A model that reasons: effort and summary appear.", fn: "Panel", minHeight: 640 },
];
export function Basic() {
  const [value, setValue] = useState<PlaygroundParameters>({ instructions });
  return (
    <ParameterPopover
      value={value}
      onValueChange={setValue}
      reasoning
      align="start"
      trigger={
        <Button size="sm" variant="secondary" leadingIcon={icons.displayOptions}>
          Parameters
        </Button>
      }
    />
  );
}
export function Panel() {
  const [value, setValue] = useState<PlaygroundParameters>({ instructions, temperature: 0.4 });
  return (
    <ParameterPanel
      layout="panel"
      className="max-w-320"
      value={value}
      onValueChange={setValue}
      reasoning={["minimal", "low", "medium", "high"]}
      maxOutputTokensLimit={128000}
    />
  );
}
export function States() {
  const none = () => undefined;
  return (
    <div className="grid w-640 grid-cols-2 gap-x-24">
      <StateSection title="In its popover" className="col-span-2">
        <div className="h-720">
          <ParameterPopover defaultOpen value={{ instructions }} onValueChange={none} reasoning align="start" />
        </div>
      </StateSection>
      <StateSection title="Defaults">
        <ParameterPanel value={{}} onValueChange={none} />
      </StateSection>
      <StateSection title="Changed, with the model's limit">
        <ParameterPanel value={{ instructions, temperature: 0.2, topP: 0.9, maxOutputTokens: 1024 }} onValueChange={none} maxOutputTokensLimit={128000} />
      </StateSection>
      <StateSection title="A model that reasons">
        <ParameterPanel value={{ reasoningEffort: "high", reasoningSummary: "detailed" }} onValueChange={none} sampling={false} reasoning />
      </StateSection>
      <StateSection title="Disabled">
        <ParameterPanel value={{ instructions }} onValueChange={none} disabled reasoning />
      </StateSection>
      <StateSection title="As a panel">
        <ParameterPanel layout="panel" value={{ temperature: 0.4 }} onValueChange={none} reasoning={["minimal", "low", "medium", "high"]} />
      </StateSection>
    </div>
  );
}
