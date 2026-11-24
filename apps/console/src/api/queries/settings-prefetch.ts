import type { QueryClient } from "@tanstack/react-query";
import { loadedOrgRights, orgRights, sessionQuery } from "~/auth/session";
import { auditLogQuery } from "./audit";
import { validateAuditSearch } from "./audit-search";
import { agentCatalogueQuery, agentQuery, agentsQuery, grantableQuery } from "./agents";
import { myConnectionsQuery } from "./connections";
import { guardrailsQuery } from "./guardrails";
import {
  instanceOrgMembersQuery,
  instanceOrgProjectsQuery,
  instanceOrgQuery,
  instanceOrgUsageQuery,
  instanceOrgsQuery,
  instanceSettingsQuery,
  instanceUsersQuery,
  systemQuery,
  updateJobsQuery,
  updatesQuery,
} from "./instance";
import { meQuery, myKeysQuery, mySessionsQuery, notificationsQuery, ssoLinksQuery } from "./me";
import {
  invitesQuery,
  logDeletionsQuery,
  logSettingsQuery,
  logStorageQuery,
  orgMembersQuery,
  orgQuery,
  orgResponseCacheQuery,
  orgStoredResponsesQuery,
  platformKeysQuery,
  securityQuery,
} from "./orgs";
import { orgProjectsQuery } from "./projects";
import { rightsCatalogueQuery, rolesQuery } from "./rights";
import { sharedLinksQuery } from "./shared-links";
import { webhooksQuery } from "./webhooks";

// The settings routes' loaders start what each page draws first, so the data is on its way while the person still
// points at the link, and the page opens without a skeleton. Each one asks only for what the page itself would ask
// for with the person's rights, so a hover never costs a request the page wouldn't make. The organization and
// instance settings share a layout whose loader runs for every page under it, so that one loader reads the page from
// the address, and so does the account settings' layout. The settings sidebar also
// starts a page's data the moment a pointer or finger reaches its link, before the router has loaded the loader's code,
// which on a phone is the difference between a skeleton and none.

export type LoggingTab = "settings" | "storage" | "deletions";

type Search = Record<string, unknown>;

const loggingTab = (tab: unknown): LoggingTab => (tab === "storage" || tab === "deletions" ? tab : "settings");
const membersTab = (tab: unknown): OrgSettingsData => (tab === "invites" || tab === "roles" ? tab : "members");
const ORG_PAGES = ["general", "security", "platform-keys", "guardrails", "webhooks", "shared-links"] as const;
const isOrgPage = (p: string | undefined): p is (typeof ORG_PAGES)[number] => (ORG_PAGES as readonly (string | undefined)[]).includes(p);

/** Starts the data of the organization settings page at `pathname`, `/settings/orgs/{org}/{page}`. */
export function prefetchOrgSettingsPage(client: QueryClient, org: string, pathname: string, search: Search): void {
  const page = pathname.split("/")[4];
  if (page === "members") prefetchOrgSettings(client, org, membersTab(search.tab));
  else if (page === "logging") prefetchOrgSettings(client, org, loggingTab(search.tab));
  else if (isOrgPage(page)) prefetchOrgSettings(client, org, page);
  else if (page === "audit-log" && loadedOrgRights(client, org).has("audit_log.view")) prefetchAuditLog(client, { org }, search);
}

const INSTANCE_PAGES = ["general", "logging", "system", "updates"] as const;
const isInstancePage = (p: string | undefined): p is (typeof INSTANCE_PAGES)[number] => (INSTANCE_PAGES as readonly (string | undefined)[]).includes(p);
// The lists of users and organizations start only as the sidebar links to them, without a search or filter; the page
// itself reads any other address, so a hover never asks for a list the page would read differently.
const unfiltered = (search: Search, keys: string[]) => keys.every((k) => search[k] === undefined);

/** Starts the data of the instance settings page at `pathname`, `/settings/instance/{page}`. */
export function prefetchInstanceSettingsPage(client: QueryClient, pathname: string, search: Search): void {
  const [, , , page, org] = pathname.split("/");
  if (isInstancePage(page)) prefetchInstanceSettings(client, { page });
  else if (page === "users" && unfiltered(search, ["q", "state"])) prefetchInstanceSettings(client, { page: "users", search: "", state: undefined });
  else if (page === "orgs" && org) prefetchInstanceSettings(client, { page: "org", org });
  else if (page === "orgs" && unfiltered(search, ["q", "sort", "sso"]))
    prefetchInstanceSettings(client, { page: "orgs", search: "", sort: "created", sso: undefined });
  else if (page === "audit-log" && client.getQueryData(sessionQuery.queryKey)?.can_administer_instance) {
    // The log names its people from the user list, and looks up only those the list doesn't name.
    void client.prefetchInfiniteQuery(instanceUsersQuery("", undefined));
    prefetchAuditLog(client, { instance: true }, search);
  }
}

function prefetchAuditLog(client: QueryClient, scope: { org: string } | { instance: true }, search: Search): void {
  const { event: _event, ...filters } = validateAuditSearch(search);
  void client.prefetchInfiniteQuery(auditLogQuery(scope, filters));
}

