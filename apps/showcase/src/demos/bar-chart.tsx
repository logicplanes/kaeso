import { BarChart, tokenKindColor, tokenKindLabel } from "@logicplanes/kaeso-ui";
import { formatTokens, formatUSD } from "@logicplanes/kaeso-ui/format";
import { useLocale, useUiText } from "@logicplanes/kaeso-ui/i18n";
import { StateSection } from "~/lib/states";
import { AsTable, Labelled } from "~/lib/chart-states";

const days = ["Oct 6", "Oct 7", "Oct 8", "Oct 9", "Oct 10", "Oct 11", "Oct 12"];
const kinds = ["input", "cacheRead", "output", "reasoning"] as const;
function useSeries() {
  const t = useUiText();
  return kinds.map((k, i) => ({
    id: k,
    label: tokenKindLabel(k, t),
    color: tokenKindColor(k),
    values: days.map((_, d) => Math.round((1.2 - i * 0.25) * 1_000_000 * (0.7 + ((d * 37 + i * 11) % 10) / 20))),
  }));
}

export const meta = {
  title: "BarChart",
  description: "Grouped and stacked bars, slender with a 4px top radius and 2px gaps between stacked segments; horizontal for rankings.",
  group: "Charts",
  entry: "charts",
  import: "BarChart",
  keyboard: [
    ["Tab", "Moves to the plot and highlights the newest day (a ranking's first row)"],
    ["← →", "Moves the highlight and tooltip a category at a time (↑ ↓ in a ranking)"],
    ["Home End", "Jumps to the first or last category"],
    ["Escape", "Hides the highlight"],
  ],
  a11y: [
    "A hidden data table carries every value; “View as table” shows it.",
    "The category under the pointer or the arrow keys sits on a quiet fill with its tooltip; its values are announced politely. Nothing moves.",
  ],
};
const MONTH = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const iso = (d: Date) => d.toISOString().slice(0, 10);
const monthDays = Array.from({ length: 15 }, (_, i) => iso(new Date(Date.UTC(2025, 7 + i, 1))));
const weekDays = Array.from({ length: 53 }, (_, i) => iso(new Date(Date.UTC(2025, 9, 13 + i * 7))));
const monthValues = monthDays.map((_, i) => Math.round((18 + i * 2.4 + ((i * 7) % 5)) * 1_000_000));
const weekValues = weekDays.map((_, i) => Math.round((4 + i * 0.12 + ((i * 13) % 7) * 0.3) * 1_000_000));
function useInputTokens(values: number[]) {
  const t = useUiText();
  return [{ id: "input", label: tokenKindLabel("input", t), color: tokenKindColor("input"), values }];
}
function useAxes() {
  const locale = useLocale();
  const write = (options: Intl.DateTimeFormatOptions) => {
    const f = new Intl.DateTimeFormat(locale, { ...options, timeZone: "UTC" });
    return (day: string) => f.format(new Date(`${day}T00:00:00Z`));
  };
  const en = locale === "en";
  const parts = (day: string) => ({ year: day.slice(0, 4), month: Number(day.slice(5, 7)) - 1, day: Number(day.slice(8)) });
  const monthYear = write({ month: "short", year: "numeric" });
  const month = write({ month: "short" });
  const year = write({ year: "numeric" });
  const dayMonth = write({ day: "numeric", month: "short" });
  return {
    months: monthDays.map((d) => (en ? `${MONTH[parts(d).month]} ${parts(d).year}` : monthYear(d))),
    monthAxis: monthDays.map((d) => (parts(d).month === 0 ? (en ? parts(d).year : year(d)) : en ? MONTH[parts(d).month]! : month(d))),
    weeks: weekDays.map((d) => (en ? `Week of ${parts(d).day} ${MONTH[parts(d).month]}` : dayMonth(d))),
    weekAxis: weekDays.map((d) => (en ? `${parts(d).day} ${MONTH[parts(d).month]}` : dayMonth(d))),
  };
}

