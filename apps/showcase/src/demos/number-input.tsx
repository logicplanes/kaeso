import { useState } from "react";
import { NumberInput } from "@logicplanes/kaeso-ui";
import { Matrix } from "~/lib/states";

export const meta = {
  title: "NumberInput",
  description: "Exact numbers: right-aligned tabular digits, a unit, steppers on hover, a decimal string value.",
  group: "Components",
  entry: "input",
  import: "NumberInput",
  keyboard: [
    ["Up", "Increases by the step"],
    ["Down", "Decreases by the step"],
  ],
  a11y: ["The value is a decimal string, so money never passes through a float.", "Thousands separators appear on blur and are removed while editing."],
};
export const examples = [
  { id: "money", title: "Money", fn: "Money" },
  { id: "rate", title: "Rate", fn: "Rate" },
];
export function Money() {
  const [v, setV] = useState("1250.00");
  return (
    <div className="flex w-280 flex-col gap-8">
      <NumberInput label="Monthly budget" value={v} onValueChange={setV} unit="USD" step="50" min="0" />
      <span className="mono text-small text-fg-secondary">value = "{v}"</span>
    </div>
  );
}
export function Rate() {
  return (
    <div className="w-240">
      <NumberInput aria-label="Requests per minute" defaultValue="600" unit="/ min" step="10" />
    </div>
  );
}
export function States() {
  return (
    <Matrix
      title="States"
      rows={[{ label: "money" }]}
      columns={[
        { label: "Rest", props: {} },
        { label: "Hover", props: { "data-force": "hover" } },
        { label: "Focus", props: { "data-force": "focus" } },
        { label: "Invalid", props: { invalid: true } },
        { label: "Disabled", props: { disabled: true } },
      ]}
      render={(_, p) => (
        <div className="w-200">
          <NumberInput aria-label="Budget" defaultValue="12500.50" unit="USD" {...p} />
        </div>
      )}
    />
  );
}
