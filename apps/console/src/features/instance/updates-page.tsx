import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Badge,
  Banner,
  Button,
  Code,
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
  Switch,
  Text,
  Tooltip,
  col,
  icons,
} from "@logicplanes/kaeso-ui";
import { formatDate, formatRelative } from "@logicplanes/kaeso-ui/format";
import { Markdown } from "@logicplanes/kaeso-ui/markdown";
import { api, data } from "~/api/client";
import type { ReleaseView, UpdateJobView, UpdatesView } from "~/api/generated/types";
import { updateJobsQuery, updatesQuery } from "~/api/queries/instance";
import { useApiMutation } from "~/auth/mutation";
import { toast } from "~/shell/toast";
import { SettingsLoading } from "~/shell/settings-page";
import { LoadFailed } from "~/features/org/load-failed";
import { InstancePage } from "./instance-page";
import { UpdateDialog, type Opened } from "./update-dialog";
import { jobResult, UPDATER_DOCS } from "./update-words";
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
  const check = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/instance/updates/check")),
    invalidates: [["instance", "updates"]],
    onSuccess: (u) =>
      u.check.error
        ? toast.warning(t("instance.updates.checkFailed", "Releases couldn't be fetched; try again later"))
        : u.latest
          ? toast.success(t("instance.updates.found", "Kaeso {version} is available", { version: u.latest.version }))
          : toast.success(t("instance.updates.none", "Kaeso is up to date")),
  });
  const u = updates.data;
  const running = u?.job?.state === "running" ? u.job : null;
  return (
    <InstancePage
      title={t("instance.updates.title", "Updates")}
      description={t("instance.updates.hint", "Kaeso releases on GitHub and how this install moves to them.")}
      actions={
        <Button size="sm" leadingIcon={icons.refresh} loading={check.isPending} disabled={!u} onClick={() => check.mutate()}>
          {t("instance.checkNow", "Check now")}
        </Button>
      }
    >
      {updates.isError && !u ? (
        <LoadFailed title={t("instance.updates.failed", "The update status couldn't load")} />
      ) : !u ? (
        <SettingsLoading rows={3} />
      ) : (
        <>
          <Versions u={u} />
          {running && (
            <Banner
              tone="info"
              title={t("instance.updates.runningTitle", "Updating to Kaeso {version}", { version: running.to })}
              actions={
                <Button size="sm" onClick={() => show({ job: running.id })}>
                  {t("instance.updates.showProgress", "Show progress")}
                </Button>
              }
            >
              {t("instance.updates.runningHint", "Kaeso is unavailable for about a minute while it switches over.")}
            </Banner>
          )}
          {u.latest && <Latest u={u} latest={u.latest} running={!!running} onUpdate={() => show({ release: u.latest as ReleaseView })} />}
          <Choices u={u} />
          <History />
        </>
      )}
      <DialogScope open={open}>{opened && <UpdateDialog opened={opened} open={open} onOpenChange={setOpen} />}</DialogScope>
    </InstancePage>
  );
}

