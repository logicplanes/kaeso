import { useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Avatar, Badge, Button, DataTable, Empty, Link, ModelLabel, ProviderBadge, Section, Text, col, icons, type SortingState } from "@logicplanes/kaeso-ui";
import { formatNumber, formatTokens } from "@logicplanes/kaeso-ui/format";
import type { ProjectView, UsageRow } from "~/api/generated/types";
import { serviceAccountLookupQuery } from "~/api/queries/members";
import { orgMembersQuery } from "~/api/queries/orgs";
import { orgUsageQuery } from "~/api/queries/usage";
import { orgCredentialsQuery } from "~/api/queries/credentials";
import { everyProjectRights, useOrgRights, useSession } from "~/auth/session";
import { useProviders } from "~/kaeso/providers";
import { totalTokens } from "~/kaeso/tokens";
import { projectPath } from "~/shell/pages";
import { usePhone } from "~/shell/use-phone";
import { costWidth, PHONE_NAME_MIN_WIDTH, UsageCost } from "~/kaeso/media-usage";
import { costColumns, saveCsv, sortUsage, sortsUsage, usageCsv } from "~/kaeso/usage-table";
import { useOrgMoney } from "~/kaeso/org-money";
import { columnWidth } from "~/format/text-width";
import { GroupBy } from "~/kaeso/group-by";
import { t } from "~/i18n";

export type OrgBreakdown = "project" | "model" | "member" | "billed_to";

const options = (): { value: OrgBreakdown; label: string }[] => [
  { value: "project", label: t("orgHome.byProject", "By project") },
  { value: "model", label: t("orgHome.byModel", "By model") },
  { value: "member", label: t("orgHome.byMember", "By member") },
  { value: "billed_to", label: t("orgHome.byBilledTo", "By key that paid") },
];
const heading = (by: OrgBreakdown) =>
  by === "project"
    ? t("common.project", "Project")
    : by === "model"
      ? t("common.model", "Model")
      : by === "billed_to"
        ? t("orgHome.billedTo", "Billed to")
        : t("orgHome.member", "Member");

