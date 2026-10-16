import { BarChart, TOKEN_KIND_LABEL, tokenKindColor } from "@logicplanes/kaeso-ui";
import { formatTokens, formatUSD } from "@logicplanes/kaeso-ui/format";
import { useLocale } from "@logicplanes/kaeso-ui/i18n";
import { StateSection } from "~/lib/states";

const days = ["Oct 6", "Oct 7", "Oct 8", "Oct 9", "Oct 10", "Oct 11", "Oct 12"];
const kinds = ["input", "cacheRead", "output", "reasoning"] as const;
const series = kinds.map((k, i) => ({
  id: k,
  label: TOKEN_KIND_LABEL[k],
  color: tokenKindColor(k),
  values: days.map((_, d) => Math.round((1.2 - i * 0.25) * 1_000_000 * (0.7 + ((d * 37 + i * 11) % 10) / 20))),
}));

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
const monthTokens = [
  {
    id: "input",
    label: TOKEN_KIND_LABEL.input,
    color: tokenKindColor("input"),
    values: monthDays.map((_, i) => Math.round((18 + i * 2.4 + ((i * 7) % 5)) * 1_000_000)),
  },
];
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
const weekTokens = [
  {
    id: "input",
    label: TOKEN_KIND_LABEL.input,
    color: tokenKindColor("input"),
    values: weekDays.map((_, i) => Math.round((4 + i * 0.12 + ((i * 13) % 7) * 0.3) * 1_000_000)),
  },
];

export const examples = [
  { id: "stacked", title: "Tokens by kind, stacked", fn: "Stacked", minHeight: 340 },
  { id: "grouped", title: "Grouped", fn: "Grouped", minHeight: 340 },
  { id: "ranking", title: "Ranking", fn: "Ranking", minHeight: 300 },
  { id: "months", title: "Months across a year", fn: "Months", minHeight: 300 },
];
export function Stacked() {
  return (
    <div className="w-full">
      <BarChart aria-label="Tokens by kind per day" categories={days} series={series} layout="stacked" formatValue={formatTokens} />
    </div>
  );
}
export function Grouped() {
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
export function States() {
  const { months, monthAxis, weeks, weekAxis } = useAxes();
  return (
    <>
      <StateSection title="Highlighted: a quiet fill behind the category and its tooltip, as on hover or with the arrow keys">
        <div className="flex w-960 flex-col gap-40">
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
            formatValue={(v) => formatUSD(v.toFixed(2))}
            highlight={1}
          />
        </div>
      </StateSection>
      <StateSection title="Axis labels thinned where they don't all fit: evenly, never overlapping, keeping where a year begins, then the newest bar">
        <div className="flex flex-col gap-40">
          <div className="w-960">
            <BarChart
              aria-label="Input tokens per month, 960px"
              categories={months}
              axisLabels={monthAxis}
              dates={monthDays}
              series={monthTokens}
              formatValue={formatTokens}
            />
          </div>
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
          <div className="w-960">
            <BarChart
              aria-label="Input tokens per week, 960px"
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
      <StateSection title="Layouts">
        <div className="flex w-960 flex-col gap-40">
          <Stacked />
          <Grouped />
          <Ranking />
        </div>
      </StateSection>
    </>
  );
}
