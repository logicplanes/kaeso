import { useCallback, useMemo, useState } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import {
  Avatar,
  Button,
  CodeBlock,
  Combobox,
  DataTable,
  DateRangePicker,
  Empty,
  FilterRow,
  InlineCopyText,
  KeyValue,
  Link,
  SearchInput,
  Section,
  Sheet,
  Skeleton,
  Text,
  Tooltip,
  col,
  icons,
} from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { isApiError } from "~/api/errors";
import type { AuditEventView } from "~/api/generated/types";
import { auditEventQuery, auditLogQuery, auditParams, type AuditFilters, type AuditScope, type AuditSearch } from "~/api/queries/audit";
import { auditActionLabel, auditActionOptions, rightsChangeSummary, snapshotName, targetKindLabel } from "~/format/audit";
import { rightName, useCatalogue } from "./catalogue";
import { partyText, type PartyLabel, type PartyRef } from "./party";
import { jsonDiff, stableJson } from "~/format/json-diff";
import { afterToday, DAY_PRESETS, pickedDay, pickerDate } from "~/format/picked-day";
import { locale, t, type Locale } from "~/i18n";
import { showExportProgress, startExport } from "~/shell/export-job";
import { patchSearch, type SearchPatch } from "~/search";
import { dayKey, inZone, yearOf } from "~/format/zone";
import { usePhone } from "~/shell/use-phone";

