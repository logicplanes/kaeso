import { createFileRoute } from "@tanstack/react-router";
import { InstanceOrgsPage, type InstanceOrgsSearch } from "~/features/instance/orgs-page";

const orgsSearch = (s: Record<string, unknown>): InstanceOrgsSearch => ({
  ...(typeof s.q === "string" && s.q.trim() ? { q: s.q.trim() } : {}),
  ...(s.sort === "deletion" ? { sort: "deletion" as const } : {}),
  ...(s.create === true || s.create === "true" ? { create: true } : {}),
});

export const Route = createFileRoute("/_app/settings/instance/orgs/")({ validateSearch: orgsSearch, component: Orgs });

function Orgs() {
  return <InstanceOrgsPage search={Route.useSearch({ select: orgsSearch, structuralSharing: true })} />;
}