export const examples = [
  { id: "stacked", title: "Tokens by kind, stacked", fn: "Stacked", minHeight: 340 },
  { id: "grouped", title: "Grouped", fn: "Grouped", minHeight: 340 },
  { id: "ranking", title: "Ranking", fn: "Ranking", minHeight: 300 },
  { id: "months", title: "Months across a year", fn: "Months", minHeight: 300 },
];
export function Stacked() {
  const series = useSeries();
  return (
    <div className="w-full">
      <BarChart aria-label="Tokens by kind per day" categories={days} series={series} layout="stacked" formatValue={formatTokens} />
    </div>
  );
}
export function Grouped() {
  const series = useSeries();
  return (
    <div className="w-full">
      <BarChart aria-label="Input and output tokens per day" categories={days} series={series.slice(0, 2)} formatValue={formatTokens} />
    </div>
  );
}
export function Ranking() {
  return (
    <div className="w-full">
      <BarChart
        aria-label="Spend by key"
        layout="horizontal"
        height={200}
        categories={["checkout-web", "checkout-worker", "ci-runner", "staging"]}
        series={[{ id: "spend", label: "Spend", color: "var(--k-viz-1)", values: [84.2, 51.9, 12.4, 3.1] }]}
        formatValue={(v) => formatUSD(v.toFixed(2))}
      />
    </div>
  );
}
export function Months() {
  const { months, monthAxis } = useAxes();
  const monthTokens = useInputTokens(monthValues);
  return (
    <div className="w-full">
      <BarChart
        aria-label="Input tokens per month"
        categories={months}
        axisLabels={monthAxis}
        dates={monthDays}
        series={monthTokens}
        formatValue={formatTokens}
      />
    </div>
  );
}
const week = ["Oct 6", "Oct 7", "Oct 8", "Oct 9", "Oct 10", "Oct 11", "Oct 12"];
const net = [{ id: "credits", label: "Credit balance change", color: "var(--k-viz-1)", values: [12.4, 8.1, -3, -6.5, 4.2, 9.8, 2.1] }];
const zeros = [{ id: "requests", label: "Requests", values: week.map(() => 0) }];
const keyNames = ["checkout-web-production-eu-west", "checkout-worker", "claude-sonnet-4-5-20250929", "staging"];
const spend = (v: number) => formatUSD(v.toFixed(2));