const NOTABLE_ERROR_RATE = 0.05;
const LABEL_WIDTH = 200;
const LABEL_MIN_WIDTH = 120;
const REQUESTS_WIDTH = 104;
const ERRORS_WIDTH = 112;
const NUMBER_WIDTH = 88;
const VALUE_MAX = 168;

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
  const navigate = useNavigate();
  const phone = usePhone();
  const rights = useOrgRights(org);
  const providers = useProviders(org);
  const money = useOrgMoney(org);
  const usage = useQuery({ ...orgUsageQuery(org, from, to, by), enabled: ready });
  const members = useQuery({ ...orgMembersQuery(org), enabled: by === "member" && rights.has("members.view") });
  const keys = useQuery({ ...orgCredentialsQuery(org), enabled: by === "billed_to" && rights.has("provider_credentials.view") });
  const paidBy = (id: string) => keys.data?.find((k) => k.id === id);
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
  const loading =
    usage.isPending ||
    (by === "project" && !projects) ||
    (by === "member" && (members.isLoading || accounts.isLoading)) ||
    (by === "billed_to" && keys.isLoading);
  const [sorting, setSorting] = useState<SortingState>([]);
  const rows = sortUsage(loading ? [] : [...(usage.data ?? [])].sort((a, b) => b.requests - a.requests || a.key.localeCompare(b.key)), sorting);
  // A row as text, for the download.
  const labelText = (r: UsageRow): string => {
    if (by === "project") return projects?.find((x) => x.id === r.key)?.name ?? r.key;
    if (by === "model") return r.key.includes("/") ? r.key : t("orgHome.noModel", "No model");
    if (by === "billed_to")
      return r.key === "none" ? t("orgHome.noProviderSent", "No provider") : (paidBy(r.key)?.name ?? t("orgHome.removedKey", "Removed key"));
    const [kind, id = r.key] = r.key.split(":");
    if (kind === "service_account") return named.find((n) => n.account.id === id)?.account.name ?? t("orgHome.serviceAccount", "Service account");
    return members.data?.find((x) => x.user.id === id)?.user.name ?? t("orgHome.formerMember", "Former member");
  };

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
      const [provider = "", ...rest] = r.key.split("/");
      const model = rest.join("/");
      if (!model) return <Text tone="secondary">{t("orgHome.noModel", "No model")}</Text>;
      return (
        <ModelLabel
          provider={provider}
          providerName={providers.name(provider)}
          model={model}
          mark={!!provider}
          {...(r.name && r.name !== model ? { name: r.name } : {})}
        />
      );
    }
    if (by === "billed_to") {
      if (r.key === "none") return <Text tone="secondary">{t("orgHome.noProviderSent", "No provider")}</Text>;
      const k = paidBy(r.key);
      if (!k) return <Pair phone={phone} main={<Text tone="secondary">{t("orgHome.removedKey", "Removed key")}</Text>} sub={<Id value={r.key} />} />;
      return (
        <span className="flex min-w-0 items-center gap-8">
          <ProviderBadge provider={k.provider} name={providers.name(k.provider)} />
          <Text truncate>{k.name}</Text>
        </span>
      );
    }
    const [kind, id = r.key] = r.key.split(":");
    if (kind === "service_account") {
      const found = named.find((n) => n.account.id === id);
      return (
        <Pair
          phone={phone}
          main={<Text truncate>{found?.account.name ?? t("orgHome.serviceAccount", "Service account")}</Text>}
          sub={
            found ? (
              <Text variant={phone ? "caption" : "small"} tone={phone ? "tertiary" : "secondary"} truncate>
                {t("orgHome.serviceAccountIn", "Service account in {project}", { project: found.project })}
              </Text>
            ) : (
              <Id value={id} />
            )
          }
        />
      );
    }
    const m = members.data?.find((x) => x.user.id === id);
    if (!m)
      return (
        <Pair
          phone={phone}
          main={<Text tone="secondary">{members.data ? t("orgHome.formerMember", "Former member") : t("orgHome.member", "Member")}</Text>}
          sub={<Id value={id} />}
        />
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
      { id: "key", header: heading(by), size: LABEL_WIDTH, minWidth: phone ? PHONE_NAME_MIN_WIDTH : LABEL_MIN_WIDTH, priority: 1, grow: true },
      (r: UsageRow) => r.key,
      (_v, r) => label(r),
    ),
    col.custom(
      {
        id: "requests",
        header: t("common.requests", "Requests"),
        size: fit(
          t("common.requests", "Requests"),
          rows.map((r) => formatNumber(r.requests)),
          phone ? NUMBER_WIDTH : REQUESTS_WIDTH,
        ),
        priority: 1,
      },
      (r: UsageRow) => r.requests,
      (v: number) => <Text tabular>{formatNumber(v)}</Text>,
      { align: "end" },
    ),
    col.custom(
      {
        id: "errors",
        header: t("orgHome.errors", "Errors"),
        size: fit(t("orgHome.errors", "Errors"), rows.map(errorsText), ERRORS_WIDTH),
        priority: 2,
        tone: (r: UsageRow) => (notable(r) ? "danger" : undefined),
      },
      (r: UsageRow) => r.errors,
      (_v, r) => <Errors errors={r.errors} requests={r.requests} />,
      { align: "end" },
    ),
    col.custom(
      {
        id: "tokens",
        header: t("common.tokens", "Tokens"),
        size: fit(
          t("common.tokens", "Tokens"),
          rows.map((r) => formatTokens(totalTokens(r))),
          NUMBER_WIDTH,
        ),
        priority: 2,
      },
      (r: UsageRow) => totalTokens(r),
      (v: number) => <Text tabular>{formatTokens(v)}</Text>,
      { align: "end" },
    ),
    ...(showCost
      ? [
          col.custom(
            {
              id: "cost",
              header: t("common.cost", "Cost"),
              size: fit(
                t("common.cost", "Cost"),
                rows.map((r) => money.period(r.converted_cost)),
                costWidth(rows, NUMBER_WIDTH, phone),
              ),
              priority: 1,
            },
            (r: UsageRow) => r.converted_cost ?? "",
            (_v: string, r: UsageRow) => <UsageCost row={r} money={money} />,
            { align: "end" },
          ),
        ]
      : []),
  ].map((c) => ({ ...c, enableSorting: sortsUsage(String(c.id)), enableResizing: false }));
  const download = () =>
    saveCsv(
      `usage-${by}-${from}-${to}.csv`,
      usageCsv(
        [
          { header: heading(by), value: labelText },
          { header: t("orgHome.id", "Id"), value: (r) => r.key },
          { header: t("common.requests", "Requests"), value: (r) => r.requests },
          { header: t("orgHome.errors", "Errors"), value: (r) => r.errors },
          { header: t("common.tokens", "Tokens"), value: (r) => totalTokens(r) },
          ...(showCost ? costColumns(money) : []),
        ],
        rows,
      ),
    );

  return (
    <Section
      title={t("orgHome.breakdown", "Breakdown")}
      description={
        showCost
          ? t("orgHome.breakdownHintCost", "Requests, errors, tokens and cost in the period")
          : t("orgHome.breakdownHint", "Requests, errors and tokens in the period")
      }
      actions={
        <span className="flex max-w-full min-w-0 flex-wrap items-center gap-8">
          <GroupBy label={t("orgHome.groupBy", "Group by")} value={by} onChange={onByChange} options={options()} />
          <Button size="sm" variant="ghost" leadingIcon={icons.download} disabled={rows.length === 0} onClick={download}>
            {t("orgHome.downloadCsv", "Download CSV")}
          </Button>
        </span>
      }
    >
      <DataTable
        aria-label={t("orgHome.usageBreakdown", "Usage breakdown")}
        data={rows}
        columns={columns}
        getRowId={(r) => r.key}
        sorting={sorting}
        onSortingChange={setSorting}
        manualSorting
        // The whole row opens the project, so a phone gets a row-high target instead of the name's line.
        {...(by === "project"
          ? {
              onRowClick: (r: UsageRow) => {
                if (projects?.some((p) => p.id === r.key)) void navigate({ href: projectPath(org, r.key) });
              },
            }
          : {})}
        loading={loading}
        error={usage.isError}
        onRetry={() => void usage.refetch()}
        empty={<Empty size="compact" title={t("orgHome.noRequests", "No requests in this period")} />}
      />
    </Section>
  );
}

