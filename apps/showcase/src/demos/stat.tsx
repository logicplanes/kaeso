import { Stat, StatGroup } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Stat",
  description: "A big number followed by its word, no box, or a tile with its label, change and trend. StatGroup lines numbers up in a row or tiles in a grid.",
  group: "Components",
  entry: "stat",
  import: "Stat, StatGroup",
};
export const examples = [
  { id: "row", title: "Metric row", fn: "Row" },
  { id: "tiles", title: "Stat tiles: four, three without cost, totals", fn: "Tiles" },
];
export function Row() {
  return (
    <StatGroup variant="row">
      <Stat value="12,480" unit="requests" />
      <Stat value="$184.20" unit="total spend" delta={{ label: "+12%", tone: "neutral" }} />
      <Stat value="1.2 s" unit="median latency" />
    </StatGroup>
  );
}
const DAYS = [12, 18, 14, 22, 30, 26, 41];
export function Tiles() {
  return (
    <div className="flex flex-col gap-24">
      <StatGroup aria-label="Usage in the period">
        <Stat variant="tile" label="Requests" value="12,480" change={{ value: 0.12 }} detail="vs. the previous 7 days" trend={DAYS} />
        <Stat variant="tile" label="Tokens" value="4.2M" change={{ value: -0.04 }} detail="vs. the previous 7 days" trend={DAYS.map((d) => d * 3)} />
        <Stat variant="tile" label="Cost" value="$184.20" detail="vs. the previous 7 days" trend={[]} />
        <Stat variant="tile" label="Errors" value="38" change={{ value: 0.3, better: "down" }} detail="0.3% of requests" trend={DAYS.map((d) => d % 5)} />
      </StatGroup>
      <StatGroup aria-label="Usage without the cost right">
        <Stat variant="tile" label="Requests" value="12,480" change={{ value: 0.12 }} detail="vs. the previous 7 days" trend={DAYS} />
        <Stat variant="tile" label="Tokens" value="4.2M" change={{ value: -0.04 }} detail="vs. the previous 7 days" trend={DAYS.map((d) => d * 3)} />
        <Stat variant="tile" label="Errors" value="38" change={{ value: 0.3, better: "down" }} detail="0.3% of requests" trend={DAYS.map((d) => d % 5)} />
      </StatGroup>
      <StatGroup aria-label="Usage in the last 30 days">
        <Stat variant="tile" label="Requests" value="334" detail="11 a day on average" />
        <Stat variant="tile" label="Tokens" value="27.2K" detail="81 per request" />
        <Stat variant="tile" label="Cost" value="$0.09" detail="$0.00027 per request" />
        <Stat variant="tile" label="Logs" value="360 KB" detail="334 entries" />
      </StatGroup>
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Sizes">
        <div className="flex w-560 flex-col gap-24">
          <Row />
          <StatGroup variant="row">
            <Stat size="sm" value="0" unit="sessions" />
            <Stat size="sm" value="$0.00" unit="average" />
            <Stat size="sm" value="$0.00" unit="total spend" />
          </StatGroup>
        </div>
      </StateSection>
      <StateSection title="Tiles: a trend, one loading, none, a single day">
        <div className="w-560">
          <StatGroup aria-label="Usage in the period">
            <Stat variant="tile" label="Requests" value="12,480" change={{ value: 0.12 }} detail="vs. the previous 7 days" trend={DAYS} />
            <Stat variant="tile" label="Tokens" value="4.2M" change={{ value: -0.04 }} detail="vs. the previous 7 days" trend={DAYS.map((d) => d * 3)} />
            <Stat variant="tile" label="Cost" value="$184.20" change={{ value: 0 }} detail="vs. the previous 7 days" trend={[]} />
            <Stat variant="tile" label="Errors" value="38" change={{ value: 0.3, better: "down" }} detail="0.3% of requests" trend={[4]} />
          </StatGroup>
        </div>
      </StateSection>
      <StateSection title="Tiles while their numbers load">
        <div className="w-560">
          <StatGroup aria-label="Loading">
            <Stat variant="tile" label="Requests" value="12,480" trend={[]} loading />
            <Stat variant="tile" label="Tokens" value="4.2M" trend={[]} loading />
            <Stat variant="tile" label="Cost" value="$184.20" trend={[]} loading />
            <Stat variant="tile" label="Errors" value="0.4%" trend={[]} loading />
          </StatGroup>
        </div>
      </StateSection>
      <StateSection title="Five tiles, a long label and value; at 320px">
        <div className="flex flex-col gap-24">
          <div className="w-560">
            <StatGroup aria-label="Five">
              <Stat variant="tile" label="Requests" value="12,480" change={{ value: 0.12 }} />
              <Stat variant="tile" label="Cost" value="$184.20" change={{ value: 0.08, better: "down" }} />
              <Stat variant="tile" label="Errors" value="0.4%" change={{ value: -0.31, better: "down" }} />
              <Stat variant="tile" label="Median latency" value="1.2 s" />
              <Stat
                variant="tile"
                label="Tokens across every project and every key this month"
                value="1,284,503,112"
                change={{ value: 2.4 }}
                detail="A long label and a long value"
              />
            </StatGroup>
          </div>
          <div className="w-320">
            <StatGroup aria-label="Narrow">
              <Stat variant="tile" label="Cost" value="$1,284,503,112.40" change={{ value: 0.08, better: "down" }} />
              <Stat variant="tile" label="Requests" value="12,480" />
            </StatGroup>
          </div>
        </div>
      </StateSection>
    </>
  );
}