export function States() {
  const { months, monthAxis, weeks, weekAxis } = useAxes();
  const series = useSeries();
  const monthTokens = useInputTokens(monthValues);
  const weekTokens = useInputTokens(weekValues);
  return (
    <>
      <StateSection title="Highlighted: a quiet fill and the tooltip, as on hover or with the arrow keys">
        <div className="flex w-640 flex-col gap-40">
          <BarChart
            aria-label="Tokens by kind per day, a day highlighted"
            categories={days}
            series={series}
            layout="stacked"
            formatValue={formatTokens}
            highlight={4}
          />
          <BarChart
            aria-label="Input and output tokens per day, a day highlighted"
            categories={days}
            series={series.slice(0, 2)}
            formatValue={formatTokens}
            highlight={2}
          />
          <BarChart
            aria-label="Spend by key, a key highlighted"
            layout="horizontal"
            height={200}
            categories={["checkout-web", "checkout-worker", "ci-runner", "staging"]}
            series={[{ id: "spend", label: "Spend", color: "var(--k-viz-1)", values: [84.2, 51.9, 12.4, 3.1] }]}
            formatValue={spend}
            highlight={1}
          />
        </div>
      </StateSection>
      <StateSection title="Edge cases: negative, zero, one category, empty, loading and error">
        <div className="grid w-640 grid-cols-2 gap-x-24 gap-y-40">
          <Labelled title="Negative values">
            <BarChart aria-label="Credit balance change per day" categories={week} series={net} formatValue={spend} height={200} />
          </Labelled>
          <Labelled title="Every value zero">
            <BarChart aria-label="Requests per day, none" categories={week} series={zeros} height={200} />
          </Labelled>
          <Labelled title="One category">
            <BarChart aria-label="Requests today" categories={["Oct 12"]} series={[{ id: "requests", label: "Requests", values: [4812] }]} height={200} />
          </Labelled>
          <Labelled title="No data">
            <BarChart aria-label="Requests per day, no data" categories={[]} series={[]} height={200} />
          </Labelled>
          <Labelled title="Loading, data on screen">
            <BarChart aria-label="Tokens per day, refreshing" categories={days} series={series.slice(0, 1)} formatValue={formatTokens} height={200} loading />
          </Labelled>
          <Labelled title="Loading, nothing yet">
            <BarChart aria-label="Tokens per day, loading" categories={[]} series={[]} height={200} loading />
          </Labelled>
          <Labelled title="Error with Try again">
            <BarChart aria-label="Tokens per day, failed" categories={days} series={series} height={200} error onRetry={() => undefined} />
          </Labelled>
          <Labelled title="Error without a retry">
            <BarChart aria-label="Tokens per day, failed, no retry" categories={days} series={series} height={200} error />
          </Labelled>
        </div>
      </StateSection>
      <StateSection title="Partial periods: today still counting, the first week cut short">
        <div className="flex w-640 flex-col gap-40">
          <BarChart
            aria-label="Tokens by kind per day, today so far"
            categories={days}
            series={series}
            layout="stacked"
            formatValue={formatTokens}
            partial={{ ongoing: true }}
          />
          <BarChart
            aria-label="Input and output tokens per day, first and last partial"
            categories={days}
            series={series.slice(0, 2)}
            formatValue={formatTokens}
            partial={{ first: true, last: true }}
          />
        </div>
      </StateSection>
      <StateSection title="Long names, two series across, huge values, and 320px">
        <div className="grid w-640 grid-cols-2 gap-x-24 gap-y-40">
          <Labelled title="Long key names, 320px">
            <div className="w-320">
              <BarChart
                aria-label="Spend by key, long names"
                layout="horizontal"
                height={200}
                categories={keyNames}
                series={[{ id: "spend", label: "Spend", color: "var(--k-viz-1)", values: [1284.5, 512.2, 96.4, 3.1] }]}
                formatValue={spend}
              />
            </div>
          </Labelled>
          <Labelled title="Two series across">
            <BarChart
              aria-label="Input and output tokens by key"
              layout="horizontal"
              height={240}
              categories={["checkout-web", "checkout-worker", "ci-runner"]}
              series={series.slice(0, 2).map((s) => ({ ...s, values: s.values.slice(0, 3) }))}
              formatValue={formatTokens}
            />
          </Labelled>
          <Labelled title="Huge values, default format">
            <BarChart
              aria-label="Tokens per day, lifetime totals"
              categories={week}
              series={[{ id: "tokens", label: "Tokens", values: week.map((_, i) => (i + 1) * 410_000_000_000) }]}
              height={200}
            />
          </Labelled>
          <Labelled title="320px, stacked with a legend">
            <div className="w-320">
              <BarChart aria-label="Tokens by kind per day, 320px" categories={days} series={series} layout="stacked" formatValue={formatTokens} height={200} />
            </div>
          </Labelled>
        </div>
      </StateSection>
      <StateSection title="As a table: the same height as the chart, scrolling inside">
        <div className="w-640">
          <AsTable>
            <BarChart
              aria-label="Input tokens per month, as a table"
              categories={months}
              axisLabels={monthAxis}
              dates={monthDays}
              series={monthTokens}
              formatValue={formatTokens}
            />
          </AsTable>
        </div>
      </StateSection>
      <StateSection title="Axis labels thinned evenly, keeping where a year begins and the newest bar">
        <div className="flex flex-col gap-40">
          <div className="w-640">
            <BarChart
              aria-label="Input tokens per month, 640px"
              categories={months}
              axisLabels={monthAxis}
              dates={monthDays}
              series={monthTokens}
              formatValue={formatTokens}
            />
          </div>
          <div className="w-360">
            <BarChart
              aria-label="Input tokens per month, 360px"
              categories={months}
              axisLabels={monthAxis}
              dates={monthDays}
              series={monthTokens}
              formatValue={formatTokens}
            />
          </div>
          <div className="w-640">
            <BarChart
              aria-label="Input tokens per week, 640px"
              categories={weeks}
              axisLabels={weekAxis}
              dates={weekDays}
              series={weekTokens}
              formatValue={formatTokens}
            />
          </div>
          <div className="w-360">
            <BarChart
              aria-label="Input tokens per week, 360px"
              categories={weeks}
              axisLabels={weekAxis}
              dates={weekDays}
              series={weekTokens}
              formatValue={formatTokens}
            />
          </div>
        </div>
      </StateSection>
    </>
  );
}
