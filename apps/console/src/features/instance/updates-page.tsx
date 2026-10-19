import { useMemo, useState } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import {
  Badge,
  Banner,
  Button,
  CodeBlock,
  DataTable,
  DialogScope,
  Empty,
  Link,
  Select,
  SettingsGroup,
  SettingsRow,
  SettingsSection,
  StatusDot,
  Text,
  Tooltip,
  col,
  icons,
} from "@logicplanes/kaeso-ui";
import { formatDate, formatRelative } from "@logicplanes/kaeso-ui/format";
import { Markdown } from "@logicplanes/kaeso-ui/markdown";
import { api, data } from "~/api/client";
import type { ReleaseView, UpdateChannel, UpdateJobView, UpdateMode, UpdateSchedule, UpdatesView, UpdateWeekday } from "~/api/generated/types";
import { updateJobsQuery, updatesQuery } from "~/api/queries/instance";
import { useApiMutation } from "~/auth/mutation";
import { toast } from "~/shell/toast";
import { SettingsLoading } from "~/shell/settings-page";
import { LoadFailed } from "~/features/org/load-failed";
import { InstancePage } from "./instance-page";
import { UpdateDialog, type Opened } from "./update-dialog";
import { checkErrorText, jobResult, UPDATER_DOCS } from "./update-words";
import { hourLabel, weekdayName } from "~/format/time-zones";
import { t } from "~/i18n";

export function InstanceUpdatesPage() {
  const [opened, setOpened] = useState<Opened | null>(null);
  const [open, setOpen] = useState(false);
  const show = (o: Opened) => {
    setOpened(o);
    setOpen(true);
  };
  const updates = useQuery({
    ...updatesQuery,
    refetchInterval: (q) => (q.state.data?.job?.state === "running" ? 5_000 : false),
  });
  const u = updates.data;
  return (
    <InstancePage
      title={t("instance.updates.title", "Updates")}
      description={t("instance.updates.hint", "The Kaeso version this install runs, and how it moves to new releases.")}
    >
      {updates.isError && !u ? (
        <LoadFailed title={t("instance.updates.failed", "The update status couldn't load")} />
      ) : !u ? (
        <SettingsLoading rows={3} />
      ) : (
        <>
          <Status u={u} onUpdate={(release) => show({ release })} onProgress={(job) => show({ job })} />
          <UpdateSettings u={u} />
          {u.latest && <Latest u={u} latest={u.latest} />}
          <History />
        </>
      )}
      <DialogScope open={open}>{opened && <UpdateDialog opened={opened} open={open} onOpenChange={setOpen} />}</DialogScope>
    </InstancePage>
  );
}

function checkedText(u: UpdatesView): string {
  const last = u.check.last_checked_at;
  const reason = u.latest && !u.can_apply && !u.manual && u.job?.state !== "running" ? blocked(u) : null;
  if (reason) return reason;
  if (last && u.check.error) return t("instance.updates.checkFailedHint", "The check {when} failed; Kaeso tries again later.", { when: formatRelative(last) });
  if (!u.settings.check) return t("instance.updates.checkOffHint", "Checking is off in the gateway's configuration.");
  if (!last) return t("instance.updates.notCheckedHint", "Not checked yet; Kaeso looks twice a day.");
  return t("instance.updates.checkedHint", "Checked {when}; Kaeso looks twice a day.", { when: formatRelative(last) });
}

function Status({ u, onUpdate, onProgress }: { u: UpdatesView; onUpdate: (release: ReleaseView) => void; onProgress: (job: string) => void }) {
  const running = u.job?.state === "running" ? u.job : null;
  const latest = u.latest;
  const action = running ? (
    <Button size="sm" onClick={() => onProgress(running.id)}>
      {t("instance.updates.showProgress", "Show progress")}
    </Button>
  ) : latest && u.can_apply ? (
    <Button size="sm" variant="primary" leadingIcon={icons.update} onClick={() => onUpdate(latest)}>
      {t("instance.updates.update", "Update to {version}", { version: latest.version })}
    </Button>
  ) : latest ? (
    <Badge tone="info">{t("instance.updates.availableVersion", "{version} available", { version: latest.version })}</Badge>
  ) : u.state === "up_to_date" ? (
    <StatusDot tone="success" icon={icons.success}>
      {t("instance.updates.upToDate", "Up to date")}
    </StatusDot>
  ) : (
    <Text tone="secondary">{t("instance.updates.notChecked", "Not checked yet")}</Text>
  );
  const description = running ? (
    t("instance.updates.runningTitle", "Updating to Kaeso {version}", { version: running.to })
  ) : u.check.error ? (
    <Tooltip content={checkErrorText(u.check.error)}>
      <span>{checkedText(u)}</span>
    </Tooltip>
  ) : (
    checkedText(u)
  );
  return (
    <SettingsGroup className="mt-40">
      <SettingsRow title={t("instance.updates.runningVersion", "Kaeso {version}", { version: u.current })} description={description} control={action} />
    </SettingsGroup>
  );
}

