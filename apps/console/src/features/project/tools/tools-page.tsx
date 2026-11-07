import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Button, Tabs, icons } from "@logicplanes/kaeso-ui";
import { portalGrantsQuery, projectServersQuery, projectToolsQuery } from "~/api/queries/tools";
import { t } from "~/i18n";
import { patchSearch, type SearchPatch } from "~/search";
import { usePageCreateAction } from "~/shell/create-action";
import { ProjectHeader } from "~/shell/project-header";
import type { Hint } from "~/kaeso/mcp-servers";
import { ConnectTab } from "./connect-tab";
import { ServersTab, type ServersSort } from "./servers-tab";
import { ToolSheet } from "./tool-sheet";
import { ToolsTab } from "./tools-tab";

export type ToolState = "on" | "off" | "changed";

export interface ToolsSearch {
  tab?: "servers" | "connect";
  tool?: string;
  server?: string;
  state?: ToolState;
  hint?: Hint;
  sort?: ServersSort;
}

export function ToolsPage({ org, project, search }: { org: string; project: string; search: ToolsSearch }) {
  const navigate = useNavigate();
  const tab = search.tab ?? "tools";
  const tools = useQuery(projectToolsQuery(org, project));
  const servers = useQuery(projectServersQuery(org, project));
  const grants = useQuery(portalGrantsQuery(org, project));
  const setSearch = (patch: SearchPatch<ToolsSearch>) =>
    void navigate({ to: "/orgs/$org/projects/$project/tools", params: { org, project }, search: patchSearch(search, patch), replace: true });
  const connect = () => setSearch({ tab: "connect", tool: undefined });
  const connectLabel = t("project.tools.connect", "Connect a client");
  usePageCreateAction(tab === "connect" ? null : { label: connectLabel, run: connect });
  return (
    <>
      <ProjectHeader
        org={org}
        project={project}
        page={t("project.tools.title", "Tools")}
        path="tools"
        tabs={
          <Tabs variant="pill" value={tab} onValueChange={(v) => setSearch({ tab: v === "servers" || v === "connect" ? v : undefined, tool: undefined })}>
            <Tabs.List aria-label={t("project.tools.title", "Tools")}>
              <Tabs.Tab value="tools" {...(tools.data ? { count: tools.data.length } : {})}>
                {t("project.tools.tabTools", "Tools")}
              </Tabs.Tab>
              <Tabs.Tab value="servers" {...(servers.data ? { count: servers.data.length } : {})}>
                {t("project.tools.tabServers", "Servers")}
              </Tabs.Tab>
              <Tabs.Tab value="connect" {...(grants.data ? { count: grants.data.length } : {})}>
                {t("project.tools.tabConnect", "Connect")}
              </Tabs.Tab>
            </Tabs.List>
          </Tabs>
        }
        actions={
          tab !== "connect" && (
            <Button size="sm" variant="primary" leadingIcon={icons.add} onClick={connect}>
              {connectLabel}
            </Button>
          )
        }
      />
      {tab === "tools" ? (
        <ToolsTab org={org} project={project} search={search} setSearch={setSearch} />
      ) : tab === "servers" ? (
        <ServersTab org={org} project={project} search={search} setSearch={setSearch} onTools={() => setSearch({ tab: undefined })} />
      ) : (
        <AppFrame.Body padding="page">
          <ConnectTab org={org} project={project} />
        </AppFrame.Body>
      )}
      <ToolSheet org={org} project={project} toolId={search.tool} onClose={() => setSearch({ tool: undefined })} />
    </>
  );
}
