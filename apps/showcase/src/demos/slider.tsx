import { Slider } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

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
export function States() {
  return (
    <StateSection title="States">
      <div className="flex w-320 flex-col gap-32 pt-24">
        <Slider aria-label="Rest" defaultValue={40} />
        <Slider aria-label="Focus" defaultValue={40} data-force="focus" />
        <Slider aria-label="Dragging" defaultValue={64} data-force="dragging" />
        <Slider aria-label="Ticks" defaultValue={30} step={10} ticks />
        <Slider aria-label="Disabled" defaultValue={40} disabled />
      </div>
    </StateSection>
  );
}
