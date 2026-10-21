import { useQuery } from "@tanstack/react-query";
import { Avatar, Badge, Link, ModelLabel, Section, Segmented, Table, Text } from "@logicplanes/kaeso-ui";
import { formatNumber, formatTokens } from "@logicplanes/kaeso-ui/format";
import type { ProjectView, UsageRow } from "~/api/generated/types";
import { serviceAccountLookupQuery } from "~/api/queries/members";
import { orgMembersQuery } from "~/api/queries/orgs";
import { orgUsageQuery } from "~/api/queries/usage";
import { everyProjectRights, useOrgRights, useSession } from "~/auth/session";
import { totalTokens } from "~/kaeso/tokens";
import { periodUSD } from "~/kaeso/usage-panels";
import { projectPath } from "~/shell/pages";
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
const NAME_COLUMN = "w-full max-w-0";
const NUMBER_COLUMN = "whitespace-nowrap";
const PHONE_HIDDEN = "hidden whitespace-nowrap sm:table-cell";

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
  const rights = useOrgRights(org);
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
  const used = usage.data ?? [];
  const idle =
    by === "project" && used.length > 0
      ? (projects ?? []).filter((p) => !p.archived_at && !used.some((r) => r.key === p.id)).map((p) => quietRow(p.id, showCost))
      : [];
  const projectName = (key: string) => projects?.find((p) => p.id === key)?.name ?? key;
  const rows = loading ? [] : [...used, ...idle].sort((a, b) => b.requests - a.requests || projectName(a.key).localeCompare(projectName(b.key)));

  const label = (r: UsageRow) => {
    if (by === "project") {
      const p = projects?.find((x) => x.id === r.key);
      if (!p) return <Id value={r.key} />;
      return (
        <span className="flex min-w-0 items-center gap-8">
          <Avatar name={p.name} {...(p.avatar_url ? { src: p.avatar_url } : {})} kind="org" size={20} decorative />
          <span className="min-w-0 truncate">
            <Link href={projectPath(org, p.id)}>{p.name}</Link>
          </span>
          {p.is_default && <Badge>{t("orgHome.default", "Default")}</Badge>}
          {p.archived_at && <Badge>{t("common.archived", "Archived")}</Badge>}
        </span>
      );
    }
    if (by === "model") {
      const [provider, ...model] = r.key.split("/");
      if (!provider || !model.join("/")) return <Text tone="secondary">{t("orgHome.noModel", "No model")}</Text>;
      return <ModelLabel className="max-w-full" provider={provider} model={model.join("/")} />;
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
        <Avatar name={m.user.name} size={20} decorative {...(m.user.avatar_url ? { src: m.user.avatar_url } : {})} />
        <Text truncate>{m.user.id === session.user.id ? t("orgHome.nameYou", "{name} (you)", { name: m.user.name }) : m.user.name}</Text>
      </span>
    );
  };

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
      <Table aria-label={t("orgHome.usageBreakdown", "Usage breakdown")} {...(loading ? { "aria-busy": true } : {})}>
        <Table.Head>
          <Table.Row>
            <Table.HeaderCell className={NAME_COLUMN}>{heading(by)}</Table.HeaderCell>
            <Table.HeaderCell align="end" className={NUMBER_COLUMN}>
              {t("common.requests", "Requests")}
            </Table.HeaderCell>
            <Table.HeaderCell align="end" className={PHONE_HIDDEN}>
              {t("orgHome.errors", "Errors")}
            </Table.HeaderCell>
            <Table.HeaderCell align="end" className={PHONE_HIDDEN}>
              {t("common.tokens", "Tokens")}
            </Table.HeaderCell>
            {showCost && (
              <Table.HeaderCell align="end" className={NUMBER_COLUMN}>
                {t("common.cost", "Cost")}
              </Table.HeaderCell>
            )}
          </Table.Row>
        </Table.Head>
        <Table.Body>
          {rows.length === 0 && !loading && (
            <Table.Row>
              <Table.Cell colSpan={showCost ? 5 : 4}>
                <Text tone="secondary">
                  {usage.isError
                    ? t("orgHome.breakdownFailed", "The breakdown couldn't load; the notification says why.")
                    : t("orgHome.noRequests", "No requests in this period.")}
                </Text>
              </Table.Cell>
            </Table.Row>
          )}
          {rows.map((r) => (
            <Table.Row key={r.key}>
              <Table.Cell className={NAME_COLUMN}>{label(r)}</Table.Cell>
              <Table.Cell align="end" className={NUMBER_COLUMN}>
                <Text tabular>{formatNumber(r.requests)}</Text>
              </Table.Cell>
              <Table.Cell align="end" className={PHONE_HIDDEN} {...(notable(r) ? { tone: "danger" as const } : {})}>
                <Errors errors={r.errors} requests={r.requests} />
              </Table.Cell>
              <Table.Cell align="end" className={PHONE_HIDDEN}>
                <Text tabular>{formatTokens(totalTokens(r))}</Text>
              </Table.Cell>
              {showCost && (
                <Table.Cell align="end" className={NUMBER_COLUMN}>
                  <Text tabular>{r.cost ? periodUSD(r.cost) : "—"}</Text>
                </Table.Cell>
              )}
            </Table.Row>
          ))}
        </Table.Body>
      </Table>
    </Section>
  );
}

const quietRow = (key: string, showCost: boolean): UsageRow => ({
  key,
  requests: 0,
  errors: 0,
  input_tokens: 0,
  output_tokens: 0,
  cache_read_tokens: 0,
  cache_write_tokens: 0,
  reasoning_tokens: 0,
  ...(showCost ? { cost: "0" } : {}),
});

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
