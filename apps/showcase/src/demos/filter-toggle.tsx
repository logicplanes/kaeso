import { useState } from "react";
import { FilterToggle, Select } from "@logicplanes/kaeso-ui";

export const meta = {
  title: "FilterToggle",
  description: "An on-or-off filter as a pill that sits in a row of selects.",
  group: "Components",
  entry: "filter-toggle",
  import: "FilterToggle",
  keyboard: [["Space or Enter", "Turns the filter on or off"]],
  a11y: ["A button with aria-pressed, named by its label; the check is decoration."],
};
export const examples = [{ id: "row", title: "In a filter row", fn: "Row" }];
export function Row() {
  const [agents, setAgents] = useState(false);
  const [overridden, setOverridden] = useState(true);
  return (
    <div className="flex flex-wrap items-center gap-8">
      <Select size="sm" aria-label="Actor" placeholder="Any actor" options={[{ value: "ada", label: "Ada Okafor" }]} value={null} onValueChange={() => {}} />
      <FilterToggle label="Agents" pressed={agents} onPressedChange={setAgents} />
      <FilterToggle label="Only overridden" pressed={overridden} onPressedChange={setOverridden} />
    </div>
  );
}
