import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ALL_PROJECT_RIGHTS, ORG, PROJECT, membership, signedIn, whoAmI } from "~/test/fixtures";
import { me } from "~/test/fixtures/instance-account";
import { listed, projectData } from "~/test/fixtures/project";
import { on, page } from "~/test/network";
import { renderApp, type RenderedApp } from "~/test/render";
import { PROJECT_PAGES } from "./pages";

const PROJECT_BASE = `/orgs/${ORG}/projects/${PROJECT}`;

function visits(router: RenderedApp["router"]): string[] {
  const seen: string[] = [];
  router.history.subscribe(({ location }) => seen.push(location.pathname));
  return seen;
}

function rows(sheet: HTMLElement, group: string): string[] {
  const region = within(sheet).getByRole("region", { name: group });
  return Array.from(region.querySelectorAll("dt"), (dt) => dt.textContent ?? "");
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

  it("shows every shortcut with a question mark, with only the go-to keys that work outside a project", async () => {
    signedIn();
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    await ready();
    await user.keyboard("?");
    const sheet = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(rows(sheet, "Anywhere")).toEqual([
      "Search and commands",
      "Create on this page",
      "Search this page",
      "Collapse or expand the sidebar",
      "Close a dialog or panel",
      "This list",
    ]);
    for (const keys of within(sheet).getAllByRole("definition")) expect(keys).toHaveClass("justify-end");
    expect(within(sheet).queryByRole("region", { name: "Go to a project page" })).not.toBeInTheDocument();
    expect(rows(sheet, "Go to")).toEqual(["Organization overview", "Organization audit log", "Organization settings"]);
    expect(rows(sheet, "On some pages")).toEqual([
      "Next entry in request logs and tool calls",
      "Previous entry in request logs and tool calls",
      "Save a route",
      "Undo in the route editor",
      "Redo in the route editor",
    ]);
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Keyboard shortcuts" })).not.toBeInTheDocument());
    await user.keyboard("?");
    expect(await screen.findByRole("dialog", { name: "Keyboard shortcuts" })).toBeInTheDocument();
  });

  it("lists a project's go-to keys from its pages, leaving out the ones the person can't open", async () => {
    signedIn({ projectRights: ALL_PROJECT_RIGHTS.filter((r) => r !== "keys.all.view") });
    projectData();
    const { user } = await renderApp(`${PROJECT_BASE}/limits`);
    await ready();
    await user.keyboard("?");
    const sheet = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    const project = within(sheet).getByRole("region", { name: "Go to a project page" });
    expect(rows(sheet, "Go to a project page")).toEqual(PROJECT_PAGES.filter((p) => p.id !== "keys").map((p) => p.label()));
    expect(within(project).getByText("Playground").closest("div")).toHaveTextContent(/G\s*then\s*Y/);
    expect(rows(sheet, "Go to")).toEqual(["Organization settings"]);
  });

  it("goes to the remaining project pages with g and a letter", async () => {
    signedIn();
    projectData();
    on("GET", "orgs/:org/projects/:project/members", page([]));
    on("GET", "orgs/:org/projects/:project/service-accounts", page([]));
    on("GET", "orgs/:org/projects/:project/audit-log", page([]));
    const { user, router } = await renderApp(`${PROJECT_BASE}/settings`);
    await ready();
    for (const [key, path] of [
      ["i", "limits"],
      ["e", "members"],
      ["v", "service-accounts"],
      ["a", "audit-log"],
      ["y", "playground"],
    ]) {
      await user.keyboard(`g${key}`);
      await waitFor(() => expect(router.state.location.pathname).toBe(`${PROJECT_BASE}/${path}`));
    }
  });

  it("opens the organization's audit log from a project whose audit log the person can't see", async () => {
    signedIn({ projectRights: ALL_PROJECT_RIGHTS.filter((r) => r !== "project.audit_log.view") });
    projectData();
    const { user, router } = await renderApp(`${PROJECT_BASE}/limits`);
    await ready();
    await user.keyboard("?");
    const sheet = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(rows(sheet, "Go to")).toEqual(["Organization audit log", "Organization settings"]);
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await user.keyboard("ga");
    await waitFor(() => expect(router.state.location.pathname).toBe(`/settings/orgs/${ORG}/audit-log`));
  });

  it("leaves the request logs out of the shortcuts when request logs are off", async () => {
    signedIn({ session: whoAmI({ request_logs: false }) });
    projectData();
    const { user } = await renderApp(`${PROJECT_BASE}/limits`);
    await ready();
    await user.keyboard("?");
    const sheet = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(rows(sheet, "Go to a project page")).toContain("Analytics");
    expect(rows(sheet, "Go to a project page")).not.toContain("Request logs");
  });

  it("shows only the account's settings away from any organization", async () => {
    signedIn({ session: whoAmI({ memberships: [] }), memberships: [] });
    on("GET", "me", me());
    const { user } = await renderApp("/settings/account/appearance");
    await screen.findByRole("heading", { name: "Appearance" });
    await user.keyboard("?");
    const sheet = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(rows(sheet, "Go to")).toEqual(["Account settings"]);
  });
});
