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
const longDays = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

export function States() {
  return (
    <>
      <StateSection title="Ramp">
        <div className="w-640">
          <Week />
        </div>
      </StateSection>
      <StateSection title="Narrow: the hours scroll sideways with a fade; long row names">
        <div className="w-280">
          <Heatmap aria-label="Requests by hour and weekday, narrow" rows={longDays} columns={hours} values={values} />
        </div>
      </StateSection>
      <StateSection title="Every hour zero, and no data">
        <div className="flex w-640 flex-col gap-40">
          <Heatmap aria-label="Errors by hour and weekday, none" rows={days} columns={hours} values={days.map(() => hours.map(() => 0))} />
          <Heatmap aria-label="Requests by hour, no data" rows={[]} columns={[]} values={[]} />
        </div>
      </StateSection>
    </>
  );
}
