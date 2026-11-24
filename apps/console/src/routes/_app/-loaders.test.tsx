import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ALL_ORG_RIGHTS, ALL_PROJECT_RIGHTS, ORG, PROJECT, membership, signedIn, whoAmI } from "~/test/fixtures";
import { on, page, requests } from "~/test/network";
import { renderApp, type RenderedApp } from "~/test/render";

// Pointing at a link, focusing it or touching it preloads its route, which runs the route's loader. These tests do what
// the router does then and check that the page's data is already on its way before the page itself mounts.

const P = "orgs/:org/projects/:project";
const START = "/settings/account/appearance";
const asked = (path: string) => requests("GET", path).length;
const groupedBy = (path: string) => requests("GET", path).map((r) => r.query.get("group_by"));

function stubPages() {
  for (const path of [
    `${P}/tools`,
    `${P}/mcp-servers`,
    `${P}/portal-grants`,
    `${P}/guardrails`,
    `${P}/sources`,
    "orgs/:org/credentials",
    "orgs/:org/mcp-servers",
    "orgs/:org/custom-providers",
    "orgs/:org/members",
    "orgs/:org/platform-keys",
    "orgs/:org/invites",
    "orgs/:org/usage",
    `${P}/usage`,
    "me/sessions",
    "me/keys",
    "me/mcp-connections",
    "me/agents",
    "me/agents/grantable",
    "instance/updates/jobs",
  ])
    on("GET", path, page([]));
  on("GET", `${P}/logs/tool-calls`, page([]));
  on("GET", `${P}/logs/settings`, { content_logging: true, keep_media: false });
  for (const path of [`${P}/response-cache`, `${P}/stored-responses`, `${P}/model-lifecycle`, "me/notifications", "me/agents/catalogue", "instance/updates"])
    on("GET", path, {});
  on("GET", "orgs/:org/audit-log", page([]));
}

// A settings layout's loader doesn't run again for a link inside the layout the person is already in (the settings
// sidebar starts those pages itself), so the account pages are preloaded from an organization's page.
async function start(path = START, heading = "Appearance"): Promise<RenderedApp> {
  stubPages();
  const app = await renderApp(path);
  await screen.findByRole("heading", { name: heading, level: 1 });
  return app;
}

const project = { org: ORG, project: PROJECT };

describe("the account settings address", () => {
  it("opens the profile", async () => {
    signedIn();
    stubPages();
    const { router } = await renderApp("/settings/account");
    await screen.findByRole("heading", { name: "Profile", level: 1 });
    expect(router.state.location.pathname).toBe("/settings/account/profile");
  });
});

