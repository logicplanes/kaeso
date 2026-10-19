import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
  Badge,
  Banner,
  Button,
  DateRangePicker,
  Empty,
  Input,
  NumberInput,
  Select,
  SettingsGroup,
  SettingsRow,
  SettingsSection,
  Skeleton,
  StatGrid,
  StatTile,
  StepDialog,
  Switch,
  Table,
  Tabs,
  Text,
  type DateRange,
} from "@logicplanes/kaeso-ui";
import { formatBytes, formatDate, formatNumber } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { OrgLogSettings } from "~/api/generated/types";
import { projectLogSettings, projectViews } from "~/api/queries/invalidation";
import { logDeletionsQuery, logSettingsQuery, logStorageQuery, orgViewKey } from "~/api/queries/orgs";
import { orgProjectsQuery, projectListKey } from "~/api/queries/projects";
import { useApiMutation } from "~/auth/mutation";
import { useOrgRights } from "~/auth/session";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { fullDayLabel, utcDay } from "~/format/period";
import { afterToday, DAY_PRESETS, pickedDay } from "~/format/picked-day";
import { toast } from "~/shell/toast";
import { LoadFailed } from "./load-failed";
import { OrgPage } from "./org-page";
import type { LoggingTab } from "~/api/queries/settings-prefetch";
import { useNeedsRight } from "~/auth/needs-right";

const ALL_PROJECTS = "all";

export function OrgLoggingPage({ org, tab }: { org: string; tab: LoggingTab }) {
  const navigate = useNavigate();
  const rights = useOrgRights(org);
  const canStorage = rights.has("logs.storage.view");
  const shown = canStorage ? tab : "settings";
  return (
    <OrgPage
      org={org}
      title={t("org.logging.title", "Logging")}
      hidden={t("org.logging.hidden", "You can't see the logging settings here")}
      right="org.view"
      description={t("org.logging.description", "What the gateway keeps of each request, for how long, and what it takes up.")}
    >
      <Tabs
        value={shown}
        onValueChange={(next) =>
          void navigate({
            to: "/settings/orgs/$org/logging",
            params: { org },
            search: next === "settings" ? {} : { tab: next as LoggingTab },
            replace: true,
          })
        }
      >
        <Tabs.List aria-label={t("org.logging.title", "Logging")}>
          <Tabs.Tab value="settings">{common.settings()}</Tabs.Tab>
          <Tabs.Tab value="storage" disabled={!canStorage}>
            {t("org.logging.storage", "Storage")}
          </Tabs.Tab>
          <Tabs.Tab value="deletions" disabled={!canStorage}>
            {t("org.logging.deletions", "Deletions")}
          </Tabs.Tab>
        </Tabs.List>
        <Tabs.Panel value="settings">
          <LogSettings org={org} />
        </Tabs.Panel>
        <Tabs.Panel value="storage">
          <Storage org={org} />
        </Tabs.Panel>
        <Tabs.Panel value="deletions">
          <Deletions org={org} />
        </Tabs.Panel>
      </Tabs>
    </OrgPage>
  );
}

function LogSettings({ org }: { org: string }) {
  const rights = useOrgRights(org);
  const settings = useQuery(logSettingsQuery(org));
  const s = settings.data;
  if (settings.isError) return <LoadFailed title={t("org.logging.settingsFailed", "The log settings couldn't load")} />;
  if (!s) return <Skeleton className="h-full w-full" />;
  return (
    <LogSettingsForm key={`${s.metadata_retention_days}-${s.content_retention_days}`} org={org} settings={s} canManage={rights.has("logs.settings.manage")} />
  );
}

