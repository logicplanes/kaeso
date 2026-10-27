import { Collapsible, Accordion } from "@logicplanes/kaeso-ui";
import { StateCell, StateMatrix, StateRow, StateSection } from "~/lib/states";

export const meta = {
  title: "Collapsible",
  description:
    "Disclosure rows: a pill row with the title 12px in, a chevron at its end that turns in 200ms, and a measured height animation; Accordion groups them.",
  group: "Components",
  entry: "collapsible",
  import: "Collapsible, Accordion",
  keyboard: [["Enter Space", "Opens and closes"]],
  a11y: [
    "The title is a button with aria-expanded; its content follows it in the reading order.",
    "The same row recipe as the sidebar's groups: a pill that fills on hover, a focus ring inside it, 44px tall with a coarse pointer.",
  ],
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
const ROW = ["Rest", "Hover", "Pressed", "Focus"];
const FORCE = [undefined, "hover", "pressed", "focus"] as const;

function Forced({ force, open }: { force: (typeof FORCE)[number]; open?: boolean }) {
  return (
    <div
      className="w-200"
      ref={(el) => {
        if (force) el?.querySelector("button")?.setAttribute("data-force", force);
      }}
    >
      <Collapsible title="Advanced" hint="2 settings" defaultOpen={open}>
        <p className="text-body text-fg-secondary">Timeouts</p>
      </Collapsible>
    </div>
  );
}

export function States() {
  return (
    <>
      <StateMatrix title="Collapsible" columns={["Closed", "Open"]}>
        {FORCE.map((f, i) => (
          <StateRow key={i} label={ROW[i]!}>
            <StateCell id={`closed-${f ?? "rest"}`} className="items-start">
              <Forced force={f} />
            </StateCell>
            <StateCell id={`open-${f ?? "rest"}`} className="items-start">
              <Forced force={f} open />
            </StateCell>
          </StateRow>
        ))}
      </StateMatrix>
      <StateSection title="A title that wraps, in a 320px column">
        <div className="w-320">
          <Collapsible title="Timeout overrides, retries and the egress dialect for every provider" hint="6 settings" defaultOpen>
            <p className="text-body text-fg-secondary">Timeout overrides and the egress dialect.</p>
          </Collapsible>
        </div>
      </StateSection>
      <StateSection title="Accordion, several open">
        <Accordion
          className="w-full max-w-480"
          multiple
          defaultValue={["retention", "content"]}
          items={[
            { value: "retention", title: "How long are logs kept?", content: "The organization sets retention, 30 days by default.", hint: "30 days" },
            { value: "content", title: "Is request content stored?", content: "Only when content logging is on for the project." },
            { value: "export", title: "Can logs be exported?", content: "As CSV or JSON Lines, up to 100,000 rows at a time." },
          ]}
        />
      </StateSection>
    </>
  );
}
