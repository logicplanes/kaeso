import type { ReactNode } from "react";
import { AppFrame, Sidebar, PageHeader, PillTabs, IconButton, Avatar, Empty, Button, FilterRow, List, Badge, Select, icons } from "@logicplanes/kaeso-ui";
import { members } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "AppFrame",
  description: "The console frame: the sidebar on the grey chrome, content in an inset rounded panel that scrolls by itself.",
  group: "Layout",
  entry: "app-frame",
  import: "AppFrame, NavSheet",
  a11y: [
    "The sidebar is a nav landmark and the panel is main.",
    "The first Tab stop is Skip to content, shown only while it has focus; it moves the focus to the panel, so the next Tab reaches the page's first control without passing the navigation.",
    "Below 1024px a bar above the panel shows the wordmark alone at the top left, 28px in and 28px down, and the navigation toggle at the right; the toggle is named Open navigation, says whether the sheet is open (aria-expanded), and takes the focus back when the sheet closes.",
    "A collapsed sidebar peeks when its expand button is held under the pointer for 750ms.",
  ],
};
export function Frame({ settings = false, page }: { settings?: boolean; page?: ReactNode }) {
  return (
    <div className="h-560 w-full overflow-hidden rounded-frame shadow-frame">
      <Sidebar.Provider shortcut={false}>
        <AppFrame
          fill="parent"
          sidebar={
            <Sidebar aria-label="Main">
              <Sidebar.Brand>
                <Sidebar.CollapseToggle />
              </Sidebar.Brand>
              <Sidebar.Content>
                {settings ? (
                  <>
                    <Sidebar.BackLink href="#" />
                    <Sidebar.Group label="Account">
                      <Sidebar.Item icon={icons.member} active>
                        Preferences
                      </Sidebar.Item>
                      <Sidebar.Item icon={icons.security}>Security</Sidebar.Item>
                    </Sidebar.Group>
                  </>
                ) : (
                  <>
                    <Sidebar.Group>
                      <Sidebar.Item icon={icons.overview}>Overview</Sidebar.Item>
                      <Sidebar.Item icon={icons.requestLog} active count={128}>
                        Request logs
                      </Sidebar.Item>
                      <Sidebar.Item icon={icons.usage}>Usage</Sidebar.Item>
                    </Sidebar.Group>
                    <Sidebar.Group label="Checkout assistant" id="project">
                      <Sidebar.Item icon={icons.key}>Keys</Sidebar.Item>
                      <Sidebar.Item icon={icons.model}>Models</Sidebar.Item>
                      <Sidebar.Item icon={icons.limit}>Limits</Sidebar.Item>
                    </Sidebar.Group>
                  </>
                )}
              </Sidebar.Content>
            </Sidebar>
          }
        >
          {page ??
            (settings ? (
              <AppFrame.Settings title="Preferences">
                <p className="mt-24 text-body text-fg-secondary">Settings sections go here.</p>
              </AppFrame.Settings>
            ) : (
              <>
                <PageHeader
                  start={<AppFrame.MenuButton />}
                  title="Request logs"
                  tabs={
                    <PillTabs defaultValue="all">
                      <PillTabs.List aria-label="Logs">
                        <PillTabs.Tab value="all">All</PillTabs.Tab>
                        <PillTabs.Tab value="errors">Errors</PillTabs.Tab>
                      </PillTabs.List>
                    </PillTabs>
                  }
                  actions={<IconButton icon={icons.displayOptions} aria-label="Display options" variant="secondary" />}
                />
                <AppFrame.Body padding="page">
                  <Empty
                    title="No requests yet"
                    description="Send a request with a key and it appears here."
                    actions={<Button variant="primary">Create key</Button>}
                  />
                </AppFrame.Body>
              </>
            ))}
        </AppFrame>
      </Sidebar.Provider>
    </div>
  );
}
export const examples = [
  { id: "app", title: "App", fn: "App", minHeight: 600 },
  { id: "settings", title: "Settings mode: a settings list", fn: "SettingsMode", minHeight: 600 },
];
export function App() {
  return <Frame />;
}
export function SettingsMode() {
  return <Frame settings page={<MembersPage />} />;
}
function MembersPage({ empty = false }: { empty?: boolean }) {
  return (
    <AppFrame.Settings
      title="Members"
      description="People in the organization and their rights in it."
      actions={
        <Button size="sm" variant="primary" leadingIcon={icons.add}>
          Invite member
        </Button>
      }
    >
      <FilterRow
        tabs={
          <PillTabs defaultValue={empty ? "suspended" : "all"}>
            <PillTabs.List aria-label="State">
              <PillTabs.Tab value="all">All</PillTabs.Tab>
              <PillTabs.Tab value="active">Active</PillTabs.Tab>
              <PillTabs.Tab value="suspended">Suspended</PillTabs.Tab>
            </PillTabs.List>
          </PillTabs>
        }
      >
        <Select
          aria-label="Preset"
          size="sm"
          defaultValue="all"
          options={[{ value: "all", label: "Every preset" }, ...["Owner", "Admin", "Developer", "Viewer"].map((p) => ({ value: p, label: p }))]}
        />
      </FilterRow>
      <List
        aria-label="Members"
        items={
          empty
            ? []
            : members.map((m) => ({
                id: m.id,
                title: m.name,
                subtitle: m.email,
                leading: <Avatar name={m.name} size={20} decorative />,
                meta: <Badge>{m.preset}</Badge>,
              }))
        }
        empty={<Empty size="md" title="No suspended members" description="Suspended members can't sign in until an owner restores them." />}
      />
    </AppFrame.Settings>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Frames">
        <div className="flex w-1200 flex-col gap-24">
          <Frame />
          <Frame settings />
        </div>
      </StateSection>
      <StateSection title="Settings lists: header action on the title's line, filter row, list card, empty card">
        <div className="flex w-1200 flex-col gap-24">
          <Frame settings page={<MembersPage />} />
          <Frame settings page={<MembersPage empty />} />
        </div>
      </StateSection>
    </>
  );
}