function LogSettingsForm({ org, settings, canManage }: { org: string; settings: OrgLogSettings; canManage: boolean }) {
  const needs = useNeedsRight(org);
  const [metadata, setMetadata] = useState(String(settings.metadata_retention_days));
  const [content, setContent] = useState(String(settings.content_retention_days));
  const save = useApiMutation({
    mutationFn: async (body: { content_logging?: boolean; metadata_retention_days?: number; content_retention_days?: number }) =>
      data(await api.PATCH("/platform/v1/orgs/{org}/logs/settings", { params: { path: { org } }, body })),
    invalidates: [["orgs", org, "logs"], orgViewKey(org), projectListKey(org), projectViews(org), projectLogSettings(org)],
    onSuccess: (_s, body) =>
      toast.success(
        body.content_logging === undefined
          ? t("org.logging.retentionSaved", "Retention saved")
          : body.content_logging
            ? t("org.logging.contentOn", "Content logging turned on")
            : t("org.logging.contentOff", "Content logging turned off"),
      ),
  });
  const m = Number(metadata);
  const c = Number(content);
  const whole = (n: number) => Number.isInteger(n) && n >= 1;
  const changed = m !== settings.metadata_retention_days || c !== settings.content_retention_days;
  const maxMetadata = t("org.days", "{count, plural, one {# day} other {# days}}", { count: settings.max_metadata_retention_days });
  const maxContent = t("org.days", "{count, plural, one {# day} other {# days}}", { count: settings.max_content_retention_days });
  const days = t("org.daysUnit", "days");
  const reason = !canManage
    ? needs("logs.settings.manage")
    : !whole(m) || !whole(c)
      ? t("org.wholeDays", "Enter whole days, at least 1")
      : m > settings.max_metadata_retention_days
        ? t("org.logging.metadataMax", "Metadata can be kept up to {days} on this install", { days: maxMetadata })
        : c > settings.max_content_retention_days
          ? t("org.logging.contentMax", "Content can be kept up to {days} on this install", { days: maxContent })
          : c > m
            ? t("org.logging.contentLonger", "Content can't be kept longer than metadata")
            : !changed
              ? t("org.noChange", "No change to save")
              : undefined;
  return (
    <div>
      <SettingsSection title={t("org.logging.content", "Content logging")}>
        <SettingsGroup>
          <SettingsRow
            title={t("org.logging.logContent", "Log request content")}
            description={t("org.logging.logContentHint", "Keeps request and reply bodies; projects may turn it off.")}
            control={
              <Switch
                aria-label={t("org.logging.logContent", "Log request content")}
                checked={settings.content_logging}
                disabled={!canManage}
                {...(canManage ? {} : { disabledReason: needs("logs.settings.manage") })}
                loading={save.isPending && save.variables?.content_logging !== undefined}
                onCheckedChange={(on) => save.mutate({ content_logging: on })}
              />
            }
          />
        </SettingsGroup>
      </SettingsSection>
      <SettingsSection
        title={t("org.logging.retention", "Retention")}
        description={t("org.logging.retentionHint", "Shortening deletes older entries; lengthening applies to new ones.")}
      >
        <SettingsGroup>
          <SettingsRow
            title={t("org.logging.metadata", "Metadata")}
            description={t("org.logging.metadataHint", "Timing, tokens, cost and errors; up to {days}.", { days: maxMetadata })}
            control={
              <div className="w-160">
                <NumberInput
                  aria-label={t("org.logging.metadataRetention", "Metadata retention")}
                  size="sm"
                  value={metadata}
                  onValueChange={setMetadata}
                  min="1"
                  step="1"
                  unit={days}
                  disabled={!canManage}
                />
              </div>
            }
          />
          <SettingsRow
            title={t("org.logging.contentTitle", "Content")}
            description={t("org.logging.contentHint", "Bodies, up to {days}, no longer than metadata.", { days: maxContent })}
            control={
              <div className="w-160">
                <NumberInput
                  aria-label={t("org.logging.contentRetention", "Content retention")}
                  size="sm"
                  value={content}
                  onValueChange={setContent}
                  min="1"
                  step="1"
                  unit={days}
                  disabled={!canManage}
                />
              </div>
            }
          />
        </SettingsGroup>
        <div className="mt-12 flex justify-end">
          <Button
            size="sm"
            variant="primary"
            disabled={!!reason}
            {...(reason ? { disabledReason: reason } : {})}
            loading={save.isPending && save.variables?.content_logging === undefined}
            onClick={() => save.mutate({ metadata_retention_days: m, content_retention_days: c })}
          >
            {t("org.logging.saveRetention", "Save retention")}
          </Button>
        </div>
      </SettingsSection>
    </div>
  );
}

const NUMBER = "text-nowrap";
const PHONE_HIDDEN = "hidden text-nowrap sm:table-cell";

