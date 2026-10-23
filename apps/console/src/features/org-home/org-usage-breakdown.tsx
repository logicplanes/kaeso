import { useQuery } from "@tanstack/react-query";
import { Avatar, Badge, DataTable, Empty, Link, ModelLabel, Section, Segmented, Text, col } from "@logicplanes/kaeso-ui";
import { formatNumber, formatTokens } from "@logicplanes/kaeso-ui/format";
import type { ProjectView, UsageRow } from "~/api/generated/types";
import { serviceAccountLookupQuery } from "~/api/queries/members";
import { orgMembersQuery } from "~/api/queries/orgs";
import { orgUsageQuery } from "~/api/queries/usage";
import { everyProjectRights, useOrgRights, useSession } from "~/auth/session";
import { useProviders } from "~/kaeso/providers";
import { totalTokens } from "~/kaeso/tokens";
import { periodUSD } from "~/kaeso/usage-panels";
import { projectPath } from "~/shell/pages";
import { usePhone } from "~/shell/use-phone";
import { t } from "~/i18n";

export type OrgBreakdown = "project" | "model" | "member";

const options = (): { value: OrgBreakdown; label: string }[] => [
  { value: "project", label: t("orgHome.byProject", "By project") },
  { value: "model", label: t("orgHome.byModel", "By model") },
  { value: "member", label: t("orgHome.byMember", "By member") },
];
const heading = (by: OrgBreakdown) =>
  by === "project" ? t("common.project", "Project") : by === "model" ? t("common.model", "Model") : t("orgHome.member", "Member");

const NOTABLE_ERROR_RATE = 0.05;
const LABEL_WIDTH = 200;
const LABEL_MIN_WIDTH = 120;
const REQUESTS_WIDTH = 88;
const ERRORS_WIDTH = 112;
const NUMBER_WIDTH = 88;

