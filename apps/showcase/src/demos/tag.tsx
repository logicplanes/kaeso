import { useState } from "react";
import { Tag, icons } from "@logicplanes/kaeso-ui";
import { Matrix, StateSection } from "~/lib/states";

export const meta = {
  title: "Tag",
  description: "Metadata pills: 28px, raised and ringed, with an optional icon and remove circle.",
  group: "Components",
  entry: "tag",
  import: "Tag",
  a11y: ["Clickable tags are buttons; toggle pills carry aria-pressed.", "Dots support the word; they never replace it."],
};
export const examples = [
  { id: "meta", title: "Metadata", fn: "Meta" },
  { id: "toggle", title: "Toggle pills", fn: "Toggle" },
];
export function Meta() {
  return (
    <div className="flex flex-wrap items-center gap-6">
      <Tag>gpt-5</Tag>
      <Tag>claude-sonnet-4-5</Tag>
      <Tag icon={icons.calendar}>Oct 12</Tag>
      <Tag>Active</Tag>
      <Tag onRemove={() => undefined}>checkout</Tag>
    </div>
  );
}
export function Toggle() {
  const [on, setOn] = useState(["model", "cost"]);
  return (
    <div className="flex flex-wrap gap-6">
      {["model", "key", "latency", "cost"].map((p) => (
        <Tag
          key={p}
          size="sm"
          variant={on.includes(p) ? "soft" : "outline"}
          pressed={on.includes(p)}
          onClick={() => setOn((x) => (x.includes(p) ? x.filter((y) => y !== p) : [...x, p]))}
        >
          {p}
        </Tag>
      ))}
    </div>
  );
}
export function States() {
  return (
    <>
      <Matrix
        title="Clickable"
        rows={[{ label: "raised" }, { label: "outline" }, { label: "soft" }]}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Focus", props: { "data-force": "focus" } },
          { label: "Invalid", props: { invalid: true } },
        ]}
        render={(r, p) => (
          <Tag variant={r.label as "raised"} onClick={() => undefined} {...p}>
            gpt-5
          </Tag>
        )}
      />
      <StateSection title="Content">
        <Meta />
      </StateSection>
    </>
  );
}
