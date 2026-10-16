import { TimeseriesChart } from "@logicplanes/kaeso-ui";
import { formatUSD } from "@logicplanes/kaeso-ui/format";
import { usageSeries } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

const series = usageSeries();
const money = (v: number) => formatUSD(v.toFixed(2));

export const meta = {
  title: "TimeseriesChart",
  description: "Spend over time: y labels on the right, a dashed grid, smart time ticks, a crosshair and tooltip.",
  group: "Charts",
  entry: "charts",
  import: "TimeseriesChart",
  keyboard: [
    ["Tab", "Moves to the plot and shows the newest point"],
    ["← →", "Moves the crosshair and tooltip a point at a time"],
    ["Home End", "Jumps to the first or last point"],
    ["Escape", "Hides the crosshair"],
  ],
  a11y: [
    "A hidden data table carries every value; “View as table” shows it.",
    "The plot is one tab stop: the arrow keys move the crosshair and tooltip, and each point's values are announced politely.",
    "Series keep their color in every chart for the session.",
    "Legend pills toggle series and are buttons with aria-pressed.",
  ],
};
export const examples = [
  { id: "line", title: "Lines", fn: "Lines", minHeight: 360 },
  { id: "area", title: "Area with a budget line", fn: "Area", minHeight: 360 },
  { id: "stacked", title: "Stacked", fn: "Stacked", minHeight: 360 },
];
export function Lines() {
  return (
    <div className="w-full">
      <TimeseriesChart aria-label="Spend per model, last 30 days" series={series} formatValue={money} />
    </div>
  );
}
export function Area() {
  return (
    <div className="w-full">
      <TimeseriesChart
        aria-label="Spend, last 30 days"
        kind="area"
        series={series.slice(0, 1)}
        formatValue={money}
        threshold={{ value: 9, label: "Daily budget $9.00" }}
        hideLegend
      />
    </div>
  );
}
export function Stacked() {
  return (
    <div className="w-full">
      <TimeseriesChart aria-label="Spend by model, stacked" kind="stacked" series={series} formatValue={money} />
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Highlighted: the crosshair, a dot on each line and the tooltip, as on hover or with the arrow keys">
        <div className="flex w-960 flex-col gap-40">
          <TimeseriesChart aria-label="Spend per model, a point highlighted" series={series} formatValue={money} highlight={20} />
          <TimeseriesChart aria-label="Spend by model, stacked, a point highlighted" kind="stacked" series={series} formatValue={money} highlight={8} />
        </div>
      </StateSection>
      <StateSection title="Kinds and states">
        <div className="flex w-960 flex-col gap-40">
          <Lines />
          <Area />
          <Stacked />
          <TimeseriesChart aria-label="Loading" series={series.map((s) => ({ ...s, points: s.points.map((p) => ({ ...p, v: 0 })) }))} loading empty={false} />
          <TimeseriesChart aria-label="Empty" series={series.map((s) => ({ ...s, points: s.points.map((p) => ({ ...p, v: 0 })) }))} />
          <TimeseriesChart aria-label="Error" series={series} error="The usage query timed out." onRetry={() => undefined} />
        </div>
      </StateSection>
    </>
  );
}