function Storage({ org }: { org: string }) {
  const storage = useQuery(logStorageQuery(org));
  const projects = useQuery(orgProjectsQuery(org, true));
  const s = storage.data;
  const name = (id: string) => projects.data?.find((p) => p.id === id)?.name ?? id;
  const loadingValue = <Skeleton shape="line" className="w-64" />;
  return (
    <div className="mt-40 flex flex-col gap-24">
      {storage.isError ? (
        <LoadFailed title={t("org.logging.storageFailed", "The log storage couldn't load")} />
      ) : (
        <StatGrid aria-label={t("org.logging.totals", "Log storage in total")}>
          <StatTile
            label={t("org.logging.entries", "Entries")}
            value={s ? formatNumber(s.entries) : loadingValue}
            detail={
              s
                ? t("org.logging.withContentCount", "{count} with content", {
                    count: formatNumber(s.projects.reduce((sum, p) => sum + p.entries_with_content, 0)),
                  })
                : undefined
            }
          />
          <StatTile
            label={t("org.logging.logicalSize", "Logical size")}
            value={s ? formatBytes(s.logical_bytes) : loadingValue}
            detail={s ? t("org.logging.asReceived", "As received") : undefined}
          />
          <StatTile
            label={t("org.logging.onDisk", "On disk")}
            value={s ? formatBytes(s.estimated_disk_bytes) : loadingValue}
            detail={s ? t("org.logging.estimated", "Estimated") : undefined}
          />
        </StatGrid>
      )}
      {s && (
        <>
          {s.projects.length === 0 ? (
            <Empty title={t("org.logging.noLogs", "No request logs yet")} />
          ) : (
            <Table aria-label={t("org.logging.byProject", "Storage by project")}>
              <Table.Head>
                <Table.Row>
                  <Table.HeaderCell>{common.project()}</Table.HeaderCell>
                  <Table.HeaderCell align="end" className={NUMBER}>
                    {t("org.logging.entries", "Entries")}
                  </Table.HeaderCell>
                  <Table.HeaderCell align="end" className={PHONE_HIDDEN}>
                    {t("org.logging.withContent", "With content")}
                  </Table.HeaderCell>
                  <Table.HeaderCell align="end" className={PHONE_HIDDEN}>
                    {t("org.logging.logical", "Logical")}
                  </Table.HeaderCell>
                  <Table.HeaderCell align="end" className={NUMBER}>
                    {t("org.logging.onDisk", "On disk")}
                  </Table.HeaderCell>
                </Table.Row>
              </Table.Head>
              <Table.Body>
                {[...s.projects]
                  .sort((a, b) => b.entries - a.entries)
                  .map((p) => (
                    <Table.Row key={p.project_id}>
                      <Table.Cell>{name(p.project_id)}</Table.Cell>
                      <Table.Cell align="end" className={NUMBER}>
                        {formatNumber(p.entries)}
                      </Table.Cell>
                      <Table.Cell align="end" className={PHONE_HIDDEN}>
                        {formatNumber(p.entries_with_content)}
                      </Table.Cell>
                      <Table.Cell align="end" className={PHONE_HIDDEN}>
                        {formatBytes(p.logical_bytes)}
                      </Table.Cell>
                      <Table.Cell align="end" className={NUMBER}>
                        {formatBytes(p.estimated_disk_bytes)}
                      </Table.Cell>
                    </Table.Row>
                  ))}
              </Table.Body>
            </Table>
          )}
        </>
      )}
      {(s || storage.isError) && (
        <Banner tone="neutral" title={t("org.logging.backupTitle", "Back up the master keys")}>
          {t(
            "org.logging.backupText",
            "Provider keys are encrypted with the install's master keys. Database backups are useless for provider keys without them: back the master keys up separately.",
          )}
        </Banner>
      )}
    </div>
  );
}

const jobKind = (k: string): string =>
  k === "deletion"
    ? t("org.logging.kindDeletion", "Deletion")
    : k === "shrink_metadata"
      ? t("org.logging.kindShrinkMetadata", "Shorter metadata retention")
      : k === "shrink_content"
        ? t("org.logging.kindShrinkContent", "Shorter content retention")
        : k;

function Deletions({ org }: { org: string }) {
  const needs = useNeedsRight(org);
  const rights = useOrgRights(org);
  const jobs = useQuery(logDeletionsQuery(org));
  const projects = useQuery(orgProjectsQuery(org, true));
  const [deleting, setDeleting] = useState(false);
  const canDelete = rights.has("logs.delete");
  const name = (id: string | null | undefined) => (id ? (projects.data?.find((p) => p.id === id)?.name ?? id) : t("org.logging.everyProject", "Every project"));
  const list = jobs.data ?? [];
  return (
    <div>
      <SettingsSection
        title={t("org.logging.delete", "Delete logs")}
        description={t("org.logging.deleteHint", "Deletes a period's entries for good, in one project or all.")}
        actions={
          <Button
            size="sm"
            variant="danger-secondary"
            disabled={!canDelete}
            {...(canDelete ? {} : { disabledReason: needs("logs.delete") })}
            onClick={() => setDeleting(true)}
          >
            {t("org.logging.delete", "Delete logs")}
          </Button>
        }
      >
        {jobs.isError ? (
          <LoadFailed title={t("org.logging.deletionsFailed", "The deletions couldn't load")} size="md" />
        ) : jobs.isPending ? (
          <Skeleton className="w-full" />
        ) : list.length === 0 ? (
          <Text variant="small" tone="secondary">
            {t("org.logging.noDeletions", "No deletions yet.")}
          </Text>
        ) : (
          <Table aria-label={t("org.logging.deletionsTable", "Log deletions")}>
            <Table.Head>
              <Table.Row>
                <Table.HeaderCell>{t("org.logging.kind", "Kind")}</Table.HeaderCell>
                <Table.HeaderCell>{t("org.logging.period", "Period")}</Table.HeaderCell>
                <Table.HeaderCell>{common.project()}</Table.HeaderCell>
                <Table.HeaderCell>{t("org.logging.state", "State")}</Table.HeaderCell>
                <Table.HeaderCell align="end">{t("org.logging.doneAt", "Done")}</Table.HeaderCell>
              </Table.Row>
            </Table.Head>
            <Table.Body>
              {list.map((j) => (
                <Table.Row key={j.id}>
                  <Table.Cell>{jobKind(j.kind)}</Table.Cell>
                  <Table.Cell>{`${fullDayLabel(utcDay(new Date(j.from)))} – ${fullDayLabel(utcDay(new Date(Date.parse(j.to) - 1)))}`}</Table.Cell>
                  <Table.Cell>{name(j.project_id)}</Table.Cell>
                  <Table.Cell>
                    <span className="flex flex-col gap-4">
                      <Badge tone={j.state === "done" ? "success" : "neutral"}>
                        {j.state === "done" ? t("org.logging.done", "Done") : t("org.logging.queued", "Queued")}
                      </Badge>
                      {j.last_error && (
                        <Text variant="small" tone="secondary">
                          {t("org.logging.retrying", "Retrying: {error}", { error: j.last_error })}
                        </Text>
                      )}
                    </span>
                  </Table.Cell>
                  <Table.Cell align="end">{j.done_at ? formatDate(j.done_at) : t("org.logging.notYet", "Not yet")}</Table.Cell>
                </Table.Row>
              ))}
            </Table.Body>
          </Table>
        )}
      </SettingsSection>
      {deleting && <DeleteLogsDialog org={org} onClose={() => setDeleting(false)} />}
    </div>
  );
}

