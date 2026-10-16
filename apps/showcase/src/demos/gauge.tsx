import { Gauge } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = { title: "Gauge", description: "A semicircle for “budget used”; thresholds as ticks.", group: "Charts", entry: "charts", import: "Gauge" };
export const examples = [{ id: "budget", title: "Budget", fn: "Budget" }];
export function Budget() {
  return <Gauge aria-label="Monthly budget used" value={0.64} label="of $500.00 budget" valueLabel="$320.00" />;
}
export function States() {
  return (
    <StateSection title="Levels">
      <div className="flex flex-wrap gap-32">
        <Gauge aria-label="Low" value={0.2} label="used" />
        <Gauge aria-label="Warning" value={0.86} label="used" />
        <Gauge aria-label="Over" value={1.05} label="over budget" />
      </div>
    </StateSection>
  );
}
