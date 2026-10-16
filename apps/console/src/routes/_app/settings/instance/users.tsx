import { createFileRoute } from "@tanstack/react-router";
import { InstanceUsersPage, type UsersSearch } from "~/features/instance/users-page";

const usersSearch = (s: Record<string, unknown>): UsersSearch => ({
  ...(typeof s.q === "string" && s.q.trim() ? { q: s.q.trim() } : {}),
  ...(s.state === "active" || s.state === "deactivated" || s.state === "deleted" ? { state: s.state } : {}),
  ...(typeof s.user === "string" ? { user: s.user } : {}),
  ...(s.invite === true || s.invite === "true" ? { invite: true } : {}),
});

export const Route = createFileRoute("/_app/settings/instance/users")({ validateSearch: usersSearch, component: Users });

function Users() {
  return <InstanceUsersPage search={Route.useSearch({ select: usersSearch, structuralSharing: true })} />;
}
