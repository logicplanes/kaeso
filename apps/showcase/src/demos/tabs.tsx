import { Tabs, PillTabs, InsetContent, icons } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Tabs",
  description: "Underline tabs for detail pages and pill tabs for page headers.",
  group: "Components",
  entry: "tabs",
  import: "Tabs, PillTabs",
  keyboard: [
    ["Left Right", "Moves between tabs"],
    ["Home End", "First and last tab"],
  ],
  a11y: ["role=tablist with tabs and panels wired by Base UI."],
};
export const examples = [
  { id: "underline", title: "Underline", fn: "Underline" },
  { id: "pill", title: "Pill tabs", fn: "Pills" },
];
export function Underline() {
  return (
    <Tabs defaultValue="overview" className="w-480">
      <Tabs.List aria-label="Request">
        <Tabs.Tab value="overview">Overview</Tabs.Tab>
        <Tabs.Tab value="conversation">Conversation</Tabs.Tab>
        <Tabs.Tab value="request">Request</Tabs.Tab>
        <Tabs.Tab value="response">Response</Tabs.Tab>
      </Tabs.List>
      <Tabs.Panel value="overview" className="pt-16 text-body text-fg-secondary">
        The gateway answered in 1.2 s.
      </Tabs.Panel>
    </Tabs>
  );
}
export function Pills() {
  return (
    <PillTabs defaultValue="all">
      <PillTabs.List aria-label="Keys">
        <PillTabs.Tab value="all" count={6}>
          All
        </PillTabs.Tab>
        <PillTabs.Tab value="active" count={3}>
          Active
        </PillTabs.Tab>
        <PillTabs.Tab value="disabled">Disabled</PillTabs.Tab>
        <PillTabs.Tab value="archived" icon={icons.stack}>
          Archived
        </PillTabs.Tab>
      </PillTabs.List>
    </PillTabs>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Pill tab states">
        <PillTabs defaultValue="a">
          <PillTabs.List aria-label="States">
            <PillTabs.Tab value="a">Selected</PillTabs.Tab>
            <PillTabs.Tab value="b">Rest</PillTabs.Tab>
            <PillTabs.Tab value="c" data-force="hover">
              Hover
            </PillTabs.Tab>
            <PillTabs.Tab value="d" data-force="focus">
              Focus
            </PillTabs.Tab>
            <PillTabs.Tab value="e" disabled>
              Disabled
            </PillTabs.Tab>
          </PillTabs.List>
        </PillTabs>
      </StateSection>
      <StateSection title="Pill tabs in a settings column: the first label on the title's edge">
        <div className="w-480 bg-panel p-24">
          <h2 className="mb-16 text-title-lg font-semibold text-fg">Keys</h2>
          <InsetContent>
            <PillTabs defaultValue="all">
              <PillTabs.List aria-label="Scope">
                <PillTabs.Tab value="all">All</PillTabs.Tab>
                <PillTabs.Tab value="active" data-force="hover">
                  Active
                </PillTabs.Tab>
              </PillTabs.List>
            </PillTabs>
          </InsetContent>
        </div>
      </StateSection>
      <StateSection title="Underline tab states">
        <Tabs defaultValue="a" className="w-560">
          <Tabs.List aria-label="States">
            <Tabs.Tab value="a">Selected</Tabs.Tab>
            <Tabs.Tab value="b">Rest</Tabs.Tab>
            <Tabs.Tab value="c" data-force="hover">
              Hover
            </Tabs.Tab>
            <Tabs.Tab value="d" data-force="focus">
              Focus
            </Tabs.Tab>
            <Tabs.Tab value="e" disabled>
              Disabled
            </Tabs.Tab>
          </Tabs.List>
        </Tabs>
      </StateSection>
    </>
  );
}
