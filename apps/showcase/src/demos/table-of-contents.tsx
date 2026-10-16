import { TableOfContents } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "TableOfContents",
  description: "“On this page”: the heading in view in text color with a 2px ink bar.",
  group: "Components",
  entry: "toc",
  import: "TableOfContents",
};
const items = [
  { id: "installation", label: "Installation" },
  { id: "usage", label: "Usage" },
  { id: "examples", label: "Examples" },
  { id: "sizes", label: "Sizes", depth: 1 as const },
  { id: "api", label: "API reference" },
];
export const examples = [{ id: "basic", title: "Basic", fn: "Basic" }];
export function Basic() {
  return <TableOfContents items={items} active="examples" className="w-200" />;
}
export function States() {
  return (
    <StateSection title="Active item">
      <Basic />
    </StateSection>
  );
}
