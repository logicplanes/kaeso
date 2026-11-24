import { screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ALL_PROJECT_RIGHTS, ORG, OTHER_PROJECT, PROJECT, membership, myMembership, project, signedIn, user as person, whoAmI } from "~/test/fixtures";
import { apiKey, catalogueModel, projectData, projectMember, serviceAccount } from "~/test/fixtures/project";
import { me } from "~/test/fixtures/instance-account";
import { apiError, on, page as paged, requests } from "~/test/network";
import { credential, member, platformKey } from "~/test/fixtures/org";
import { renderApp } from "~/test/render";

const LIMITS = `/orgs/${ORG}/projects/${PROJECT}/limits`;
const PLACEHOLDER = "Search pages, keys and models";

async function openPalette(user: UserEvent) {
  await screen.findByRole("navigation", { name: "Console" });
  await user.keyboard("{Meta>}k{/Meta}");
  await screen.findByPlaceholderText(/^Search pages, /);
  return screen.getByRole("listbox", { name: "Results" });
}

async function openPaletteIn(user: UserEvent, navigation: string) {
  await screen.findByRole("navigation", { name: navigation });
  await user.keyboard("{Meta>}k{/Meta}");
  await screen.findByPlaceholderText(/^Search pages, /);
  return screen.getByRole("listbox", { name: "Results" });
}

const idOf = (o: Element) => o.id.replace(/^.*?-(?=[a-z-]+:)/, "");

// The list draws only the rows in view, but the keyboard reaches every row and the active one is always drawn.
async function walk(user: UserEvent, until?: string) {
  const search = screen.getByPlaceholderText(/^Search pages, /);
  const active = () => document.getElementById(search.getAttribute("aria-activedescendant") ?? "");
  await user.keyboard("{Home}");
  const ids: string[] = [];
  for (let row = active(); row && !ids.includes(idOf(row)); row = active()) {
    ids.push(idOf(row));
    if (idOf(row) === until) return { ids, row };
    await user.keyboard("{ArrowDown}");
  }
  return { ids, row: null };
}

const option = (user: UserEvent, id: string) =>
  waitFor(async () => {
    const { row } = await walk(user, id);
    if (!row) throw new Error(`No option ${id}`);
    return row;
  });

const optionIds = async (user: UserEvent) => (await walk(user)).ids;

