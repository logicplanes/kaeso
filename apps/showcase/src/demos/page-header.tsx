import { PageHeader, Tabs, Button, icons } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "PageHeader",
  description: "The 52px header: the page title, pill tabs, at most one ink button and round actions.",
  group: "Layout",
  entry: "page-header",
  import: "PageHeader",
};
const tabs = (
  <Tabs variant="pill" defaultValue="all">
    <Tabs.List aria-label="Logs">
      <Tabs.Tab value="all">All</Tabs.Tab>
      <Tabs.Tab value="errors">Errors</Tabs.Tab>
      <Tabs.Tab value="slow">Slow</Tabs.Tab>
    </Tabs.List>
  </Tabs>
);
const actions = (
  <>
    <Button size="sm" icon={icons.filter} aria-label="Filter" variant="secondary" />
    <Button size="sm" icon={icons.displayOptions} aria-label="Display options" variant="secondary" />
    <Button size="sm" icon={icons.sidePanel} aria-label="Toggle side panel" variant="secondary" />
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
          <Tabs variant="pill" defaultValue="active">
            <Tabs.List aria-label="Projects">
              <Tabs.Tab value="all">All</Tabs.Tab>
              <Tabs.Tab value="active">Active</Tabs.Tab>
            </Tabs.List>
          </Tabs>
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
    <>
      <StateSection title="Headers">
        <div className="flex w-640 flex-col gap-16">
          <WithTabs />
          <WithPrimary />
          <WithTabsAndPrimary />
        </div>
      </StateSection>
      <StateSection title="A long German title shrinks first and shows in full on hover and focus">
        <div className="w-480 rounded-well shadow-frame">
          <PageHeader
            title="Anfrageprotokolle für alle Projekte dieser Organisation"
            tabs={
              <Tabs variant="pill" defaultValue="all">
                <Tabs.List aria-label="Protokolle">
                  <Tabs.Tab value="all">Alle</Tabs.Tab>
                  <Tabs.Tab value="errors">Fehler</Tabs.Tab>
                </Tabs.List>
              </Tabs>
            }
            actions={<Button size="sm" icon={icons.filter} aria-label="Filtern" variant="secondary" />}
          />
        </div>
      </StateSection>
    </>
  );
}