export type OrgSettingsData =
  "general" | "members" | "invites" | "roles" | "security" | "platform-keys" | "guardrails" | "webhooks" | "shared-links" | LoggingTab;

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
      if (!rights.has("members.view")) return;
      void client.prefetchQuery(orgMembersQuery(org));
      prefetchRoles(client, org, rights);
      return;
    case "invites":
      if (!rights.has("members.view")) return;
      void client.prefetchQuery(invitesQuery(org, true));
      void client.prefetchQuery(orgMembersQuery(org));
      void client.prefetchQuery(orgProjectsQuery(org, true));
      return;
    case "roles":
      if (rights.has("members.view")) prefetchRoles(client, org, rights);
      return;
    case "security":
      if (rights.has("org.view")) void client.prefetchQuery(securityQuery(org));
      return;
    case "platform-keys":
      if (rights.has("platform_keys.view")) void client.prefetchQuery(platformKeysQuery(org, false));
      return;
    case "guardrails":
      if (rights.has("guardrails.view")) void client.prefetchQuery(guardrailsQuery(org));
      return;
    case "webhooks":
      if (rights.has("webhooks.view")) void client.prefetchQuery(webhooksQuery(org));
      return;
    case "shared-links":
      if (!rights.has("usage.share")) return;
      void client.prefetchQuery(sharedLinksQuery(org));
      void client.prefetchQuery(orgProjectsQuery(org));
      return;
    case "settings":
      if (!rights.has("org.view")) return;
      if (session.request_logs !== false) void client.prefetchQuery(logSettingsQuery(org));
      void client.prefetchQuery(orgResponseCacheQuery(org));
      void client.prefetchQuery(orgStoredResponsesQuery(org));
      return;
    case "storage":
    case "deletions":
      if (!rights.has("logs.storage.view") || session.request_logs === false) return;
      if (page === "storage") void client.prefetchQuery(logStorageQuery(org));
      else void client.prefetchQuery(logDeletionsQuery(org));
      void client.prefetchQuery(orgProjectsQuery(org, true));
      return;
  }
}

const ACCOUNT_PAGES = ["profile", "security", "sessions", "keys", "connections", "notifications", "agents"] as const;
const isAccountPage = (p: string | undefined): p is (typeof ACCOUNT_PAGES)[number] => (ACCOUNT_PAGES as readonly (string | undefined)[]).includes(p);

/** Starts the data of the account settings page at `pathname`, `/settings/account/{page}[/{agent}]`. */
export function prefetchAccountSettingsPage(client: QueryClient, pathname: string): void {
  const [, , , page, agent] = pathname.split("/");
  if (page === "agents" && agent) prefetchAccountSettings(client, { agent });
  else if (isAccountPage(page)) prefetchAccountSettings(client, page);
}

/** Starts the data of the settings page a sidebar link leads to; the sidebar's links carry no search. */
export function prefetchSettingsLink(client: QueryClient, href: string): void {
  const pathname = href.replace(/[?#].*$/, "");
  const [, , area, a] = pathname.split("/");
  if (area === "orgs" && a) prefetchOrgSettingsPage(client, a, pathname, {});
  else if (area === "instance") prefetchInstanceSettingsPage(client, pathname, {});
  else if (area === "account") prefetchAccountSettingsPage(client, pathname);
}

export type AccountSettingsData = "profile" | "security" | "sessions" | "keys" | "connections" | "notifications" | "agents" | { agent: string };

export function prefetchAccountSettings(client: QueryClient, page: AccountSettingsData): void {
  if (typeof page === "object") {
    void client.prefetchQuery(agentQuery(page.agent));
    void client.prefetchQuery(agentCatalogueQuery);
    void client.prefetchQuery(grantableQuery);
    return;
  }
  switch (page) {
    case "profile":
      void client.prefetchQuery(meQuery);
      return;
    case "security":
      void client.prefetchQuery(meQuery);
      void client.prefetchQuery(ssoLinksQuery);
      return;
    case "sessions":
      void client.prefetchQuery(mySessionsQuery);
      return;
    case "keys":
      void client.prefetchQuery(myKeysQuery(false));
      return;
    case "connections":
      void client.prefetchQuery(myConnectionsQuery);
      return;
    case "notifications":
      void client.prefetchQuery(notificationsQuery);
      return;
    case "agents":
      void client.prefetchQuery(agentsQuery);
      void client.prefetchQuery(agentCatalogueQuery);
      void client.prefetchQuery(grantableQuery);
      return;
  }
}

export type InstanceSettingsData =
  | { page: "users"; search: string; state: string | undefined }
  | { page: "orgs"; search: string; sort: "created" | "deletion"; sso: boolean | undefined }
  | { page: "org"; org: string }
  | { page: "general" | "logging" | "system" | "updates" };

export function prefetchInstanceSettings(client: QueryClient, data: InstanceSettingsData): void {
  const session = client.getQueryData(sessionQuery.queryKey);
  if (!session?.can_administer_instance) return;
  switch (data.page) {
    case "users":
      void client.prefetchInfiniteQuery(instanceUsersQuery(data.search, data.state));
      return;
    case "orgs":
      void client.prefetchInfiniteQuery(instanceOrgsQuery(data.search, data.sort, data.sso));
      return;
    case "org":
      void client.prefetchQuery(instanceOrgQuery(data.org));
      void client.prefetchQuery(instanceOrgUsageQuery(data.org));
      void client.prefetchQuery(instanceOrgMembersQuery(data.org));
      void client.prefetchQuery(instanceOrgProjectsQuery(data.org));
      return;
    case "general":
      void client.prefetchQuery(instanceSettingsQuery);
      return;
    case "logging":
      if (session.request_logs !== false) void client.prefetchQuery(instanceSettingsQuery);
      return;
    case "system":
      void client.prefetchQuery(systemQuery);
      void client.prefetchQuery(instanceSettingsQuery);
      return;
    case "updates":
      void client.prefetchQuery(updatesQuery);
      void client.prefetchInfiniteQuery(updateJobsQuery);
      return;
  }
}

function prefetchRoles(client: QueryClient, org: string, rights: ReadonlySet<string>): void {
  if (!rights.has("org.view")) return;
  void client.prefetchQuery(rolesQuery(org));
  void client.prefetchQuery(rightsCatalogueQuery(org));
}