const dateTag = () => (locale() === "en" ? "en-GB" : locale());
const shortTimes = new Map<string, Intl.DateTimeFormat>();
function shortTime(v: string): string {
  const tag = dateTag();
  const zone = inZone();
  const key = `${tag}|${String(zone.timeZone)}`;
  let f = shortTimes.get(key);
  if (!f)
    shortTimes.set(key, (f = new Intl.DateTimeFormat(tag, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit", hourCycle: "h23", ...zone })));
  return f.format(new Date(v));
}
const TIME_WIDTH: Record<Locale, number> = { en: 140, de: 148, "zh-CN": 148 };
const eventTime = (v: string) => (yearOf(new Date(v)) === yearOf(new Date()) ? shortTime(v) : formatDate(v, { time: true }));
function compactTime(v: string): string {
  const at = new Date(v);
  const tag = dateTag();
  if (dayKey(at) === dayKey(new Date())) return new Intl.DateTimeFormat(tag, { hour: "2-digit", minute: "2-digit", hourCycle: "h23", ...inZone() }).format(at);
  const thisYear = yearOf(at) === yearOf(new Date());
  return new Intl.DateTimeFormat(tag, { day: "numeric", month: "short", ...(thisYear ? {} : { year: "2-digit" }), ...inZone() }).format(at);
}
function zoneName(at: Date): string {
  const parts = new Intl.DateTimeFormat(dateTag(), { timeZoneName: "short", ...inZone() }).formatToParts(at);
  return parts
    .filter((p) => p.type === "timeZoneName")
    .map((p) => p.value)
    .join("");
}

export function AuditLog({
  scope,
  search,
  onSearch,
  partyLabel,
  actors,
  onActorSearch,
  actorsLoading,
  targetHref,
  targetName,
}: {
  scope: AuditScope;
  search: AuditSearch;
  onSearch: (next: AuditSearch) => void;
  partyLabel: (p: PartyRef) => PartyLabel;
  actors: { id: string; name: string; email?: string | null | undefined; avatar_url?: string | null | undefined }[];
  onActorSearch?: ((query: string) => void) | undefined;
  actorsLoading?: boolean | undefined;
  targetHref?: (kind: string, id: string) => string | undefined;
  targetName?: (kind: string, id: string) => string | undefined;
}) {
  const { event, ...filters } = search;
  const log = useInfiniteQuery(auditLogQuery(scope, filters));
  const [target, setTarget] = useState(filters.target ?? "");
  const rows = useMemo(() => log.data?.pages.flatMap((p) => p.data) ?? [], [log.data]);
  const set = (patch: SearchPatch<AuditSearch>) => onSearch(patchSearch(search, patch));
  const filtered = !!(filters.actor || filters.action || filters.target || filters.from);
  const snapshotNames = useMemo(() => {
    const names = new Map<string, string>();
    for (const r of rows) {
      const name = r.target_id && snapshotName(r);
      if (r.target_id && name && !names.has(r.target_id)) names.set(r.target_id, name);
    }
    return names;
  }, [rows]);
  const nameOf = useCallback((kind: string, id: string) => targetName?.(kind, id) ?? snapshotNames.get(id), [targetName, snapshotNames]);

  const lang = locale();
  const phone = usePhone();
  const columns = useMemo(
    () =>
      [
        col.custom(
          {
            id: "time",
            header: phone ? zoneName(new Date()) : t("kaeso.audit.timeIn", "Time ({zone})", { zone: zoneName(new Date()) }),
            size: phone ? 88 : TIME_WIDTH[lang],
            priority: 1,
          },
          (r: AuditEventView) => r.at,
          (v: string) => (
            <Text tabular>
              <time dateTime={v}>{phone ? compactTime(v) : eventTime(v)}</time>
            </Text>
          ),
        ),
        col.custom(
          { id: "actor", header: t("kaeso.audit.actor", "Actor"), size: phone ? 64 : 168, priority: 1 },
          (r: AuditEventView) => r.actor,
          (v: PartyRef) => {
            const label = partyLabel(v);
            if (phone)
              return (
                <Tooltip content={partyText(label)}>
                  <span className="flex">
                    <Avatar name={partyText(label)} src={label.avatarUrl} size={20} />
                  </span>
                </Tooltip>
              );
            return (
              <span className="flex min-w-0 items-center gap-8">
                {v.kind === "user" && <Avatar name={label.name} src={label.avatarUrl} size={20} decorative />}
                <span className="truncate">{partyText(label)}</span>
              </span>
            );
          },
        ),
        col.custom(
          { id: "action", header: t("kaeso.audit.action", "Action"), size: 196, priority: 1, grow: true },
          (r: AuditEventView) => r.action,
          (v: string) => <Text truncate>{auditActionLabel(v)}</Text>,
        ),
        col.custom(
          { id: "target", header: t("kaeso.audit.target", "Target"), size: 152, priority: 2 },
          (r: AuditEventView) => r.target_id ?? "",
          (_v, r) => <TargetCell kind={r.target_kind} id={r.target_id} href={targetHref} name={nameOf} bare />,
        ),
        col.text({ id: "ip", header: t("kaeso.audit.ip", "IP"), size: 136, priority: 3 }, (r: AuditEventView) => r.ip ?? ""),
        col.custom(
          { id: "agent", header: t("kaeso.audit.userAgent", "User agent"), size: 200, priority: 4 },
          (r: AuditEventView) => r.user_agent ?? "",
          (v: string) =>
            v ? (
              <Tooltip content={v}>
                <span className="block truncate">{v}</span>
              </Tooltip>
            ) : null,
        ),
      ].map((c) => ({ ...c, enableSorting: false })),
    [lang, phone, partyLabel, targetHref, nameOf],
  );

  return (
    <>
      <FilterRow filtersLabel={t("kaeso.audit.filters", "Filter the audit log")}>
        <div className="w-160">
          <Combobox
            aria-label={t("kaeso.audit.actor", "Actor")}
            size="sm"
            placeholder={t("kaeso.audit.anyActor", "Any actor")}
            options={actors.map((a) => ({
              value: a.id,
              label: a.name,
              description: a.email ?? undefined,
              leading: <Avatar name={a.name} src={a.avatar_url ?? undefined} size={20} decorative />,
            }))}
            value={filters.actor ?? null}
            onInputValueChange={onActorSearch}
            searchDelay={250}
            loading={actorsLoading}
            onValueChange={(v: string | null) => set({ actor: v ?? undefined, event: undefined })}
          />
        </div>
        <div className="w-160">
          <Combobox
            aria-label={t("kaeso.audit.action", "Action")}
            size="sm"
            placeholder={t("kaeso.audit.anyAction", "Any action")}
            options={auditActionOptions()}
            value={filters.action ?? null}
            onValueChange={(v: string | null) => set({ action: v ?? undefined, event: undefined })}
          />
        </div>
        <div className="w-160">
          <SearchInput
            aria-label={t("kaeso.audit.targetId", "Target id")}
            size="sm"
            placeholder={t("kaeso.audit.targetId", "Target id")}
            value={target}
            onChange={(e) => setTarget(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && set({ target: target.trim() || undefined, event: undefined })}
            onBlur={() => target.trim() !== (filters.target ?? "") && set({ target: target.trim() || undefined, event: undefined })}
          />
        </div>
        <DateRangePicker
          size="sm"
          aria-label={t("kaeso.audit.days", "Days")}
          placeholder={t("kaeso.audit.anyDay", "Any day")}
          value={filters.from && filters.to ? { from: pickerDate(filters.from), to: pickerDate(filters.to) } : null}
          onValueChange={(r) => set({ from: r?.from ? pickedDay(r.from) : undefined, to: r?.to ? pickedDay(r.to) : undefined, event: undefined })}
          presets={DAY_PRESETS}
          disabledDays={(d) => afterToday(d)}
        />
      </FilterRow>
      <DataTable
        aria-label={t("kaeso.audit.title", "Audit log")}
        data={rows}
        columns={columns}
        getRowId={(r) => r.id}
        loading={log.isPending}
        error={log.isError && rows.length === 0 ? t("kaeso.audit.loadFailed", "The audit log couldn't load") : undefined}
        onRetry={() => void log.refetch()}
        empty={
          <Empty
            title={filtered ? t("kaeso.audit.noMatch", "No events match") : t("kaeso.audit.empty", "No events yet")}
            description={
              filtered ? t("kaeso.audit.noMatchHint", "Remove a filter or widen the days.") : t("kaeso.audit.emptyHint", "Every change is recorded here.")
            }
          />
        }
        hasMore={log.hasNextPage}
        loadingMore={log.isFetchingNextPage}
        onLoadMore={() => void log.fetchNextPage()}
        onRowClick={(r) => set({ event: r.id })}
        activeRowId={event}
      />
      <AuditEventSheet
        scope={scope}
        eventId={event}
        row={rows.find((r) => r.id === event)}
        rowsLoading={log.isPending}
        partyLabel={partyLabel}
        targetHref={targetHref}
        targetName={nameOf}
        onClose={() => set({ event: undefined })}
      />
    </>
  );
}

function TargetCell({
  kind,
  id,
  href,
  name,
  bare,
}: {
  kind?: string | null | undefined;
  id?: string | null | undefined;
  href?: ((kind: string, id: string) => string | undefined) | undefined;
  name?: ((kind: string, id: string) => string | undefined) | undefined;
  bare?: boolean | undefined;
}) {
  if (!kind || !id) return <Text tone="tertiary">{t("common.none", "None")}</Text>;
  const to = href?.(kind, id);
  const named = name?.(kind, id);
  const label = named ? (
    <Text truncate>{named}</Text>
  ) : (
    <Text mono variant="small" tone="secondary" truncate>
      {id}
    </Text>
  );
  return (
    <span className="flex min-w-0 items-center gap-8">
      {!bare && (
        <Text tone="secondary" className="shrink-0">
          {targetKindLabel(kind)}
        </Text>
      )}
      <span className="flex min-w-0 overflow-hidden">
        {to ? (
          <Link href={to} className="min-w-0 truncate">
            {label}
          </Link>
        ) : (
          label
        )}
      </span>
    </span>
  );
}

function AuditEventSheet({
  scope,
  eventId,
  row,
  rowsLoading,
  partyLabel,
  targetHref,
  targetName,
  onClose,
}: {
  scope: AuditScope;
  eventId: string | undefined;
  row: AuditEventView | undefined;
  rowsLoading: boolean;
  partyLabel: (p: PartyRef) => PartyLabel;
  targetHref?: ((kind: string, id: string) => string | undefined) | undefined;
  targetName?: ((kind: string, id: string) => string | undefined) | undefined;
  onClose: () => void;
}) {
  const own = "org" in scope && !!scope.project;
  const event = useQuery({ ...auditEventQuery(scope, eventId ?? ""), enabled: !!eventId && !own });
  const e = own ? row : event.data;
  const cat = useCatalogue("org" in scope ? scope.org : "");
  const summary = e && "org" in scope ? rightsChangeSummary(e.before, e.after, (id) => rightName(cat, id)) : null;
  return (
    <Sheet
      open={!!eventId}
      onOpenChange={(o) => !o && onClose()}
      modal={false}
      size="lg"
      title={e ? auditActionLabel(e.action) : t("kaeso.audit.event", "Event")}
      subtitle={e?.action}
    >
      {(own ? rowsLoading : event.isPending) ? (
        <Skeleton className="w-full" />
      ) : !e ? (
        <Empty
          title={
            isApiError(event.error) && event.error.type === "not_found"
              ? t("kaeso.audit.eventMissing", "This event doesn't exist")
              : t("kaeso.audit.eventUnavailable", "This event can't be shown right now")
          }
          actions={<Button onClick={onClose}>{t("kaeso.audit.back", "Back to the audit log")}</Button>}
        />
      ) : (
        <div className="flex flex-col gap-24">
          <KeyValue
            items={[
              {
                label: t("kaeso.audit.time", "Time"),
                value: t("kaeso.audit.timeWithZone", "{time} {zone}", {
                  time: formatDate(e.at, { time: true, seconds: true }),
                  zone: zoneName(new Date(e.at)),
                }),
              },
              { label: t("kaeso.audit.actor", "Actor"), value: partyText(partyLabel(e.actor)) },
              {
                label: t("kaeso.audit.target", "Target"),
                value:
                  e.target_kind && e.target_id ? (
                    <span className="flex min-w-0 flex-col gap-4">
                      <TargetCell kind={e.target_kind} id={e.target_id} href={targetHref} name={targetName} />
                      {targetName?.(e.target_kind, e.target_id) && (
                        <InlineCopyText value={e.target_id} label={t("kaeso.audit.copyTargetId", "Copy target id")} />
                      )}
                    </span>
                  ) : (
                    t("common.none", "None")
                  ),
              },
              ...(e.ip ? [{ label: t("kaeso.audit.ip", "IP"), value: e.ip, mono: true }] : []),
              ...(e.user_agent ? [{ label: t("kaeso.audit.userAgent", "User agent"), value: e.user_agent, wrap: true }] : []),
              ...(e.request_id
                ? [
                    {
                      label: t("kaeso.audit.requestId", "Request id"),
                      value: <InlineCopyText value={e.request_id} label={t("kaeso.audit.copyRequestId", "Copy request id")} />,
                    },
                  ]
                : []),
            ]}
          />
          {summary && (
            <Section title={t("kaeso.audit.rights", "Rights")} spacing="none">
              <Text>{summary}</Text>
            </Section>
          )}
          <Section title={t("kaeso.audit.change", "Change")} spacing="none">
            {e.before || e.after ? (
              <CodeBlock
                code={e.before && e.after ? jsonDiff(e.before, e.after) : stableJson(e.after ?? e.before)}
                lang={e.before && e.after ? "diff" : "json"}
                title={
                  e.before && e.after
                    ? t("kaeso.audit.beforeAfter", "Before → after")
                    : e.after
                      ? t("kaeso.audit.after", "After")
                      : t("kaeso.audit.before", "Before")
                }
                collapseAfter={0}
              />
            ) : (
              <Text tone="secondary">{t("kaeso.audit.noValues", "No before or after values are kept for this kind of change.")}</Text>
            )}
          </Section>
        </div>
      )}
    </Sheet>
  );
}

export function AuditExportButton({ org, filters }: { org: string; filters: AuditFilters }) {
  const exportLog = () => {
    if (showExportProgress()) return;
    const q = new URLSearchParams(Object.entries(auditParams(filters)).map(([k, v]) => [k, String(v)]));
    startExport({
      title: (size) =>
        size ? t("kaeso.audit.exportingSize", "Exporting audit events · {size}", { size }) : t("kaeso.audit.exporting", "Exporting audit events"),
      done: t("kaeso.audit.exported", "Audit log exported"),
      url: `/platform/v1/orgs/${org}/audit-log/export?${q.toString()}`,
      check: `/platform/v1/orgs/${org}/audit-log?${q.toString()}&limit=1`,
      file: { name: `kaeso-audit-log-${org}.jsonl`, description: t("kaeso.audit.jsonLines", "JSON Lines"), mime: "application/x-ndjson", extension: "jsonl" },
    });
  };
  return (
    <Button size="sm" leadingIcon={icons.download} onClick={exportLog}>
      {t("kaeso.audit.exportJsonl", "Export JSON Lines")}
    </Button>
  );
}
