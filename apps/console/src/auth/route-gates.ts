import { redirect } from "@tanstack/react-router";
import type { QueryClient } from "@tanstack/react-query";
import { orgProjectsQuery, projectQuery } from "~/api/queries/projects";
import { internalPath } from "~/auth/next";
import { RETURN_KEY } from "~/auth/recent-sign-in-store";
import { keptWhileReadOnly } from "~/auth/rights";
import { membershipsQuery, projectRightsQuery, sessionQuery } from "~/auth/session";
import { writeLast } from "~/shell/last";
import { homePlace } from "~/shell/place";
import { toast } from "~/shell/toast";
import { takeStored } from "~/storage";

function takeReturn(): string | null {
  return internalPath(takeStored(RETURN_KEY, "session"));
}

export async function goHome(queryClient: QueryClient): Promise<void> {
  const back = takeReturn();
  if (back) throw redirect({ href: back });
  const place = homePlace(await queryClient.ensureQueryData(membershipsQuery));
  if (place?.project) throw redirect({ to: "/orgs/$org/projects/$project", params: { org: place.org, project: place.project } });
  if (place) throw redirect({ to: "/orgs/$org", params: { org: place.org } });
}

const PAGES: [path: string, right: string][] = [
  ["", "usage.view"],
  ["playground", "playground.use"],
  ["keys", "keys.all.view"],
  ["logs", "logs.metadata.view"],
  ["models", "models.view"],
  ["limits", "limits.view"],
  ["members", "project.members.view"],
  ["service-accounts", "service_accounts.view"],
  ["audit-log", "project.audit_log.view"],
  ["settings", "project.view"],
];

export async function projectGate({
  context: { queryClient },
  params: { org, project },
  location,
  preload,
}: {
  context: { queryClient: QueryClient };
  params: { org: string; project: string };
  location: { pathname: string };
  preload: boolean;
}): Promise<void> {
  const session = await queryClient.ensureQueryData(sessionQuery);
  const [rights] = await Promise.all([
    queryClient.ensureQueryData(projectRightsQuery(session, org, project)),
    queryClient.ensureQueryData(projectQuery(org, project)),
    queryClient.ensureQueryData(orgProjectsQuery(org)).catch(() => undefined),
  ]);
  const readOnly = session.memberships.find((m) => m.org_id === org)?.org_state === "scheduled_for_deletion";
  const holds = (right: string) => rights.has(right) && (!readOnly || keptWhileReadOnly(right));
  const base = `/orgs/${org}/projects/${project}`;
  const page = PAGES.find(([path]) => path === (location.pathname.slice(base.length).split("/")[1] ?? ""));
  const first = PAGES.find(([, right]) => holds(right));
  if (page && first && !holds(page[1])) {
    if (!preload && page[0]) {
      const right = page[1];
      void import("~/auth/needs-right").then(async (m) => toast(await m.explainMissingRight(queryClient, org, project, right)));
    }
    throw redirect({ href: first[0] ? `${base}/${first[0]}` : base, replace: true });
  }
  if (!preload) writeLast({ org, project });
}
