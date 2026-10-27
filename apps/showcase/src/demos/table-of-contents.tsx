import { TableOfContents } from "@logicplanes/kaeso-ui";
import { StateSection, forceIn } from "~/lib/states";

export const meta = {
  title: "TableOfContents",
  description: "“On this page”: the heading in view in text color with a 2px ink bar on the rail.",
  group: "Components",
  entry: "toc",
  import: "TableOfContents",
  a11y: [
    "A navigation landmark named by its title; the heading in view has aria-current=location.",
    "Links are 32px tall, 44px with a coarse pointer; the rail and the bar sit on the start side, so they move to the right in right-to-left pages.",
  ],
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
    <>
      <StateSection title="Active, hovered and focused">
        <div className="flex gap-32">
          <TableOfContents items={items} active="examples" className="w-200" />
          <div ref={forceIn('a[href="#usage"]', "hover")}>
            <TableOfContents items={items} active="sizes" className="w-200" />
          </div>
          <div ref={forceIn('a[href="#api"]', "focus")}>
            <TableOfContents items={items} active="installation" className="w-160" />
          </div>
        </div>
      </StateSection>
      <StateSection title="A label that wraps, and right to left">
        <div className="flex gap-32">
          <TableOfContents
            items={[...items.slice(0, 2), { id: "routing", label: "Routing requests to the cheapest provider that serves the model" }]}
            active="routing"
            className="w-200"
          />
          <div dir="rtl">
            <TableOfContents items={items} active="usage" className="w-200" />
          </div>
        </div>
      </StateSection>
    </>
  );
}
