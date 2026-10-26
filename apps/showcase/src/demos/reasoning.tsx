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
  const cells = [
    { id: "rest", label: "Rest", force: undefined },
    { id: "hover", label: "Hover", force: "hover" },
    { id: "focus", label: "Focus", force: "focus" },
    { id: "pressed", label: "Pressed", force: "pressed" },
  ];
  return (
    <div className="flex w-640 flex-col">
      <StateSection title="Done, closed">
        <div className="flex flex-col gap-8">
          {cells.map((c) => (
            <div key={c.id} data-cell={`done-${c.id}`} className="flex items-center gap-16">
              <Reasoning text={REASONING} duration={4200} {...(c.force ? { "data-force": c.force } : {})} className="min-w-0 flex-1" />
              <span className="w-80 shrink-0 text-caption text-fg-tertiary">{c.label}</span>
            </div>
          ))}
        </div>
      </StateSection>
      <StateSection title="Thinking, before any text">
        <Reasoning text="" live />
      </StateSection>
      <StateSection title="Thinking: the latest line">
        <Reasoning text={REASONING} live />
      </StateSection>
      <StateSection title="Thinking, open">
        <Reasoning text={REASONING} live defaultOpen />
      </StateSection>
      <StateSection title="Done, open">
        <Reasoning text={REASONING} duration={4200} defaultOpen />
      </StateSection>
      <StateSection title="Under a second">
        <Reasoning text={REASONING} duration={653} />
      </StateSection>
      <StateSection title="Redacted, in a request log">
        <InALog />
      </StateSection>
    </div>
  );
}
