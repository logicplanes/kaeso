import { Text } from "@logicplanes/kaeso-ui";
import { formatBytes, formatDuration } from "@logicplanes/kaeso-ui/format";
import { saveExport, type ExportFile } from "~/api/download";
import { toastApiError } from "~/api/toast-error";
import { t } from "~/i18n";
import { closeToast, progressToast, toast, updateProgressToast } from "./toast";

const TOAST = "export-progress";

export type ExportTitle = (size: string | null) => string;

interface Job {
  title: ExportTitle;
  controller: AbortController;
  saving: boolean;
  bytes: number;
}

let job: Job | null = null;

const title = (text: ExportTitle, size: string | null) => (
  <Text tone="inherit" weight="medium" tabular>
    {text(size)}
  </Text>
);
const reserve = (text: ExportTitle) => [title(text, null), title(text, "00.0 KB"), title(text, "00.0 MB"), title(text, "00.0 GB")];

function showProgress(current: Job) {
  progressToast(TOAST, title(current.title, current.bytes ? formatBytes(current.bytes) : null), {
    reserve: reserve(current.title),
    action: { label: t("common.cancel", "Cancel"), onClick: () => current.controller.abort() },
  });
}

export function showExportProgress(): boolean {
  if (!job) return false;
  if (job.saving) showProgress(job);
  return true;
}

const warnOnLeave = (e: BeforeUnloadEvent) => e.preventDefault();

export function startExport({ title: text, done, url, check, file }: { title: ExportTitle; done: string; url: string; check: string; file: ExportFile }): void {
  if (showExportProgress()) return;
  const current: Job = { title: text, controller: new AbortController(), saving: false, bytes: 0 };
  job = current;
  let started = 0;
  let shownAt = 0;
  const onSaving = () => {
    started = performance.now();
    current.saving = true;
    window.addEventListener("beforeunload", warnOnLeave);
    showProgress(current);
  };
  const onProgress = (bytes: number) => {
    current.bytes = bytes;
    const now = performance.now();
    if (now - shownAt < 500) return;
    shownAt = now;
    updateProgressToast(TOAST, title(text, formatBytes(bytes)));
  };
  void saveExport(url, file, { check, signal: current.controller.signal, onSaving, onProgress })
    .then((outcome) => {
      closeToast(TOAST);
      if (outcome.kind === "saved")
        toast.success(done, {
          description: t("shell.export.saved", "{size} in {duration}", {
            size: formatBytes(outcome.bytes),
            duration: formatDuration(performance.now() - started),
          }),
        });
      else if (outcome.kind === "browser")
        toast(t("shell.export.started", "Export started"), { description: t("shell.export.inBrowser", "Your browser shows its progress.") });
      else if (outcome.kind === "canceled") toast(t("shell.export.canceled", "Export canceled"));
    })
    .catch((e: unknown) => {
      closeToast(TOAST);
      toastApiError(e, { fallback: t("shell.export.failed", "The export stopped before the end. Try again.") });
    })
    .finally(() => {
      window.removeEventListener("beforeunload", warnOnLeave);
      job = null;
    });
}
