import { Radio, RadioGroup, RadioCards } from "@logicplanes/kaeso-ui";
import { Matrix, StateSection } from "~/lib/states";

export const meta = {
  title: "Radio",
  description: "One of a few choices: an ink donut around a panel center; RadioCards for choices with descriptions.",
  group: "Components",
  entry: "radio",
  import: "Radio, RadioGroup, RadioCards",
  keyboard: [
    ["Up Down", "Moves between choices"],
    ["Space", "Selects"],
  ],
  a11y: ["Arrow keys move and select within the group; Tab leaves it."],
};
export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  { id: "cards", title: "RadioCards", fn: "Cards" },
];
export function Basic() {
  return (
    <RadioGroup defaultValue="30" aria-label="Retention">
      <Radio value="7" label="7 days" />
      <Radio value="30" label="30 days" />
      <Radio value="90" label="90 days" />
    </RadioGroup>
  );
}
export function Cards() {
  return (
    <div className="w-480">
      <RadioCards defaultValue="responses" aria-label="OpenAI dialect">
        <Radio value="responses" label="Responses" description="The default. Reasoning and tools work best here." />
        <Radio value="chat" label="Chat Completions" description="For models or proxies that only speak the older API." />
      </RadioCards>
    </div>
  );
}
export function States() {
  return (
    <>
      <Matrix
        title="Circle"
        rows={[{ label: "unselected" }, { label: "selected" }]}
        render={(r, p) => (
          <RadioGroup defaultValue={r.label === "selected" ? "a" : ""} aria-label="Example">
            <Radio value="a" aria-label="Option" {...p} />
          </RadioGroup>
        )}
      />
      <StateSection title="Cards">
        <Cards />
      </StateSection>
    </>
  );
}
