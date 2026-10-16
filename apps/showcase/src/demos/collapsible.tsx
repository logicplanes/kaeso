import { Collapsible, Accordion } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Collapsible",
  description: "Disclosure rows with a chevron that turns in 180ms and a measured height animation; Accordion groups them.",
  group: "Components",
  entry: "collapsible",
  import: "Collapsible, Accordion",
  keyboard: [["Enter Space", "Opens and closes"]],
};
export const examples = [
  { id: "basic", title: "Collapsible", fn: "Basic" },
  { id: "accordion", title: "Accordion", fn: "AccordionDemo" },
];
export function Basic() {
  return (
    <div className="w-480">
      <Collapsible title="Advanced" hint="2 settings">
        <p className="text-body text-fg-secondary">Timeout overrides and the egress dialect.</p>
      </Collapsible>
    </div>
  );
}
export function AccordionDemo() {
  return (
    <Accordion
      className="w-480"
      defaultValue={["retention"]}
      items={[
        { value: "retention", title: "How long are logs kept?", content: "The organization sets retention, 30 days by default." },
        { value: "content", title: "Is request content stored?", content: "Only when content logging is on for the project." },
      ]}
    />
  );
}
export function States() {
  return (
    <StateSection title="Open and closed">
      <div className="flex flex-col gap-24">
        <Basic />
        <div className="w-480">
          <Collapsible title="Open" defaultOpen>
            <p className="text-body text-fg-secondary">Visible content.</p>
          </Collapsible>
        </div>
        <AccordionDemo />
      </div>
    </StateSection>
  );
}
