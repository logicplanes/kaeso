import { Donut, ShareBar } from "@logicplanes/kaeso-ui";
import { formatUSD } from "@logicplanes/kaeso-ui/format";
import { StateSection } from "~/lib/states";

const slices = [
  { id: "gpt-5", label: "gpt-5", value: 84.2 },
  { id: "claude-sonnet-4-5", label: "claude-sonnet-4-5", value: 61.4 },
  { id: "gpt-5-mini", label: "gpt-5-mini", value: 22.8 },
  { id: "claude-haiku-4-5", label: "claude-haiku-4-5", value: 14.1 },
  { id: "gpt-4.1-nano", label: "gpt-4.1-nano", value: 1.2 },
];
const money = (v: number) => formatUSD(v.toFixed(2));

export const meta = {
  title: "Donut",
  description: "A ring with rounded caps and 2px gaps, the total in the center; ShareBar is the same split in one pill.",
  group: "Charts",
  entry: "charts",
  import: "Donut, ShareBar",
};
export const examples = [
  { id: "donut", title: "Spend by model", fn: "DonutDemo", minHeight: 300 },
  { id: "share", title: "ShareBar", fn: "Share" },
];
export function DonutDemo() {
  return <Donut aria-label="Spend by model" slices={slices} formatValue={money} totalLabel="Total spend" />;
}
export function Share() {
  return (
    <div className="w-480">
      <ShareBar
        aria-label="Requests by provider"
        slices={[
          { id: "openai", label: "OpenAI", value: 7420 },
          { id: "anthropic", label: "Anthropic", value: 5060 },
        ]}
      />
    </div>
  );
}
export function States() {
  return (
    <StateSection title="Sizes">
      <div className="flex flex-wrap items-start gap-48">
        <Donut aria-label="Small" size="sm" slices={slices} formatValue={money} />
        <Donut aria-label="Medium" slices={slices} formatValue={money} />
        <Donut aria-label="Large" size="lg" slices={slices} formatValue={money} />
        <div className="w-400">
          <Share />
        </div>
      </div>
    </StateSection>
  );
}
