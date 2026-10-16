import { Heatmap } from "@logicplanes/kaeso-ui";
import { rng } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

const r = rng(3);
const hours = Array.from({ length: 24 }, (_, h) => String(h).padStart(2, "0"));
const days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const values = days.map((_, d) =>
  hours.map((_, h) => Math.round((d < 5 ? 1 : 0.3) * Math.max(0, Math.sin(((h - 6) / 24) * Math.PI)) * 400 * (0.6 + r() * 0.5))),
);

export const meta = {
  title: "Heatmap",
  description: "Hour by weekday on a five-step ramp from fill to series 1; never a rainbow.",
  group: "Charts",
  entry: "charts",
  import: "Heatmap",
};
export const examples = [{ id: "week", title: "Requests by hour", fn: "Week" }];
export function Week() {
  return <Heatmap aria-label="Requests by hour and weekday" rows={days} columns={hours} values={values} />;
}
export function States() {
  return (
    <StateSection title="Ramp">
      <Week />
    </StateSection>
  );
}
