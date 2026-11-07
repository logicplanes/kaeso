import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Avatar, Button, CellInset, Dashboard, DashboardCell, ModelLabel, RankList, StatusCode, Text, type RankItem } from "@logicplanes/kaeso-ui";
import { formatDuration, formatPercent, formatRelative } from "@logicplanes/kaeso-ui/format";
import { allKeysQuery } from "~/api/queries/keys";
import { limitsQuery } from "~/api/queries/limits";
import { newestLogsQuery } from "~/api/queries/logs";
import { projectMembersQuery } from "~/api/queries/members";
import { catalogueQuery } from "~/api/queries/models";
import { projectUsageQuery } from "~/api/queries/usage";
import { untoldReason } from "~/api/load-error";
import { useProjectRights } from "~/auth/rights";
import { useRequestLogs, useSession } from "~/auth/session";
import { shiftDay, utcDay } from "~/format/period";
import { t } from "~/i18n";
import { grantLabel } from "~/kaeso/grant";
import { Limit } from "~/kaeso/overview/limit";
import { RecentChanges } from "~/kaeso/overview/recent-changes";
import { useProviders } from "~/kaeso/providers";
import { projectPath } from "~/shell/pages";
import { keyLabel, usePartyName } from "./keys/party";

const LIVE_EVERY = 5_000;
const FEED_ROWS = 7;

export function ProjectGlance({ org, project }: { org: string; project: string }) {
  const rights = useProjectRights(org, project);
  const logsOn = useRequestLogs() && rights.has("logs.metadata.view");
  const audit = rights.has("project.audit_log.view");
  return (
    <div className="mt-48">
      <Dashboard aria-label={t("project.home.label", "This project at a glance")} columns={3}>
        {logsOn && <LiveCell org={org} project={project} className={rights.has("limits.view") ? "lg:col-span-2" : "lg:col-span-3"} />}
        <LimitsCell org={org} project={project} className={logsOn ? "lg:col-span-1" : "lg:col-span-3"} />
        <ModelsCell org={org} project={project} />
        <KeysCell org={org} project={project} />
        <MembersCell org={org} project={project} />
        {audit && <ChangesCell org={org} project={project} className="lg:col-span-3" />}
      </Dashboard>
    </div>
  );
}

const SHARE_DAYS = 14;

function ModelsCell({ org, project }: { org: string; project: string }) {
  const rights = useProjectRights(org, project);
  const providers = useProviders(org);
  const navigate = useNavigate();
  const base = projectPath(org, project);
  const today = utcDay(new Date());
  const models = useQuery({ ...catalogueQuery(org, project), enabled: rights.has("models.view") });
  const usage = useQuery(projectUsageQuery(org, project, shiftDay(today, -(SHARE_DAYS - 1)), today, "model"));
  if (!rights.has("models.view")) return null;
  const list = models.data ?? [];
  const used = (m: (typeof list)[number]) => usage.data?.find((r) => r.key === `${m.provider}/${m.model_id}`)?.requests ?? 0;
  const total = list.reduce((a, m) => a + used(m), 0) || 1;
  const sorted = [...list].sort((a, b) => used(b) - used(a) || a.model_id.localeCompare(b.model_id));
  return (
    <DashboardCell
      className="lg:col-span-1"
      title={t("project.home.models", "{count, plural, one {# model} other {# models}}", { count: list.length })}
      href={`${base}/analytics/models`}
      actions={
        <Button size="sm" variant="ghost" href={`${base}/models`}>
          {t("project.home.manage", "Manage")}
        </Button>
      }
    >
      <RankList
        aria-label={t("common.models", "Models")}
        loading={models.isPending}
        items={sorted.slice(0, 5).map((m) => ({
          id: `${m.provider}/${m.model_id}`,
          label: (
            <ModelLabel
              className="min-w-0"
              provider={m.provider}
              providerName={providers.name(m.provider)}
              model={m.model_id}
              {...(m.effective.display_name && m.effective.display_name !== m.model_id ? { name: m.effective.display_name } : {})}
            />
          ),
          ...(usage.data ? { share: used(m) / total, value: formatPercent(used(m) / total, 0) } : {}),
        }))}
        empty={t("project.home.noModels", "No models yet")}
        onOpen={(id) => void navigate({ href: `${base}/models?provider=${encodeURIComponent(id.slice(0, id.indexOf("/")))}` })}
      />
    </DashboardCell>
  );
}

function KeysCell({ org, project }: { org: string; project: string }) {
  const rights = useProjectRights(org, project);
  const navigate = useNavigate();
  const base = projectPath(org, project);
  const keys = useQuery({ ...allKeysQuery(org, project), enabled: rights.has("keys.all.view") });
  if (!rights.has("keys.all.view")) return null;
  const list = [...(keys.data ?? [])].sort((a, b) => (b.last_used_at ?? "").localeCompare(a.last_used_at ?? ""));
  return (
    <DashboardCell
      className="lg:col-span-1"
      title={t("project.home.keys", "{count, plural, one {# API key} other {# API keys}}", { count: list.length })}
      href={`${base}/analytics/keys`}
      actions={
        <Button size="sm" variant="ghost" href={`${base}/keys`}>
          {t("project.home.manage", "Manage")}
        </Button>
      }
    >
      <RankList
        aria-label={t("shell.page.apiKeys", "API keys")}
        loading={keys.isPending}
        items={list.slice(0, 5).map((k) => ({
          id: k.id,
          label: <span className="min-w-0 truncate">{keyLabel(k)}</span>,
          secondary: k.last_used_at ? formatRelative(k.last_used_at) : t("project.home.neverUsed", "Never used"),
        }))}
        empty={t("project.home.noKeys", "No API keys yet")}
        onOpen={(id) => void navigate({ href: `${base}/keys?key=${encodeURIComponent(id)}` })}
      />
    </DashboardCell>
  );
}

