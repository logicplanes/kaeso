import { useQueries, useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
  Avatar,
  Button,
  Dashboard,
  DashboardCell,
  Empty,
  ProviderBadge,
  RankList,
  ShareBar,
  Sparkline,
  Status,
  Text,
  icons,
  type RankItem,
} from "@logicplanes/kaeso-ui";
import { formatNumber, formatRelative } from "@logicplanes/kaeso-ui/format";
import { orgCredentialsQuery } from "~/api/queries/credentials";
import { mcpServersQuery } from "~/api/queries/mcp-servers";
import { invitesQuery, orgMembersQuery } from "~/api/queries/orgs";
import { orgProjectsQuery } from "~/api/queries/projects";
import { orgUsageQuery, projectUsageQuery } from "~/api/queries/usage";
import { useOrgRights, useSession } from "~/auth/session";
import { shiftDay, utcDay } from "~/format/period";
import { t } from "~/i18n";
import { grantLabel } from "~/kaeso/grant";
import { statusWord } from "~/kaeso/mcp-servers";
import { RecentChanges } from "~/kaeso/overview/recent-changes";
import { useProviders } from "~/kaeso/providers";
import { byBucket, periodUSD } from "~/kaeso/usage-panels";
import { orgPath, projectPath } from "~/shell/pages";

const REFRESH = 30_000;
const PROJECT_ROWS = 6;

export function OrgGlance({ org }: { org: string }) {
  return (
    <div className="mt-48">
      <Dashboard aria-label={t("orgHome.label", "This organization at a glance")} columns={3}>
        <ProjectsCell org={org} />
        <MembersCell org={org} />
        <ProvidersCell org={org} />
        <McpCell org={org} />
        <ChangesCell org={org} />
      </Dashboard>
    </div>
  );
}

export function CostByProject({ org }: { org: string }) {
  const today = utcDay(new Date());
  const monthStart = `${today.slice(0, 8)}01`;
  const byProject = useQuery({ ...orgUsageQuery(org, monthStart, today, "project"), refetchInterval: REFRESH });
  const projects = useQuery(orgProjectsQuery(org, true));
  const slices = [...(byProject.data ?? [])]
    .filter((r) => Number(r.cost ?? 0) > 0)
    .sort((a, b) => Number(b.cost) - Number(a.cost))
    .slice(0, 4)
    .map((r) => ({ id: r.key, label: projects.data?.find((p) => p.id === r.key)?.name ?? r.key, value: Number(r.cost) }));
  if (!slices.length) return null;
  return <ShareBar aria-label={t("kaeso.overview.costByProject", "Cost by project")} slices={slices} formatValue={(v) => periodUSD(v.toFixed(6))} />;
}