function DeleteLogsDialog({ org, onClose }: { org: string; onClose: () => void }) {
  const projects = useQuery(orgProjectsQuery(org, true));
  const [range, setRange] = useState<DateRange | null>(null);
  const [project, setProject] = useState(ALL_PROJECTS);
  const [typed, setTyped] = useState("");
  const remove = useApiMutation({
    mutationFn: async ({ from, to }: { from: Date; to: Date }) =>
      data(
        await api.POST("/platform/v1/orgs/{org}/logs/deletions", {
          params: { path: { org } },
          body: {
            from: `${pickedDay(from)}T00:00:00Z`,
            to: new Date(Date.parse(`${pickedDay(to)}T00:00:00Z`) + 86_400_000).toISOString(),
            ...(project === ALL_PROJECTS ? {} : { project }),
          },
        }),
      ),
    invalidates: [["orgs", org, "logs"]],
    onSuccess: () => {
      toast(t("org.logging.queuedToast", "Deletion queued; it runs in the background"));
      onClose();
    },
  });
  const periodReason = !range?.from || !range.to ? t("org.logging.choosePeriod", "Choose the period") : undefined;
  const word = t("org.logging.confirmWord", "delete");
  const typedWord = typed.trim() === word;
  return (
    <StepDialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t("org.logging.deleteTitle", "Delete request logs")}
      tone="danger"
      submitLabel={t("org.logging.delete", "Delete logs")}
      submitting={remove.isPending}
      onSubmit={() => range?.from && range.to && typedWord && remove.mutate({ from: range.from, to: range.to })}
      steps={[
        {
          id: "period",
          title: t("org.logging.whichDays", "Which days?"),
          description:
            range?.from && range.to
              ? t("org.logging.days", "{from} to {to}, whole UTC days.", { from: fullDayLabel(pickedDay(range.from)), to: fullDayLabel(pickedDay(range.to)) })
              : t("org.logging.daysHint", "Whole UTC days; usage and costs stay."),
          blockedReason: periodReason,
          content: (
            <DateRangePicker
              aria-label={t("org.logging.period", "Period")}
              value={range}
              onValueChange={setRange}
              presets={DAY_PRESETS}
              disabledDays={(d) => afterToday(d)}
            />
          ),
        },
        {
          id: "project",
          title: t("org.logging.whichProjects", "From which projects?"),
          content: (
            <Select
              aria-label={common.project()}
              fullWidth
              value={project}
              onValueChange={setProject}
              options={[
                { value: ALL_PROJECTS, label: t("org.logging.everyProject", "Every project") },
                ...(projects.data ?? []).map((p) => ({ value: p.id, label: p.name })),
              ]}
            />
          ),
        },
        {
          id: "confirm",
          title: t("org.logging.typeToConfirm", "Type {word} to confirm", { word }),
          description: t("org.logging.cannotRecover", "Deleted log entries cannot be recovered."),
          blockedReason: typedWord ? undefined : t("org.logging.typeToConfirm", "Type {word} to confirm", { word }),
          content: (
            <Input
              aria-label={t("org.logging.typeToConfirm", "Type {word} to confirm", { word })}
              placeholder={word}
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
              autoComplete="off"
            />
          ),
        },
      ]}
    />
  );
}
