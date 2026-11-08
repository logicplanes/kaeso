import { useQuery } from "@tanstack/react-query";
import { DataTable, Empty, ModelLabel, ProviderBadge, Section, Segmented, Text, col } from "@logicplanes/kaeso-ui";
import { formatNumber, formatPercent, formatTokens } from "@logicplanes/kaeso-ui/format";
import type { UsageRow } from "~/api/generated/types";
import { allKeysQuery } from "~/api/queries/keys";
import { useProviders } from "~/kaeso/providers";
import { totalTokens } from "~/kaeso/tokens";
import { periodUSD } from "~/kaeso/usage-panels";
import { bucketLabel, type Granularity } from "~/format/buckets";
import { keyLabel, serviceAccountName, usePartyName, usePartyNamesLoading } from "./keys/party";
import type { AnalyticsSearch } from "~/kaeso/overview/views";
import { useProjectRights } from "~/auth/rights";
import { t } from "~/i18n";
import { common } from "~/i18n/common";

const byOptions = (g: Granularity) => [
  { value: "day", label: t("project.overview.byTime", "{granularity, select, week {By week} month {By month} other {By day}}", { granularity: g }) },
  { value: "model", label: t("project.overview.byModel", "By model") },
  { value: "key", label: t("project.overview.byKey", "By key") },
  { value: "member", label: t("project.overview.byMember", "By member") },
  { value: "route", label: t("project.overview.byRoute", "By route") },
];
const timeHeader = (g: Granularity) => t("project.overview.time", "{granularity, select, week {Week} month {Month} other {Day}}", { granularity: g });

const LABEL_WIDTH = 200;
const LABEL_MIN_WIDTH = 120;
const NUMBER_WIDTH = 88;
const COST_WIDTH = 96;

const NOTABLE_ERROR_RATE = 0.05;

