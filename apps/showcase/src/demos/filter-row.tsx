import { useState, type ReactNode } from "react";
import { FilterRow, PillTabs, Select, SearchInput, List, Empty, Badge, InsetContent, type ListItemData } from "@logicplanes/kaeso-ui";
import { members, projects } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "FilterRow",
  description: "Pill tabs at the start and filters at the end, between a page's header and its list. Grows with its content and wraps on a narrow screen.",
  group: "Layout",
  entry: "filter-row",
  import: "FilterRow",
  keyboard: [
    ["Left Right", "Moves between tabs, or between filters"],
    ["Tab", "From the tabs to the filters"],
  ],
  a11y: [
    "The filters are a toolbar of their own (named Filters by default), so its arrow keys never reach the tablist and the tabs keep theirs.",
    "Without filters there is no toolbar: the row is layout only.",
  ],
};
export const examples = [
  { id: "settings", title: "In a settings column", fn: "Settings" },
  { id: "page", title: "On an edge-to-edge page", fn: "Page" },
];

const presets = ["Owner", "Admin", "Developer", "Viewer"];
const presetOptions = [{ value: "all", label: "Every preset" }, ...presets.map((p) => ({ value: p, label: p }))];

const stateTabs = (value: string, onChange: (v: string) => void) => (
  <PillTabs value={value} onValueChange={onChange}>
    <PillTabs.List aria-label="State">
      <PillTabs.Tab value="all">All</PillTabs.Tab>
      <PillTabs.Tab value="active">Active</PillTabs.Tab>
      <PillTabs.Tab value="suspended">Suspended</PillTabs.Tab>
    </PillTabs.List>
  </PillTabs>
);

function Column({ width = "w-640", children }: { width?: string; children: ReactNode }) {
  return (
    <div className={`${width} bg-panel p-24`}>
      <h2 className="text-title-lg font-semibold text-fg">Members</h2>
      <div className="mt-8">
        <InsetContent>{children}</InsetContent>
      </div>
    </div>
  );
}

export function Settings() {
  const [state, setState] = useState("all");
  const [preset, setPreset] = useState("all");
  const items: ListItemData[] = members
    .filter((m) => preset === "all" || m.preset === preset)
    .filter(() => state !== "suspended")
    .map((m) => ({ id: m.id, title: m.name, subtitle: m.email, meta: <Badge>{m.preset}</Badge> }));
  return (
    <Column>
      <FilterRow tabs={stateTabs(state, setState)}>
        <Select aria-label="Preset" size="sm" value={preset} onValueChange={(v) => setPreset(String(v))} options={presetOptions} />
      </FilterRow>
      <List
        aria-label="Members"
        items={items}
        empty={<Empty size="md" title="No suspended members" description="Suspended members can't sign in. Everyone in Northwind Labs is active." />}
      />
    </Column>
  );
}

export function Page() {
  const [state, setState] = useState("all");
  return (
    <div className="w-full rounded-well bg-panel shadow-frame">
      <FilterRow tabs={stateTabs(state, setState)}>
        <div className="w-240">
          <SearchInput aria-label="Search members" placeholder="Name or email" />
        </div>
        <Select
          aria-label="Project"
          size="sm"
          defaultValue="all"
          options={[{ value: "all", label: "Every project" }, ...projects.map((p) => ({ value: p.id, label: p.name }))]}
        />
      </FilterRow>
    </div>
  );
}

export function States() {
  return (
    <>
      <StateSection title="In a settings column: tabs and a filter, tabs only, a filter only">
        <div className="flex flex-col gap-24">
          <Column>
            <FilterRow tabs={stateTabs("all", () => undefined)}>
              <Select aria-label="Preset" size="sm" defaultValue="all" options={presetOptions} />
            </FilterRow>
            <List
              aria-label="Members"
              items={members.slice(0, 2).map((m) => ({ id: m.id, title: m.name, subtitle: m.email, meta: <Badge>{m.preset}</Badge> }))}
            />
          </Column>
          <Column>
            <FilterRow tabs={stateTabs("active", () => undefined)} />
            <List aria-label="Members" items={members.slice(0, 1).map((m) => ({ id: m.id, title: m.name, subtitle: m.email }))} />
          </Column>
          <Column>
            <FilterRow>
              <Select aria-label="Preset" size="sm" defaultValue="all" options={presetOptions} />
            </FilterRow>
            <List aria-label="Members" items={members.slice(0, 1).map((m) => ({ id: m.id, title: m.name, subtitle: m.email }))} />
          </Column>
        </div>
      </StateSection>
      <StateSection title="Narrow: the filters wrap under the tabs">
        <Column width="w-360">
          <FilterRow tabs={stateTabs("all", () => undefined)}>
            <div className="w-240">
              <SearchInput aria-label="Search members" placeholder="Name or email" />
            </div>
          </FilterRow>
          <List
            aria-label="Members"
            items={members.slice(0, 2).map((m) => ({ id: m.id, title: m.name, subtitle: m.email, meta: <Badge>{m.preset}</Badge> }))}
          />
        </Column>
      </StateSection>
      <StateSection title="On an edge-to-edge page">
        <div className="w-640">
          <Page />
        </div>
      </StateSection>
    </>
  );
}
