import { useState } from "react";
import { CellInset, Dashboard, DashboardCell, Delta, Legend, MetricTabs, RankList, Readout, TimeseriesChart } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Dashboard",
  description: "An overview's totals, charts and top lists on one surface split by hairlines; the totals are tabs that pick what the chart shows.",
  group: "Components",
  entry: "dashboard",
  import: "Dashboard, DashboardCell, CellInset, MetricTabs, Readout, RankList, Legend",
};
export const examples = [{ id: "overview", title: "A project's overview", fn: "Overview" }];

const START = Date.UTC(2026, 9, 9);
const NOW = [
  1840, 1910, 2120, 1610, 1730, 1980, 1820, 1860, 1880, 1930, 1870, 1850, 1900, 4170, 1950, 2090, 2080, 2060, 2010, 1990, 1970, 1960, 2200, 2050, 1880, 1990,
  2010, 2060, 2030, 1460,
];
const BEFORE = NOW.map((v, i) => Math.round(v * 0.86 + ((i * 37) % 120) - 60));
const points = (values: number[]) => values.map((v, i) => ({ t: START + i * 86_400_000, v }));

const TABS = [
  { id: "requests", label: "Requests", value: "59,415", change: { value: 0.17 }, detail: "vs. prior 30 days" },
  { id: "tokens", label: "Tokens", value: "137.1M", change: { value: 0.19 }, detail: "vs. prior 30 days" },
  { id: "cost", label: "Cost", value: "$62.19", change: { value: 0.18 }, detail: "vs. prior 30 days" },
  { id: "errors", label: "Error rate", value: "0.7%", change: { value: 0.23, better: "down" as const }, detail: "412 failed" },
];

const MODELS = [
  { id: "haiku", label: "Claude Haiku 5.5", value: "31,629", secondary: "53%", share: 0.53 },
  { id: "embed", label: "text-embedding-3-small", value: "13,043", secondary: "22%", share: 0.22 },
  { id: "sonnet", label: "Claude Sonnet 5.5", value: "8,007", secondary: "13%", share: 0.13 },
  { id: "mini", label: "GPT-5.4 mini", value: "4,655", secondary: "8%", share: 0.08 },
];

export function Overview() {
  const [metric, setMetric] = useState("requests");
  return (
    <div className="w-full">
      <Dashboard aria-label="Usage in the period">
        <MetricTabs aria-label="Metric to chart" items={TABS} value={metric} onValueChange={setMetric} controls="dashboard-chart" />
        <DashboardCell className="lg:col-span-8" title="Requests per day, UTC">
          <CellInset grow>
            <Legend
              items={[
                { id: "now", label: "This period", value: "59,415", color: "var(--k-viz-1)" },
                { id: "before", label: "vs. prior 30 days", value: "50,819", color: "var(--k-text-tertiary)", dashed: true },
              ]}
            />
            <div id="dashboard-chart" className="mt-16">
              <TimeseriesChart
                aria-label="Requests per day"
                kind="area"
                hideLegend
                height={220}
                timeZone="UTC"
                series={[
                  { id: "now", label: "This period", points: points(NOW) },
                  { id: "before", label: "vs. prior 30 days", color: "var(--k-text-tertiary)", dashed: true, points: points(BEFORE) },
                ]}
              />
            </div>
          </CellInset>
        </DashboardCell>
        <DashboardCell className="lg:col-span-4" title="Cache hit">
          <Readout value="48%" change={{ value: 0.005 }} detail="62.6M input tokens from cache" />
          <div className="mt-20 grid grid-cols-2">
            <Readout size="sm" label="November so far" value="$13.39" />
            <Readout size="sm" label="Forecast" value="$60.45" />
          </div>
        </DashboardCell>
        <DashboardCell className="lg:col-span-12" title="Models by requests">
          <RankList aria-label="Models by requests" items={MODELS} onOpen={() => {}} />
        </DashboardCell>
      </Dashboard>
    </div>
  );
}

export function States() {
  return (
    <>
      <StateSection title="Totals while they load, and an odd count on a narrow screen">
        <div className="flex w-560 flex-col gap-24">
          <Dashboard aria-label="Loading">
            <MetricTabs aria-label="Metric to chart" items={TABS} value="requests" onValueChange={() => {}} loading />
          </Dashboard>
          <Dashboard aria-label="Three totals">
            <MetricTabs aria-label="Metric to chart" items={TABS.slice(0, 3)} value="tokens" onValueChange={() => {}} />
          </Dashboard>
        </div>
      </StateSection>
      <StateSection title="Rows: loading, plain, empty">
        <div className="w-560">
          <Dashboard aria-label="Rows">
            <DashboardCell className="lg:col-span-4" title="Loading">
              <RankList aria-label="Loading" items={[]} loading rows={3} />
            </DashboardCell>
            <DashboardCell className="lg:col-span-4" title="Recent errors">
              <RankList aria-label="Recent errors" items={[{ id: "a", label: "Provider overloaded", secondary: "1 h ago" }]} />
            </DashboardCell>
            <DashboardCell className="lg:col-span-4" title="Keys by requests">
              <RankList aria-label="Keys by requests" items={[]} empty="Nothing in this period" />
            </DashboardCell>
          </Dashboard>
        </div>
      </StateSection>
      <StateSection title="Changes written as text">
        <div className="flex gap-16">
          <Delta value={0.17} />
          <Delta value={-0.04} />
          <Delta value={0.23} better="down" />
          <Delta value={0} />
        </div>
      </StateSection>
    </>
  );
}
