import type { UpdateJobView } from "~/api/generated/types";
import { t } from "~/i18n";

export const UPDATER_DOCS = "https://docs.kaeso.app/self-hosting/upgrades#one-click-updates";
export const BACKUP_DOCS = "https://docs.kaeso.app/self-hosting/backups";

export const STEPS = ["verifying", "downloading", "switching", "migrating", "online"] as const;

export type Tone = "success" | "warning" | "danger" | "info" | "neutral";

export function stepLabel(step: string): string {
  switch (step) {
    case "downloading":
      return t("instance.updates.stepDownloading", "Download the images");
    case "verifying":
      return t("instance.updates.stepVerifying", "Check the release's signatures");
    case "switching":
      return t("instance.updates.stepSwitching", "Switch to the new version");
    case "migrating":
      return t("instance.updates.stepMigrating", "Migrate the database");
    default:
      return t("instance.updates.stepOnline", "Back online");
  }
}

export function reasonText(reason: string | null | undefined): string | null {
  switch (reason) {
    case null:
    case undefined:
      return null;
    case "manifest_unavailable":
      return t("instance.updates.whyManifest", "The release's files couldn't be downloaded from GitHub.");
    case "manifest_invalid":
      return t("instance.updates.whyInvalid", "The release's manifest isn't valid, so nothing was installed.");
    case "manual_steps":
      return t("instance.updates.whyManual", "The release asks for steps by hand; follow the upgrade guide.");
    case "major":
      return t("instance.updates.whyMajor", "The release is a new major version; follow the upgrade guide.");
    case "not_newer":
      return t("instance.updates.whyNotNewer", "This version runs already.");
    case "updater_outdated":
      return t("instance.updates.whyOutdated", "The updater is too old for this release; update by hand once.");
    case "compose_changed":
      return t("instance.updates.whyCompose", "Your compose.yaml has changes of its own; update by hand.");
    case "compose_unwritable":
      return t("instance.updates.whyUnwritable", "The updater couldn't write compose.yaml.");
    case "services_not_found":
      return t("instance.updates.whyServices", "The updater couldn't find the gateway and console containers.");
    case "pull_failed":
      return t("instance.updates.whyPull", "The new images couldn't be downloaded.");
    case "unverified":
      return t("instance.updates.whyUnverified", "The release's signatures didn't check out, so nothing was installed.");
    case "docker_unavailable":
      return t("instance.updates.whyDocker", "The updater couldn't reach Docker.");
    case "unhealthy":
      return t("instance.updates.whyUnhealthy", "The new version didn't start.");
    case "interrupted":
      return t("instance.updates.whyInterrupted", "The update was interrupted.");
    case "updater_unreachable":
      return t("instance.updates.whyUnreachable", "The updater couldn't be reached.");
    case "updater_refused":
      return t("instance.updates.whyRefused", "The updater refused the update; its logs say why.");
    case "state_unwritable":
      return t("instance.updates.whyState", "The updater couldn't save its progress, so it didn't start.");
    default:
      return reason;
  }
}

export function jobResult(job: UpdateJobView): { tone: Tone; label: string; detail: string | null } {
  const detail = reasonText(job.reason);
  switch (job.state) {
    case "running":
      return { tone: "info", label: t("instance.updates.stateRunning", "Running"), detail: null };
    case "succeeded":
      return { tone: detail ? "warning" : "success", label: t("instance.updates.stateSucceeded", "Updated"), detail };
    case "needs_attention":
      return { tone: "danger", label: t("instance.updates.stateNeedsAttention", "Needs attention"), detail };
    case "rolled_back":
      return { tone: "warning", label: t("instance.updates.stateRolledBack", "Rolled back"), detail };
    case "rollback_failed":
      return { tone: "danger", label: t("instance.updates.stateRollbackFailed", "Rollback failed"), detail };
    default:
      return { tone: "danger", label: t("instance.updates.stateFailed", "Not installed"), detail };
  }
}
