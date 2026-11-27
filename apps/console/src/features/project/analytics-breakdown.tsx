import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Button, DataTable, Empty, ModelLabel, ProviderBadge, Section, Text, col, icons, type SortingState } from "@logicplanes/kaeso-ui";
import { formatNumber, formatPercent, formatTokens } from "@logicplanes/kaeso-ui/format";
import type { UsageRow } from "~/api/generated/types";
import { allKeysQuery } from "~/api/queries/keys";
import { keyLogFilter, useSourceNames } from "./sources/names";
import { ReportedBy } from "./sources/reported-by";
import { useProviders } from "~/kaeso/providers";
import { totalTokens } from "~/kaeso/tokens";
import { costWidth, PHONE_NAME_MIN_WIDTH, hasMedia, mediaSummary, UsageCost } from "~/kaeso/media-usage";
import { bucketLabel, lastDay, type Granularity } from "~/format/buckets";
import { costColumns, saveCsv, sortUsage, sortsUsage, usageCsv } from "~/kaeso/usage-table";
import { useOrgMoney } from "~/kaeso/org-money";
import { GroupBy } from "~/kaeso/group-by";
import { useRequestLogs } from "~/auth/session";
import { keyLabel, serviceAccountName, usePartyName, usePartyNamesLoading } from "./keys/party";
import type { AnalyticsSearch } from "~/kaeso/overview/views";
import { useProjectRights } from "~/auth/rights";
import { clientLabel } from "~/format/clients";
import { t } from "~/i18n";
import { usePhone } from "~/shell/use-phone";
import { columnWidth } from "~/format/text-width";
import { common } from "~/i18n/common";

type By = NonNullable<AnalyticsSearch["by"]>;

const byOptions = (g: Granularity, clients: boolean, sources: boolean): { value: By; label: string }[] => [
  { value: "day", label: t("project.overview.byTime", "{granularity, select, week {By week} month {By month} other {By day}}", { granularity: g }) },
  { value: "model", label: t("project.overview.byModel", "By model") },
  { value: "key", label: t("project.overview.byKey", "By key") },
  { value: "member", label: t("project.overview.byMember", "By member") },
  { value: "route", label: t("project.overview.byRoute", "By route") },
  ...(clients ? [{ value: "client" as const, label: t("project.overview.byClient", "By client") }] : []),
  ...(sources ? [{ value: "source" as const, label: t("project.overview.bySource", "By source") }] : []),
];
const timeHeader = (g: Granularity) => t("project.overview.time", "{granularity, select, week {Week} month {Month} other {Day}}", { granularity: g });

const LABEL_WIDTH = 200;
const LABEL_MIN_WIDTH = 120;
const NUMBER_WIDTH = 88;
const COST_WIDTH = 96;
const VALUE_MAX = 168;

const fit = (header: string, values: string[], min: number) => columnWidth(header, values, { min, max: VALUE_MAX });

const NOTABLE_ERROR_RATE = 0.05;

