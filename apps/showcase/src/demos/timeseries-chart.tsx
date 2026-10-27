import { useEffect, useRef, type ReactNode } from "react";
import { TimeseriesChart } from "@logicplanes/kaeso-ui";
import { formatUSD } from "@logicplanes/kaeso-ui/format";
import { usageSeries } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";
import { AsTable, Labelled } from "~/lib/chart-states";

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
const week = usageSeries(7);
const day = (offset: number) => new Date(Date.UTC(2026, 9, 6 + offset, 0, 0));
const net = [
  {
    id: "credits",
    label: "Credit balance change",
    points: [12.4, 8.1, -3, -6.5, 4.2, 9.8, 2.1].map((v, i) => ({ t: day(i), v })),
  },
];
const quiet = [{ id: "errors", label: "Errors", points: Array.from({ length: 7 }, (_, i) => ({ t: day(i), v: 0 })) }];
const one = [{ id: "requests", label: "Requests", points: [{ t: day(6), v: 4812 }] }];

function Hidden({ count, children }: { count: number; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const pills = ref.current?.querySelectorAll<HTMLButtonElement>("figure ul button");
    for (let i = 0; i < count; i++) pills?.[i]?.click();
  }, [count]);
  return <div ref={ref}>{children}</div>;
}

export function States() {
  return (
    <>
      <StateSection title="Highlighted: the crosshair, a dot on each line and the tooltip, as on hover or with the arrow keys">
        <div className="flex w-640 flex-col gap-40">
          <TimeseriesChart aria-label="Spend per model, a point highlighted" series={series} formatValue={money} highlight={20} />
          <TimeseriesChart aria-label="Spend by model, stacked, a point highlighted" kind="stacked" series={series} formatValue={money} highlight={8} />
        </div>
      </StateSection>
      <StateSection title="Edge cases: negative, zero, one point, empty, loading and error">
        <div className="grid w-640 grid-cols-2 gap-x-24 gap-y-40">
          <Labelled title="Negative values">
            <TimeseriesChart aria-label="Credit balance change per day" series={net} formatValue={money} height={200} />
          </Labelled>
          <Labelled title="Every value zero">
            <TimeseriesChart aria-label="Errors per day, none" series={quiet} height={200} />
          </Labelled>
          <Labelled title="One point">
            <TimeseriesChart aria-label="Requests today" series={one} height={200} />
          </Labelled>
          <Labelled title="No data">
            <TimeseriesChart aria-label="Spend, no data" series={[]} height={200} />
          </Labelled>
          <Labelled title="Loading, data on screen">
            <TimeseriesChart aria-label="Spend per model, refreshing" series={week} formatValue={money} height={200} loading />
          </Labelled>
          <Labelled title="Loading, nothing yet">
            <TimeseriesChart aria-label="Spend per model, loading" series={[]} height={200} loading />
          </Labelled>
          <Labelled title="Error with Try again">
            <TimeseriesChart aria-label="Spend, failed" series={week} height={200} error onRetry={() => undefined} />
          </Labelled>
          <Labelled title="Error without a retry">
            <TimeseriesChart aria-label="Spend, failed, no retry" series={week} height={200} error />
          </Labelled>
        </div>
      </StateSection>
      <StateSection title="Partial periods: today still counting, the first day cut short">
        <div className="flex w-640 flex-col gap-40">
          <TimeseriesChart aria-label="Spend per model, today so far" series={week} formatValue={money} partial={{ ongoing: true }} />
          <TimeseriesChart
            aria-label="Spend by model, stacked, first and last partial"
            kind="stacked"
            series={week}
            formatValue={money}
            partial={{ first: true, last: true }}
          />
        </div>
      </StateSection>
      <StateSection title="The last 7 days, a budget line, ticks at the points">
        <div className="flex w-640 flex-col gap-40">
          <TimeseriesChart aria-label="Spend per model, last 7 days" series={week} formatValue={money} />
          <TimeseriesChart
            aria-label="Spend, last 7 days, budget above the data"
            kind="area"
            series={week.slice(0, 1)}
            formatValue={money}
            threshold={{ value: 12, label: "Daily budget $12.00" }}
            hideLegend
          />
          <TimeseriesChart aria-label="Spend per model, a tick at each day" series={week} formatValue={money} tickAt="points" />
        </div>
      </StateSection>
      <StateSection title="Legend: a series hidden, every series hidden">
        <div className="flex w-640 flex-col gap-40">
          <Hidden count={1}>
            <TimeseriesChart aria-label="Spend per model, one hidden" series={week} formatValue={money} />
          </Hidden>
          <Hidden count={week.length}>
            <TimeseriesChart aria-label="Spend per model, all hidden" series={week} formatValue={money} height={200} />
          </Hidden>
        </div>
      </StateSection>
      <StateSection title="320px, highlighted, and as a table">
        <div className="grid w-640 grid-cols-2 gap-x-24 gap-y-40">
          <div className="w-320">
            <TimeseriesChart aria-label="Spend per model, 320px" series={series} formatValue={money} highlight={12} />
          </div>
          <div className="w-280">
            <AsTable>
              <TimeseriesChart aria-label="Spend per model, as a table" series={week} formatValue={money} />
            </AsTable>
          </div>
        </div>
      </StateSection>
    </>
  );
}
