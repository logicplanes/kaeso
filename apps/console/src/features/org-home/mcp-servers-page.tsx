import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Badge, Button, Code, DataTable, DialogScope, Empty, Status, Text, Tooltip, col, icons } from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import type { McpServerView, ProjectView } from "~/api/generated/types";
import { mcpServersQuery } from "~/api/queries/mcp-servers";
import { orgProjectsQuery } from "~/api/queries/projects";
import { useNeedsRight } from "~/auth/needs-right";
import { useOrgRights } from "~/auth/session";
import { t } from "~/i18n";
import { AddServerDialog } from "~/kaeso/mcp-server-dialog";
import { connectionWord, hostOf, statusWord } from "~/kaeso/mcp-servers";
import { usePageCreateAction } from "~/shell/create-action";
import { OrgHeader } from "~/shell/org-header";
import { toast } from "~/shell/toast";
import { McpServerSheet } from "./mcp-server-sheet";

export type SignInResult = "connected" | "declined" | "expired" | "failed";

export interface McpServersSearch {
  server?: string;
  result?: SignInResult;
  add?: boolean;
}

function signInToast(result: SignInResult, server: McpServerView | undefined) {
  const name = server?.name ?? t("orgHome.mcp.theServer", "the server");
  if (result === "connected") toast.success(t("orgHome.mcp.signedInTo", "Signed in to {name}", { name }));
  else if (result === "declined") toast.error(t("orgHome.mcp.signInDeclined", "The sign-in to {name} was declined.", { name }));
  else if (result === "expired") toast.error(t("orgHome.mcp.signInExpired", "The sign-in took too long; start it again."));
  else toast.error(t("orgHome.mcp.signInFailed", "The sign-in to {name} didn't finish; try again.", { name }));
}

const projectNames = (s: McpServerView, projects: ProjectView[]) =>
  s.projects.map((id) => projects.find((p) => p.id === id)?.name ?? id).join(t("orgHome.listSeparator", ", "));