export function Breakdown({
  org,
  project,
  from,
  to,
  clients = false,
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
  /** The period's first and last UTC day, which the rows count and their requests are found in. */
  from: string;
  to: string;
  /** Whether usage can be broken down by client: the hourly timeline covers the period. */
  clients?: boolean | undefined;
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
  const navigate = useNavigate();
  const logs = useRequestLogs() && rights.has("logs.metadata.view");
  const [sorting, setSorting] = useState<SortingState>([]);
  const providers = useProviders(org);
  const money = useOrgMoney(org);
  const phone = usePhone();
  const keys = useQuery({ ...allKeysQuery(org, project, { include_archived: true }), enabled: by === "key" && rights.has("keys.all.view") });
  const partyName = usePartyName(org, project);
  const partiesLoading = usePartyNamesLoading(org, project);
  const sources = useSourceNames(org, project);
  const keyName = (id: string) => {
    if (id === "playground") return t("project.overview.playground", "Playground");
    if (id.startsWith("source:")) return sources.asKey(id.slice(7));
    const key = keys.data?.find((k) => k.id === id);
    return key ? keyLabel(key) : id;
  };
  const waiting =
    !failed &&
    (loading || (by === "key" && (keys.isLoading || sources.loading)) || (by === "source" && sources.loading) || (by === "member" && partiesLoading));
  const sorted = waiting ? [] : by === "day" ? [...rows].sort((a, b) => b.key.localeCompare(a.key)) : [...rows].sort((a, b) => b.requests - a.requests);
  const shown = sortUsage(sorted, sorting);
  const memberOf = (key: string) => {
    const [kind, id = ""] = key.split(":");
    return { sa: kind === "service_account", id };
  };
  const labelText = (r: UsageRow): string => {
    if (by === "day") return bucketLabel(r.key, granularity, withYear);
    if (by === "model") return modelName(r.key);
    if (by === "key") return keyName(r.key);
    if (by === "route") return routeText(r.key, providers.name);
    if (by === "client") return clientLabel(r.key);
    if (by === "source") return r.key === "gateway" ? t("project.overview.gateway", "Gateway") : sources.name(r.key);
    const m = memberOf(r.key);
    const name = partyName({ kind: m.sa ? "service_account" : "user", id: m.id });
    return m.sa ? serviceAccountName(name) : name;
  };
  // The request log for what a row counts, in the period's UTC days; none for what the log
  // can't filter by, such as a route that wasn't recorded.
  const logHref = (r: UsageRow): string | undefined => {
    const period = { from, to };
    const filter = ((): Record<string, string> | undefined => {
      if (by === "day") return { from: r.key < from ? from : r.key, to: lastDay(r.key, granularity) > to ? to : lastDay(r.key, granularity) };
      if (by === "model") {
        const [provider = "", ...rest] = r.key.split("/");
        return rest.length ? { ...period, model: rest.join("/"), provider } : undefined;
      }
      if (by === "key") return { ...period, ...keyLogFilter(r.key) };
      // The gateway's own requests are the API's and the Playground's, which the log filters apart.
      if (by === "source") return r.key === "gateway" ? undefined : { ...period, source_id: r.key };
      if (by === "route") {
        const [ingress = "", egress = "", provider = ""] = r.key.split(":");
        return ingress ? { ...period, ingress, egress, provider } : undefined;
      }
      // Requests without a user agent name no client to filter by.
      if (by === "client") return r.key ? { ...period, client: r.key } : undefined;
      const m = memberOf(r.key);
      return { ...period, member: `${m.sa ? "sa" : "user"}:${m.id}` };
    })();
    return filter && `/orgs/${org}/projects/${project}/logs?${new URLSearchParams(filter).toString()}`;
  };
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
    if (by === "source")
      return r.key === "gateway" ? <Text truncate>{t("project.overview.gateway", "Gateway")}</Text> : <ReportedBy source={sources.source(r.key)} />;
    if (by === "route") return <RouteLabel route={r.key} providerName={providers.name} />;
    if (by === "client")
      return (
        <Text truncate tone={r.key ? undefined : "secondary"}>
          {clientLabel(r.key)}
        </Text>
      );
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
            : by === "client"
              ? t("project.overview.client", "Client")
              : by === "source"
                ? t("project.overview.source", "Source")
                : t("project.overview.member", "Member");
  const cacheHeader = t("project.overview.cacheHit", "Cache hit");
  const errorsHeader = t("project.overview.errors", "Errors");
  const cache = col.custom(
    {
      id: "cache",
      header: cacheHeader,
      size: fit(
        cacheHeader,
        sorted.map((r) => (r.cache_hit_rate == null ? "—" : formatPercent(r.cache_hit_rate, 0))),
        NUMBER_WIDTH,
      ),
      priority: by === "route" ? 1 : 3,
    },
    (r: UsageRow) => r.cache_hit_rate ?? null,
    (v: number | null) => (v === null ? "—" : formatPercent(v, 0)),
    { align: "end" },
  );
  const columns = [
    col.custom(
      { id: "key", header, size: LABEL_WIDTH, minWidth: phone ? PHONE_NAME_MIN_WIDTH : LABEL_MIN_WIDTH, priority: 1, grow: true },
      (r: UsageRow) => r.key,
      (_v, r) => label(r),
    ),
    col.custom(
      {
        id: "requests",
        header: common.requests(),
        size: fit(
          common.requests(),
          sorted.map((r) => formatNumber(r.requests)),
          NUMBER_WIDTH,
        ),
        priority: 1,
      },
      (r: UsageRow) => r.requests,
      (v: number) => formatNumber(v),
      {
        align: "end",
      },
    ),
    col.custom(
      {
        id: "errors",
        header: errorsHeader,
        size: fit(
          errorsHeader,
          sorted.map((r) => formatNumber(r.errors)),
          NUMBER_WIDTH,
        ),
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
      {
        id: "tokens",
        header: common.tokens(),
        size: fit(
          common.tokens(),
          sorted.map((r) => formatTokens(totalTokens(r))),
          NUMBER_WIDTH,
        ),
        priority: 2,
      },
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
            {
              id: "cost",
              header: common.cost(),
              size: fit(
                common.cost(),
                sorted.map((r) => money.period(r.converted_cost)),
                costWidth(sorted, COST_WIDTH, phone),
              ),
              priority: 1,
            },
            (r: UsageRow) => r.converted_cost ?? "",
            (_v: string, r: UsageRow) => <UsageCost row={r} money={money} />,
            { align: "end" },
          ),
        ]
      : []),
    ...(by === "route" ? [cache] : []),
    ...(hasMedia(sorted)
      ? [
          col.custom(
            { id: "media", header: t("kaeso.usage.media.header", "Media"), size: MEDIA_WIDTH, priority: 3 },
            (r: UsageRow) => mediaSummary(r.media),
            (v: string) => (v ? <Text truncate>{v}</Text> : <Text tone="secondary">—</Text>),
          ),
        ]
      : []),
  ].map((c) => ({ ...c, enableSorting: sortsUsage(String(c.id)), enableResizing: false }));
  const download = () =>
    saveCsv(
      `usage-${by}-${from}-${to}.csv`,
      usageCsv(
        [
          { header, value: labelText },
          { header: t("project.overview.id", "Id"), value: (r) => r.key },
          { header: common.requests(), value: (r) => r.requests },
          { header: t("project.overview.errors", "Errors"), value: (r) => r.errors },
          { header: common.tokens(), value: (r) => totalTokens(r) },
          { header: t("project.overview.cacheHit", "Cache hit"), value: (r) => r.cache_hit_rate },
          ...(showCost ? costColumns(money) : []),
          ...(hasMedia(shown) ? [{ header: t("kaeso.usage.media.header", "Media"), value: (r: UsageRow) => mediaSummary(r.media) }] : []),
        ],
        shown,
      ),
    );
  return (
    <Section
      title={t("project.overview.breakdown", "Breakdown")}
      description={
        showCost
          ? t("project.overview.breakdownWithCost", "Requests, errors, tokens, cache hits and cost in the period")
          : t("project.overview.breakdownHint", "Requests, errors, tokens and cache hits in the period")
      }
      actions={
        <span className="flex max-w-full min-w-0 flex-wrap items-center gap-8">
          <GroupBy
            label={t("project.overview.groupBy", "Group by")}
            value={by}
            onChange={onByChange}
            options={byOptions(granularity, clients, rights.has("sources.view"))}
          />
          <Button size="sm" variant="ghost" leadingIcon={icons.download} disabled={shown.length === 0} onClick={download}>
            {t("project.overview.downloadCsv", "Download CSV")}
          </Button>
        </span>
      }
    >
      <DataTable
        aria-label={t("project.overview.usageBreakdown", "Usage breakdown")}
        data={shown}
        columns={columns}
        getRowId={(r) => r.key}
        sorting={sorting}
        onSortingChange={setSorting}
        manualSorting
        {...(logs
          ? {
              onRowClick: (r: UsageRow) => {
                const href = logHref(r);
                if (href) void navigate({ href });
              },
            }
          : {})}
        loading={waiting}
        error={failed}
        onRetry={onRetry}
        empty={<Empty size="compact" title={t("project.overview.noRequests", "No requests in this period")} />}
      />
    </Section>
  );
}