function ProjectsCell({ org }: { org: string }) {
  const rights = useOrgRights(org);
  const navigate = useNavigate();
  const canUsage = rights.has("usage.org.view");
  const today = utcDay(new Date());
  const from = shiftDay(today, -13);
  const projects = useQuery(orgProjectsQuery(org, true));
  const byProject = useQuery({ ...orgUsageQuery(org, from, today, "project"), enabled: canUsage, refetchInterval: REFRESH });
  const usageOf = (id: string) => byProject.data?.find((r) => r.key === id);
  const shown = (projects.data ?? [])
    .filter((p) => !p.archived_at)
    .sort((a, b) => (usageOf(b.id)?.requests ?? 0) - (usageOf(a.id)?.requests ?? 0) || a.name.localeCompare(b.name))
    .slice(0, PROJECT_ROWS);
  const daily = useQueries({
    queries: shown.map((p) => ({ ...projectUsageQuery(org, p.id, from, today, "day"), enabled: canUsage, refetchInterval: REFRESH, retry: false })),
  });
  const items: RankItem[] = shown.map((p, i) => {
    const days = byBucket(daily[i]?.data ?? [], from, today);
    const todayRow = days.at(-1)?.row;
    return {
      id: p.id,
      label: (
        <>
          <Avatar name={p.name} {...(p.avatar_url ? { src: p.avatar_url } : {})} kind="org" size={20} decorative />
          <span className="min-w-0 truncate">{p.name}</span>
        </>
      ),
      ...(canUsage
        ? {
            secondary: (
              <span className="hidden w-120 items-center sm:flex">
                {daily[i]?.data && (
                  <Sparkline
                    values={days.map((d) => d.row?.requests ?? 0)}
                    fluid
                    aria-label={t("orgHome.projectTrend", "{project}: requests per day, the last 14 days", { project: p.name })}
                  />
                )}
              </span>
            ),
            value: formatNumber(todayRow?.requests ?? 0),
          }
        : {}),
    };
  });
  const total = projects.data?.filter((p) => !p.archived_at).length ?? 0;
  return (
    <DashboardCell
      className="lg:col-span-2"
      title={canUsage ? t("orgHome.projectsToday", "Projects, requests today") : t("common.projects", "Projects")}
      {...(canUsage ? { href: `${orgPath(org)}/analytics/projects` } : {})}
      actions={
        <Button size="sm" variant="ghost" href={`${orgPath(org)}/projects`}>
          {total > PROJECT_ROWS ? t("orgHome.allCount", "All {count}", { count: total }) : t("orgHome.allProjects", "View all projects")}
        </Button>
      }
    >
      {!projects.isPending && total === 0 ? (
        <Empty
          size="compact"
          title={t("orgHome.noProjects", "No projects you can see")}
          description={t("orgHome.noProjectsHint", "Projects you belong to are listed here.")}
          actions={
            rights.has("projects.create") && (
              <Button variant="primary" leadingIcon={icons.add} href={`${orgPath(org)}/projects?create=true`}>
                {t("orgHome.createProject", "Create project")}
              </Button>
            )
          }
        />
      ) : (
        <RankList
          aria-label={t("common.projects", "Projects")}
          items={items}
          loading={projects.isPending || (canUsage && byProject.isPending)}
          rows={PROJECT_ROWS}
          onOpen={(id) => void navigate({ href: projectPath(org, id) })}
        />
      )}
    </DashboardCell>
  );
}

function ProvidersCell({ org }: { org: string }) {
  const rights = useOrgRights(org);
  const providers = useProviders(org);
  const navigate = useNavigate();
  const keys = useQuery({ ...orgCredentialsQuery(org), enabled: rights.has("provider_credentials.view") });
  if (!rights.has("provider_credentials.view")) return null;
  const list = [...(keys.data ?? [])].sort((a, b) => (b.last_used_at ?? "").localeCompare(a.last_used_at ?? ""));
  return (
    <DashboardCell
      className="lg:col-span-1"
      title={t("orgHome.home.providerKeys", "{count, plural, one {# provider key} other {# provider keys}}", { count: list.length })}
      actions={
        <Button size="sm" variant="ghost" href={`${orgPath(org)}/providers`}>
          {t("project.home.manage", "Manage")}
        </Button>
      }
    >
      <RankList
        aria-label={t("shell.page.providers", "Providers")}
        loading={keys.isPending}
        items={list.slice(0, 5).map((k) => ({
          id: k.id,
          label: (
            <>
              <ProviderBadge provider={k.provider} name={providers.name(k.provider)} size="sm" />
              <span className="min-w-0 truncate">{providers.name(k.provider)}</span>
              {k.name && (
                <Text variant="small" tone="tertiary" truncate>
                  {k.name}
                </Text>
              )}
            </>
          ),
          secondary: k.auth_error ? (
            <Status tone="warning">{k.auth_error === "signed_out" ? t("orgHome.signedOut", "Signed out") : t("orgHome.noApiAccess", "No API access")}</Status>
          ) : k.last_used_at ? (
            formatRelative(k.last_used_at)
          ) : (
            t("project.home.neverUsed", "Never used")
          ),
        }))}
        empty={t("orgHome.noProviderKeys", "No provider keys yet")}
        onOpen={() => void navigate({ href: `${orgPath(org)}/providers` })}
      />
    </DashboardCell>
  );
}