function manualReason(u: UpdatesView, version: string): string {
  switch (u.manual) {
    case "major":
      return t("instance.updates.manualMajor", "Kaeso {version} is a new major version: update by hand with the upgrade guide.", { version });
    case "manual_steps":
      return t("instance.updates.manualSteps", "A release on the way asks for steps by hand: follow the upgrade guide.");
    default:
      return t("instance.updates.manualUnprepared", "This release can't be applied from here: update by hand with the upgrade guide.");
  }
}

function blocked(u: UpdatesView): string | null {
  switch (u.updater.state) {
    case "not_installed":
      return t("instance.updates.noUpdater", "The updater isn't installed, so this install updates by hand.");
    case "unreachable":
      return t("instance.updates.updaterDown", "The updater doesn't answer; check that its container runs.");
    case "outdated":
      return t("instance.updates.updaterOld", "The updater is too old for this release; update by hand once.");
    default:
      return null;
  }
}

function Latest({ u, latest }: { u: UpdatesView; latest: ReleaseView }) {
  const download = `https://github.com/logicplanes/kaeso/releases/download/v${latest.version}`;
  const commands = [
    `curl -fsSLO ${download}/compose.yaml`,
    `curl -fsSLO ${download}/release.json`,
    "gh attestation verify release.json --repo logicplanes/kaeso --signer-workflow logicplanes/kaeso/.github/workflows/release.yml --source-ref refs/heads/prod",
    'echo "$(jq -r .compose.sha256 release.json)  compose.yaml" | sha256sum -c -',
    "docker compose pull",
    "docker compose up -d --wait",
  ].join("\n");
  return (
    <SettingsSection
      title={t("instance.updates.release", "Kaeso {version}", { version: latest.version })}
      description={t("instance.updates.published", "Published {date}", { date: formatDate(latest.published_at) })}
    >
      <div className="flex flex-col gap-16">
        {u.manual ? (
          <Banner
            tone="warning"
            title={t("instance.updates.byHand", "Update by hand")}
            actions={
              <Button size="sm" href={u.upgrade_guide} target="_blank" rel="noopener noreferrer" trailingIcon={icons.externalLink}>
                {t("instance.updates.guide", "Open the upgrade guide")}
              </Button>
            }
          >
            {manualReason(u, latest.version)}
          </Banner>
        ) : (
          !u.can_apply && (
            <div className="flex flex-col gap-12">
              <CodeBlock title={t("instance.updates.byHandTitle", "Update by hand, in the install's folder")} lang="bash" code={commands} collapseAfter={0} />
              <Link href={UPDATER_DOCS} external>
                {t("instance.updates.setUpUpdater", "Set up the updater")}
              </Link>
            </div>
          )
        )}
        {u.releases.map((r) => (
          <Notes key={r.version} release={r} />
        ))}
      </div>
    </SettingsSection>
  );
}

function Notes({ release }: { release: ReleaseView }) {
  return (
    <section aria-label={t("instance.updates.notesOf", "Release notes of {version}", { version: release.version })} className="mt-8 flex flex-col gap-8">
      <div className="flex flex-col gap-8">
        <span className="flex flex-wrap items-center gap-8">
          <Text weight="medium">{release.version}</Text>
          {release.prerelease && <Badge>{t("instance.updates.prerelease", "Pre-release")}</Badge>}
          {release.major && <Badge tone="warning">{t("instance.updates.major", "Major version")}</Badge>}
          <Text variant="small" tone="secondary">
            {formatDate(release.published_at)}
          </Text>
          <Link href={release.url} external variant="quiet" className="ml-auto">
            {t("instance.updates.onGitHub", "On GitHub")}
          </Link>
        </span>
        {release.notes.trim() ? (
          <Markdown text={release.notes} headingLevel={4} size="small" collapseAfter={2400} />
        ) : (
          <Text variant="small" tone="secondary">
            {t("instance.updates.noNotes", "This release has no notes.")}
          </Text>
        )}
      </div>
    </section>
  );
}

