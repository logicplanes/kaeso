import { useEffect, useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Banner, Button, Code, Dialog, Link, SettingsGroup, SettingsRow, StatusDot, Text } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { ReleaseView, UpdateJobView } from "~/api/generated/types";
import { updateJobQuery } from "~/api/queries/instance";
import { useApiMutation } from "~/auth/mutation";
import { BACKUP_DOCS, STEPS, jobResult, reasonText, stepLabel, type Tone } from "./update-words";
import { t } from "~/i18n";

export type Opened = { release: ReleaseView } | { job: string };

interface Shown {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function UpdateDialog({ opened, open, onOpenChange }: Shown & { opened: Opened }) {
  const [started, setStarted] = useState<string | null>("job" in opened ? opened.job : null);
  const start = useApiMutation({
    mutationFn: async (version: string) => data(await api.POST("/platform/v1/instance/updates/apply", { body: { version } })),
    invalidates: [["instance", "updates"]],
    onSuccess: (j) => setStarted(j.id),
  });
  const job = useQuery({
    ...updateJobQuery(started ?? ""),
    enabled: started !== null,
    retry: true,
    retryDelay: 2_000,
    refetchInterval: (q) => (q.state.data && q.state.data.state !== "running" ? false : 2_000),
  });
  const j = started ? job.data : undefined;
  const finished = !!j && j.state !== "running";
  const queryClient = useQueryClient();
  useEffect(() => {
    if (finished) void queryClient.invalidateQueries({ queryKey: ["instance", "updates"] });
  }, [finished, queryClient]);

  if (started === null && "release" in opened) {
    const version = opened.release.version;
    return (
      <Dialog
        open={open}
        onOpenChange={onOpenChange}
        busy={start.isPending}
        title={t("instance.updates.confirmTitle", "Update to Kaeso {version}?", { version })}
        description={t("instance.updates.confirmHint", "Kaeso is unavailable for about a minute; apps get an error that asks them to try again.")}
        onSubmit={() => start.mutate(version)}
        footer={
          <>
            <Dialog.Close>
              <Button>{t("common.cancel", "Cancel")}</Button>
            </Dialog.Close>
            <Button variant="primary" loading={start.isPending} onClick={() => start.mutate(version)}>
              {t("instance.updates.updateNow", "Update now")}
            </Button>
          </>
        }
      >
        <div className="flex flex-col gap-12">
          <Text tone="secondary">
            {t(
              "instance.updates.backup",
              "Back up PostgreSQL and ClickHouse first: once the new version has migrated them, going back means restoring those backups.",
            )}
          </Text>
          <Text tone="secondary">
            {t(
              "instance.updates.verifyNote",
              "The updater installs only images built and signed by Kaeso's release workflow; if the new version doesn't start, it goes back to this one unless the database was already migrated.",
            )}
          </Text>
          <Link href={BACKUP_DOCS} external>
            {t("instance.updates.howBackup", "How to back up")}
          </Link>
        </div>
      </Dialog>
    );
  }

  const away = job.isError || job.failureCount > 0;
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={
        j?.state === "succeeded"
          ? t("instance.updates.doneTitle", "Kaeso {version} is running", { version: j.to })
          : t("instance.updates.progressTitle", "Updating to Kaeso {version}", { version: j?.to ?? ("release" in opened ? opened.release.version : "") })
      }
      description={
        finished
          ? undefined
          : away
            ? t("instance.updates.away", "Kaeso is switching over; this window reconnects by itself.")
            : t("instance.updates.progressHint", "You can close this window; the update goes on without it.")
      }
      footer={
        j?.state === "succeeded" ? (
          <Button variant="primary" onClick={() => window.location.reload()}>
            {t("instance.updates.reload", "Reload the console")}
          </Button>
        ) : (
          <Dialog.Close>
            <Button>{t("common.close", "Close")}</Button>
          </Dialog.Close>
        )
      }
    >
      <div className="flex flex-col gap-16">
        {j && (
          <SettingsGroup aria-label={t("instance.updates.steps", "Steps")}>
            {STEPS.map((step) => {
              const s = stepState(j, step);
              return <SettingsRow key={step} title={stepLabel(step)} control={<StatusDot tone={s.tone}>{s.label}</StatusDot>} />;
            })}
          </SettingsGroup>
        )}
        {finished && j && <Outcome job={j} />}
      </div>
    </Dialog>
  );
}

function stepState(job: UpdateJobView, step: string): { tone: Tone; label: string } {
  const at = STEPS.indexOf(job.step as (typeof STEPS)[number]);
  const index = STEPS.indexOf(step as (typeof STEPS)[number]);
  if (job.state === "succeeded" || index < at) return { tone: "success", label: t("instance.updates.stepDone", "Done") };
  if (index > at) return { tone: "neutral", label: t("instance.updates.stepWaiting", "Waiting") };
  if (job.state === "running") return { tone: "info", label: t("instance.updates.stepNow", "In progress") };
  return { tone: "danger", label: t("instance.updates.stepFailed", "Stopped") };
}

function Outcome({ job: j }: { job: UpdateJobView }): ReactNode {
  const result = jobResult(j);
  if (j.state === "succeeded" && !j.reason) return null;
  return (
    <Banner
      tone={result.tone === "danger" ? "danger" : "warning"}
      title={j.state === "succeeded" ? t("instance.updates.leftToDo", "One step is left") : result.label}
    >
      <span className="flex flex-col gap-8">
        <span>{reasonText(j.reason) ?? t("instance.updates.whyUnknown", "The update didn't finish.")}</span>
        {j.state === "succeeded" && (
          <span>{t("instance.updates.composeByHand", "Download the compose.yaml of Kaeso {version} over the install's.", { version: j.to })}</span>
        )}
        {j.state === "rolled_back" && <span>{t("instance.updates.rolledBackHint", "Kaeso {version} runs again, unchanged.", { version: j.from })}</span>}
        {j.state === "rollback_failed" && (
          <span>{t("instance.updates.rollbackFailedHint", "The previous version didn't start again; follow the upgrade guide to recover.")}</span>
        )}
        {j.state === "needs_attention" && j.reason !== "migrated" && (
          <span>{t("instance.updates.attentionHint", "Kaeso needs someone to look at its containers; the detail below says what happened.")}</span>
        )}
        {j.state === "needs_attention" && j.reason === "migrated" && (
          <span>
            {t(
              "instance.updates.needsAttentionHint",
              "The new gateway may have migrated the database, so Kaeso {version} wasn't started again; follow the upgrade guide.",
              { version: j.from },
            )}
          </span>
        )}
        {j.detail && <Code>{j.detail}</Code>}
      </span>
    </Banner>
  );
}
