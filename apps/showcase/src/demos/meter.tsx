import { Badge, Meter, Progress } from "@logicplanes/kaeso-ui";
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
  { id: "gauge", title: "Gauge", fn: "GaugeDemo", description: '`shape="gauge"`: a half circle for the one number a page is about.' },
  { id: "progress", title: "Progress", fn: "ProgressDemo" },
];
export function Budget() {
  return <Meter className="w-400" label="Monthly budget" value={0.642} valueLabel="$32.10 of $50.00" thresholds={[0.8]} caption="Resets in 12 days" />;
}
export function GaugeDemo() {
  return <Meter shape="gauge" aria-label="Monthly budget used" value={0.64} label="of $500.00 budget" valueLabel="$320.00" />;
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
    <>
      <StateSection title="Levels: empty, under, at the threshold, near, over, neutral">
        <div className="flex w-400 flex-col gap-24">
          <Meter label="Requests per minute" value={0} valueLabel="0 of 600" thresholds={[0.8]} />
          <Meter label="Requests per minute" value={0.31} valueLabel="186 of 600" thresholds={[0.8]} />
          <Meter label="Requests per minute" value={0.8} valueLabel="480 of 600" thresholds={[0.8]} caption="Alert sent at 80%" />
          <Meter label="Monthly budget" value={0.86} valueLabel="$43.00 of $50.00" thresholds={[0.5, 0.8]} caption="Alerts sent at 50% and 80%" />
          <Meter label="Tokens per day" value={1.04} valueLabel="1.04M of 1M" thresholds={[0.8]} caption="Over the limit: requests are refused" />
          <Meter label="Storage" tone="neutral" value={0.42} valueLabel="4.2 GB of 10 GB" />
        </div>
      </StateSection>
      <StateSection title="In a card, with leading content and a long label">
        <div className="flex w-400 flex-col gap-24 rounded-group bg-raised p-20 shadow-xs">
          <Meter
            label="Output tokens per day for the checkout assistant production key"
            leading={<Badge>Key</Badge>}
            value={0.64}
            valueLabel="640K of 1M"
            thresholds={[0.5, 0.8]}
          />
        </div>
      </StateSection>
      <StateSection title="Progress: determinate and indeterminate">
        <ProgressDemo />
      </StateSection>
      <StateSection title="Gauge levels">
        <div className="flex w-640 flex-wrap gap-32">
          <Meter shape="gauge" aria-label="Low" value={0.2} label="used" />
          <Meter shape="gauge" aria-label="Warning" value={0.86} label="used" />
          <Meter shape="gauge" aria-label="Over" value={1.05} label="over budget" />
          <Meter shape="gauge" aria-label="Nothing used" value={0} label="used" />
        </div>
      </StateSection>
      <StateSection title="Gauge with a value label, several thresholds, a long label">
        <div className="flex w-640 flex-wrap gap-32">
          <Meter shape="gauge" aria-label="Monthly budget used" value={0.64} label="of $2,000.00 budget" valueLabel="$1,284.50" />
          <Meter shape="gauge" aria-label="Tokens used" value={0.72} thresholds={[0.5, 0.75, 0.9]} label="of 50M tokens" valueLabel="36M" />
          <Meter
            shape="gauge"
            aria-label="Checkout budget used"
            value={0.93}
            label="of $2,000.00 monthly budget for checkout-worker-production"
            valueLabel="$1,860.00"
          />
        </div>
      </StateSection>
    </>
  );
}
