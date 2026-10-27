import { Textarea } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";
import { FIELD_STATES, StateGrid } from "~/lib/state-grid";

export const meta = {
  title: "Textarea",
  description: "Multi-line text at radius 14: grows to eight lines, then scrolls.",
  group: "Components",
  entry: "input",
  import: "Textarea",
  a11y: ["Takes label, description and error like Input."],
};
export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  { id: "mono", title: "JSON", fn: "Mono" },
];
export function Basic() {
  return (
    <div className="w-400">
      <Textarea label="System prompt" placeholder="You help customers finish checkout." />
    </div>
  );
}
export function Mono() {
  return (
    <div className="w-400">
      <Textarea label="Metadata" mono defaultValue={'{\n  "team": "checkout",\n  "env": "production"\n}'} />
    </div>
  );
}
export function States() {
  return (
    <>
      <StateGrid
        title="States"
        kinds={[
          { label: "Text", id: "text" },
          { label: "Placeholder", id: "empty" },
        ]}
        states={FIELD_STATES}
        render={(k, p) => (
          <div className="w-240">
            {k.id === "text" ? (
              <Textarea aria-label="Notes" rows={2} defaultValue="Rotate this key every 90 days." {...p} />
            ) : (
              <Textarea aria-label="Notes" rows={2} placeholder="You help customers finish checkout." {...p} />
            )}
          </div>
        )}
      />
      <StateSection title="Mono, growing to eight lines then scrolling">
        <div className="flex w-640 flex-col gap-16">
          <Textarea
            label="Request body"
            mono
            defaultValue={
              '{\n  "model": "gpt-5",\n  "input": "Write one sentence about gateways.",\n  "metadata": {\n    "team": "checkout",\n    "env": "production",\n    "region": "eu-west-1",\n    "owner": "lin@northwind.example",\n    "ticket": "OPS-2214"\n  }\n}'
            }
          />
          <Textarea label="System prompt" layout="horizontal" description="Sent before every message." defaultValue="You help customers finish checkout." />
        </div>
      </StateSection>
    </>
  );
}
