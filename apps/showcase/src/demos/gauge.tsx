import { Gauge } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = { title: "Gauge", description: "A semicircle for “budget used”; thresholds as ticks.", group: "Charts", entry: "charts", import: "Gauge" };
export const examples = [{ id: "budget", title: "Budget", fn: "Budget" }];
export function Budget() {
  return <Gauge aria-label="Monthly budget used" value={0.64} label="of $500.00 budget" valueLabel="$320.00" />;
}
export function States() {
  return (
    <>
      <StateSection title="Levels">
        <div className="flex w-640 flex-wrap gap-32">
          <Gauge aria-label="Low" value={0.2} label="used" />
          <Gauge aria-label="Warning" value={0.86} label="used" />
          <Gauge aria-label="Over" value={1.05} label="over budget" />
          <Gauge aria-label="Nothing used" value={0} label="used" />
        </div>
      </StateSection>
      <StateSection title="A value label, several thresholds, a long label">
        <div className="flex w-640 flex-wrap gap-32">
          <Gauge aria-label="Monthly budget used" value={0.64} label="of $2,000.00 budget" valueLabel="$1,284.50" />
          <Gauge aria-label="Tokens used" value={0.72} thresholds={[0.5, 0.75, 0.9]} label="of 50M tokens" valueLabel="36M" />
          <Gauge aria-label="Checkout budget used" value={0.93} label="of $2,000.00 monthly budget for checkout-worker-production" valueLabel="$1,860.00" />
        </div>
      </StateSection>
    </>
  );
}
