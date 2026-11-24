import { SOURCE_NAMES, SourceBadge } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "SourceBadge",
  description: "A source's client, such as Claude Code, Codex or LiteLLM, as a tile with its mark, and its name beside it.",
  group: "Kaeso",
  entry: "kaeso",
  import: "SourceBadge",
  a11y: [
    "The tile is an image named after the source; beside its name it hides from assistive technology.",
    "Marks keep their makers' colours, in shades that stay legible on the tile in light and dark; black-and-white marks take the text colour.",
  ],
};
export const examples = [
  { id: "kinds", title: "Kinds", fn: "Kinds" },
  { id: "sizes", title: "Sizes", fn: "Sizes" },
];

export function Kinds() {
  return (
    <div className="grid grid-cols-2 gap-12">
      {Object.keys(SOURCE_NAMES).map((kind) => (
        <SourceBadge key={kind} kind={kind} withName />
      ))}
    </div>
  );
}

export function Sizes() {
  return (
    <div className="flex items-center gap-12">
      <SourceBadge kind="claude_code" size="sm" />
      <SourceBadge kind="claude_code" />
      <SourceBadge kind="claude_code" size="lg" />
      <SourceBadge kind="claude_code" size="xl" />
    </div>
  );
}

export function States() {
  return (
    <StateSection title="Beside a source's own name">
      <div className="flex flex-col gap-8">
        <SourceBadge kind="claude_code" name="Claude Code, Ada's laptop" withName />
        <SourceBadge kind="litellm" name="LiteLLM proxy, production" withName />
        <SourceBadge kind="opentelemetry" name="Support agent" withName />
      </div>
    </StateSection>
  );
}
