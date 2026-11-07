import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  AlertDialog,
  AppFrame,
  Button,
  Combobox,
  DialogScope,
  Empty,
  List,
  Menu,
  Meter,
  Popover,
  Select,
  Status,
  Text,
  Toolbar,
  icons,
} from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { ProjectMcpServerView } from "~/api/generated/types";
import { projectToolViews } from "~/api/queries/invalidation";
import { mcpServersQuery } from "~/api/queries/mcp-servers";
import { projectServersQuery } from "~/api/queries/tools";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useProjectRights } from "~/auth/rights";
import { useOrgRights } from "~/auth/session";
import { LoadFailed } from "~/features/org/load-failed";
import { compareNames } from "~/format/collate";
import { t } from "~/i18n";
import { LastUsed } from "~/kaeso/last-used";
import { hostOf, projectServerWord } from "~/kaeso/mcp-servers";
import type { SearchPatch } from "~/search";
import { toast } from "~/shell/toast";
import type { ToolsSearch } from "./tools-page";

export type ServersSort = "name" | "calls" | "used" | "on";

function sorted(list: ProjectMcpServerView[], sort: ServersSort): ProjectMcpServerView[] {
  const byName = (a: ProjectMcpServerView, b: ProjectMcpServerView) => compareNames(a.name, b.name);
  const by = {
    name: byName,
    calls: (a: ProjectMcpServerView, b: ProjectMcpServerView) => (b.calls_7d ?? -1) - (a.calls_7d ?? -1) || byName(a, b),
    used: (a: ProjectMcpServerView, b: ProjectMcpServerView) => (b.last_call_at ?? "").localeCompare(a.last_call_at ?? "") || byName(a, b),
    on: (a: ProjectMcpServerView, b: ProjectMcpServerView) => b.tools_on - a.tools_on || b.tools - a.tools || byName(a, b),
  }[sort];
  return [...list].sort(by);
}

export function ServersTab({
  org,
  project,
  search,
  setSearch,
  onTools,
}: {
  org: string;
  project: string;
  search: ToolsSearch;
  setSearch: (patch: SearchPatch<ToolsSearch>) => void;
  onTools: () => void;
}) {
  const rights = useProjectRights(org, project);
  const orgRights = useOrgRights(org);
  const needs = useNeedsRight(org, project);
  const orgNeeds = useNeedsRight(org);
  const canManage = rights.has("tools.manage");
  const canPick = orgRights.has("mcp_servers.view");
  const reason = !canManage ? needs("tools.manage") : !canPick ? orgNeeds("mcp_servers.view") : undefined;
  const servers = useQuery(projectServersQuery(org, project));
  const [stopping, setStopping] = useState<ProjectMcpServerView | null>(null);
  const [stopOpen, setStopOpen] = useState(false);
  const sort = search.sort ?? "name";
  const list = servers.data ?? [];
  const use = (
    <UseServer
      org={org}
      project={project}
      used={list}
      {...(reason ? { reason } : {})}
      onUsed={(name) => toast(t("project.tools.serverUsed", "{name} is in use; turn its tools on", { name }))}
    />
  );
  return (
    <>
      <Toolbar aria-label={t("project.tools.serversToolbar", "Servers")}>
        <Select
          aria-label={t("project.tools.sort", "Sort")}
          size="sm"
          icon={icons.sort}
          value={sort}
          onValueChange={(v) => setSearch({ sort: v === "calls" || v === "used" || v === "on" ? v : undefined })}
          options={[
            { value: "name", label: t("project.tools.sortName", "Name") },
            { value: "calls", label: t("project.tools.sortCalls", "Calls in 7 days") },
            { value: "used", label: t("project.tools.sortUsed", "Last used") },
            { value: "on", label: t("project.tools.sortOn", "Tools on") },
          ]}
        />
        <Toolbar.Spacer />
        {use}
      </Toolbar>
      <AppFrame.Body padding="list">
        <Servers
          list={sorted(list, sort)}
          pending={servers.isPending}
          failed={servers.isError}
          canManage={canManage}
          onTools={onTools}
          onStop={(s) => {
            setStopping(s);
            setStopOpen(true);
          }}
        />
      </AppFrame.Body>
      {stopping && (
        <DialogScope open={stopOpen}>
          <StopUsing org={org} project={project} server={stopping} used={list} open={stopOpen} onOpenChange={setStopOpen} />
        </DialogScope>
      )}
    </>
  );
}

