import { Slider } from "@logicplanes/kaeso-ui";
import { Matrix, StateSection } from "~/lib/states";

export const meta = {
  title: "Slider",
  description: "A 4px track, an ink range and an ink thumb; a value bubble while dragging; ticks and ranges.",
  group: "Components",
  entry: "slider",
  import: "Slider",
  keyboard: [
    ["Left", "Decreases"],
    ["Right", "Increases"],
    ["Home", "Minimum"],
    ["End", "Maximum"],
  ],
  a11y: ["Each thumb is a slider with its own name (getAriaLabel for ranges)."],
};
export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  { id: "range", title: "Range with ticks", fn: "Range" },
];
export function Basic() {
  return (
    <div className="w-320">
      <Slider aria-label="Alert threshold" defaultValue={80} formatValue={(v) => `${v}%`} />
    </div>
  );
}
export function Range() {
  return (
    <div className="w-320">
      <Slider getAriaLabel={(i) => (i === 0 ? "Minimum" : "Maximum")} defaultValue={[20, 60]} step={10} ticks />
    </div>
  );
}
function Threshold(props: Record<string, unknown>) {
  return (
    <div className="w-140 pt-32">
      <Slider aria-label="Alert threshold" defaultValue={60} formatValue={(v) => `${v}%`} {...props} />
    </div>
  );
}

export function States() {
  return (
    <>
      <Matrix
        title="Thumb"
        rows={[{ label: "value" }]}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Disabled", props: { disabled: true } },
        ]}
        render={(_, p) => <Threshold {...p} />}
      />
      <Matrix
        title="Keyboard and pointer"
        rows={[{ label: "value" }]}
        columns={[
          { label: "Focus", props: { "data-force": "focus" } },
          { label: "Dragging", props: { "data-force": "dragging" } },
        ]}
        render={(_, p) => <Threshold {...p} />}
      />
      <StateSection title="Ends, ticks and ranges">
        <div className="flex w-320 flex-col gap-24">
          <Slider aria-label="Minimum" defaultValue={0} />
          <Slider aria-label="Maximum" defaultValue={100} />
          <Slider aria-label="Ticks" defaultValue={30} step={10} ticks />
          <Slider getAriaLabel={(i) => (i === 0 ? "Minimum" : "Maximum")} defaultValue={[20, 60]} step={10} ticks />
        </div>
      </StateSection>
    </>
  );
}
