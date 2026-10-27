import { Donut, ShareBar } from "@logicplanes/kaeso-ui";
import { formatUSD } from "@logicplanes/kaeso-ui/format";
import { StateSection } from "~/lib/states";
import { Labelled } from "~/lib/chart-states";

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
const many = [
  ...slices,
  { id: "o3", label: "o3", value: 9.6 },
  { id: "gemini-2.5-pro", label: "gemini-2.5-pro", value: 7.4 },
  { id: "deepseek-chat", label: "deepseek-chat", value: 5.2 },
  { id: "grok-4", label: "grok-4", value: 4.8 },
  { id: "mistral-large", label: "mistral-large", value: 3.9 },
];
const long = [
  { id: "claude-sonnet-4-5-20250929", label: "claude-sonnet-4-5-20250929 (checkout-worker-production)", value: 1284.5 },
  { id: "gpt-5", label: "gpt-5", value: 938.25 },
];

export function States() {
  return (
    <>
      <StateSection title="Sizes">
        <div className="flex w-640 flex-col gap-40">
          <Donut aria-label="Small" size="sm" slices={slices} formatValue={money} />
          <Donut aria-label="Medium" slices={slices} formatValue={money} />
          <Donut aria-label="Large" size="lg" slices={slices} formatValue={money} />
        </div>
      </StateSection>
      <StateSection title="Many slices, long names, a huge total, 320px">
        <div className="flex w-640 flex-col gap-40">
          <Donut aria-label="Spend by model, twelve models" slices={many} formatValue={money} />
          <Donut aria-label="Spend by model, long names" slices={long} formatValue={money} />
          <Donut
            aria-label="Spend, lifetime"
            slices={[
              { id: "a", label: "Checkout assistant", value: 1_684_220.71 },
              { id: "b", label: "Support triage", value: 537_001.5 },
            ]}
            formatValue={money}
          />
          <div className="w-320">
            <Donut aria-label="Spend by model, 320px" slices={slices} formatValue={money} />
          </div>
        </div>
      </StateSection>
      <StateSection title="One slice, empty, loading, error">
        <div className="grid w-640 grid-cols-2 gap-x-24 gap-y-40">
          <Labelled title="One slice">
            <Donut aria-label="Spend, one model" size="sm" slices={[{ id: "gpt-5", label: "gpt-5", value: 84.2 }]} formatValue={money} />
          </Labelled>
          <Labelled title="Nothing spent">
            <Donut aria-label="Spend, nothing" size="sm" slices={slices.map((s) => ({ ...s, value: 0 }))} formatValue={money} />
          </Labelled>
          <Labelled title="Loading">
            <Donut aria-label="Spend, loading" size="sm" slices={[]} loading />
          </Labelled>
          <Labelled title="Error">
            <Donut aria-label="Spend, failed" size="sm" slices={slices} error onRetry={() => undefined} />
          </Labelled>
        </div>
      </StateSection>
      <StateSection title="ShareBar: three equal parts, without a legend">
        <div className="flex w-400 flex-col gap-24">
          <Share />
          <ShareBar
            aria-label="Requests by region"
            slices={[
              { id: "eu", label: "EU", value: 1200 },
              { id: "us", label: "US", value: 1200 },
              { id: "ap", label: "Asia Pacific", value: 1200 },
            ]}
          />
          <ShareBar
            aria-label="Requests by provider, no legend"
            legend={false}
            slices={[
              { id: "openai", label: "OpenAI", value: 7420 },
              { id: "anthropic", label: "Anthropic", value: 5060 },
            ]}
          />
        </div>
      </StateSection>
    </>
  );
}
