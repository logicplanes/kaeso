import { redirect } from "@tanstack/react-router";
import type { QueryClient } from "@tanstack/react-query";
import { orgProjectsQuery, projectQuery } from "~/api/queries/projects";
import { keptWhileReadOnly } from "~/auth/rights";
import { membershipsQuery, projectRightsQuery, sessionQuery } from "~/auth/session";
import { writeLast } from "~/shell/last";
import { PROJECT_PAGES } from "~/shell/pages";
import { homePlace } from "~/shell/place";
import { toast } from "~/shell/toast";

export async function goHome(queryClient: QueryClient): Promise<void> {
  const place = homePlace(await queryClient.ensureQueryData(membershipsQuery));
  if (place?.project) throw redirect({ to: "/orgs/$org/projects/$project", params: { org: place.org, project: place.project } });
  if (place) throw redirect({ to: "/orgs/$org", params: { org: place.org } });
}

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
  const page = PROJECT_PAGES.find((p) => p.path === (location.pathname.slice(base.length).split("/")[1] ?? ""));
  const first = PROJECT_PAGES.find((p) => holds(p.right));
  if (page && first && !holds(page.right)) {
    if (!preload && page.path) {
      const right = page.right;
      void import("~/auth/needs-right").then(async (m) => toast(await m.explainMissingRight(queryClient, org, project, right)));
    }
    throw redirect({ href: first.path ? `${base}/${first.path}` : base, replace: true });
  }
  if (!preload) writeLast({ org, project });
}