describe("a route's loader on hover, focus or touch", () => {
  it("starts the tools page's lists", async () => {
    signedIn();
    const { router } = await start();
    await router.preloadRoute({ to: "/orgs/$org/projects/$project/tools", params: project });
    await waitFor(() => expect([asked(`${P}/tools`), asked(`${P}/mcp-servers`), asked(`${P}/portal-grants`)]).toEqual([1, 1, 1]));
    expect(router.state.location.pathname).toBe(START);
  });

  it("asks once when the person then opens the page", async () => {
    signedIn();
    const { router } = await start();
    await router.preloadRoute({ to: "/orgs/$org/projects/$project/tools", params: project });
    await waitFor(() => expect(asked(`${P}/tools`)).toBe(1));
    await router.navigate({ to: "/orgs/$org/projects/$project/tools", params: project });
    expect(await screen.findByRole("heading", { name: "Tools", level: 1 })).toBeInTheDocument();
    expect([asked(`${P}/tools`), asked(`${P}/mcp-servers`), asked(`${P}/portal-grants`)]).toEqual([1, 1, 1]);
  });

  it("starts the tool calls, which the request log's own loader used to leave to the page", async () => {
    signedIn();
    const { router } = await start();
    await router.preloadRoute({ to: "/orgs/$org/projects/$project/logs", params: project, search: { kind: "tools", call_status: "error" } });
    await waitFor(() => expect(requests("GET", `${P}/logs/tool-calls`).map((r) => r.query.get("status"))).toEqual(["error"]));
    await waitFor(() => expect([asked(`${P}/tools`), asked(`${P}/mcp-servers`), asked(`${P}/sources`)]).toEqual([1, 1, 1]));
  });

  it("starts each section of the project's settings", async () => {
    signedIn();
    const { router } = await start();
    await router.preloadRoute({ to: "/orgs/$org/projects/$project/settings", params: project });
    const sections = [`${P}/logs/settings`, `${P}/response-cache`, `${P}/guardrails`, `${P}/stored-responses`, `${P}/model-lifecycle`];
    await waitFor(() => expect(sections.map(asked)).toEqual([1, 1, 1, 1, 1]));
  });

  it("starts the organization's usage by day for its analytics", async () => {
    signedIn();
    const { router } = await start();
    await router.preloadRoute({ to: "/orgs/$org/analytics", params: { org: ORG }, search: { period: "today" } });
    await waitFor(() => expect(groupedBy("orgs/:org/usage")).toEqual(["day"]));
  });

  it("starts the organization's MCP servers, provider keys and projects' usage", async () => {
    signedIn();
    const { router } = await start();
    await router.preloadRoute({ to: "/orgs/$org/mcp-servers", params: { org: ORG } });
    await router.preloadRoute({ to: "/orgs/$org/providers", params: { org: ORG } });
    await router.preloadRoute({ to: "/orgs/$org/projects", params: { org: ORG } });
    await waitFor(() =>
      expect([asked("orgs/:org/mcp-servers"), asked("orgs/:org/credentials"), asked("orgs/:org/custom-providers"), asked("orgs/:org/platform-keys")]).toEqual([
        1, 1, 1, 1,
      ]),
    );
    await waitFor(() => expect(groupedBy("orgs/:org/usage")).toEqual(["project"]));
  });

  it("starts the settings pages' data", async () => {
    signedIn();
    const { router } = await start(`/orgs/${ORG}/mcp-servers`, "MCP servers");
    await router.preloadRoute({ to: "/settings/orgs/$org/members", params: { org: ORG }, search: { tab: "invites" } });
    await router.preloadRoute({ to: "/settings/orgs/$org/audit-log", params: { org: ORG }, search: {} });
    await router.preloadRoute({ to: "/settings/account/sessions" });
    await router.preloadRoute({ to: "/settings/account/keys", search: {} });
    await router.preloadRoute({ to: "/settings/account/notifications" });
    await router.preloadRoute({ to: "/settings/account/connections", search: {} });
    await router.preloadRoute({ to: "/settings/account/agents" });
    await router.preloadRoute({ to: "/settings/instance/updates" });
    const pages = [
      "orgs/:org/invites",
      "orgs/:org/audit-log",
      "me/sessions",
      "me/keys",
      "me/notifications",
      "me/mcp-connections",
      "me/agents",
      "instance/updates/jobs",
    ];
    await waitFor(() => expect(pages.map(asked)).toEqual(pages.map(() => 1)));
  });

  it("leaves out the request log settings on an install without request logs", async () => {
    signedIn({ session: whoAmI({ request_logs: false }), projectRights: ALL_PROJECT_RIGHTS.filter((r) => r !== "security.key_policy.project.manage") });
    const { router } = await start();
    await router.preloadRoute({ to: "/orgs/$org/projects/$project/settings", params: project });
    await waitFor(() => expect(asked(`${P}/stored-responses`)).toBe(1));
    expect(asked(`${P}/logs/settings`)).toBe(0);
  });

  it("asks for nothing the person's rights keep the page from asking for", async () => {
    const orgRights = ALL_ORG_RIGHTS.filter((r) => !["mcp_servers.view", "provider_credentials.view", "usage.org.view", "audit_log.view"].includes(r));
    signedIn({
      session: whoAmI({ memberships: [membership({ rights: orgRights })] }),
      projectRights: ALL_PROJECT_RIGHTS.filter((r) => r !== "models.view" && r !== "sources.view"),
    });
    const { router } = await start();
    await router.preloadRoute({ to: "/orgs/$org/mcp-servers", params: { org: ORG } });
    await router.preloadRoute({ to: "/orgs/$org/providers", params: { org: ORG } });
    await router.preloadRoute({ to: "/orgs/$org/analytics", params: { org: ORG }, search: {} });
    await router.preloadRoute({ to: "/settings/orgs/$org/audit-log", params: { org: ORG }, search: {} });
    await router.preloadRoute({ to: "/orgs/$org/projects/$project/logs", params: project, search: { kind: "tools" } });
    await router.preloadRoute({ to: "/orgs/$org/projects/$project/settings", params: project });
    await waitFor(() => expect([asked(`${P}/logs/tool-calls`), asked(`${P}/stored-responses`), asked("orgs/:org/custom-providers")]).toEqual([1, 1, 1]));
    expect(asked("orgs/:org/mcp-servers")).toBe(0);
    expect(asked("orgs/:org/credentials")).toBe(0);
    expect(asked("orgs/:org/usage")).toBe(0);
    expect(asked("orgs/:org/audit-log")).toBe(0);
    expect(asked(`${P}/sources`)).toBe(0);
    expect(asked(`${P}/model-lifecycle`)).toBe(0);
  });
});