export function OrgMcpServersPage({ org, search = {} }: { org: string; search?: McpServersSearch }) {
  const rights = useOrgRights(org);
  const needs = useNeedsRight(org);
  const navigate = useNavigate();
  const canView = rights.has("mcp_servers.view");
  const canManage = rights.has("mcp_servers.manage");
  const servers = useQuery({ ...mcpServersQuery(org), enabled: canView });
  const projects = useQuery({ ...orgProjectsQuery(org), enabled: canView });
  const [adding, setAdding] = useState(!!search.add);
  const list = useMemo(() => servers.data ?? [], [servers.data]);
  const all = useMemo(() => projects.data ?? [], [projects.data]);
  const addReason = canManage ? undefined : needs("mcp_servers.manage");
  const addLabel = t("orgHome.mcp.add", "Add server");
  usePageCreateAction(canView ? { label: addLabel, run: () => setAdding(true), disabledReason: addReason } : null);

  const open = (server: string | undefined) =>
    void navigate({ to: "/orgs/$org/mcp-servers", params: { org }, search: server ? { server } : {}, replace: true });
  const result = search.result;
  const loaded = servers.isSuccess || servers.isError || !canView;
  useEffect(() => {
    if (!result || !loaded) return;
    signInToast(
      result,
      list.find((s) => s.id === search.server),
    );
    void navigate({ to: "/orgs/$org/mcp-servers", params: { org }, search: search.server ? { server: search.server } : {}, replace: true });
  }, [result, loaded, list, search.server, navigate, org]);

  const columns = useMemo(
    () =>
      [
        col.custom(
          { id: "server", header: t("orgHome.mcp.server", "Server"), size: 240, minWidth: 160, priority: 1, grow: true },
          (s: McpServerView) => s,
          (s: McpServerView) => (
            <span className="flex min-w-0 flex-col">
              <Text weight="medium" truncate>
                {s.name}
              </Text>
              <Text variant="small" tone="secondary" truncate>
                {hostOf(s.url)}
              </Text>
            </span>
          ),
        ),
        col.custom(
          { id: "handle", header: t("orgHome.mcp.handle", "Handle"), size: 120, priority: 2 },
          (s: McpServerView) => s.handle,
          (handle: string) => <Code>{handle}</Code>,
        ),
        col.status({ id: "connection", header: t("orgHome.mcp.connection", "Connection"), size: 208, priority: 1 }, (s: McpServerView) => connectionWord(s)),
        col.custom(
          { id: "tools", header: t("orgHome.mcp.tools", "Tools"), size: 136, priority: 2 },
          (s: McpServerView) => s.tools,
          (tools: McpServerView["tools"]) => (
            <span className="flex items-center gap-8">
              <Text variant="small" tone={tools.total ? "default" : "tertiary"}>
                {tools.total}
              </Text>
              {tools.changed > 0 && <Badge tone="warning">{t("orgHome.mcp.changedCount", "{count} changed", { count: tools.changed })}</Badge>}
            </span>
          ),
        ),
        col.custom(
          { id: "projects", header: t("common.projects", "Projects"), size: 184, priority: 3 },
          (s: McpServerView) => s,
          (s: McpServerView) =>
            s.projects.length ? (
              <Tooltip content={projectNames(s, all)}>
                <Text variant="small" tone="secondary" truncate>
                  {projectNames(s, all)}
                </Text>
              </Tooltip>
            ) : (
              <Text variant="small" tone="tertiary">
                {t("orgHome.noProject", "No project")}
              </Text>
            ),
        ),
        col.custom(
          { id: "checked", header: t("orgHome.mcp.checked", "Checked"), size: 160, priority: 2 },
          (s: McpServerView) => s,
          (s: McpServerView) => {
            const word = statusWord(s.status);
            if (s.status === "ok" && s.last_checked_at)
              return (
                <Text variant="small" tone="secondary">
                  {formatRelative(s.last_checked_at)}
                </Text>
              );
            const shown = <Status tone={word.tone}>{word.label}</Status>;
            return s.last_error ? <Tooltip content={s.last_error}>{shown}</Tooltip> : shown;
          },
        ),
      ].map((c) => ({ ...c, enableSorting: false })),
    [all],
  );

  return (
    <>
      <OrgHeader
        org={org}
        page={t("orgHome.mcp.title", "MCP servers")}
        actions={
          canView &&
          !(servers.isSuccess && list.length === 0) && (
            <Button
              size="sm"
              variant="primary"
              leadingIcon={icons.add}
              disabled={!canManage}
              {...(addReason ? { disabledReason: addReason } : {})}
              onClick={() => setAdding(true)}
            >
              {addLabel}
            </Button>
          )
        }
      />
      <AppFrame.Body padding="list">
        {!canView ? (
          <Empty title={t("orgHome.mcp.hidden", "You can't see the MCP servers here")} description={needs("mcp_servers.view")} />
        ) : (
          <DataTable
            aria-label={t("orgHome.mcp.title", "MCP servers")}
            data={list}
            columns={columns}
            getRowId={(s) => s.id}
            density="comfortable"
            loading={servers.isPending}
            error={servers.isError && list.length === 0 ? t("orgHome.mcp.loadFailed", "The MCP servers couldn't load") : undefined}
            onRetry={() => void servers.refetch()}
            onRowClick={(s) => open(s.id)}
            activeRowId={search.server}
            empty={
              <Empty
                title={t("orgHome.mcp.empty", "No MCP servers yet")}
                description={t("orgHome.mcp.emptyHint", "Add a server so your projects' portals can offer its tools.")}
                actions={
                  <Button
                    variant="primary"
                    leadingIcon={icons.add}
                    disabled={!canManage}
                    {...(addReason ? { disabledReason: addReason } : {})}
                    onClick={() => setAdding(true)}
                  >
                    {t("orgHome.mcp.addFirst", "Add a server")}
                  </Button>
                }
              />
            }
          />
        )}
      </AppFrame.Body>
      {canView && <McpServerSheet org={org} serverId={search.server} onClose={() => open(undefined)} />}
      <DialogScope open={adding}>
        <AddServerDialog
          org={org}
          open={adding}
          onClose={() => {
            setAdding(false);
            if (search.add) open(search.server);
          }}
          onAdded={(s) => open(s.id)}
        />
      </DialogScope>
    </>
  );
}
