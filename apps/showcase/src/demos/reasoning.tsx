import { Reasoning } from "@logicplanes/kaeso-ui/playground";
import { Text } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

const REASONING =
  "The user wants a short comparison. REST is resource-oriented over HTTP/1.1 with JSON; gRPC uses HTTP/2 and Protocol Buffers. I'll keep it to three bullets: format, contracts, and where each fits.";

export const meta = {
  title: "Reasoning",
  description: "A model's reasoning behind a quiet disclosure: “Thought for 4.2 s” when it is done, “Thinking” with its latest line while it streams.",
  group: "Playground",
  entry: "playground",
  import: "Reasoning",
  barrel: false,
  a11y: ["A disclosure button: Enter or Space opens and closes it, and the reasoning follows it in reading order."],
};
export const examples = [
  { id: "basic", title: "Finished reasoning", fn: "Basic" },
  { id: "log", title: "In a request log", fn: "InALog" },
];
export function Basic() {
  return <Reasoning text={REASONING} duration={4200} />;
}
export function InALog() {
  return (
    <Reasoning title="Reasoning" hint="Redacted by the provider">
      <Text variant="small" tone="secondary">
        The provider redacted this reasoning; only its encrypted form was returned.
      </Text>
    </Reasoning>
  );
}
export function States() {
  return (
    <div className="flex w-640 flex-col">
      <StateSection title="Thinking">
        <Reasoning text={REASONING} live />
      </StateSection>
      <StateSection title="Done, closed">
        <Basic />
      </StateSection>
      <StateSection title="Done, open">
        <Reasoning text={REASONING} duration={4200} defaultOpen />
      </StateSection>
      <StateSection title="Redacted, in a request log">
        <InALog />
      </StateSection>
    </div>
  );
}
