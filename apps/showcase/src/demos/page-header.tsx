import { PageHeader, PillTabs, IconButton, Button, icons } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "PageHeader",
  description: "The 52px header: the page title, pill tabs, at most one ink button and round actions.",
  group: "Layout",
  entry: "page-header",
  import: "PageHeader",
};
const tabs = (
  <PillTabs defaultValue="all">
    <PillTabs.List aria-label="Logs">
      <PillTabs.Tab value="all">All</PillTabs.Tab>
      <PillTabs.Tab value="errors">Errors</PillTabs.Tab>
      <PillTabs.Tab value="slow">Slow</PillTabs.Tab>
    </PillTabs.List>
  </PillTabs>
);
const actions = (
  <>
    <IconButton icon={icons.filter} aria-label="Filter" variant="secondary" />
    <IconButton icon={icons.displayOptions} aria-label="Display options" variant="secondary" />
    <IconButton icon={icons.sidePanel} aria-label="Toggle side panel" variant="secondary" />
  </>
);
export const examples = [
  { id: "tabs", title: "Title, tabs and actions", fn: "WithTabs" },
  { id: "primary", title: "A title and a primary action", fn: "WithPrimary" },
];
export function WithTabs() {
  return (
    <div className="w-full rounded-well shadow-frame">
      <PageHeader title="Request logs" icon={icons.requestLog} tabs={tabs} actions={actions} />
    </div>
  );
}
export function WithPrimary() {
  return (
    <div className="w-full rounded-well shadow-frame">
      <PageHeader
        title="Keys"
        icon={icons.key}
        actions={
          <Button variant="primary" size="sm" leadingIcon={icons.add}>
            Create key
          </Button>
        }
      />
    </div>
  );
}
export function WithTabsAndPrimary() {
  return (
    <div className="w-full rounded-well shadow-frame">
      <PageHeader
        title="Projects"
        tabs={
          <PillTabs defaultValue="active">
            <PillTabs.List aria-label="Projects">
              <PillTabs.Tab value="all">All</PillTabs.Tab>
              <PillTabs.Tab value="active">Active</PillTabs.Tab>
            </PillTabs.List>
          </PillTabs>
        }
        actions={
          <Button variant="primary" size="sm" leadingIcon={icons.add}>
            Create project
          </Button>
        }
      />
    </div>
  );
}
export function States() {
  return (
    <StateSection title="Headers">
      <div className="flex w-960 flex-col gap-16">
        <WithTabs />
        <WithPrimary />
        <WithTabsAndPrimary />
      </div>
    </StateSection>
  );
}
