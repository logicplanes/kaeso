import type { ReactNode } from "react";
import { Button, icons } from "@logicplanes/kaeso-ui";
import { useProjectRights } from "~/auth/rights";
import { useRequestLogs } from "~/auth/session";
import { t } from "~/i18n";

/**
 * A sheet's way to what its key, member, service account or model did and who changed it: the
 * request log and the project audit log, already filtered, each for someone who may read it.
 */
export function SheetLinks({
  org,
  project,
  requests,
  target,
  more,
}: {
  org: string;
  project: string;
  /** The request log's filters for this thing, such as `{ key: "key_…" }`. */
  requests: Record<string, string>;
  /** The id the audit log records it under; none where the sheet has its own audit links. */
  target?: string | undefined;
  more?: ReactNode;
}) {
  const rights = useProjectRights(org, project);
  const logs = useRequestLogs() && rights.has("logs.metadata.view");
  const base = `/orgs/${org}/projects/${project}`;
  const history = target && rights.has("project.audit_log.view") ? `${base}/audit-log?${new URLSearchParams({ target }).toString()}` : undefined;
  if (!logs && !history && !more) return null;
  return (
    <div className="flex flex-wrap gap-8">
      {logs && (
        <Button size="sm" variant="ghost" leadingIcon={icons.requestLog} href={`${base}/logs?${new URLSearchParams(requests).toString()}`}>
          {t("kaeso.sheet.seeRequests", "See requests")}
        </Button>
      )}
      {history && (
        <Button size="sm" variant="ghost" leadingIcon={icons.audit} href={history}>
          {t("kaeso.sheet.history", "History")}
        </Button>
      )}
      {more}
    </div>
  );
}