function MembersCell({ org, project }: { org: string; project: string }) {
  const rights = useProjectRights(org, project);
  const session = useSession();
  const navigate = useNavigate();
  const base = projectPath(org, project);
  const members = useQuery({ ...projectMembersQuery(org, project), enabled: rights.has("project.members.view") });
  if (!rights.has("project.members.view")) return null;
  const list = members.data ?? [];
  return (
    <DashboardCell
      className="lg:col-span-1"
      title={t("project.home.members", "{count, plural, one {# member} other {# members}}", { count: list.length })}
      href={`${base}/analytics/members`}
      actions={
        <Button size="sm" variant="ghost" href={`${base}/members`}>
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
        onOpen={() => void navigate({ href: `${base}/members` })}
      />
    </DashboardCell>
  );
}

function LimitsCell({ org, project, className }: { org: string; project: string; className: string }) {
  const rights = useProjectRights(org, project);
  const base = projectPath(org, project);
  const limits = useQuery({ ...limitsQuery(org, project, { level: "project" }), enabled: rights.has("limits.view") });
  if (!rights.has("limits.view")) return null;
  const list = limits.data ?? [];
  return (
    <DashboardCell
      className={className}
      title={t("project.overview.limits", "Limits")}
      actions={
        <Button size="sm" variant="ghost" href={`${base}/limits`}>
          {t("project.home.manage", "Manage")}
        </Button>
      }
    >
      <CellInset>
        <div className="flex flex-col gap-20">
          {limits.isError ? (
            <Text variant="small" tone="tertiary">
              {untoldReason(limits.error) ?? t("project.overview.limitsFailed", "The limits couldn't load; the notification says why.")}
            </Text>
          ) : !limits.isPending && list.length === 0 ? (
            <Text variant="small" tone="tertiary">
              {t("project.overview.noLimits", "No project limits. Keys and members can still have their own.")}
            </Text>
          ) : (
            list.map((l) => <Limit key={l.id} limit={l} />)
          )}
        </div>
      </CellInset>
    </DashboardCell>
  );
}

function LiveCell({ org, project, className }: { org: string; project: string; className: string }) {
  const navigate = useNavigate();
  const base = projectPath(org, project);
  const logs = useQuery({ ...newestLogsQuery(org, project, {}), refetchInterval: LIVE_EVERY });
  const keys = useQuery(allKeysQuery(org, project, { include_archived: true }));
  const rights = useProjectRights(org, project);
  const providers = useProviders(org);
  const models = useQuery({ ...catalogueQuery(org, project), enabled: rights.has("models.view") });
  const modelOf = (provider: string, model: string, asked: string) => {
    if (!provider || !model) return <span className="min-w-0 truncate">{asked}</span>;
    const name = models.data?.find((m) => m.provider === provider && m.model_id === model)?.effective.display_name;
    return (
      <ModelLabel className="min-w-0" provider={provider} providerName={providers.name(provider)} model={model} {...(name && name !== model ? { name } : {})} />
    );
  };
  const keyName = (id: string | null | undefined) => {
    if (!id) return t("project.overview.playground", "Playground");
    const k = keys.data?.find((x) => x.id === id);
    return k ? keyLabel(k) : "";
  };
  const items: RankItem[] = (logs.data?.rows ?? []).slice(0, FEED_ROWS).map((l) => ({
    id: l.id,
    label: (
      <>
        <StatusCode code={l.status} />
        {modelOf(l.provider, l.model, l.requested_model)}
        <Text variant="small" tone="tertiary" truncate>
          {keyName(l.key_id)}
        </Text>
      </>
    ),
    secondary: formatRelative(l.started_at),
    value: formatDuration(l.duration_ms),
  }));
  return (
    <DashboardCell
      className={className}
      title={t("project.home.latest", "Latest requests")}
      actions={
        <Button size="sm" variant="ghost" href={`${base}/logs`}>
          {t("project.home.requestLogs", "Request logs")}
        </Button>
      }
    >
      {logs.isError ? (
        <CellInset>
          <Text variant="small" tone="tertiary">
            {untoldReason(logs.error) ?? t("project.home.latestFailed", "The latest requests couldn't load; the notification says why.")}
          </Text>
        </CellInset>
      ) : (
        <RankList
          aria-label={t("project.home.latest", "Latest requests")}
          items={items}
          loading={logs.isPending}
          rows={FEED_ROWS}
          empty={t("project.home.noRequests", "No requests yet; they show here as they arrive")}
          onOpen={(id) => void navigate({ href: `${base}/logs?log=${encodeURIComponent(id)}` })}
        />
      )}
    </DashboardCell>
  );
}

function ChangesCell({ org, project, className }: { org: string; project: string; className: string }) {
  const partyName = usePartyName(org, project);
  const base = projectPath(org, project);
  return (
    <DashboardCell
      className={className}
      title={t("kaeso.home.recentChanges", "Recent changes")}
      actions={
        <Button size="sm" variant="ghost" href={`${base}/audit-log`}>
          {t("project.home.auditLog", "Audit log")}
        </Button>
      }
    >
      <RecentChanges
        scope={{ org, project }}
        href={`${base}/audit-log`}
        actorName={(e) => partyName({ kind: e.actor.kind === "service_account" ? "service_account" : "user", id: e.actor.id })}
      />
    </DashboardCell>
  );
}
