import { useState } from "react";
import { Sidebar, AppFrame, PageHeader, Avatar, Badge, Button, Menu, icons, type IconData } from "@logicplanes/kaeso-ui";
import { members, org, projects } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Sidebar",
  description: "The console navigation: the place it is in, 32px items with pill fills, groups that remember, a quiet resize edge, collapse and rail.",
  group: "Layout",
  entry: "sidebar",
  import: "Sidebar",
  keyboard: [
    ["[", "Collapses and expands"],
    ["Left Right", "Resize, on the edge"],
    ["Enter", "Collapse, on the edge"],
  ],
  a11y: [
    "The current page has aria-current=page.",
    "In rail mode labels move into tooltips and become accessible names.",
    "A collapsed sidebar is inert, so its links leave the tab order.",
  ],
};

type Page = { id: string; label: string; icon: IconData; count?: number };
const general: Page[] = [
  { id: "overview", label: "Overview", icon: icons.overview },
  { id: "logs", label: "Request logs", icon: icons.requestLog, count: 128 },
  { id: "usage", label: "Usage", icon: icons.usage },
];
const project: Page[] = [
  { id: "keys", label: "Keys", icon: icons.key, count: 6 },
  { id: "models", label: "Models", icon: icons.model },
  { id: "limits", label: "Limits", icon: icons.limit },
];
const organization: Page[] = [
  { id: "members", label: "Members", icon: icons.members, count: 5 },
  { id: "audit", label: "Audit log", icon: icons.audit },
  { id: "settings", label: "Settings", icon: icons.settings },
];
const all = [...general, ...project, ...organization];

function Items({ pages, current, onSelect }: { pages: Page[]; current: string; onSelect: (id: string) => void }) {
  return pages.map((p) => (
    <Sidebar.Item key={p.id} icon={p.icon} active={current === p.id} onClick={() => onSelect(p.id)} {...(p.count === undefined ? {} : { count: p.count })}>
      {p.label}
    </Sidebar.Item>
  ));
}

export function OrgSwitcher() {
  const me = members[0]!;
  return (
    <Sidebar.Switcher
      avatar={<Avatar name={org.name} kind="org" size={24} decorative />}
      label={org.name}
      header={<Menu.Header avatar={<Avatar name={me.name} size={40} decorative />} name={me.name} email={me.email} />}
    >
      <Menu.Group label="Organizations">
        <Menu.Item icon={icons.check}>{org.name}</Menu.Item>
        <Menu.Item>Southwind Retail</Menu.Item>
      </Menu.Group>
      <Menu.Separator />
      <Menu.Item icon={icons.settings} shortcut={[["g"], ["s"]]}>
        Organization settings
      </Menu.Item>
      <Menu.Item icon={icons.member}>Account</Menu.Item>
      <Menu.Separator />
      <Menu.Item icon={icons.signOut}>Sign out</Menu.Item>
    </Sidebar.Switcher>
  );
}

export function ProjectPlace({ force }: { force?: string }) {
  const [current, setCurrent] = useState(projects[0]!.id);
  const name = projects.find((p) => p.id === current)!.name;
  return (
    <Sidebar.Place
      avatar={<Avatar name={name} kind="org" size={20} decorative />}
      label={name}
      aria-label={`${name}: switch project`}
      {...(force ? { "data-force": force } : {})}
    >
      <Menu.Group label="Projects">
        <Menu.RadioGroup value={current} onValueChange={setCurrent}>
          {projects.map((p, i) => (
            <Menu.RadioItem
              key={p.id}
              value={p.id}
              leading={<Avatar name={p.name} kind="org" size={20} decorative />}
              {...(i === 0 ? { hint: "Default" } : {})}
            >
              {p.name}
            </Menu.RadioItem>
          ))}
        </Menu.RadioGroup>
      </Menu.Group>
      <Menu.Separator />
      <Menu.Item icon={icons.project}>All projects</Menu.Item>
    </Sidebar.Place>
  );
}

function Nav({ current, onSelect, rail = false }: { current: string; onSelect: (id: string) => void; rail?: boolean }) {
  return (
    <Sidebar aria-label="Console">
      <Sidebar.Brand>{rail ? <Sidebar.RailToggle /> : <Sidebar.CollapseToggle />}</Sidebar.Brand>
      <Sidebar.Content>
        <Sidebar.Group>
          <Items pages={general} current={current} onSelect={onSelect} />
        </Sidebar.Group>
        <div className="flex flex-col gap-4">
          <ProjectPlace />
          <Sidebar.Group>
            <Items pages={project} current={current} onSelect={onSelect} />
          </Sidebar.Group>
        </div>
        <Sidebar.Group label="Organization" id="organization">
          <Items pages={organization} current={current} onSelect={onSelect} />
        </Sidebar.Group>
      </Sidebar.Content>
      <Sidebar.Footer>
        <OrgSwitcher />
      </Sidebar.Footer>
    </Sidebar>
  );
}

