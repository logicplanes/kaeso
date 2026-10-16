import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ORG, project, signedIn, user, whoAmI } from "~/test/fixtures";
import { instanceOrg, instanceSettings, instanceUser, me, member, modelsDev, orgUsage, system } from "~/test/fixtures/instance-account";
import { logJob, logSettings, paged, platformKey, security, storage } from "~/test/fixtures/org";
import { auditEvent } from "~/test/fixtures/shell";
import { on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const sent = (path: string) => requests("GET", path).map((r) => Object.fromEntries(r.query));

describe("settings addresses that moved", () => {
  it.each([
    ["/settings", "/settings/account/profile", {}],
    ["/settings/instance", "/settings/instance/users", {}],
    ["/settings/instance/models", "/settings/instance/model-sources", {}],
    [`/settings/orgs/${ORG}`, `/settings/orgs/${ORG}/general`, {}],
    [`/settings/orgs/${ORG}/invites?invite=true`, `/settings/orgs/${ORG}/members`, { tab: "invites", invite: true }],
    [`/settings/orgs/${ORG}/logs?tab=storage`, `/settings/orgs/${ORG}/logging`, { tab: "storage" }],
    [`/settings/orgs/${ORG}/projects?create=true`, `/orgs/${ORG}/projects`, { create: true }],
    [`/settings/orgs/${ORG}/provider-keys`, `/orgs/${ORG}/providers`, {}],
    [`/settings/orgs/${ORG}/usage?from=2026-10-01&to=2026-10-07`, `/orgs/${ORG}`, { from: "2026-10-01", to: "2026-10-07" }],
  ])("%s opens %s", async (from, to, search) => {
    signedIn();
    on("GET", "me", me());
    on("GET", "instance/users", paged([]));
    on("GET", "instance/models-dev", modelsDev());
    on("GET", "orgs/:org/members", paged([]));
    on("GET", "orgs/:org/invites", paged([]));
    on("GET", "orgs/:org/logs/storage", storage());
    on("GET", "orgs/:org/credentials", { data: [] });
    on("GET", "orgs/:org/platform-keys", paged([]));
    on("GET", "orgs/:org/usage/span", { first_day: null, last_day: null });
    on("GET", "orgs/:org/usage", { data: [] });
    const { router } = await renderApp(from);
    await waitFor(() => expect(router.state.location.pathname).toBe(to));
    expect(router.state.location.search).toEqual(search);
  });

  it("keeps only the logging tabs it knows when moving the old logs address", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/settings", logSettings());
    const { router } = await renderApp(`/settings/orgs/${ORG}/logs?tab=everything`);
    await waitFor(() => expect(router.state.location.pathname).toBe(`/settings/orgs/${ORG}/logging`));
    expect(await screen.findByRole("heading", { name: "Logging", level: 1 })).toBeInTheDocument();
    expect(requests("GET", "orgs/:org/logs/settings")).toHaveLength(1);
  });
});

describe("account settings", () => {
  it.each([
    ["appearance", "Appearance"],
    ["keys", "API keys"],
    ["memberships", "Memberships"],
    ["notifications", "Notifications"],
    ["security", "Security"],
    ["sessions", "Sessions"],
  ])("opens %s", async (path, title) => {
    signedIn();
    on("GET", "me", me());
    on("GET", "me/keys", paged([]));
    on("GET", "me/notifications", { data: [] });
    on("GET", "me/sessions", { data: [] });
    await renderApp(`/settings/account/${path}`);
    expect(await screen.findByRole("heading", { name: title, level: 1 })).toBeInTheDocument();
  });
});

