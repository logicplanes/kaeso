import { createFileRoute } from "@tanstack/react-router";
import { OrgLoggingPage, type LoggingTab } from "~/features/org/logging-page";

const loggingSearch = (s: Record<string, unknown>): { tab?: LoggingTab } => (s.tab === "storage" || s.tab === "deletions" ? { tab: s.tab } : {});

export const Route = createFileRoute("/_app/settings/orgs/$org/logging")({ validateSearch: loggingSearch, component: Logging });

function Logging() {
  return <OrgLoggingPage org={Route.useParams().org} tab={Route.useSearch({ select: loggingSearch, structuralSharing: true }).tab ?? "settings"} />;
}
