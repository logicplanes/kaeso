import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api, data } from "~/api/client";
import type { McpServerView } from "~/api/generated/types";
import { mcpServerQuery } from "~/api/queries/mcp-servers";
import { useApiMutation } from "~/auth/mutation";
import { openBlankTab } from "./blank-tab";

const POLL_MS = 2000;
// The sign-in address Kaeso hands out is valid for 10 minutes.
const SIGN_IN_MS = 10 * 60_000;

type Begin = { tab: Window | null; ensure: () => Promise<string> };
type Waiting = { server: string; since: string | null };

export function useMcpSignIn(org: string) {
  const [waiting, setWaiting] = useState<Waiting | null>(null);
  const [link, setLink] = useState<string | null>(null);
  const [expired, setExpired] = useState(false);
  const [server, setServer] = useState<McpServerView | null>(null);
  const watched = useQuery({
    ...mcpServerQuery(org, waiting?.server ?? ""),
    enabled: !!waiting,
    refetchInterval: waiting ? POLL_MS : false,
  });
  const seen = watched.data;
  const connected = !!waiting && !!seen?.connection && !seen.connection.signed_out && seen.connection.updated_at !== waiting.since ? seen : null;
  if (connected) {
    setServer(connected);
    setWaiting(null);
    setLink(null);
  }
  useEffect(() => {
    if (!waiting) return;
    const end = window.setTimeout(() => {
      setWaiting(null);
      setExpired(true);
    }, SIGN_IN_MS);
    return () => window.clearTimeout(end);
  }, [waiting]);
  const begin = useApiMutation({
    mutationFn: async ({ ensure }: Begin) => {
      const id = await ensure();
      return data(await api.POST("/platform/v1/orgs/{org}/mcp-servers/{server}/connection", { params: { path: { org, server: id } }, body: { oauth: {} } }));
    },
    invalidates: [["orgs", org, "mcp-servers"]],
    onSuccess: (answer, { tab }) => {
      setExpired(false);
      setServer(null);
      setWaiting({ server: answer.server.id, since: answer.server.connection?.updated_at ?? null });
      const url = answer.redirect_url ?? null;
      if (tab && url) tab.location.href = url;
      else tab?.close();
      setLink(tab ? null : url);
    },
    onError: (_error, { tab }) => tab?.close(),
  });
  return {
    start: (ensure: () => Promise<string>) => begin.mutate({ tab: openBlankTab(), ensure }),
    starting: begin.isPending,
    waiting: !!waiting,
    link,
    expired,
    server,
  };
}
