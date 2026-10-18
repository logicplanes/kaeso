import { describe, expect, it, vi } from "vitest";
import { QueryClient } from "@tanstack/react-query";
import type { WhoAmI } from "~/api/generated/types";
import { sessionQuery } from "~/auth/session";
import { prefetchInstanceSettings, prefetchOrgSettings, prefetchSettingsPath } from "./settings-prefetch";

const ORG = "org_1";

function client(rights: string[], instanceAdmin = false) {
  const c = new QueryClient();
  c.setQueryData(sessionQuery.queryKey, { user: { is_instance_admin: instanceAdmin }, memberships: [{ org_id: ORG, rights }] } as unknown as WhoAmI);
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
    prefetchOrgSettings(c, ORG, "members");
    prefetchOrgSettings(c, ORG, "storage");
    prefetchOrgSettings(c, ORG, "platform-keys");
    prefetchOrgSettings(c, ORG, "general");
    expect(asked).toEqual([
      ["orgs", ORG, "roles"],
      ["orgs", ORG, "rights-catalogue"],
      ["orgs", ORG],
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
      ["orgs", ORG],
      ["orgs", ORG, "members"],
      ["orgs", ORG, "security"],
      ["orgs", ORG, "platform-keys", { includeArchived: false }],
      ["orgs", ORG, "logs", "settings"],
      ["orgs", ORG, "logs", "storage"],
      ["orgs", ORG, "project-list", { includeArchived: true }],
      ["orgs", ORG, "logs", "deletions"],
      ["orgs", ORG, "project-list", { includeArchived: true }],
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

describe("settings prefetch by link", () => {
  it("reads the page from a settings sidebar link", () => {
    const { c, asked } = client(["org.view", "members.view"], true);
    prefetchSettingsPath(c, `/settings/orgs/${ORG}/members`);
    prefetchSettingsPath(c, `/settings/orgs/${ORG}/logging`);
    prefetchSettingsPath(c, "/settings/instance/system");
    prefetchSettingsPath(c, "/settings/account/appearance");
    prefetchSettingsPath(c, `/settings/orgs/${ORG}/audit-log`);
    expect(asked).toEqual([
      ["orgs", ORG, "members"],
      ["orgs", ORG, "roles"],
      ["orgs", ORG, "rights-catalogue"],
      ["orgs", ORG, "logs", "settings"],
      ["instance", "system"],
      ["instance", "settings"],
    ]);
  });

  it("reads the logging tab a link opens", () => {
    const { c, asked } = client(["org.view", "logs.storage.view"]);
    prefetchSettingsPath(c, `/settings/orgs/${ORG}/logging?tab=storage`);
    prefetchSettingsPath(c, `/settings/orgs/${ORG}/logging?tab=deletions#top`);
    prefetchSettingsPath(c, `/settings/orgs/${ORG}/logging?tab=other`);
    expect(asked).toEqual([
      ["orgs", ORG, "logs", "storage"],
      ["orgs", ORG, "project-list", { includeArchived: true }],
      ["orgs", ORG, "logs", "deletions"],
      ["orgs", ORG, "project-list", { includeArchived: true }],
      ["orgs", ORG, "logs", "settings"],
    ]);
  });

  it("reads account and instance pages from their links", () => {
    const { c, asked } = client([], true);
    prefetchSettingsPath(c, "/settings/account/profile");
    prefetchSettingsPath(c, "/settings/account/security");
    prefetchSettingsPath(c, "/settings/instance/users?state=active");
    prefetchSettingsPath(c, "/settings/instance/orgs");
    prefetchSettingsPath(c, "/settings/instance/general");
    prefetchSettingsPath(c, "/settings/instance/model-sources");
    prefetchSettingsPath(c, "/settings/instance/updates");
    prefetchSettingsPath(c, "/settings/instance/logging");
    prefetchSettingsPath(c, "/settings/instance/catalogue");
    prefetchSettingsPath(c, "/settings/instance");
    prefetchSettingsPath(c, "/settings/orgs");
    expect(asked).toEqual([
      ["me"],
      ["me"],
      ["instance", "users", { search: "", state: undefined }],
      ["instance", "orgs", { search: "", sort: "created" }],
      ["instance", "settings"],
      ["instance", "models-dev"],
      ["instance", "updates"],
      ["instance", "settings"],
    ]);
  });
});
