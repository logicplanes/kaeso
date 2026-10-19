import { useNavigate } from "@tanstack/react-router";
import { useQuery } from "@tanstack/react-query";
import { Button, Empty, InsetContent, LimitMeter, List, ModelLabel, Section, Segmented, StatusCode, Table, Text } from "@logicplanes/kaeso-ui";
import { formatNumber, formatRelative, formatTokens, formatUSD } from "@logicplanes/kaeso-ui/format";
import type { LimitView, UsageRow } from "~/api/generated/types";
import { allKeysQuery } from "~/api/queries/keys";
import { limitsQuery } from "~/api/queries/limits";
import { recentErrorsQuery } from "~/api/queries/recent-errors";
import { errorKindLabel, limitKindLabel, periodLabel } from "~/format/labels";
import { alertMarks } from "~/format/alert-marks";
import { totalTokens } from "~/kaeso/tokens";
import { periodUSD } from "~/kaeso/usage-panels";
import { bucketLabel, type Granularity } from "~/format/buckets";
import { serviceAccountName, usePartyName, usePartyNamesLoading } from "./keys/party";
import { projectPath } from "~/shell/pages";
import type { OverviewSearch } from "./overview-page";
import { useProjectRights } from "~/auth/rights";
import { t } from "~/i18n";
import { common } from "~/i18n/common";

export { CostShare } from "~/kaeso/cost-share";

const byOptions = (g: Granularity) => [
  { value: "day", label: t("project.overview.byTime", "{granularity, select, week {By week} month {By month} other {By day}}", { granularity: g }) },
  { value: "model", label: t("project.overview.byModel", "By model") },
  { value: "key", label: t("project.overview.byKey", "By key") },
  { value: "member", label: t("project.overview.byMember", "By member") },
];
const timeHeader = (g: Granularity) => t("project.overview.time", "{granularity, select, week {Week} month {Month} other {Day}}", { granularity: g });

const NUMBER_COLUMN = "w-96 sm:w-120";
const PHONE_HIDDEN = "hidden w-120 sm:table-cell";

const NOTABLE_ERROR_RATE = 0.05;

