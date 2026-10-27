import type * as React from "react";
import {
  AppFrame,
  Sidebar,
  ListPage,
  OverviewPage,
  EmptyPage,
  MetricRow,
  Stat,
  TimeseriesChart,
  Button,
  Toolbar,
  SearchInput,
  icons,
  List,
} from "@logicplanes/kaeso-ui";
import { formatUSD } from "@logicplanes/kaeso-ui/format";
import { usageSeries, keys } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Page patterns",
  description: "List, table, overview and empty pages, composed from the library so the console never invents layout.",
  group: "Blocks",
  entry: "pages",
  import: "ListPage, TablePage, OverviewPage, EmptyPage, AuthPage",
  variants: "PAGES",
  source: "blocks/pages",
};
function Frame({ children }: { children: React.ReactNode }) {
  return (
    <div className="h-480 w-full overflow-hidden rounded-frame shadow-frame">
      <Sidebar.Provider shortcut={false}>
        <AppFrame
          fill="parent"
          sidebar={
            <Sidebar aria-label="Main" resizable={false}>
              <Sidebar.Content>
                <Sidebar.Group>
                  <Sidebar.Item icon={icons.overview} active>
                    Overview
                  </Sidebar.Item>
                  <Sidebar.Item icon={icons.key}>Keys</Sidebar.Item>
                </Sidebar.Group>
              </Sidebar.Content>
            </Sidebar>
          }
        >
          {children}
        </AppFrame>
      </Sidebar.Provider>
    </div>
  );
}
export const examples = [
  { id: "overview", title: "Overview", fn: "Overview", minHeight: 520 },
  { id: "list", title: "List page", fn: "ListDemo", minHeight: 520 },
  { id: "empty", title: "Empty page", fn: "EmptyDemo", minHeight: 520 },
];
export function Overview() {
  return (
    <Frame>
      <OverviewPage
        header={{ title: "Overview" }}
        metrics={
          <MetricRow>
            <Stat value="12,480" unit="requests" />
            <Stat value="$184.20" unit="total spend" />
          </MetricRow>
        }
        controls={
          <Button size="sm" trailingIcon={icons.chevronDown}>
            Last 30 days
          </Button>
        }
        chart={<TimeseriesChart aria-label="Spend" series={usageSeries().slice(0, 2)} height={200} formatValue={(v) => formatUSD(v.toFixed(2))} />}
      />
    </Frame>
  );
}
export function ListDemo() {
  return (
    <Frame>
      <ListPage
        header={{
          title: "Keys",
          actions: (
            <Button size="sm" variant="primary" leadingIcon={icons.add}>
              Create key
            </Button>
          ),
        }}
        toolbar={
          <Toolbar aria-label="Filters">
            <Toolbar.Spacer />
            <div className="w-240">
              <SearchInput aria-label="Search keys" />
            </div>
          </Toolbar>
        }
      >
        <List aria-label="Keys" items={keys.map((k) => ({ id: k.id, title: k.name, subtitle: k.redacted }))} />
      </ListPage>
    </Frame>
  );
}
export function EmptyDemo() {
  return (
    <Frame>
      <EmptyPage
        header={{ title: "Models" }}
        title="No models yet"
        description="Add a model before creating keys."
        actions={<Button variant="primary">Add model</Button>}
      />
    </Frame>
  );
}
export function States() {
  return (
    <StateSection title="Pages">
      <div className="flex w-640 flex-col gap-24">
        <Overview />
        <ListDemo />
        <EmptyDemo />
        <Frame>
          <EmptyPage title="No models yet" description="Without a header, the menu button still brings back a collapsed sidebar." />
        </Frame>
      </div>
    </StateSection>
  );
}
