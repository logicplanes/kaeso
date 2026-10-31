import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ALL_PROJECT_RIGHTS, ORG, PROJECT, membership, signedIn, whoAmI } from "~/test/fixtures";
import { me } from "~/test/fixtures/instance-account";
import { listed, projectData } from "~/test/fixtures/project";
import { on, page } from "~/test/network";
import { renderApp, type RenderedApp } from "~/test/render";

const PROJECT_BASE = `/orgs/${ORG}/projects/${PROJECT}`;

function visits(router: RenderedApp["router"]): string[] {
  const seen: string[] = [];
  router.history.subscribe(({ location }) => seen.push(location.pathname));
  return seen;
}

async function ready() {
  await screen.findByRole("navigation", { name: "Console" });
}

describe("keyboard shortcuts", () => {
  it("goes to project pages with g and a letter", async () => {
    signedIn();
    projectData();
    const { user, router } = await renderApp(`${PROJECT_BASE}/limits`);
    await ready();
    await user.keyboard("gk");
    await waitFor(() => expect(router.state.location.pathname).toBe(`${PROJECT_BASE}/keys`));
    await user.keyboard("gp");
    await waitFor(() => expect(router.state.location.pathname).toBe(`${PROJECT_BASE}/settings`));
    await user.keyboard("gm");
    await waitFor(() => expect(router.state.location.pathname).toBe(`${PROJECT_BASE}/models`));
  });

  it("goes to the project's overview with g and o", async () => {
    signedIn();
    projectData();
    on("GET", "orgs/:org/projects/:project/usage/span", { first_day: null, last_day: null });
    on("GET", "orgs/:org/projects/:project/usage", { data: [] });
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    const { user, router } = await renderApp(`${PROJECT_BASE}/limits`);
    await ready();
    await user.keyboard("go");
    await waitFor(() => expect(router.state.location.pathname).toBe(PROJECT_BASE));
    expect(await screen.findByRole("heading", { name: "Overview", level: 1 })).toBeInTheDocument();
  });

  it("skips pages the person can't open", async () => {
    signedIn({ projectRights: ALL_PROJECT_RIGHTS.filter((r) => r !== "keys.all.view") });
    projectData();
    const { user, router } = await renderApp(`${PROJECT_BASE}/limits`);
    await ready();
    const seen = visits(router);
    await user.keyboard("gk");
    await user.keyboard("gl");
    await waitFor(() => expect(router.state.location.pathname).toBe(`${PROJECT_BASE}/logs`));
    expect(seen).toEqual([`${PROJECT_BASE}/logs`]);
  });

  it("has no shortcut to request logs on an install without them", async () => {
    signedIn({ session: whoAmI({ request_logs: false }) });
    projectData();
    const { user, router } = await renderApp(`${PROJECT_BASE}/limits`);
    await ready();
    const seen = visits(router);
    await user.keyboard("gl");
    await user.keyboard("gk");
    await waitFor(() => expect(router.state.location.pathname).toBe(`${PROJECT_BASE}/keys`));
    expect(seen).toEqual([`${PROJECT_BASE}/keys`]);
  });

  it("goes to the organization overview and its settings", async () => {
    signedIn();
    const { user, router } = await renderApp(`/orgs/${ORG}/projects`);
    await ready();
    await user.keyboard("go");
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}`));
    await user.keyboard("gs");
    await waitFor(() => expect(router.state.location.pathname).toBe(`/settings/orgs/${ORG}/general`));
  });

  it("goes to the organization's audit log", async () => {
    signedIn();
    const { user, router } = await renderApp(`/orgs/${ORG}/projects`);
    await ready();
    await user.keyboard("ga");
    await waitFor(() => expect(router.state.location.pathname).toBe(`/settings/orgs/${ORG}/audit-log`));
  });

  it("leaves out the organization's pages for people who can't view them", async () => {
    signedIn({ session: whoAmI({ memberships: [membership({ rights: [] })] }) });
    on("GET", "me", me());
    const { user, router } = await renderApp(`/orgs/${ORG}/projects`);
    await ready();
    const seen = visits(router);
    await user.keyboard("go");
    await user.keyboard("ga");
    await user.keyboard("gs");
    await waitFor(() => expect(router.state.location.pathname).toBe("/settings/account/profile"));
    expect(seen).toEqual(["/settings/account/profile"]);
  });

  it("goes to the first organization settings page the person may see", async () => {
    signedIn({ session: whoAmI({ memberships: [membership({ rights: ["members.view"] })] }) });
    on("GET", "orgs/:org/members", page([]));
    const { user, router } = await renderApp(`/orgs/${ORG}/projects`);
    await ready();
    await user.keyboard("gs");
    await waitFor(() => expect(router.state.location.pathname).toBe(`/settings/orgs/${ORG}/members`));
  });

  it("goes to the account's settings away from any organization", async () => {
    signedIn();
    on("GET", "me", me());
    const { user, router } = await renderApp("/settings/account/appearance");
    await screen.findByRole("heading", { name: "Appearance" });
    await user.keyboard("gs");
    await waitFor(() => expect(router.state.location.pathname).toBe("/settings/account/profile"));
  });

  it("ignores keys with modifiers, while typing and while a dialog is open", async () => {
    signedIn();
    const { user, router } = await renderApp(`/orgs/${ORG}/projects`);
    await ready();
    await user.keyboard("{Control>}g{/Control}s");
    expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects`);
    await user.keyboard("c");
    const dialog = await screen.findByRole("dialog", { name: "Create project" });
    await user.keyboard("?");
    expect(within(dialog).getByRole("textbox", { name: "Name" })).toHaveValue("?");
    expect(screen.queryByRole("dialog", { name: "Keyboard shortcuts" })).not.toBeInTheDocument();
  });

  it("does nothing on c when the page's create action is blocked", async () => {
    signedIn({ projectRights: ALL_PROJECT_RIGHTS.filter((r) => r !== "limits.manage") });
    projectData();
    const { user } = await renderApp(`${PROJECT_BASE}/limits`);
    await ready();
    await screen.findByRole("heading", { name: "Limits", level: 1 });
    await user.keyboard("c");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("focuses the page's search with a slash", async () => {
    signedIn();
    on("GET", "instance/users", page([]));
    const { user } = await renderApp("/settings/instance/users");
    const search = await screen.findByRole("searchbox", { name: "Search users" });
    await user.click(screen.getByRole("heading", { name: "Users", level: 1 }));
    await user.keyboard("/");
    expect(search).toHaveFocus();
    expect(search).toHaveValue("");
  });

  it("leaves the focus alone on a slash when the page has no search", async () => {
    signedIn();
    const { user } = await renderApp("/settings/account/appearance");
    await screen.findByRole("heading", { name: "Appearance", level: 1 });
    await user.keyboard("/");
    expect(document.body).toHaveFocus();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("shows every shortcut with a question mark", async () => {
    signedIn();
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    await ready();
    await user.keyboard("?");
    const sheet = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(within(sheet).getByText("Search and commands")).toBeInTheDocument();
    expect(within(sheet).getByText("Go to the audit log")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Keyboard shortcuts" })).not.toBeInTheDocument());
    await user.keyboard("?");
    expect(await screen.findByRole("dialog", { name: "Keyboard shortcuts" })).toBeInTheDocument();
  });
});
