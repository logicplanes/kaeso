import type { QueryClient } from "@tanstack/react-query";
import type { LoggingTab } from "~/features/org/logging-page";
import { orgRights, sessionQuery } from "~/auth/session";
import { meQuery } from "./me";
import { logDeletionsQuery, logSettingsQuery, logStorageQuery, orgMembersQuery, orgQuery, platformKeysQuery, securityQuery } from "./orgs";
import { orgProjectsQuery } from "./projects";
import { rightsCatalogueQuery, rolesQuery } from "./rights";
import { instanceOrgsQuery, instanceSettingsQuery, instanceUsersQuery, modelsDevQuery, systemQuery } from "./instance";

export function prefetchSettingsPath(client: QueryClient, path: string): void {
  const [, , area, a, b] = path.split(/[/?#]/);
  if (area === "account" && (a === "profile" || a === "security")) prefetchAccountSettings(client);
  else if (area === "orgs" && a && isOrgPage(b)) prefetchOrgSettings(client, a, b === "logging" ? "settings" : b);
  else if (area === "instance" && a)
    prefetchInstanceSettings(
      client,
      a === "users" ? { page: a, search: "", state: undefined } : a === "orgs" ? { page: a, search: "" } : isInstancePage(a) ? { page: a } : null,
    );
}

const ORG_PAGES = ["general", "members", "security", "platform-keys", "logging"] as const;
const isOrgPage = (p: string | undefined): p is (typeof ORG_PAGES)[number] => (ORG_PAGES as readonly (string | undefined)[]).includes(p);
const INSTANCE_PAGES = ["settings", "model-sources", "system"] as const;
const isInstancePage = (p: string): p is (typeof INSTANCE_PAGES)[number] => (INSTANCE_PAGES as readonly string[]).includes(p);

export type OrgSettingsData = "general" | "members" | "security" | "platform-keys" | LoggingTab;

export function prefetchOrgSettings(client: QueryClient, org: string, page: OrgSettingsData): void {
  const session = client.getQueryData(sessionQuery.queryKey);
  if (!session) return;
  const rights = orgRights(session, org);
  switch (page) {
    case "general":
      if (rights.has("org.view")) void client.prefetchQuery(orgQuery(org));
      if (rights.has("org.ownership.transfer") && rights.has("members.view")) void client.prefetchQuery(orgMembersQuery(org));
      return;
    case "members":
      if (rights.has("members.view")) void client.prefetchQuery(orgMembersQuery(org));
      prefetchRoles(client, org, rights);
      return;
    case "security":
      if (rights.has("org.view")) void client.prefetchQuery(securityQuery(org));
      return;
    case "platform-keys":
      if (rights.has("platform_keys.view")) void client.prefetchQuery(platformKeysQuery(org, false));
      return;
    case "settings":
      if (rights.has("org.view")) void client.prefetchQuery(logSettingsQuery(org));
      return;
    case "storage":
    case "deletions":
      if (!rights.has("logs.storage.view")) return;
      if (page === "storage") void client.prefetchQuery(logStorageQuery(org));
      else void client.prefetchQuery(logDeletionsQuery(org));
      void client.prefetchQuery(orgProjectsQuery(org, true));
      return;
  }
}

function prefetchAccountSettings(client: QueryClient): void {
  void client.prefetchQuery(meQuery);
}

export type InstanceSettingsData =
  { page: "users"; search: string; state: string | undefined } | { page: "orgs"; search: string } | { page: "settings" | "model-sources" | "system" };

export function prefetchInstanceSettings(client: QueryClient, data: InstanceSettingsData | null): void {
  if (!data || !client.getQueryData(sessionQuery.queryKey)?.user.is_instance_admin) return;
  switch (data.page) {
    case "users":
      void client.prefetchInfiniteQuery(instanceUsersQuery(data.search, data.state));
      return;
    case "orgs":
      void client.prefetchInfiniteQuery(instanceOrgsQuery(data.search));
      return;
    case "settings":
      void client.prefetchQuery(instanceSettingsQuery);
      return;
    case "model-sources":
      void client.prefetchQuery(modelsDevQuery);
      return;
    case "system":
      void client.prefetchQuery(systemQuery);
      return;
  }
}

function prefetchRoles(client: QueryClient, org: string, rights: ReadonlySet<string>): void {
  if (!rights.has("org.view")) return;
  void client.prefetchQuery(rolesQuery(org));
  void client.prefetchQuery(rightsCatalogueQuery(org));
}