const errorRate = (r: Pick<UsageRow, "errors" | "requests">) => {
  const rate = r.requests ? r.errors / r.requests : 0;
  return formatNumber(rate, rate === 1 ? { style: "percent" } : { style: "percent", minimumFractionDigits: 1, maximumFractionDigits: 1 });
};
// The errors cell as text, to size its column: the rate and the count beside it.
const errorsText = (r: UsageRow) => (r.errors === 0 ? "0" : `${errorRate(r)} ${formatNumber(r.errors)}`);
const fit = (header: string, values: string[], min: number) => columnWidth(header, values, { min, max: VALUE_MAX });

const notable = (r: UsageRow) => r.errors > 0 && r.requests > 0 && r.errors / r.requests >= NOTABLE_ERROR_RATE;

function Errors({ errors, requests }: { errors: number; requests: number }) {
  if (errors === 0) return <Text tone="tertiary">0</Text>;
  return (
    <span className="inline-flex items-baseline gap-6">
      <Text variant="small" tone="tertiary" tabular>
        {errorRate({ errors, requests })}
      </Text>
      <Text tone="inherit" tabular>
        {formatNumber(errors)}
      </Text>
    </span>
  );
}

/** A row's name with what tells it apart: beside it, or under it on a phone, where the column is too narrow for both. */
function Pair({ phone, main, sub }: { phone: boolean; main: ReactNode; sub: ReactNode }) {
  return (
    <span className={phone ? "flex min-w-0 flex-col" : "flex min-w-0 items-center gap-8"}>
      {main}
      {sub}
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