const MEDIA_WIDTH = 220;

const SHORT_DIALECTS: Record<string, string> = {
  anthropic_messages: "Messages",
  openai_chat: "Chat",
  openai_responses: "Responses",
  openai_embeddings: "Embeddings",
  gemini: "Gemini",
  gemini_embeddings: "Gemini Embeddings",
  openai_images: "Images",
  openai_speech: "Speech",
  openai_transcriptions: "Transcriptions",
  openai_videos: "Videos",
  elevenlabs: "ElevenLabs",
  minimax_images: "MiniMax Images",
  minimax_speech: "MiniMax Speech",
  minimax_videos: "MiniMax Videos",
  xai_videos: "SpaceX Videos",
  zai_videos: "Z.ai Videos",
  openrouter_videos: "OpenRouter Videos",
  veo: "Veo",
  gemini_interactions: "Gemini Interactions",
  dashscope: "DashScope",
  deepgram: "Deepgram",
  openai_decisions: "Decisions",
  system_one: "System One",
  gemini_decisions: "Gemini Decisions",
  cohere_rerank: "Rerank",
  voyage_rerank: "Voyage Rerank",
  jina_rerank: "Jina Rerank",
  fal: "fal",
  replicate: "Replicate",
  cartesia: "Cartesia",
  assemblyai: "AssemblyAI",
};
const shortDialectLabel = (d: string) => SHORT_DIALECTS[d] ?? d;

// A route as text, for the download: its formats and provider, or that it wasn't recorded.
function routeText(route: string, providerName: (id: string) => string): string {
  const [ingress = "", egress = "", provider = ""] = route.split(":");
  if (!ingress) return t("project.overview.routeNotRecorded", "Route not recorded");
  const formats = egress ? `${shortDialectLabel(ingress)} → ${shortDialectLabel(egress)}` : shortDialectLabel(ingress);
  return provider ? `${formats} (${providerName(provider)})` : formats;
}

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
