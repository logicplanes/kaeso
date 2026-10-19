import { Stat, MetricRow, StatGrid, StatTile } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Stat",
  description: "A big number followed by its word, no box. MetricRow lines up three to five; StatGrid holds tiles.",
  group: "Components",
  entry: "stat",
  import: "Stat, MetricRow, StatGrid, StatTile",
};
export const examples = [
  { id: "row", title: "Metric row", fn: "Row" },
  { id: "tiles", title: "Stat tiles: four, three without cost, totals", fn: "Tiles" },
];
export function Row() {
  return (
    <MetricRow>
      <Stat value="12,480" unit="requests" />
      <Stat value="$184.20" unit="total spend" delta={{ label: "+12%", tone: "neutral" }} />
      <Stat value="1.2 s" unit="median latency" />
    </MetricRow>
  );
}
const DAYS = [12, 18, 14, 22, 30, 26, 41];
export function Tiles() {
  return (
    <div className="flex flex-col gap-24">
      <StatGrid aria-label="Usage in the period">
        <StatTile label="Requests" value="12,480" change={{ value: 0.12 }} detail="vs. the previous 7 days" trend={DAYS} />
        <StatTile label="Tokens" value="4.2M" change={{ value: -0.04 }} detail="vs. the previous 7 days" trend={DAYS.map((d) => d * 3)} />
        <StatTile label="Cost" value="$184.20" detail="vs. the previous 7 days" trend={[]} />
        <StatTile label="Errors" value="38" change={{ value: 0.3, better: "down" }} detail="0.3% of requests" trend={DAYS.map((d) => d % 5)} />
      </StatGrid>
      <StatGrid aria-label="Usage without the cost right">
        <StatTile label="Requests" value="12,480" change={{ value: 0.12 }} detail="vs. the previous 7 days" trend={DAYS} />
        <StatTile label="Tokens" value="4.2M" change={{ value: -0.04 }} detail="vs. the previous 7 days" trend={DAYS.map((d) => d * 3)} />
        <StatTile label="Errors" value="38" change={{ value: 0.3, better: "down" }} detail="0.3% of requests" trend={DAYS.map((d) => d % 5)} />
      </StatGrid>
      <StatGrid aria-label="Usage in the last 30 days">
        <StatTile label="Requests" value="334" detail="11 a day on average" />
        <StatTile label="Tokens" value="27.2K" detail="81 per request" />
        <StatTile label="Cost" value="$0.09" detail="$0.00027 per request" />
        <StatTile label="Logs" value="360 KB" detail="334 entries" />
      </StatGrid>
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Sizes">
        <div className="flex flex-col gap-24">
          <Row />
          <MetricRow>
            <Stat size="sm" value="0" unit="sessions" />
            <Stat size="sm" value="$0.00" unit="average" />
            <Stat size="sm" value="$0.00" unit="total spend" />
          </MetricRow>
        </div>
      </StateSection>
      <StateSection title="Tiles: with a trend, loading one, and without">
        <Tiles />
      </StateSection>
      <StateSection title="Tiles while their numbers load">
        <div className="w-960">
          <StatGrid aria-label="Loading">
            <StatTile label="Requests" value="12,480" trend={[]} loading />
            <StatTile label="Tokens" value="4.2M" trend={[]} loading />
            <StatTile label="Cost" value="$184.20" trend={[]} loading />
            <StatTile label="Errors" value="0.4%" trend={[]} loading />
          </StatGrid>
        </div>
      </StateSection>
      <StateSection title="Five tiles (the last widens over the empty cells), a long label and value">
        <div className="w-960">
          <StatGrid aria-label="Five">
            <StatTile label="Requests" value="12,480" change={{ value: 0.12 }} />
            <StatTile label="Cost" value="$184.20" change={{ value: 0.08, better: "down" }} />
            <StatTile label="Errors" value="0.4%" change={{ value: -0.31, better: "down" }} />
            <StatTile label="Median latency" value="1.2 s" />
            <StatTile
              label="Tokens across every project and every key this month"
              value="1,284,503,112"
              change={{ value: 2.4 }}
              detail="A long label and a long value"
            />
          </StatGrid>
        </div>
      </StateSection>
    </>
  );
}