describe("the command palette", () => {
  it("offers the pages, actions and switches the person may use in a project", async () => {
    signedIn();
    projectData();
    const { user } = await renderApp(LIMITS);
    await openPalette(user);
    const ids = await optionIds(user);
    expect(ids).toEqual(expect.arrayContaining(["page:keys", "org:providers", "org-settings:members", "account:profile", "instance:system"]));
    expect(ids).toEqual(
      expect.arrayContaining(["create:page", "create:key", "create:project", "create:invite", "switch:project", "theme:dark", "help:shortcuts"]),
    );
    expect(ids).not.toContain("create:limit");
    expect(ids).not.toContain("switch:org");
    const create = await option(user, "create:page");
    expect(create).toHaveAccessibleName(/Add limit/);
    expect(create).toHaveTextContent("C");
  });

  it("copies the project's portal address and adds an MCP server", async () => {
    signedIn();
    projectData();
    on("GET", "orgs/:org/mcp-servers", paged([]));
    const { user, router } = await renderApp(LIMITS);
    const writeText = vi.spyOn(navigator.clipboard, "writeText");
    await openPalette(user);
    await user.click(await option(user, "copy:portal"));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith(`${window.location.origin}/mcp/${PROJECT}`));
    expect(await screen.findByText("Portal address copied")).toBeInTheDocument();
    await openPalette(user);
    await user.click(await option(user, "create:mcp-server"));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/mcp-servers`));
    const dialog = await screen.findByRole("dialog", { name: "Add MCP server" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("goes to a page and closes", async () => {
    signedIn();
    projectData();
    const { user, router } = await renderApp(LIMITS);
    await openPalette(user);
    await user.click(await option(user, "page:keys"));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/keys`));
    expect(screen.queryByPlaceholderText(PLACEHOLDER)).not.toBeInTheDocument();
  });

  it("filters by what is typed and toggles with the shortcut", async () => {
    signedIn();
    projectData();
    const { user } = await renderApp(LIMITS);
    await screen.findByRole("navigation", { name: "Console" });
    await user.keyboard("{Meta>}k{/Meta}");
    await user.keyboard("api");
    const input = await screen.findByPlaceholderText(PLACEHOLDER);
    await waitFor(() => expect(input).toHaveValue("api"));
    expect(await option(user, "page:keys")).toBeInTheDocument();
    expect(await optionIds(user)).not.toContain("page:limits");
    await user.keyboard("{Meta>}k{/Meta}");
    await waitFor(() => expect(screen.queryByPlaceholderText(PLACEHOLDER)).not.toBeInTheDocument());
  });

  it("finds keys, models, members and service accounts in the project", async () => {
    signedIn();
    projectData({
      keys: [apiKey({ name: "north-web" }), apiKey({ id: "key_other", name: "billing" })],
      members: [projectMember({ user: { id: "usr_grace", name: "Grace Hopper", email: "grace@north.example" } })],
      accounts: [serviceAccount({ id: "sa_north", name: "north-batch" })],
      models: [catalogueModel({ id: "cm_north", provider: "openai", model_id: "north-chat", effective: { display_name: "North chat" } })],
    });
    const { user, router } = await renderApp(LIMITS);
    await openPalette(user);
    await user.type(screen.getByPlaceholderText(PLACEHOLDER), "north");
    const found = await screen.findByRole("option", { name: /north-batch/ });
    const list = screen.getByRole("listbox", { name: "Results" });
    expect(within(list).getByText("Find (in the first 100 of each list)")).toBeInTheDocument();
    expect(within(list).getByRole("option", { name: /north-web/ })).toHaveTextContent("Key");
    expect(await within(list).findByRole("option", { name: /north-chat/ })).toHaveTextContent("Model");
    expect(within(list).getByRole("option", { name: /Grace Hopper/ })).toHaveTextContent("Member");
    expect(found).toHaveTextContent("Service account");
    expect(within(list).queryByRole("option", { name: /billing/ })).not.toBeInTheDocument();
    await user.click(found);
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/service-accounts`));
    expect(router.state.location.search).toMatchObject({ account: "sa_north" });
  });

  it("opens a pasted request id in the project's request log", async () => {
    signedIn();
    projectData();
    const id = "req_01m7g2tr3hegy8qg6fav4nea97";
    const { user, router } = await renderApp(LIMITS);
    await openPalette(user);
    await user.type(screen.getByPlaceholderText(PLACEHOLDER), "abc");
    expect(screen.queryByRole("option", { name: /Open request/ })).not.toBeInTheDocument();
    await user.clear(screen.getByPlaceholderText(PLACEHOLDER));
    await user.paste(`  ${id} `);
    const opened = await screen.findByRole("option", { name: new RegExp(`Open request ${id}`) });
    expect(opened).toHaveTextContent(/In /);
    await user.click(opened);
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/logs`));
    expect(router.state.location.search).toMatchObject({ log: id });
  });

  it("names the project a pasted request id is looked up in by its id while the project's name is unknown", async () => {
    signedIn({ projects: [project({ id: OTHER_PROJECT, name: "Support triage" })] });
    on("GET", "orgs/:org/projects/:project", apiError(500, "internal", "The gateway hit an error."));
    const { user } = await renderApp(`/orgs/${ORG}/projects/${PROJECT}`);
    expect(await screen.findByText("This page couldn't load")).toBeInTheDocument();
    await openPalette(user);
    await user.paste("req_01m7g2tr3hegy8qg6fav4nea97");
    expect(await screen.findByRole("option", { name: /Open request req_01m7g2tr3hegy8qg6fav4nea97/ })).toHaveTextContent(`In ${PROJECT}`);
  });

  it("offers no request to open without the right to read the request log", async () => {
    signedIn({ projectRights: ALL_PROJECT_RIGHTS.filter((r) => r !== "logs.metadata.view") });
    projectData();
    const { user } = await renderApp(LIMITS);
    await openPalette(user);
    await user.click(screen.getByPlaceholderText(PLACEHOLDER));
    await user.paste("req_01m7g2tr3hegy8qg6fav4nea97");
    await screen.findByRole("listbox", { name: "Results" });
    expect(screen.queryByRole("option", { name: /Open request/ })).not.toBeInTheDocument();
  });

  it("finds the organization's projects, members and keys on its pages, and offers its pages, not a project's", async () => {
    signedIn();
    projectData();
    on("GET", "orgs/:org/members", paged([member(), member({ user: { id: "usr_lin", name: "Lin Park", email: "lin@example.com" } })]));
    on("GET", "orgs/:org/platform-keys", paged([platformKey({ id: "opk_lint", name: "linter-ci" })]));
    on("GET", "orgs/:org/credentials", { data: [credential({ id: "cred_lin", name: "Lindy OpenAI" })] });
    const { user, router } = await renderApp(`/orgs/${ORG}`);
    await openPalette(user);
    expect(screen.getByPlaceholderText("Search pages, projects and members")).toBeInTheDocument();
    const ids = await optionIds(user);
    expect(ids).toEqual(expect.arrayContaining(["org:providers", "org-settings:members"]));
    expect(ids.filter((id) => id.startsWith("page:"))).toEqual([]);
    await user.type(screen.getByPlaceholderText("Search pages, projects and members"), "Lin");
    const lin = await screen.findByRole("option", { name: /Lin Park/ });
    expect(lin).toHaveTextContent("Member");
    expect(await screen.findByRole("option", { name: /linter-ci/ })).toHaveTextContent("Platform key");
    expect(await screen.findByRole("option", { name: /Lindy OpenAI/ })).toHaveTextContent("Provider key");
    await user.click(lin);
    await waitFor(() => expect(router.state.location.pathname).toBe(`/settings/orgs/${ORG}/members`));
    expect(router.state.location.search).toMatchObject({ member: "usr_lin" });
  });

  it("finds a project by its name on the organization's pages, and no project's keys", async () => {
    signedIn();
    projectData({ keys: [apiKey({ name: "support-bot" })] });
    window.localStorage.setItem("kaeso:last", JSON.stringify({ org: ORG, project: PROJECT }));
    const { user, router } = await renderApp(`/orgs/${ORG}`);
    await openPalette(user);
    await user.type(screen.getByPlaceholderText("Search pages, projects and members"), "support");
    const triage = await screen.findByRole("option", { name: /Support triage/ });
    expect(screen.queryByRole("option", { name: /support-bot/ })).not.toBeInTheDocument();
    expect(requests("GET", "orgs/:org/projects/:project/keys")).toHaveLength(0);
    await user.click(triage);
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${OTHER_PROJECT}`));
  });

  it("finds another project by its name inside a project", async () => {
    signedIn();
    projectData();
    const { user, router } = await renderApp(LIMITS);
    await openPalette(user);
    await user.type(screen.getByPlaceholderText(PLACEHOLDER), "triage");
    const triage = await screen.findByRole("option", { name: /Support triage/ });
    expect(triage).toHaveTextContent("Project");
    await user.click(triage);
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${OTHER_PROJECT}`));
  });

  it("tells recent requests apart by their project and time", async () => {
    signedIn();
    projectData();
    window.localStorage.setItem(
      "kaeso:recent",
      JSON.stringify([
        { kind: "request", id: "req_1", label: "200 responses · mistral-small-latest", href: "/a", detail: "Research agent, 20 Oct 2026, 05:23:01" },
        { kind: "request", id: "req_2", label: "200 responses · mistral-small-latest", href: "/b", detail: "Research agent, 20 Oct 2026, 05:24:12" },
      ]),
    );
    const { user } = await renderApp(LIMITS);
    const list = await openPalette(user);
    const recent = within(list).getAllByRole("option", { name: /mistral-small-latest/ });
    expect(recent.map((o) => o.textContent)).toEqual([
      expect.stringContaining("Request · Research agent, 20 Oct 2026, 05:23:01"),
      expect.stringContaining("Request · Research agent, 20 Oct 2026, 05:24:12"),
    ]);
  });

  it("forgets the search when the shortcut closes it", async () => {
    signedIn();
    projectData({ keys: [apiKey({ name: "north-web" })] });
    const { user } = await renderApp(LIMITS);
    await openPalette(user);
    await user.type(screen.getByPlaceholderText(PLACEHOLDER), "north");
    await screen.findByRole("option", { name: /north-web/ });
    await user.keyboard("{Meta>}k{/Meta}");
    await waitFor(() => expect(screen.queryByPlaceholderText(PLACEHOLDER)).not.toBeInTheDocument());
    const list = await openPalette(user);
    expect(screen.getByPlaceholderText(PLACEHOLDER)).toHaveValue("");
    expect(within(list).queryByText("Find (in the first 100 of each list)")).not.toBeInTheDocument();
    expect(within(list).queryByRole("option", { name: /north-web/ })).not.toBeInTheDocument();
  });

  it("only looks in the lists the person may see", async () => {
    signedIn({ projectRights: ALL_PROJECT_RIGHTS.filter((r) => !["keys.all.view", "models.view", "project.members.view"].includes(r)) });
    projectData({ accounts: [serviceAccount({ name: "north-batch" })] });
    const { user } = await renderApp(LIMITS);
    await openPalette(user);
    await user.type(screen.getByPlaceholderText(PLACEHOLDER), "north");
    expect(await screen.findByRole("option", { name: /north-batch/ })).toBeInTheDocument();
    expect(requests("GET", "orgs/:org/projects/:project/keys")).toHaveLength(0);
    expect(requests("GET", "orgs/:org/projects/:project/members")).toHaveLength(0);
    expect(requests("GET", "orgs/:org/projects/:project/models")).toHaveLength(0);
  });

  it("lists what was opened recently", async () => {
    signedIn();
    projectData();
    window.localStorage.setItem(
      "kaeso:recent",
      JSON.stringify([
        { kind: "key", id: "key_web", label: "checkout-web", href: `/orgs/${ORG}/projects/${PROJECT}/keys?key=key_web` },
        { kind: "request", id: "req_1", label: "req_1" },
        null,
      ]),
    );
    const { user, router } = await renderApp(LIMITS);
    const list = await openPalette(user);
    expect(within(list).getByText("Recent")).toBeInTheDocument();
    expect(within(list).queryByRole("option", { name: /req_1/ })).not.toBeInTheDocument();
    await user.click(within(list).getByRole("option", { name: /checkout-web/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/keys`));
    expect(router.state.location.search).toMatchObject({ key: "key_web" });
  });

  it("switches to another organization", async () => {
    const globex = { org_id: "org_globex", org_name: "Globex" };
    signedIn({ session: whoAmI({ memberships: [membership(), membership(globex)] }), memberships: [myMembership(), myMembership(globex)] });
    const { user, router } = await renderApp(`/orgs/${ORG}/projects`);
    await openPalette(user);
    await user.click(await option(user, "switch:org"));
    await user.click(await screen.findByRole("option", { name: /Globex/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/orgs/org_globex"));
  });

  it("switches to another project", async () => {
    signedIn();
    projectData();
    const { user, router } = await renderApp(LIMITS);
    await openPalette(user);
    await user.click(await option(user, "switch:project"));
    await user.click(await screen.findByRole("option", { name: /Support triage/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${OTHER_PROJECT}`));
  });

  it("runs the page's own create action", async () => {
    signedIn();
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    await openPalette(user);
    expect(await optionIds(user)).not.toContain("create:project");
    await user.click(await option(user, "create:page"));
    expect(await screen.findByRole("dialog", { name: "Create project" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Create project" })).not.toBeInTheDocument());
  });

  it("invites a member from anywhere in the organization", async () => {
    signedIn();
    const { user, router } = await renderApp(`/orgs/${ORG}/projects`);
    await openPalette(user);
    await user.click(await option(user, "create:invite"));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/settings/orgs/${ORG}/members`));
    expect(router.state.location.search).toMatchObject({ tab: "invites", invite: true });
  });

  it("creates a key in the project the person is in", async () => {
    signedIn();
    projectData();
    const { user, router } = await renderApp(LIMITS);
    await openPalette(user);
    await user.click(await option(user, "create:key"));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/keys`));
    expect(router.state.location.search).toMatchObject({ create: true });
  });

  it("offers no project's actions on an organization's pages before the person opened a project", async () => {
    signedIn();
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    await openPalette(user);
    expect(await optionIds(user)).not.toContain("create:key");
    expect((await optionIds(user)).filter((id) => id.startsWith("page:"))).toEqual([]);
  });

  it("leaves out the instance for people who don't run it", async () => {
    signedIn({ session: whoAmI({ user: person({ is_instance_admin: false }) }) });
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    await openPalette(user);
    expect((await optionIds(user)).some((id) => id.startsWith("instance:"))).toBe(false);
    expect(await optionIds(user)).toContain("account:profile");
  });

  it("changes the theme", async () => {
    signedIn();
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    await openPalette(user);
    await user.click(await option(user, "theme:dark"));
    await waitFor(() => expect(document.documentElement).toHaveAttribute("data-theme", "dark"));
    await openPalette(user);
    await user.click(await option(user, "theme:light"));
    await waitFor(() => expect(document.documentElement).toHaveAttribute("data-theme", "light"));
    await openPalette(user);
    await user.click(await option(user, "theme:system"));
    await waitFor(() => expect(document.documentElement).not.toHaveAttribute("data-theme"));
  });

  it("opens the list of keyboard shortcuts", async () => {
    signedIn();
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    await openPalette(user);
    await user.click(await option(user, "help:shortcuts"));
    const sheet = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(within(sheet).getByText("Single keys work when no field has focus.")).toBeInTheDocument();
    expect(within(sheet).getByText("Organization audit log")).toBeInTheDocument();
  });

  it("offers only the account and the theme to someone outside every organization", async () => {
    signedIn({ session: whoAmI({ memberships: [] }), memberships: [] });
    const { user } = await renderApp("/");
    await openPaletteIn(user, "Console");
    const ids = await optionIds(user);
    expect(ids).toEqual(expect.arrayContaining(["account:profile", "theme:dark", "help:shortcuts"]));
    expect(ids.some((id) => /^(page|org|org-settings|create|switch):/.test(id))).toBe(false);
    expect(requests("GET", "orgs/:org/projects")).toHaveLength(0);
  });

  it("works in the last place from settings", async () => {
    signedIn();
    projectData();
    on("GET", "me", me());
    window.localStorage.setItem("kaeso:last", JSON.stringify({ org: ORG, project: OTHER_PROJECT }));
    const { user, router } = await renderApp("/settings/account/profile");
    await openPaletteIn(user, "Settings");
    await user.click(await option(user, "page:keys"));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${OTHER_PROJECT}/keys`));
  });

  it("picks no project for the person from settings when there is no last place", async () => {
    const globex = { org_id: "org_globex", org_name: "Globex" };
    signedIn({
      session: whoAmI({ memberships: [membership(), membership(globex)] }),
      memberships: [myMembership(), myMembership({ ...globex, projects: [] })],
    });
    projectData();
    on("GET", "me", me());
    const { user, router } = await renderApp("/settings/account/profile");
    await openPaletteIn(user, "Settings");
    expect((await optionIds(user)).filter((id) => id.startsWith("page:"))).toEqual([]);
    await user.click(await option(user, "org:providers"));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/providers`));
  });

  it("offers the organization's pages, not another organization's project, on its settings pages", async () => {
    const globex = { org_id: "org_globex", org_name: "Globex" };
    signedIn({
      session: whoAmI({ memberships: [membership(globex), membership()] }),
      memberships: [myMembership({ ...globex, projects: [] }), myMembership()],
    });
    projectData();
    on("GET", "me", me());
    window.localStorage.setItem("kaeso:last", JSON.stringify({ org: "org_globex", project: "proj_globex" }));
    const { user, router } = await renderApp(`/settings/orgs/${ORG}/general`);
    await openPaletteIn(user, "Settings");
    expect((await optionIds(user)).filter((id) => id.startsWith("page:"))).toEqual([]);
    await user.click(await option(user, "org:providers"));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/providers`));
  });
});
