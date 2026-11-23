import { useQuery } from "@tanstack/react-query";
import { Banner, Spinner } from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import type { ExternalSourceView } from "~/api/generated/types";
import { sourceQuery } from "~/api/queries/sources";
import { t } from "~/i18n";
import { refusalText, totals } from "./labels";

/** How often a source waiting for its first event is asked again. */
export const WATCH_MS = 3_000;

/**
 * Whether a source has reported yet, asked again every few seconds until it has: waiting, then
 * connected, or what Kaeso refused. One banner whose words change, so its status is announced once.
 */
export function Connection({ org, project, source }: { org: string; project: string; source: ExternalSourceView }) {
  const live = useQuery({
    ...sourceQuery(org, project, source.id),
    initialData: source,
    refetchInterval: (q) => (q.state.data?.first_event_at ? false : WATCH_MS),
  });
  const s = live.data;
  const sum = totals(s);
  if (s.first_event_at)
    return (
      <Banner tone="success" title={t("project.sources.connected", "Connected")}>
        {t(
          "project.sources.connectedHint",
          "First event {when}; {requests, plural, one {# request} other {# requests}} and {calls, plural, one {# tool call} other {# tool calls}} so far.",
          { when: formatRelative(s.first_event_at), requests: sum.requests, calls: sum.toolCalls },
        )}
      </Banner>
    );
  if (s.last_error)
    return (
      <Banner tone="warning" title={t("project.sources.refusedTitle", "Kaeso refused what it sent")}>
        {refusalText(s.last_error)}
      </Banner>
    );
  return (
    <Banner tone="info">
      <span className="flex items-center gap-8">
        <Spinner size="sm" />
        {t("project.sources.waitingFirst", "Waiting for the first event; use the client once.")}
      </span>
    </Banner>
  );
}