function Servers({
  list,
  pending,
  failed,
  canManage,
  onTools,
  onStop,
}: {
  list: ProjectMcpServerView[];
  pending: boolean;
  failed: boolean;
  canManage: boolean;
  onTools: () => void;
  onStop: (s: ProjectMcpServerView) => void;
}) {
  if (pending) return <List aria-label={t("project.tools.tabServers", "Servers")} items={[]} loading />;
  if (failed) return <LoadFailed title={t("project.tools.serversFailed", "The servers couldn't load")} />;
  if (list.length === 0)
    return (
      <Empty
        size="lg"
        title={t("project.tools.noServers", "No servers yet")}
        description={t("project.tools.noServersHint", "Use one of your organization's MCP servers to offer its tools.")}
      />
    );
  return (
    <List
      aria-label={t("project.tools.tabServers", "Servers")}
      onOpen={onTools}
      items={list.map((s) => {
        const word = projectServerWord(s);
        return {
          id: s.id,
          textValue: s.name,
          title: s.name,
          subtitle: hostOf(s.url),
          meta: (
            <span className="flex flex-wrap items-center gap-x-16 gap-y-4">
              <Status tone={word.tone} className="shrink-0 text-nowrap">
                {word.label}
              </Status>
              <span className="flex shrink-0 items-center gap-8">
                <Text variant="small" tone="secondary" tabular className="text-nowrap">
                  {t("project.tools.onCount", "{on} of {total} on", { on: s.tools_on, total: s.tools })}
                </Text>
                <Meter
                  tone="neutral"
                  value={s.tools ? s.tools_on / s.tools : 0}
                  aria-label={t("project.tools.onCount", "{on} of {total} on", { on: s.tools_on, total: s.tools })}
                  className="hidden w-80 md:flex"
                />
              </span>
              {s.calls_7d != null && (
                <Text variant="small" tone="secondary" tabular className="shrink-0 text-nowrap">
                  {t("project.tools.calls7dCount", "{count, plural, one {# call} other {# calls}} in 7 days", { count: s.calls_7d })}
                </Text>
              )}
            </span>
          ),
          date: <LastUsed at={s.last_call_at} />,
          ...(canManage
            ? {
                actions: (
                  <Menu>
                    <Menu.Trigger>
                      <Button
                        variant="ghost"
                        icon={icons.more}
                        size="xs"
                        aria-label={t("project.tools.serverActions", "Actions for {name}", { name: s.name })}
                      />
                    </Menu.Trigger>
                    <Menu.Content align="end">
                      <Menu.Item icon={icons.delete} variant="danger" onClick={() => onStop(s)}>
                        {t("project.tools.stopUsing", "Stop using")}
                      </Menu.Item>
                    </Menu.Content>
                  </Menu>
                ),
              }
            : {}),
        };
      })}
    />
  );
}

function setServers(org: string, project: string, servers: string[]) {
  return api.PUT("/platform/v1/orgs/{org}/projects/{project}/mcp-servers", { params: { path: { org, project } }, body: { servers } });
}

function UseServer({
  org,
  project,
  used,
  reason,
  onUsed,
}: {
  org: string;
  project: string;
  used: ProjectMcpServerView[];
  reason?: string;
  onUsed: (name: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const all = useQuery({ ...mcpServersQuery(org), enabled: open });
  const options = (all.data ?? []).filter((s) => !used.some((u) => u.id === s.id)).map((s) => ({ value: s.id, label: s.name, description: hostOf(s.url) }));
  const add = useApiMutation({
    mutationFn: async (server: { id: string; name: string }) => data(await setServers(org, project, [...used.map((u) => u.id), server.id])),
    invalidates: [projectToolViews(org)],
    onSuccess: (_list, server) => {
      setOpen(false);
      onUsed(server.name);
    },
  });
  const label = t("project.tools.useServer", "Use a server");
  if (reason)
    return (
      <Button size="sm" leadingIcon={icons.add} disabled disabledReason={reason}>
        {label}
      </Button>
    );
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <Popover.Trigger>
        <Button size="sm" leadingIcon={icons.add} loading={add.isPending}>
          {label}
        </Button>
      </Popover.Trigger>
      <Popover.Content aria-label={label} align="end">
        <div className="w-320 max-w-full">
          <Combobox
            aria-label={t("project.tools.server", "Server")}
            placeholder={t("project.tools.pickServer", "Find a server")}
            options={options}
            loading={all.isPending}
            emptyText={t("project.tools.noOtherServers", "Every server of the organization is in use.")}
            value={null}
            onValueChange={(v) => {
              const picked = options.find((o) => o.value === v);
              if (picked) add.mutate({ id: picked.value, name: picked.label });
            }}
          />
        </div>
      </Popover.Content>
    </Popover>
  );
}

function StopUsing({
  org,
  project,
  server,
  used,
  open,
  onOpenChange,
}: {
  org: string;
  project: string;
  server: ProjectMcpServerView;
  used: ProjectMcpServerView[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const stop = useApiMutation({
    mutationFn: async () =>
      data(
        await setServers(
          org,
          project,
          used.filter((u) => u.id !== server.id).map((u) => u.id),
        ),
      ),
    invalidates: [projectToolViews(org)],
    onSuccess: () => {
      onOpenChange(false);
      toast(t("project.tools.serverStopped", "{name} isn't used anymore", { name: server.name }));
    },
    onError: () => onOpenChange(false),
  });
  return (
    <AlertDialog
      open={open}
      onOpenChange={onOpenChange}
      tone="danger"
      title={t("project.tools.stopTitle", "Stop using {name}?", { name: server.name })}
      description={t("project.tools.stopHint", "Its tools leave this project's portal; using it again starts with every tool off.")}
      confirmLabel={t("project.tools.stopUsing", "Stop using")}
      loading={stop.isPending}
      onConfirm={() => stop.mutateAsync()}
    />
  );
}