function Versions({ u }: { u: UpdatesView }) {
  const newest = u.latest ? (
    <span className="flex items-center gap-8">
      <Code>{u.latest.version}</Code>
      <Badge tone="info">{t("instance.updates.available", "Update available")}</Badge>
    </span>
  ) : u.state === "up_to_date" ? (
    <StatusDot tone="success">{t("instance.updates.upToDate", "Up to date")}</StatusDot>
  ) : (
    <Text tone="secondary">{t("instance.updates.notChecked", "Not checked yet")}</Text>
  );
  const checked = u.check.last_checked_at ? (
    u.check.error ? (
      <Tooltip content={u.check.error}>
        <span className="inline-flex">
          <StatusDot tone="danger">{t("instance.updates.checkFailedAt", "Failed {when}", { when: formatRelative(u.check.last_checked_at) })}</StatusDot>
        </span>
      </Tooltip>
    ) : (
      <Text tone="secondary">{formatRelative(u.check.last_checked_at)}</Text>
    )
  ) : (
    <Text tone="secondary">{t("instance.updates.never", "Never")}</Text>
  );
  return (
    <SettingsSection title={t("instance.version", "Version")}>
      <SettingsGroup>
        <SettingsRow title={t("instance.updates.running", "Running")} control={<Code>{u.current}</Code>} />
        <SettingsRow title={t("instance.updates.newest", "Newest release")} control={newest} />
        <SettingsRow
          title={t("instance.updates.lastChecked", "Last checked")}
          description={
            u.check.error
              ? t("instance.updates.retryHint", "GitHub couldn't be reached; Kaeso tries again later.")
              : u.settings.check
                ? t("instance.updates.checkHint", "Kaeso looks for new releases twice a day.")
                : t("instance.updates.checkOffHint", "Automatic checks are off.")
          }
          control={checked}
        />
      </SettingsGroup>
    </SettingsSection>
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
  if (!u.settings.apply) return t("instance.updates.applyOff", "Turn on one-click updates below to update from here.");
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

function Latest({ u, latest, running, onUpdate }: { u: UpdatesView; latest: ReleaseView; running: boolean; onUpdate: () => void }) {
  const commands = [
    `curl -fsSLO https://github.com/logicplanes/kaeso/releases/download/v${latest.version}/compose.yaml`,
    "docker compose pull",
    "docker compose up -d",
  ].join("\n");
  const reason = blocked(u);
  return (
    <SettingsSection
      title={t("instance.updates.release", "Kaeso {version}", { version: latest.version })}
      description={t("instance.updates.published", "Published {date}", { date: formatDate(latest.published_at) })}
      actions={
        u.can_apply ? (
          <Button size="sm" variant="primary" leadingIcon={icons.update} disabled={running} onClick={onUpdate}>
            {t("instance.updates.update", "Update to {version}", { version: latest.version })}
          </Button>
        ) : undefined
      }
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
              {reason && <Text tone="secondary">{reason}</Text>}
              <CodeBlock title={t("instance.updates.byHandTitle", "Update by hand, in the install's folder")} lang="bash" code={commands} collapseAfter={0} />
              <Link href={UPDATER_DOCS} external>
                {t("instance.updates.setUpUpdater", "Set up one-click updates")}
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

function Choices({ u }: { u: UpdatesView }) {
  const save = useApiMutation({
    mutationFn: async (body: { check?: boolean; channel?: "stable" | "prerelease"; apply?: boolean }) =>
      data(await api.PATCH("/platform/v1/instance/updates/settings", { body })),
    invalidates: [["instance", "updates"]],
    onSuccess: (_, body) =>
      toast.success(
        body.apply !== undefined
          ? body.apply
            ? t("instance.updates.applyOn", "One-click updates are on")
            : t("instance.updates.applyOffSaved", "One-click updates are off")
          : body.check !== undefined
            ? body.check
              ? t("instance.updates.checkOn", "Kaeso looks for releases again")
              : t("instance.updates.checkOffSaved", "Kaeso stopped looking for releases")
            : t("instance.updates.channelSaved", "Channel saved"),
      ),
  });
  const pending = save.isPending ? save.variables : undefined;
  return (
    <SettingsSection title={t("common.settings", "Settings")}>
      <SettingsGroup>
        <SettingsRow
          title={t("instance.updates.checkTitle", "Look for new releases")}
          description={t("instance.updates.checkRowHint", "Twice a day; off for installs without internet.")}
          control={
            <Switch
              aria-label={t("instance.updates.checkTitle", "Look for new releases")}
              checked={u.settings.check}
              loading={pending?.check !== undefined}
              onCheckedChange={(check) => save.mutate({ check })}
            />
          }
        />
        <SettingsRow
          title={t("instance.updates.channel", "Releases")}
          description={t("instance.updates.channelHint", "Which releases this install is offered.")}
          control={
            <div className="w-200">
              <Select
                aria-label={t("instance.updates.channel", "Releases")}
                size="sm"
                value={u.settings.channel}
                onValueChange={(channel) => save.mutate({ channel: channel === "prerelease" ? "prerelease" : "stable" })}
                options={[
                  { value: "stable", label: t("instance.updates.stable", "Stable only") },
                  { value: "prerelease", label: t("instance.updates.withPre", "With pre-releases") },
                ]}
              />
            </div>
          }
        />
        <SettingsRow
          title={t("instance.updates.applyTitle", "One-click updates")}
          description={t("instance.updates.applyHint", "Apply updates from here through the updater.")}
          control={
            <Switch
              aria-label={t("instance.updates.applyTitle", "One-click updates")}
              checked={u.settings.apply}
              loading={pending?.apply !== undefined}
              onCheckedChange={(apply) => save.mutate({ apply })}
            />
          }
        />
      </SettingsGroup>
    </SettingsSection>
  );
}

function History() {
  const jobs = useQuery(updateJobsQuery);
  const rows = jobs.data ?? [];
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
      (j: UpdateJobView) => j.started_by?.name ?? "",
      (name: string) => <Text truncate>{name || t("instance.updates.deletedUser", "Deleted user")}</Text>,
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