function McpCell({ org }: { org: string }) {
  const rights = useOrgRights(org);
  const navigate = useNavigate();
  const servers = useQuery({ ...mcpServersQuery(org), enabled: rights.has("mcp_servers.view") });
  if (!rights.has("mcp_servers.view")) return null;
  const list = servers.data ?? [];
  return (
    <DashboardCell
      className="lg:col-span-1"
      title={t("orgHome.mcpCount", "{count, plural, one {# MCP server} other {# MCP servers}}", { count: list.length })}
      actions={
        <Button size="sm" variant="ghost" href={`${orgPath(org)}/mcp-servers`}>
          {t("project.home.manage", "Manage")}
        </Button>
      }
    >
      <RankList
        aria-label={t("shell.page.mcpServers", "MCP servers")}
        loading={servers.isPending}
        items={list.slice(0, 5).map((s) => {
          const word = statusWord(s.status);
          return {
            id: s.id,
            label: <span className="min-w-0 truncate">{s.name}</span>,
            secondary: s.status === "ok" && s.last_checked_at ? formatRelative(s.last_checked_at) : <Status tone={word.tone}>{word.label}</Status>,
          };
        })}
        empty={t("orgHome.noMcp", "No MCP servers yet")}
        onOpen={(id) => void navigate({ href: `${orgPath(org)}/mcp-servers?server=${encodeURIComponent(id)}` })}
      />
    </DashboardCell>
  );
}

function MembersCell({ org }: { org: string }) {
  const rights = useOrgRights(org);
  const session = useSession();
  const navigate = useNavigate();
  const members = useQuery({ ...orgMembersQuery(org), enabled: rights.has("members.view") });
  const invites = useQuery({ ...invitesQuery(org, true), enabled: rights.has("members.view") });
  if (!rights.has("members.view")) return null;
  const list = members.data ?? [];
  const pending = invites.data?.length ?? 0;
  const href = `/settings/orgs/${org}/members`;
  return (
    <DashboardCell
      className="lg:col-span-1"
      {...(rights.has("usage.org.view") ? { href: `${orgPath(org)}/analytics/members` } : {})}
      title={
        pending
          ? t("orgHome.membersInvited", "{count, plural, one {# member} other {# members}}, {pending} invited", { count: list.length, pending })
          : t("project.home.members", "{count, plural, one {# member} other {# members}}", { count: list.length })
      }
      actions={
        <Button size="sm" variant="ghost" href={href}>
          {t("project.home.manage", "Manage")}
        </Button>
      }
    >
      <RankList
        aria-label={t("common.members", "Members")}
        loading={members.isPending}
        items={list.slice(0, 5).map((m) => ({
          id: m.user.id,
          label: (
            <>
              <Avatar name={m.user.name} size={20} decorative {...(m.user.avatar_url ? { src: m.user.avatar_url } : {})} />
              <span className="min-w-0 truncate">
                {m.user.id === session.user.id ? t("orgHome.nameYou", "{name} (you)", { name: m.user.name }) : m.user.name}
              </span>
            </>
          ),
          secondary: grantLabel(m.grant),
        }))}
        empty={t("project.home.noMembers", "No members yet")}
        onOpen={() => void navigate({ href })}
      />
    </DashboardCell>
  );
}

function ChangesCell({ org }: { org: string }) {
  const rights = useOrgRights(org);
  const members = useQuery({ ...orgMembersQuery(org), enabled: rights.has("members.view") });
  if (!rights.has("audit_log.view")) return null;
  const nameOf = (id: string | null | undefined) => members.data?.find((m) => m.user.id === id)?.user.name ?? t("orgHome.serviceAccount", "Service account");
  const href = `/settings/orgs/${org}/audit-log`;
  return (
    <DashboardCell
      className="lg:col-span-1"
      title={t("kaeso.home.recentChanges", "Recent changes")}
      actions={
        <Button size="sm" variant="ghost" href={href}>
          {t("project.home.auditLog", "Audit log")}
        </Button>
      }
    >
      <RecentChanges scope={{ org }} href={href} actorName={(e) => nameOf(e.actor.id)} rows={4} />
    </DashboardCell>
  );
}