describe("instance settings", () => {
  it("keeps instance pages for instance admins", async () => {
    signedIn({ session: whoAmI({ user: user({ is_instance_admin: false }) }) });
    await renderApp("/settings/instance/system");
    expect(await screen.findByText("For instance admins")).toBeInTheDocument();
    expect(requests("GET", "instance/system")).toHaveLength(0);
  });

  it("searches users and opens one from the address", async () => {
    signedIn();
    on("GET", "instance/users", paged([instanceUser()]));
    on("GET", "instance/users/:user", instanceUser());
    await renderApp("/settings/instance/users?q=%20grace%20&state=deactivated&user=usr_grace");
    await waitFor(() => expect(sent("instance/users")).toContainEqual({ limit: "100", search: "grace", state: "deactivated" }));
    await waitFor(() => expect(requests("GET", "instance/users/:user").map((r) => r.params.user)).toContain("usr_grace"));
    expect(await screen.findByRole("searchbox", { name: "Search users" })).toHaveValue("grace");
  });

  it("leaves out a state it doesn't know and opens the invite dialog", async () => {
    signedIn();
    on("GET", "instance/users", paged([instanceUser()], "cur_2"));
    await renderApp("/settings/instance/users?state=banned&q=%20%20&invite=true");
    expect(await screen.findByRole("dialog", { name: "Invite user" })).toBeInTheDocument();
    expect(sent("instance/users")[0]).toEqual({ limit: "100" });
  });

  it("searches organizations by deletion date and opens the create dialog", async () => {
    signedIn();
    on("GET", "instance/orgs", paged([instanceOrg()]));
    await renderApp("/settings/instance/orgs?q=north&sort=deletion&create=true");
    expect(await screen.findByRole("dialog", { name: "Create organization" })).toBeInTheDocument();
    await waitFor(() => expect(sent("instance/orgs")).toContainEqual({ limit: "100", search: "north", sort: "deletion" }));
  });

  it("leaves out a sort it doesn't know", async () => {
    signedIn();
    on("GET", "instance/orgs", paged([instanceOrg()]));
    await renderApp("/settings/instance/orgs?sort=size&create=no");
    expect(await screen.findByRole("heading", { name: "Organizations", level: 1 })).toBeInTheDocument();
    await waitFor(() => expect(sent("instance/orgs")).toContainEqual({ limit: "100" }));
    expect(screen.queryByRole("dialog", { name: "Create organization" })).not.toBeInTheDocument();
  });

  it("shows one organization with its usage, members and projects", async () => {
    signedIn();
    on("GET", "instance/orgs/:org", instanceOrg());
    on("GET", "instance/orgs/:org/usage", orgUsage());
    on("GET", "instance/orgs/:org/members", paged([member({ role: "owner" })]));
    on("GET", "instance/orgs/:org/projects", page([project()]));
    await renderApp(`/settings/instance/orgs/${ORG}`);
    expect(await screen.findByRole("heading", { name: "Northwind Labs", level: 1 })).toBeInTheDocument();
    await waitFor(() => expect(requests("GET", "instance/orgs/:org/members")).toHaveLength(1));
    expect(sent("instance/orgs/:org/projects")).toEqual([{ limit: "100" }]);
  });

  it.each([
    ["settings", "Settings", "instance/settings", instanceSettings()],
    ["model-sources", "Model sources", "instance/models-dev", modelsDev()],
    ["system", "System", "instance/system", system()],
  ])("opens %s with its data", async (path, title, api, reply) => {
    signedIn();
    on("GET", api, reply);
    await renderApp(`/settings/instance/${path}`);
    expect(await screen.findByRole("heading", { name: title, level: 1 })).toBeInTheDocument();
    expect(requests("GET", api).length).toBeGreaterThan(0);
  });

  it("filters the instance audit log and opens an event", async () => {
    signedIn();
    on("GET", "instance/audit-log", paged([auditEvent()]));
    on("GET", "instance/audit-log/:event", auditEvent());
    on("GET", "instance/users", paged([]));
    on("GET", "instance/orgs", paged([]));
    await renderApp("/settings/instance/audit-log?action=key.created&event=evt_1");
    await waitFor(() => expect(sent("instance/audit-log")).toContainEqual({ action: "key.created", limit: "100" }));
    await waitFor(() => expect(requests("GET", "instance/audit-log/:event").map((r) => r.params.event)).toEqual(["evt_1"]));
  });
});

describe("organization settings", () => {
  it.each([
    ["general", "General", "orgs/:org", undefined],
    ["security", "Security", "orgs/:org/security", security()],
    ["platform-keys", "Platform keys", "orgs/:org/platform-keys", paged([platformKey()])],
    ["logging", "Logging", "orgs/:org/logs/settings", logSettings()],
  ])("opens %s with its data", async (path, title, api, reply) => {
    signedIn();
    on("GET", "orgs/:org/members", paged([]));
    if (reply) on("GET", api, reply);
    await renderApp(`/settings/orgs/${ORG}/${path}`);
    expect(await screen.findByRole("heading", { name: title, level: 1 })).toBeInTheDocument();
    expect(requests("GET", api).length).toBeGreaterThan(0);
  });

  it("lists every page of platform keys", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", ({ query }: { query: URLSearchParams }) =>
      query.get("after") ? paged([platformKey({ id: "opk_ci", name: "CI pipeline" })]) : paged([platformKey()], "cur_2"),
    );
    await renderApp(`/settings/orgs/${ORG}/platform-keys`);
    expect(await screen.findByText("CI pipeline")).toBeInTheDocument();
    expect(screen.getByText("Terraform")).toBeInTheDocument();
    expect(sent("orgs/:org/platform-keys")).toContainEqual({ limit: "100", include_archived: "false", after: "cur_2" });
  });

  it("opens the deletions tab of logging", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/deletions", { data: [logJob()] });
    await renderApp(`/settings/orgs/${ORG}/logging?tab=deletions`);
    await waitFor(() => expect(requests("GET", "orgs/:org/logs/deletions")).toHaveLength(1));
    expect(sent("orgs/:org/projects")).toContainEqual({ limit: "100", include_archived: "true" });
  });

  it("opens the roles tab of members", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([]));
    await renderApp(`/settings/orgs/${ORG}/members?tab=roles`);
    expect(await screen.findByRole("tab", { name: /Roles/, selected: true })).toBeInTheDocument();
  });

  it("reads an old preset as the role to open", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([]));
    await renderApp(`/settings/orgs/${ORG}/members?tab=roles&preset=org:owner`);
    expect(await screen.findByRole("dialog", { name: /Owner/ })).toBeInTheDocument();
  });

  it("opens the members tab when the address names an unknown one", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([]));
    await renderApp(`/settings/orgs/${ORG}/members?tab=secrets&create=true`);
    expect(await screen.findByRole("tab", { name: /Members/, selected: true })).toBeInTheDocument();
  });

  it("filters the organization audit log and opens the event it names", async () => {
    signedIn();
    on("GET", "orgs/:org/audit-log", paged([auditEvent()]));
    on("GET", "orgs/:org/audit-log/:event", auditEvent());
    on("GET", "orgs/:org/members", paged([]));
    on("GET", "orgs/:org/credentials", { data: [] });
    on("GET", "orgs/:org/platform-keys", paged([]));
    await renderApp(`/settings/orgs/${ORG}/audit-log?target=key_web&event=evt_1`);
    await waitFor(() => expect(sent("orgs/:org/audit-log")).toContainEqual({ target: "key_web", limit: "100" }));
    await waitFor(() => expect(requests("GET", "orgs/:org/audit-log/:event").map((r) => r.params.event)).toEqual(["evt_1"]));
  });
});
