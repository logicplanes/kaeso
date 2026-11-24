import { describe, expect, it, vi } from "vitest";
import { QueryClient } from "@tanstack/react-query";
import type { WhoAmI } from "~/api/generated/types";
import { sessionQuery } from "~/auth/session";
import {
  prefetchAccountSettings,
  prefetchInstanceSettings,
  prefetchInstanceSettingsPage,
  prefetchOrgSettings,
  prefetchAccountSettingsPage,
  prefetchOrgSettingsPage,
  prefetchSettingsLink,
} from "./settings-prefetch";

const ORG = "org_1";

function client(rights: string[], instanceAdmin = false) {
  const c = new QueryClient();
  c.setQueryData(sessionQuery.queryKey, {
    user: { is_instance_admin: instanceAdmin },
    can_administer_instance: instanceAdmin,
    memberships: [{ org_id: ORG, rights }],
  } as unknown as WhoAmI);
  const asked: unknown[] = [];
  vi.spyOn(c, "prefetchQuery").mockImplementation(async (o) => void asked.push(o.queryKey));
  vi.spyOn(c, "prefetchInfiniteQuery").mockImplementation(async (o) => void asked.push(o.queryKey));
  return { c, asked };
}

describe("settings prefetch", () => {
  it("starts a page's data with its route", () => {
    const { c, asked } = client(["org.view", "members.view"]);
    prefetchOrgSettings(c, ORG, "members");
    expect(asked).toEqual([
      ["orgs", ORG, "members"],
      ["orgs", ORG, "roles"],
      ["orgs", ORG, "rights-catalogue"],
    ]);
  });

  it("never asks for what the person's rights don't let the page ask for", () => {
    const { c, asked } = client(["org.view"]);
    for (const page of ["members", "invites", "roles", "storage", "platform-keys", "guardrails", "webhooks", "shared-links", "general"] as const)
      prefetchOrgSettings(c, ORG, page);
    expect(asked).toEqual([["orgs", ORG, "view"]]);
  });

  it("starts the tab of the members page the address opens", () => {
    const { c, asked } = client(["org.view", "members.view"]);
    prefetchOrgSettings(c, ORG, "invites");
    prefetchOrgSettings(c, ORG, "roles");
    expect(asked).toEqual([
      ["orgs", ORG, "invites", { pendingOnly: true }],
      ["orgs", ORG, "members"],
      ["orgs", ORG, "project-list"],
      ["orgs", ORG, "roles"],
      ["orgs", ORG, "rights-catalogue"],
    ]);
  });

  it("starts the guardrails, webhooks and shared links for whoever may see them", () => {
    const { c, asked } = client(["guardrails.view", "webhooks.view", "usage.share"]);
    prefetchOrgSettings(c, ORG, "guardrails");
    prefetchOrgSettings(c, ORG, "webhooks");
    prefetchOrgSettings(c, ORG, "shared-links");
    expect(asked).toEqual([
      ["orgs", ORG, "guardrails"],
      ["orgs", ORG, "webhooks"],
      ["orgs", ORG, "shared-links"],
      ["orgs", ORG, "project-list"],
    ]);
  });

  it("starts each organization settings page's data when the person may see it", () => {
    const { c, asked } = client(["org.view", "org.ownership.transfer", "members.view", "platform_keys.view", "logs.storage.view"]);
    prefetchOrgSettings(c, ORG, "general");
    prefetchOrgSettings(c, ORG, "security");
    prefetchOrgSettings(c, ORG, "platform-keys");
    prefetchOrgSettings(c, ORG, "settings");
    prefetchOrgSettings(c, ORG, "storage");
    prefetchOrgSettings(c, ORG, "deletions");
    expect(asked).toEqual([
      ["orgs", ORG, "view"],
      ["orgs", ORG, "members"],
      ["orgs", ORG, "security"],
      ["orgs", ORG, "platform-keys", { includeArchived: false }],
      ["orgs", ORG, "logs", "settings"],
      ["orgs", ORG, "response-cache"],
      ["orgs", ORG, "stored-responses"],
      ["orgs", ORG, "logs", "storage"],
      ["orgs", ORG, "project-list"],
      ["orgs", ORG, "logs", "deletions"],
      ["orgs", ORG, "project-list"],
    ]);
  });

  it("asks for nothing a page needs rights for when the person holds none", () => {
    const { c, asked } = client(["org.ownership.transfer"]);
    for (const page of ["general", "members", "security", "platform-keys", "settings", "storage", "deletions"] as const) prefetchOrgSettings(c, ORG, page);
    expect(asked).toEqual([]);
  });

  it("waits for the session before asking for anything", () => {
    const c = new QueryClient();
    const prefetch = vi.spyOn(c, "prefetchQuery");
    prefetchOrgSettings(c, ORG, "general");
    prefetchInstanceSettings(c, { page: "system" });
    expect(prefetch).not.toHaveBeenCalled();
  });

  it("asks for instance data only for instance admins", () => {
    const member = client([]);
    prefetchInstanceSettings(member.c, { page: "system" });
    expect(member.asked).toEqual([]);
    const admin = client([], true);
    prefetchInstanceSettings(admin.c, { page: "users", search: "", state: undefined });
    expect(admin.asked).toEqual([["instance", "users", { search: "", state: undefined }]]);
  });
});

