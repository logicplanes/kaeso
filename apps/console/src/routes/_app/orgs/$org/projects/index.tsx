import { createFileRoute } from "@tanstack/react-router";
import { OrgProjectsPage, type OrgProjectsSearch } from "~/features/org-home/org-projects-page";

const projectsSearch = (s: Record<string, unknown>): OrgProjectsSearch => (s.create === true || s.create === "true" ? { create: true } : {});

export const Route = createFileRoute("/_app/orgs/$org/projects/")({ validateSearch: projectsSearch, component: Projects });

function Projects() {
  return <OrgProjectsPage org={Route.useParams().org} search={Route.useSearch({ select: projectsSearch, structuralSharing: true })} />;
}
