import { Heading } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Heading",
  description: "Headings on the title tokens, balanced across lines.",
  group: "Components",
  entry: "heading",
  import: "Heading",
  a11y: ["The document level follows from the size; set level when the outline needs another one."],
};
export const examples = [{ id: "sizes", title: "Sizes", fn: "Sizes" }];
export function Sizes() {
  return (
    <div className="flex flex-col gap-16">
      <Heading size="display">Your gateway</Heading>
      <Heading size="title-lg">Preferences</Heading>
      <Heading size="title">No keys yet</Heading>
      <Heading size="title-sm">Interface and theme</Heading>
    </div>
  );
}
export function States() {
  return (
    <StateSection title="Sizes and balance">
      <div className="flex w-480 flex-col gap-16">
        <Sizes />
        <Heading size="title">A long heading wraps into balanced lines instead of leaving one word alone</Heading>
      </div>
    </StateSection>
  );
}