type SettingsChange = {
  channel?: UpdateChannel;
  mode?: UpdateMode;
  schedule?: UpdateSchedule;
};

function savedText(body: SettingsChange): string {
  if (body.mode !== undefined)
    return body.mode === "automatic"
      ? t("instance.updates.autoOn", "Updates now install automatically")
      : t("instance.updates.autoOff", "You now install each update yourself");
  if (body.schedule) return t("instance.updates.scheduleSaved", "Update window saved");
  return t("instance.updates.channelSaved", "Channel saved");
}

const WEEKDAYS: UpdateWeekday[] = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];
const HOURS = Array.from({ length: 24 }, (_, h) => h);

function windowText(s: UpdatesView["settings"]): string {
  return s.next_auto_update_at
    ? t("instance.updates.nextWindowHint", "Next {date}, {zone} time.", { date: formatDate(s.next_auto_update_at, { time: true }), zone: s.schedule.timezone })
    : t("instance.updates.noWindowHint", "No window planned; {zone} time.", { zone: s.schedule.timezone });
}

function UpdateSettings({ u }: { u: UpdatesView }) {
  const save = useApiMutation({
    mutationFn: async (body: SettingsChange) => data(await api.PATCH("/platform/v1/instance/updates/settings", { body })),
    invalidates: [["instance", "updates"]],
    onSuccess: (_, body) => toast.success(savedText(body)),
  });
  const s = u.settings;
  const schedule = s.schedule;
  const plan = (change: Partial<UpdateSchedule>) =>
    save.mutate({ schedule: { frequency: schedule.frequency, weekday: schedule.weekday, hour: schedule.hour, ...change } });
  return (
    <SettingsSection title={t("common.settings", "Settings")}>
      <SettingsGroup>
        <SettingsRow
          title={t("instance.updates.channel", "Releases")}
          description={t("instance.updates.channelHint", "Which releases this install is offered.")}
          control={
            <div className="w-240">
              <Select
                aria-label={t("instance.updates.channel", "Releases")}
                size="sm"
                fullWidth
                value={s.channel}
                disabled={save.isPending}
                onValueChange={(channel) => channel !== s.channel && save.mutate({ channel: channel === "prerelease" ? "prerelease" : "stable" })}
                options={[
                  {
                    value: "stable",
                    label: t("instance.updates.stable", "Stable releases"),
                    description: t("instance.updates.stableHint", "Tested releases; the right choice for almost every install."),
                  },
                  {
                    value: "prerelease",
                    label: t("instance.updates.withPre", "Stable and pre-releases"),
                    description: t("instance.updates.withPreHint", "Also release candidates, a few days before each release."),
                  },
                ]}
              />
            </div>
          }
        />
        <SettingsRow
          title={t("instance.updates.mode", "How updates install")}
          description={
            s.mode === "automatic"
              ? t("instance.updates.automaticHint", "In the update window; major versions wait for you.")
              : t("instance.updates.manualHint", "You start each update with the button above.")
          }
          control={
            <div className="w-240">
              <Select
                aria-label={t("instance.updates.mode", "How updates install")}
                size="sm"
                fullWidth
                value={s.mode}
                disabled={save.isPending}
                onValueChange={(mode) => mode !== s.mode && save.mutate({ mode: mode === "automatic" ? "automatic" : "manual" })}
                options={[
                  { value: "manual", label: t("instance.updates.manual", "Manually") },
                  {
                    value: "automatic",
                    label: t("instance.updates.automatic", "Automatically"),
                    disabled: u.updater.state === "not_installed",
                  },
                ]}
              />
            </div>
          }
        />
        {s.mode === "automatic" && (
          <SettingsRow
            title={t("instance.updates.window", "Update window")}
            description={windowText(s)}
            control={
              <div className="flex flex-wrap items-center gap-8">
                <div className="w-120">
                  <Select
                    aria-label={t("instance.updates.frequency", "How often")}
                    size="sm"
                    fullWidth
                    value={schedule.frequency}
                    disabled={save.isPending}
                    onValueChange={(f) => f !== schedule.frequency && plan({ frequency: f === "weekly" ? "weekly" : "daily" })}
                    options={[
                      { value: "daily", label: t("instance.updates.daily", "Daily") },
                      { value: "weekly", label: t("instance.updates.weekly", "Weekly") },
                    ]}
                  />
                </div>
                {schedule.frequency === "weekly" && (
                  <div className="w-140">
                    <Select
                      aria-label={t("instance.updates.day", "Day")}
                      size="sm"
                      fullWidth
                      value={schedule.weekday}
                      disabled={save.isPending}
                      onValueChange={(d) => d !== schedule.weekday && plan({ weekday: d as UpdateWeekday })}
                      options={WEEKDAYS.map((value, i) => ({ value, label: weekdayName(i) }))}
                    />
                  </div>
                )}
                <div className="w-96">
                  <Select
                    aria-label={t("instance.updates.time", "Time")}
                    size="sm"
                    fullWidth
                    value={String(schedule.hour)}
                    disabled={save.isPending}
                    onValueChange={(h) => Number(h) !== schedule.hour && plan({ hour: Number(h) })}
                    options={HOURS.map((h) => ({ value: String(h), label: hourLabel(h) }))}
                  />
                </div>
              </div>
            }
          />
        )}
      </SettingsGroup>
    </SettingsSection>
  );
}