export function Breakdown({
  org,
  project,
  rows,
  by,
  granularity = "day",
  showCost,
  modelName,
  loading,
  failed,
  onRetry,
  onByChange,
}: {
  org: string;
  project: string;
  rows: UsageRow[];
  by: NonNullable<OverviewSearch["by"]>;
  granularity?: Granularity | undefined;
  showCost: boolean;
  modelName: (key: string) => string;
  loading: boolean;
  failed: boolean;
  onRetry: () => void;
  onByChange: (by: NonNullable<OverviewSearch["by"]>) => void;
}) {
  const rights = useProjectRights(org, project);
  const keys = useQuery({ ...allKeysQuery(org, project, { include_archived: true }), enabled: by === "key" && rights.has("keys.all.view") });
  const partyName = usePartyName(org, project);
  const partiesLoading = usePartyNamesLoading(org, project);
  const keyName = (id: string) => (id === "playground" ? t("project.overview.playground", "Playground") : (keys.data?.find((k) => k.id === id)?.name ?? id));
  const waiting = !failed && (loading || (by === "key" && keys.isLoading) || (by === "member" && partiesLoading));
  const sorted = waiting ? [] : by === "day" ? [...rows].sort((a, b) => b.key.localeCompare(a.key)) : [...rows].sort((a, b) => b.requests - a.requests);
  const label = (r: UsageRow) => {
    if (by === "day") return <Text tabular>{bucketLabel(r.key, granularity)}</Text>;
    if (by === "model") {
      const [provider = "", ...rest] = r.key.split("/");
      const model = rest.join("/");
      const name = modelName(r.key);
      if (!model) return <Text tone="secondary">{name}</Text>;
      return <ModelLabel className="max-w-full" provider={provider} model={model} {...(name !== model ? { name } : {})} />;
    }
    if (by === "key") return <Text truncate>{keyName(r.key)}</Text>;
    const [kind, id = ""] = r.key.split(":");
    const sa = kind === "service_account";
    const name = partyName({ kind: sa ? "service_account" : "user", id });
    return <Text truncate>{sa ? serviceAccountName(name) : name}</Text>;
  };
  return (
    <Section
      title={t("project.overview.breakdown", "Breakdown")}
      description={
        showCost
          ? t("project.overview.breakdownWithCost", "Requests, errors, tokens and cost in the period")
          : t("project.overview.breakdownHint", "Requests, errors and tokens in the period")
      }
      actions={
        <Segmented
          aria-label={t("project.overview.groupBy", "Group by")}
          size="sm"
          value={by}
          onValueChange={(v) => onByChange(v as NonNullable<OverviewSearch["by"]>)}
          options={byOptions(granularity)}
        />
      }
    >
      <Table aria-label={t("project.overview.usageBreakdown", "Usage breakdown")} className="table-fixed" {...(waiting ? { "aria-busy": true } : {})}>
        <Table.Head>
          <Table.Row>
            <Table.HeaderCell>
              {by === "day"
                ? timeHeader(granularity)
                : by === "model"
                  ? common.model()
                  : by === "key"
                    ? t("project.overview.key", "Key")
                    : t("project.overview.member", "Member")}
            </Table.HeaderCell>
            <Table.HeaderCell align="end" className={NUMBER_COLUMN}>
              {common.requests()}
            </Table.HeaderCell>
            <Table.HeaderCell align="end" className={PHONE_HIDDEN}>
              {t("project.overview.errors", "Errors")}
            </Table.HeaderCell>
            <Table.HeaderCell align="end" className={PHONE_HIDDEN}>
              {common.tokens()}
            </Table.HeaderCell>
            {showCost && (
              <Table.HeaderCell align="end" className={NUMBER_COLUMN}>
                {common.cost()}
              </Table.HeaderCell>
            )}
          </Table.Row>
        </Table.Head>
        <Table.Body>
          {failed ? (
            <Table.Row>
              <Table.Cell colSpan={showCost ? 5 : 4}>
                <Empty
                  size="compact"
                  title={t("project.overview.rowsFailed", "Couldn't load these rows")}
                  actions={
                    <Button size="sm" onClick={onRetry}>
                      {common.tryAgain()}
                    </Button>
                  }
                />
              </Table.Cell>
            </Table.Row>
          ) : (
            sorted.length === 0 &&
            !waiting && (
              <Table.Row>
                <Table.Cell colSpan={showCost ? 5 : 4}>
                  <Text tone="secondary">{t("project.overview.noRequests", "No requests in this period.")}</Text>
                </Table.Cell>
              </Table.Row>
            )
          )}
          {sorted.map((r) => (
            <Table.Row key={r.key}>
              <Table.Cell>{label(r)}</Table.Cell>
              <Table.Cell align="end">{formatNumber(r.requests)}</Table.Cell>
              <Table.Cell
                align="end"
                className={PHONE_HIDDEN}
                {...(r.errors > 0 && r.errors / r.requests >= NOTABLE_ERROR_RATE ? { tone: "danger" as const } : {})}
              >
                {r.errors === 0 ? (
                  <Text tabular tone="secondary">
                    0
                  </Text>
                ) : (
                  <Text tone="inherit" tabular>
                    {formatNumber(r.errors)}
                  </Text>
                )}
              </Table.Cell>
              <Table.Cell align="end" className={PHONE_HIDDEN}>
                {formatTokens(totalTokens(r))}
              </Table.Cell>
              {showCost && <Table.Cell align="end">{r.cost === null || r.cost === undefined ? "—" : periodUSD(r.cost)}</Table.Cell>}
            </Table.Row>
          ))}
        </Table.Body>
      </Table>
    </Section>
  );
}