describe("account and instance settings prefetch", () => {
  it("starts each account page's data", () => {
    const { c, asked } = client([]);
    for (const page of ["profile", "security", "sessions", "keys", "connections", "notifications", "agents"] as const) prefetchAccountSettings(c, page);
    prefetchAccountSettings(c, { agent: "agt_1" });
    expect(asked).toEqual([
      ["me"],
      ["me"],
      ["me", "sso-links"],
      ["me", "sessions"],
      ["me", "keys", { includeArchived: false }],
      ["me", "mcp-connections"],
      ["me", "notifications"],
      ["me", "agents"],
      ["agent-catalogue"],
      ["me", "agents", "grantable"],
      ["me", "agents", "agt_1"],
      ["agent-catalogue"],
      ["me", "agents", "grantable"],
    ]);
  });

  it("starts the instance pages with the address's search", () => {
    const { c, asked } = client([], true);
    prefetchInstanceSettings(c, { page: "orgs", search: "north", sort: "deletion", sso: true });
    prefetchInstanceSettings(c, { page: "org", org: ORG });
    prefetchInstanceSettings(c, { page: "updates" });
    expect(asked).toEqual([
      ["instance", "orgs", { search: "north", sort: "deletion", sso: true }],
      ["instance", "orgs", "one", ORG],
      ["instance", "orgs", "one", ORG, "usage"],
      ["instance", "orgs", "one", ORG, "members"],
      ["instance", "orgs", "one", ORG, "projects"],
      ["instance", "updates"],
      ["instance", "updates", "jobs"],
    ]);
  });

  it("asks for no logging data on an install without request logs", () => {
    const { c, asked } = client(["org.view", "logs.storage.view"], true);
    c.setQueryData(sessionQuery.queryKey, (session) => ({ ...(session as WhoAmI), request_logs: false }));
    prefetchOrgSettings(c, ORG, "storage");
    prefetchOrgSettings(c, ORG, "deletions");
    prefetchInstanceSettings(c, { page: "logging" });
    prefetchOrgSettings(c, ORG, "settings");
    expect(asked).toEqual([
      ["orgs", ORG, "response-cache"],
      ["orgs", ORG, "stored-responses"],
    ]);
  });
});

describe("settings prefetch by address", () => {
  it("reads the organization settings page and its tab from the address", () => {
    const { c, asked } = client(["org.view", "members.view", "logs.storage.view", "audit_log.view"]);
    prefetchOrgSettingsPage(c, ORG, `/settings/orgs/${ORG}/members`, { tab: "roles" });
    prefetchOrgSettingsPage(c, ORG, `/settings/orgs/${ORG}/logging`, { tab: "storage" });
    prefetchOrgSettingsPage(c, ORG, `/settings/orgs/${ORG}/logging`, { tab: "other" });
    prefetchOrgSettingsPage(c, ORG, `/settings/orgs/${ORG}/audit-log`, { actor: " usr_ada ", event: "evt_1" });
    prefetchOrgSettingsPage(c, ORG, `/settings/orgs/${ORG}/elsewhere`, {});
    expect(asked).toEqual([
      ["orgs", ORG, "roles"],
      ["orgs", ORG, "rights-catalogue"],
      ["orgs", ORG, "logs", "storage"],
      ["orgs", ORG, "project-list"],
      ["orgs", ORG, "logs", "settings"],
      ["orgs", ORG, "response-cache"],
      ["orgs", ORG, "stored-responses"],
      ["audit-log", "org", ORG, { actor: "usr_ada" }],
    ]);
  });

  it("starts the instance lists only as the sidebar links to them, unfiltered", () => {
    const { c, asked } = client([], true);
    prefetchInstanceSettingsPage(c, "/settings/instance/users", {});
    prefetchInstanceSettingsPage(c, "/settings/instance/users", { q: "ada" });
    prefetchInstanceSettingsPage(c, "/settings/instance/orgs", {});
    prefetchInstanceSettingsPage(c, "/settings/instance/orgs", { sort: "deletion" });
    prefetchInstanceSettingsPage(c, `/settings/instance/orgs/${ORG}`, {});
    prefetchInstanceSettingsPage(c, "/settings/instance/system", {});
    prefetchInstanceSettingsPage(c, "/settings/instance/audit-log", {});
    expect(asked).toEqual([
      ["instance", "users", { search: "", state: undefined }],
      ["instance", "orgs", { search: "", sort: "created", sso: undefined }],
      ["instance", "orgs", "one", ORG],
      ["instance", "orgs", "one", ORG, "usage"],
      ["instance", "orgs", "one", ORG, "members"],
      ["instance", "orgs", "one", ORG, "projects"],
      ["instance", "system"],
      ["instance", "settings"],
      ["instance", "users", { search: "", state: undefined }],
      ["audit-log", "instance", {}],
    ]);
  });

  it("asks for no audit log without the right to see it", () => {
    const { c, asked } = client(["org.view"]);
    prefetchOrgSettingsPage(c, ORG, `/settings/orgs/${ORG}/audit-log`, {});
    prefetchInstanceSettingsPage(c, "/settings/instance/audit-log", {});
    expect(asked).toEqual([]);
  });
});

describe("settings prefetch by sidebar link", () => {
  it("reads the page from the link", () => {
    const { c, asked } = client(["org.view", "members.view"], true);
    prefetchSettingsLink(c, `/settings/orgs/${ORG}/members`);
    prefetchSettingsLink(c, "/settings/instance/system");
    prefetchSettingsLink(c, "/settings/account/sessions#top");
    prefetchSettingsLink(c, "/settings/account/appearance");
    prefetchSettingsLink(c, "/settings/elsewhere");
    prefetchAccountSettingsPage(c, "/settings/account/agents/agt_1");
    expect(asked).toEqual([
      ["orgs", ORG, "members"],
      ["orgs", ORG, "roles"],
      ["orgs", ORG, "rights-catalogue"],
      ["instance", "system"],
      ["instance", "settings"],
      ["me", "sessions"],
      ["me", "agents", "agt_1"],
      ["agent-catalogue"],
      ["me", "agents", "grantable"],
    ]);
  });
});