function Console({ rail = false }: { rail?: boolean }) {
  const [current, setCurrent] = useState("logs");
  const title = all.find((p) => p.id === current)!.label;
  return (
    <div className="h-520 w-full overflow-hidden rounded-frame shadow-frame">
      <Sidebar.Provider shortcut={false} defaultRail={rail}>
        <AppFrame fill="parent" sidebar={<Nav current={current} onSelect={setCurrent} rail={rail} />}>
          <PageHeader start={<AppFrame.MenuButton />} title={title} />
          <AppFrame.Body />
        </AppFrame>
      </Sidebar.Provider>
    </div>
  );
}

export const examples = [
  {
    id: "console",
    title: "In the console",
    description:
      "Click an item to move the current page. Collapse with the button in the header, or click the right edge; the button in the page header brings it back, and resting on it peeks.",
    fn: "Expanded",
    minHeight: 560,
  },
  { id: "rail", title: "Rail", description: "Icons only, with labels in tooltips. For narrow tools that keep navigation in view.", fn: "Rail", minHeight: 560 },
];
export function Expanded() {
  return <Console />;
}
export function Rail() {
  return <Console rail />;
}

function DeletionOrgs() {
  const [current, setCurrent] = useState(org.id);
  return (
    <Menu.Group label="Organizations">
      <Menu.RadioGroup value={current} onValueChange={setCurrent}>
        <Menu.RadioItem
          value={org.id}
          leading={<Avatar name={org.name} kind="org" size={20} decorative />}
          description="Deletion on 20 Oct"
          descriptionTone="danger"
        >
          {org.name}
        </Menu.RadioItem>
        <Menu.RadioItem value="org_01J9Z7" leading={<Avatar name="Southwind Retail" kind="org" size={20} decorative />}>
          Southwind Retail
        </Menu.RadioItem>
      </Menu.RadioGroup>
    </Menu.Group>
  );
}

function Still({
  rail = false,
  loading = false,
  switcher,
  place,
  deletion = false,
  name = org.name,
}: {
  rail?: boolean;
  loading?: boolean;
  switcher?: string;
  place?: string;
  deletion?: boolean;
  name?: string;
}) {
  return (
    <div className="h-480 overflow-hidden rounded-frame bg-chrome shadow-frame">
      <Sidebar.Provider shortcut={false} defaultRail={rail}>
        <Sidebar aria-label="Console" resizable={false}>
          <Sidebar.Brand>{rail ? <Sidebar.RailToggle /> : <Sidebar.CollapseToggle />}</Sidebar.Brand>
          <Sidebar.Content>
            {loading ? (
              <Sidebar.Loading rows={6} />
            ) : (
              <div className="flex flex-col gap-4">
                <ProjectPlace {...(place ? { force: place } : {})} />
                <Sidebar.Group>
                  <Sidebar.Item icon={icons.overview}>Overview</Sidebar.Item>
                  <Sidebar.Item icon={icons.requestLog} active count={128}>
                    Request logs
                  </Sidebar.Item>
                  <Sidebar.Item icon={icons.usage} data-force="hover">
                    Usage
                  </Sidebar.Item>
                  <Sidebar.Item icon={icons.key} data-force="focus">
                    Keys
                  </Sidebar.Item>
                  <Sidebar.Item icon={icons.project} depth={1}>
                    {projects[0]!.name}
                  </Sidebar.Item>
                  <Sidebar.Item leading={<Avatar name={projects[1]!.name} kind="org" size={20} decorative />} trailing={<Badge>Default</Badge>}>
                    {projects[1]!.name}
                  </Sidebar.Item>
                </Sidebar.Group>
              </div>
            )}
          </Sidebar.Content>
          <Sidebar.Footer>
            <Sidebar.Switcher
              avatar={<Avatar name={name} kind="org" size={24} decorative />}
              label={name}
              {...(deletion ? { caption: "Deletion on 20 Oct", captionTone: "danger" as const } : {})}
              {...(switcher ? { "data-force": switcher } : {})}
            >
              {deletion ? (
                <>
                  <DeletionOrgs />
                  <Menu.Separator />
                  <Menu.Item icon={icons.settings}>Organization settings</Menu.Item>
                </>
              ) : (
                <Menu.Item>{org.name}</Menu.Item>
              )}
            </Sidebar.Switcher>
          </Sidebar.Footer>
        </Sidebar>
      </Sidebar.Provider>
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="The place, default, current, hover, keyboard focus, nested; rail; loading; the place and the switcher hovered and open">
        <div className="flex gap-24">
          <Still />
          <Still rail />
          <Still loading switcher="hover" />
          <Still switcher="open" place="hover" />
        </div>
      </StateSection>
      <StateSection title="Scheduled for deletion: the switcher's caption at rest, hovered, in rail mode (its accessible description), and open with a long name">
        <div className="flex gap-24">
          <Still deletion />
          <Still deletion switcher="hover" />
          <Still deletion rail />
          <Still deletion switcher="open" name="Northwind Labs Research and Development" />
        </div>
      </StateSection>
      <StateSection title="The switcher's menu: the organizations, the date in the danger tone on one line">
        <div className="h-240">
          <Menu open>
            <Menu.Trigger>
              <Button data-force="open">Northwind Labs</Button>
            </Menu.Trigger>
            <Menu.Content>
              <DeletionOrgs />
              <Menu.Separator />
              <Menu.Item icon={icons.settings}>Organization settings</Menu.Item>
            </Menu.Content>
          </Menu>
        </div>
      </StateSection>
    </>
  );
}