export function Breakdown({
  org,
  project,
  rows,
  by,
  granularity = "day",
  withYear = false,
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
  by: NonNullable<AnalyticsSearch["by"]>;
  granularity?: Granularity | undefined;
  withYear?: boolean | undefined;
  showCost: boolean;
  modelName: (key: string) => string;
  loading: boolean;
  failed: boolean;
  onRetry: () => void;
  onByChange: (by: NonNullable<AnalyticsSearch["by"]>) => void;
}) {
  const rights = useProjectRights(org, project);
  const providers = useProviders(org);
  const keys = useQuery({ ...allKeysQuery(org, project, { include_archived: true }), enabled: by === "key" && rights.has("keys.all.view") });
  const partyName = usePartyName(org, project);
  const partiesLoading = usePartyNamesLoading(org, project);
  const keyName = (id: string) => {
    if (id === "playground") return t("project.overview.playground", "Playground");
    const key = keys.data?.find((k) => k.id === id);
    return key ? keyLabel(key) : id;
  };
  const waiting = !failed && (loading || (by === "key" && keys.isLoading) || (by === "member" && partiesLoading));
  const sorted = waiting ? [] : by === "day" ? [...rows].sort((a, b) => b.key.localeCompare(a.key)) : [...rows].sort((a, b) => b.requests - a.requests);
  const label = (r: UsageRow) => {
    if (by === "day") return <Text tabular>{bucketLabel(r.key, granularity, withYear)}</Text>;
    if (by === "model") {
      const [provider = "", ...rest] = r.key.split("/");
      const model = rest.join("/");
      const name = modelName(r.key);
      if (!model) return <Text tone="secondary">{name}</Text>;
      return (
        <ModelLabel className="max-w-full" provider={provider} providerName={providers.name(provider)} model={model} {...(name !== model ? { name } : {})} />
      );
    }
    if (by === "key") return <Text truncate>{keyName(r.key)}</Text>;
    if (by === "route") return <RouteLabel route={r.key} providerName={providers.name} />;
    const [kind, id = ""] = r.key.split(":");
    const sa = kind === "service_account";
    const name = partyName({ kind: sa ? "service_account" : "user", id });
    return <Text truncate>{sa ? serviceAccountName(name) : name}</Text>;
  };
  const header =
    by === "day"
      ? timeHeader(granularity)
      : by === "model"
        ? common.model()
        : by === "key"
          ? t("project.overview.key", "Key")
          : by === "route"
            ? t("project.overview.route", "Route")
            : t("project.overview.member", "Member");
  const cache = col.custom(
    { id: "cache", header: t("project.overview.cacheHit", "Cache hit"), size: NUMBER_WIDTH, priority: by === "route" ? 1 : 3 },
    (r: UsageRow) => r.cache_hit_rate ?? null,
    (v: number | null) => (v === null ? "—" : formatPercent(v, 0)),
    { align: "end" },
  );
  const columns = [
    col.custom(
      { id: "key", header, size: LABEL_WIDTH, minWidth: LABEL_MIN_WIDTH, priority: 1, grow: true },
      (r: UsageRow) => r.key,
      (_v, r) => label(r),
    ),
    col.custom(
      { id: "requests", header: common.requests(), size: NUMBER_WIDTH, priority: 1 },
      (r: UsageRow) => r.requests,
      (v: number) => formatNumber(v),
      {
        align: "end",
      },
    ),
    col.custom(
      {
        id: "errors",
        header: t("project.overview.errors", "Errors"),
        size: NUMBER_WIDTH,
        priority: 2,
        tone: (r: UsageRow) => (r.errors > 0 && r.errors / r.requests >= NOTABLE_ERROR_RATE ? "danger" : undefined),
      },
      (r: UsageRow) => r.errors,
      (v: number) =>
        v === 0 ? (
          <Text tabular tone="secondary">
            0
          </Text>
        ) : (
          <Text tone="inherit" tabular>
            {formatNumber(v)}
          </Text>
        ),
      { align: "end" },
    ),
    col.custom(
      { id: "tokens", header: common.tokens(), size: NUMBER_WIDTH, priority: 2 },
      (r: UsageRow) => totalTokens(r),
      (v: number) => formatTokens(v),
      {
        align: "end",
      },
    ),
    ...(by === "route" ? [] : [cache]),
    ...(showCost
      ? [
          col.custom(
            { id: "cost", header: common.cost(), size: COST_WIDTH, priority: 1 },
            (r: UsageRow) => r.cost ?? "",
            (v: string) => (v ? periodUSD(v) : "—"),
            { align: "end" },
          ),
        ]
      : []),
    ...(by === "route" ? [cache] : []),
  ].map((c) => ({ ...c, enableSorting: false, enableResizing: false }));
  return (
    <Section
      title={t("project.overview.breakdown", "Breakdown")}
      description={
        showCost
          ? t("project.overview.breakdownWithCost", "Requests, errors, tokens, cache hits and cost in the period")
          : t("project.overview.breakdownHint", "Requests, errors, tokens and cache hits in the period")
      }
      actions={
        <Segmented
          aria-label={t("project.overview.groupBy", "Group by")}
          size="sm"
          value={by}
          onValueChange={(v) => onByChange(v as NonNullable<AnalyticsSearch["by"]>)}
          options={byOptions(granularity)}
        />
      }
    >
      <DataTable
        aria-label={t("project.overview.usageBreakdown", "Usage breakdown")}
        data={sorted}
        columns={columns}
        getRowId={(r) => r.key}
        loading={waiting}
        error={failed}
        onRetry={onRetry}
        empty={<Empty size="compact" title={t("project.overview.noRequests", "No requests in this period")} />}
      />
    </Section>
  );
}

const SHORT_DIALECTS: Record<string, string> = {
  anthropic_messages: "Messages",
  openai_chat: "Chat",
  openai_responses: "Responses",
  openai_embeddings: "Embeddings",
  gemini: "Gemini",
  gemini_embeddings: "Gemini Embeddings",
};
const shortDialectLabel = (d: string) => SHORT_DIALECTS[d] ?? d;

function RouteLabel({ route, providerName }: { route: string; providerName: (id: string) => string }) {
  const [ingress = "", egress = "", provider = ""] = route.split(":");
  return (
    <span className="flex min-w-0 items-center gap-8">
      {provider && <ProviderBadge provider={provider} name={providerName(provider)} size="sm" />}
      {ingress ? (
        <span className="flex min-w-0 flex-col">
          <Text truncate>{shortDialectLabel(ingress)}</Text>
          {egress && (
            <Text variant="caption" tone="tertiary" truncate>
              {`→ ${shortDialectLabel(egress)}`}
            </Text>
          )}
        </span>
      ) : (
        <Text tone="secondary">{t("project.overview.routeNotRecorded", "Route not recorded")}</Text>
      )}
    </span>
  );
}