export function OrgUsageBreakdown({
  org,
  from,
  to,
  by,
  onByChange,
  projects,
  showCost,
  ready = true,
}: {
  org: string;
  from: string;
  to: string;
  by: OrgBreakdown;
  onByChange: (by: OrgBreakdown) => void;
  projects: ProjectView[] | undefined;
  showCost: boolean;
  ready?: boolean | undefined;
}) {
  const session = useSession();
  const phone = usePhone();
  const rights = useOrgRights(org);
  const providers = useProviders(org);
  const usage = useQuery({ ...orgUsageQuery(org, from, to, by), enabled: ready });
  const members = useQuery({ ...orgMembersQuery(org), enabled: by === "member" && rights.has("members.view") });
  const everyProject = everyProjectRights(session, org).has("service_accounts.view");
  const accountIds = by === "member" ? (usage.data ?? []).flatMap((r) => (r.key.startsWith("service_account:") ? [r.key.slice(16)] : [])).sort() : [];
  const accounts = useQuery({
    ...serviceAccountLookupQuery(
      org,
      (projects ?? []).map((p) => ({ id: p.id, name: p.name })),
      accountIds,
    ),
    enabled: everyProject && !!projects && accountIds.length > 0,
  });
  const named = accounts.data ?? [];
  const loading = usage.isPending || (by === "project" && !projects) || (by === "member" && (members.isLoading || accounts.isLoading));
  const rows = loading ? [] : [...(usage.data ?? [])].sort((a, b) => b.requests - a.requests || a.key.localeCompare(b.key));

  const label = (r: UsageRow) => {
    if (by === "project") {
      const p = projects?.find((x) => x.id === r.key);
      if (!p) return <Id value={r.key} />;
      return (
        <span className="flex min-w-0 items-center gap-8">
          {!phone && <Avatar name={p.name} {...(p.avatar_url ? { src: p.avatar_url } : {})} kind="org" size={20} decorative />}
          <Link href={projectPath(org, p.id)} className="min-w-0 truncate">
            {p.name}
          </Link>
          {p.archived_at && <Badge>{t("common.archived", "Archived")}</Badge>}
        </span>
      );
    }
    if (by === "model") {
      const [provider, ...model] = r.key.split("/");
      if (!provider || !model.join("/")) return <Text tone="secondary">{t("orgHome.noModel", "No model")}</Text>;
      return <ModelLabel provider={provider} providerName={providers.name(provider)} model={model.join("/")} />;
    }
    const [kind, id = r.key] = r.key.split(":");
    if (kind === "service_account") {
      const found = named.find((n) => n.account.id === id);
      return (
        <span className="flex min-w-0 items-center gap-8">
          <Text truncate>{found?.account.name ?? t("orgHome.serviceAccount", "Service account")}</Text>
          {found ? (
            <Text variant="small" tone="secondary" truncate>
              {t("orgHome.serviceAccountIn", "Service account in {project}", { project: found.project })}
            </Text>
          ) : (
            <Id value={id} />
          )}
        </span>
      );
    }
    const m = members.data?.find((x) => x.user.id === id);
    if (!m)
      return (
        <span className="flex min-w-0 items-center gap-8">
          <Text tone="secondary">{members.data ? t("orgHome.formerMember", "Former member") : t("orgHome.member", "Member")}</Text>
          <Id value={id} />
        </span>
      );
    return (
      <span className="flex min-w-0 items-center gap-8">
        {!phone && <Avatar name={m.user.name} size={20} decorative {...(m.user.avatar_url ? { src: m.user.avatar_url } : {})} />}
        <Text truncate>{m.user.id === session.user.id ? t("orgHome.nameYou", "{name} (you)", { name: m.user.name }) : m.user.name}</Text>
      </span>
    );
  };

  const columns = [
    col.custom(
      { id: "key", header: heading(by), size: LABEL_WIDTH, minWidth: LABEL_MIN_WIDTH, priority: 1, grow: true },
      (r: UsageRow) => r.key,
      (_v, r) => label(r),
    ),
    col.custom(
      { id: "requests", header: t("common.requests", "Requests"), size: REQUESTS_WIDTH, priority: 1 },
      (r: UsageRow) => r.requests,
      (v: number) => <Text tabular>{formatNumber(v)}</Text>,
      { align: "end" },
    ),
    col.custom(
      { id: "errors", header: t("orgHome.errors", "Errors"), size: ERRORS_WIDTH, priority: 2, tone: (r: UsageRow) => (notable(r) ? "danger" : undefined) },
      (r: UsageRow) => r.errors,
      (_v, r) => <Errors errors={r.errors} requests={r.requests} />,
      { align: "end" },
    ),
    col.custom(
      { id: "tokens", header: t("common.tokens", "Tokens"), size: NUMBER_WIDTH, priority: 2 },
      (r: UsageRow) => totalTokens(r),
      (v: number) => <Text tabular>{formatTokens(v)}</Text>,
      { align: "end" },
    ),
    ...(showCost
      ? [
          col.custom(
            { id: "cost", header: t("common.cost", "Cost"), size: NUMBER_WIDTH, priority: 1 },
            (r: UsageRow) => r.cost ?? "",
            (v: string) => <Text tabular>{v ? periodUSD(v) : "—"}</Text>,
            { align: "end" },
          ),
        ]
      : []),
  ].map((c) => ({ ...c, enableSorting: false, enableResizing: false }));

  return (
    <Section
      title={t("orgHome.breakdown", "Breakdown")}
      description={
        showCost
          ? t("orgHome.breakdownHintCost", "Requests, errors, tokens and cost in the period")
          : t("orgHome.breakdownHint", "Requests, errors and tokens in the period")
      }
      actions={
        <Segmented
          aria-label={t("orgHome.groupBy", "Group by")}
          size="sm"
          value={by}
          onValueChange={(v) => onByChange(v as OrgBreakdown)}
          options={options()}
        />
      }
    >
      <DataTable
        aria-label={t("orgHome.usageBreakdown", "Usage breakdown")}
        data={rows}
        columns={columns}
        getRowId={(r) => r.key}
        loading={loading}
        error={usage.isError}
        onRetry={() => void usage.refetch()}
        empty={<Empty size="compact" title={t("orgHome.noRequests", "No requests in this period")} />}
      />
    </Section>
  );
}

const notable = (r: UsageRow) => r.errors > 0 && r.requests > 0 && r.errors / r.requests >= NOTABLE_ERROR_RATE;

function Errors({ errors, requests }: { errors: number; requests: number }) {
  if (errors === 0) return <Text tone="tertiary">0</Text>;
  const rate = requests ? errors / requests : 0;
  return (
    <span className="inline-flex items-baseline gap-6">
      <Text variant="small" tone="tertiary" tabular>
        {formatNumber(rate, rate === 1 ? { style: "percent" } : { style: "percent", minimumFractionDigits: 1, maximumFractionDigits: 1 })}
      </Text>
      <Text tone="inherit" tabular>
        {formatNumber(errors)}
      </Text>
    </span>
  );
}

function Id({ value }: { value: string }) {
  return (
    <Text mono variant="small" tone="secondary" truncate>
      {value}
    </Text>
  );
}
