import { useState } from "react";
import { ResponseMetrics } from "@logicplanes/kaeso-ui/playground";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "ResponseMetrics",
  description:
    "A response's numbers on one line: time to first token, total time, tokens in and out, reasoning, cost. Empty before a response; quiet placeholders hold their place while one streams.",
  group: "Playground",
  entry: "playground",
  import: "ResponseMetrics, formatLatency",
  barrel: false,
  a11y: ["A description list: each label is its term. Tokens read as “1,204 in, 856 out”."],
};
export const examples = [
  { id: "basic", title: "A finished response", fn: "Basic" },
  { id: "streaming", title: "While it streams", fn: "Streaming" },
];
export function Basic() {
  return <ResponseMetrics timeToFirstToken={420} duration={3140} inputTokens={1204} outputTokens={856} reasoningTokens={312} cost="0.009725" />;
}
export function Streaming() {
  return <ResponseMetrics live reasoning timeToFirstToken={1180} />;
}
function Counting() {
  const [startedAt] = useState(() => Date.now() - 2_400);
  return <ResponseMetrics live reasoning startedAt={startedAt} timeToFirstToken={1180} />;
}
export function States() {
  return (
    <div className="flex w-640 flex-col">
      <StateSection title="Before a response: empty, its height kept">
        <ResponseMetrics />
      </StateSection>
      <StateSection title="Waiting for the first token">
        <ResponseMetrics live reasoning />
      </StateSection>
      <StateSection title="First token">
        <Streaming />
      </StateSection>
      <StateSection title="First token, Total counting">
        <Counting />
      </StateSection>
      <StateSection title="Done">
        <Basic />
      </StateSection>
      <StateSection title="Done, no reasoning">
        <ResponseMetrics timeToFirstToken={280} duration={1460} inputTokens={98} outputTokens={41} cost="0.000026" />
      </StateSection>
      <StateSection title="Done, reasoning not reported">
        <ResponseMetrics reasoning timeToFirstToken={2920} duration={4380} inputTokens={412} outputTokens={168} cost="0.003756" />
      </StateSection>
      <StateSection title="Slow and long">
        <ResponseMetrics timeToFirstToken={12_400} duration={71_000} inputTokens={182_400} outputTokens={12_031} reasoningTokens={9_870} cost="1.2311" />
      </StateSection>
      <StateSection title="Free, and below a millionth of a dollar">
        <div className="flex flex-col gap-8">
          <ResponseMetrics timeToFirstToken={310} duration={980} inputTokens={64} outputTokens={22} cost="0" />
          <ResponseMetrics timeToFirstToken={190} duration={640} inputTokens={12} outputTokens={3} cost="0.0000004" />
        </div>
      </StateSection>
      <StateSection title="Failed: nothing to show">
        <ResponseMetrics live={false} />
      </StateSection>
      <StateSection title="Medium">
        <ResponseMetrics size="md" timeToFirstToken={420} duration={3140} inputTokens={1204} outputTokens={856} cost="0.009725" />
      </StateSection>
      <StateSection title="Medium, while it streams">
        <ResponseMetrics size="md" live reasoning timeToFirstToken={420} />
      </StateSection>
      <StateSection title="In a narrow column">
        <div className="w-240">
          <Basic />
        </div>
      </StateSection>
    </div>
  );
}
