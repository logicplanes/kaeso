import { UsageBreakdown, LimitMeter } from "@logicplanes/kaeso-ui";
import { tokenTotals } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "UsageBreakdown",
  description: "Token kinds in their fixed colors, and limits with their scope, window, kind and reset.",
  group: "Kaeso",
  entry: "kaeso",
  import: "UsageBreakdown, LimitMeter",
  a11y: [
    "Token kinds always map to the same series: input blue, cache read a soft blue edged in blue, cache write amber, output emerald, reasoning violet.",
    "Limits say hard or soft in words, and say so when they are over.",
  ],
};
export const examples = [
  { id: "tokens", title: "UsageBreakdown", fn: "Tokens" },
  { id: "limits", title: "LimitMeter", fn: "Limits" },
];
export function Tokens() {
  return <UsageBreakdown tokens={tokenTotals} className="w-560" />;
}
export function Limits() {
  return (
    <div className="flex w-480 flex-col gap-24">
      <LimitMeter
        kind="spend"
        scope="Checkout assistant"
        window="Per month"
        used={321}
        limit={500}
        valueLabel="$321.00 of $500.00"
        alerts={[0.5, 0.8]}
        resets="Resets in 19 days"
      />
      <LimitMeter kind="requests" scope="Key checkout-web" window="Per minute" used={540} limit={600} valueLabel="540 of 600" resets="Resets every minute" />
      <LimitMeter
        kind="tokens"
        scope="Member Tomás Ruiz"
        window="Per day"
        used={1_050_000}
        limit={1_000_000}
        valueLabel="1.05M of 1M"
        hard={false}
        resets="Resets at 00:00 UTC"
      />
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Tokens">
        <div className="flex w-560 flex-col gap-32">
          <Tokens />
          <UsageBreakdown tokens={{ input: 8_420 }} />
          <UsageBreakdown tokens={{ input: 0, output: 0, cacheRead: 0 }} />
          <UsageBreakdown tokens={{ input: 103_000_000_000, cacheRead: 41_000_000_000, output: 9_800_000_000, reasoning: 4_100_000_000 }} />
        </div>
      </StateSection>
      <StateSection title="Limits">
        <Limits />
      </StateSection>
      <StateSection title="Over a hard limit, a long scope, 320px">
        <div className="flex w-320 flex-col gap-24">
          <LimitMeter
            kind="spend"
            scope="Checkout assistant production worker in eu-west"
            window="Per month"
            used={1200}
            limit={500}
            valueLabel="$1,200.00 of $500.00"
            resets="Resets in 19 days"
          />
          <UsageBreakdown tokens={tokenTotals} />
        </div>
      </StateSection>
    </>
  );
}