function History() {
  const jobs = useInfiniteQuery(updateJobsQuery);
  const rows = useMemo(() => jobs.data?.pages.flatMap((p) => p.data) ?? [], [jobs.data]);
  const columns = [
    col.custom(
      { id: "when", header: t("instance.updates.when", "When"), size: 160, priority: 1 },
      (j: UpdateJobView) => j.started_at,
      (at: string) => <Text tabular>{formatDate(at, { time: true })}</Text>,
    ),
    col.custom(
      { id: "versions", header: t("instance.updates.versions", "Versions"), size: 160, priority: 1, grow: true },
      (j: UpdateJobView) => j,
      (j: UpdateJobView) => (
        <Text tabular truncate>
          {t("instance.updates.fromTo", "{from} to {to}", { from: j.from, to: j.to })}
        </Text>
      ),
    ),
    col.custom(
      { id: "result", header: t("instance.updates.result", "Result"), size: 168, priority: 1 },
      (j: UpdateJobView) => j,
      (j: UpdateJobView) => {
        const r = jobResult(j);
        return r.detail ? (
          <Tooltip content={r.detail}>
            <span className="inline-flex">
              <StatusDot tone={r.tone}>{r.label}</StatusDot>
            </span>
          </Tooltip>
        ) : (
          <StatusDot tone={r.tone}>{r.label}</StatusDot>
        );
      },
    ),
    col.custom(
      { id: "by", header: t("instance.updates.by", "Started by"), size: 160, priority: 2 },
      (j: UpdateJobView) =>
        j.automatic ? t("instance.updates.automatically", "Automatically") : (j.started_by?.name ?? t("instance.updates.deletedUser", "Deleted user")),
      (name: string) => <Text truncate>{name}</Text>,
    ),
  ].map((c) => ({ ...c, enableSorting: false }));
  return (
    <SettingsSection
      title={t("instance.updates.history", "History")}
      description={t("instance.updates.historyHint", "Every update is also in the instance audit log.")}
    >
      <DataTable
        aria-label={t("instance.updates.history", "History")}
        data={rows}
        columns={columns}
        getRowId={(r) => r.id}
        density="comfortable"
        loading={jobs.isPending}
        error={jobs.isError && rows.length === 0 ? t("instance.updates.historyFailed", "The update history couldn't load") : undefined}
        onRetry={() => void jobs.refetch()}
        hasMore={jobs.hasNextPage}
        loadingMore={jobs.isFetchingNextPage}
        onLoadMore={() => void jobs.fetchNextPage()}
        empty={
          <Empty
            title={t("instance.updates.noHistory", "No updates yet")}
            description={t("instance.updates.noHistoryHint", "Updates applied from here show up with who started them and how they ended.")}
          />
        }
      />
    </SettingsSection>
  );
}