export function Aside({ org, project, modelCount }: { org: string; project: string; modelCount: number | undefined }) {
  const rights = useProjectRights(org, project);
  const navigate = useNavigate();
  const base = projectPath(org, project);
  const limits = useQuery({ ...limitsQuery(org, project, { level: "project" }), enabled: rights.has("limits.view") });
  const errors = useQuery({ ...recentErrorsQuery(org, project), enabled: rights.has("logs.metadata.view") });
  const recent = errors.data ?? [];
  const keys = useQuery({ ...allKeysQuery(org, project), enabled: rights.has("keys.all.view") });
  const keyCount = keys.data ? formatNumber(keys.data.length) : undefined;
  const go = (href: string) => void navigate({ href });
  const settled = (q: { isPending: boolean }, on: boolean) => !on || !q.isPending;
  if (!settled(limits, rights.has("limits.view")) || !settled(errors, rights.has("logs.metadata.view")) || !settled(keys, rights.has("keys.all.view")))
    return null;
  return (
    <InsetContent>
      {rights.has("limits.view") && (
        <Section title={t("project.overview.limits", "Limits")} spacing="none">
          <div className="flex flex-col gap-16">
            {(limits.data ?? []).length === 0 && !limits.isPending && (
              <Text tone="secondary">{t("project.overview.noLimits", "No project limits. Keys and members can still have their own.")}</Text>
            )}
            {(limits.data ?? []).map((l) => (
              <ProjectLimit key={l.id} limit={l} />
            ))}
          </div>
        </Section>
      )}
      {rights.has("logs.metadata.view") && (
        <Section title={t("project.overview.recentErrors", "Recent errors")} spacing="none">
          {recent.length === 0 && !errors.isPending ? (
            <Text tone="secondary">{t("project.overview.noErrors", "No failed requests.")}</Text>
          ) : (
            <List
              aria-label={t("project.overview.recentErrors", "Recent errors")}
              density="compact"
              loading={errors.isPending}
              items={recent.map((l) => ({
                id: l.id,
                textValue: `${l.status} ${l.error_kind ? errorKindLabel(l.error_kind) : ""} ${l.requested_model}`,
                title: (
                  <span className="flex min-w-0 items-center gap-8">
                    <StatusCode code={l.status} />
                    <span className="min-w-0 truncate">{l.error_kind ? errorKindLabel(l.error_kind) : l.requested_model}</span>
                  </span>
                ),
                date: formatRelative(l.started_at),
              }))}
              onOpen={(item) => go(`${base}/logs?log=${encodeURIComponent(item.id)}`)}
            />
          )}
        </Section>
      )}
      {(modelCount !== undefined || keyCount !== undefined) && (
        <Section title={t("project.overview.thisProject", "This project")} spacing="none">
          <List
            aria-label={t("project.overview.thisProject", "This project")}
            density="compact"
            items={[
              ...(modelCount !== undefined ? [{ id: "models", title: common.models(), meta: <Text tone="secondary">{formatNumber(modelCount)}</Text> }] : []),
              ...(keyCount !== undefined ? [{ id: "keys", title: t("project.overview.keys", "Keys"), meta: <Text tone="secondary">{keyCount}</Text> }] : []),
            ]}
            onOpen={(item) => go(`${base}/${item.id}`)}
          />
        </Section>
      )}
    </InsetContent>
  );
}

function ProjectLimit({ limit }: { limit: LimitView }) {
  const used = Number(limit.used ?? 0);
  const amount = Number(limit.amount);
  const money = limit.kind === "spend_budget";
  const fmt = (v: string | number) => (money ? formatUSD(String(v)) : formatNumber(Number(v)));
  return (
    <LimitMeter
      kind={limit.kind === "spend_budget" ? "spend" : limit.kind === "request_rate" ? "requests" : "tokens"}
      scope={limitKindLabel(limit.kind)}
      window={periodLabel(limit.period)}
      used={used}
      limit={amount}
      valueLabel={t("project.overview.limitUse", "{used} of {limit}", { used: fmt(limit.used ?? 0), limit: fmt(limit.amount) })}
      alerts={alertMarks(limit)}
      hard={limit.hard}
      resets={
        limit.period === "minute"
          ? t("project.overview.lastMinute", "Over the last minute")
          : limit.resets_at
            ? t("project.overview.resets", "Resets {when}", { when: formatRelative(limit.resets_at) })
            : t("project.overview.neverResets", "Never resets")
      }
    />
  );
}
