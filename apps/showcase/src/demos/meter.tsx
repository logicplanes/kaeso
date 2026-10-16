import { Meter, Progress } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Meter",
  description: "A limit or quota: label and value, a 6px pill track, threshold ticks and a caption. Progress for work.",
  group: "Components",
  entry: "meter",
  import: "Meter, Progress",
  a11y: ["role=meter with the value as text (“$32.10 of $50.00”).", "Color supports the value and the caption; it never replaces them."],
};
export const examples = [
  { id: "budget", title: "Budget", fn: "Budget" },
  { id: "progress", title: "Progress", fn: "ProgressDemo" },
];
export function Budget() {
  return <Meter className="w-400" label="Monthly budget" value={0.642} valueLabel="$32.10 of $50.00" thresholds={[0.8]} caption="Resets in 12 days" />;
}
export function ProgressDemo() {
  return (
    <div className="flex w-400 flex-col gap-16">
      <Progress aria-label="Export" value={0.38} />
      <Progress aria-label="Loading logs" />
    </div>
  );
}
export function States() {
  return (
    <StateSection title="Levels">
      <div className="flex w-400 flex-col gap-24">
        <Meter label="Requests per minute" value={0.31} valueLabel="186 of 600" thresholds={[0.8]} />
        <Meter label="Monthly budget" value={0.86} valueLabel="$43.00 of $50.00" thresholds={[0.5, 0.8]} caption="Alerts sent at 50% and 80%" />
        <Meter label="Tokens per day" value={1.04} valueLabel="1.04M of 1M" thresholds={[0.8]} caption="Over the limit: requests are refused" />
        <Meter label="Storage" tone="neutral" value={0.42} valueLabel="4.2 GB of 10 GB" />
      </div>
    </StateSection>
  );
}
