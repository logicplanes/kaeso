import { Textarea } from "@logicplanes/kaeso-ui";
import { Matrix } from "~/lib/states";

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
    <Matrix
      title="States"
      rows={[{ label: "filled" }]}
      columns={[
        { label: "Rest", props: {} },
        { label: "Hover", props: { "data-force": "hover" } },
        { label: "Focus", props: { "data-force": "focus" } },
        { label: "Invalid", props: { invalid: true } },
        { label: "Disabled", props: { disabled: true } },
      ]}
      render={(_, p) => (
        <div className="w-240">
          <Textarea aria-label="Notes" defaultValue="Rotate this key every 90 days." {...p} />
        </div>
      )}
    />
  );
}
