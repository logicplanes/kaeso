import { KeyValue, StatusCode } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

const items = [
  { label: "Request id", value: "req_01JAR4X2KQ", mono: true, copy: "req_01JAR4X2KQ" },
  { label: "Status", value: <StatusCode code={200} showText /> },
  { label: "Model", value: "claude-sonnet-4-5", mono: true },
  { label: "Latency", value: "1.84 s" },
];
const ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36";
const route = [
  { label: "Format", value: "OpenAI Responses → Anthropic Messages" },
  { label: "Model", value: "claude-opus-5-5", mono: true },
  { label: "Provider", value: "Anthropic" },
  { label: "Upstream request", value: "req_011Cg1rpXo2L5DAnHsAAaHg2", mono: true, copy: "req_011Cg1rpXo2L5DAnHsAAaHg2" },
];
const sender = [
  { label: "Service account", value: "ci-runner" },
  { label: "Key", value: "ci-runner-prod" },
  { label: "Client IP", value: "198.51.100.17", mono: true },
  { label: "User agent", value: ua, mono: true, copy: ua },
];
const stream = [
  { label: "Events", value: "134" },
  { label: "Bytes sent", value: "55.6 KB" },
  { label: "First byte sent", value: "1.1 s" },
  { label: "Keepalives", value: "0" },
];
export const meta = {
  title: "KeyValue",
  description: "Label and value pairs for sheets: 13 secondary labels, 14 values, copyable, in columns where it's wide.",
  group: "Components",
  entry: "key-value",
  import: "KeyValue",
};
export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  { id: "wrap", title: "Long values that wrap", fn: "Wrap" },
  { id: "grid", title: "Grid: label above value, two or more to a row", fn: "Grid" },
];
export function Basic() {
  return <KeyValue items={items} className="w-400" />;
}
export function Wrap() {
  return (
    <KeyValue
      className="w-400"
      items={[
        { label: "Error", value: "Rate limited" },
        { label: "Message", value: "Rate limit reached: the project's request rate of 600 requests per minute. Retry in 50 seconds.", wrap: true },
        { label: "Request id", value: "req_01JAR4X2KQ", mono: true, copy: "req_01JAR4X2KQ" },
      ]}
    />
  );
}
export function Grid() {
  return (
    <div className="flex w-full max-w-720 flex-col gap-24">
      <KeyValue layout="grid" items={route} />
      <KeyValue layout="grid" items={sender} />
      <KeyValue layout="grid" columns={4} items={stream} />
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Layouts">
        <div className="flex gap-48">
          <Basic />
          <KeyValue items={items} layout="stacked" className="w-240" />
        </div>
      </StateSection>
      <StateSection title="Grid at 720px and 400px">
        <div className="flex flex-col gap-32">
          <div className="w-720">
            <KeyValue
              layout="grid"
              items={[
                ...route,
                { label: "Message", value: "The connection to the provider failed before the response was complete.", wrap: true, span: "row" as const },
              ]}
            />
          </div>
          <div className="w-400">
            <KeyValue layout="grid" items={sender} />
          </div>
        </div>
      </StateSection>
    </>
  );
}
