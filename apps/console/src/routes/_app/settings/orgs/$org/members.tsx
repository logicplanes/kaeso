import { createFileRoute } from "@tanstack/react-router";
import { OrgMembersPage, type OrgMembersSearch } from "~/features/org/members-page";

const membersSearch = (s: Record<string, unknown>): OrgMembersSearch => ({
  ...(s.tab === "invites" || s.tab === "roles" ? { tab: s.tab } : {}),
  ...(typeof s.member === "string" ? { member: s.member } : {}),
  ...(typeof s.role === "string" ? { role: s.role } : typeof s.preset === "string" ? { role: s.preset } : {}),
  ...(s.invite === true || s.invite === "true" ? { invite: true } : {}),
  ...(s.create === true || s.create === "true" ? { create: true } : {}),
});

export const Route = createFileRoute("/_app/settings/orgs/$org/members")({ validateSearch: membersSearch, component: Members });

function Members() {
  return <OrgMembersPage org={Route.useParams().org} search={Route.useSearch({ select: membersSearch, structuralSharing: true })} />;
}
