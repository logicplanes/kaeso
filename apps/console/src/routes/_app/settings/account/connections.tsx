import { createFileRoute } from "@tanstack/react-router";
import { ConnectionsPage, type ConnectionsSearch } from "~/features/account/connections-page";
import type { SignInResult } from "~/features/org-home/mcp-servers-page";

const RESULTS: SignInResult[] = ["connected", "declined", "expired", "failed"];

const connectionsSearch = (s: Record<string, unknown>): ConnectionsSearch => ({
  ...(typeof s.server === "string" && s.server ? { server: s.server } : {}),
  ...(RESULTS.includes(s.result as SignInResult) ? { result: s.result as SignInResult } : {}),
});

export const Route = createFileRoute("/_app/settings/account/connections")({ validateSearch: connectionsSearch, component: Connections });

function Connections() {
  return <ConnectionsPage search={Route.useSearch({ select: connectionsSearch, structuralSharing: true })} />;
}
