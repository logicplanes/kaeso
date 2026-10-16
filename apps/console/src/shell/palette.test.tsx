import { screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { ALL_PROJECT_RIGHTS, ORG, OTHER_PROJECT, PROJECT, membership, myMembership, signedIn, user as person, whoAmI } from "~/test/fixtures";
import { apiKey, catalogueModel, projectData, projectMember, serviceAccount } from "~/test/fixtures/project";
import { me } from "~/test/fixtures/instance-account";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const LIMITS = `/orgs/${ORG}/projects/${PROJECT}/limits`;
const PLACEHOLDER = "Search pages, actions, keys, models and members";

async function openPalette(user: UserEvent) {
  await screen.findByRole("navigation", { name: "Console" });
  await user.keyboard("{Meta>}k{/Meta}");
  await screen.findByPlaceholderText(PLACEHOLDER);
  return screen.getByRole("listbox", { name: "Results" });
}

async function openPaletteIn(user: UserEvent, navigation: string) {
  await screen.findByRole("navigation", { name: navigation });
  await user.keyboard("{Meta>}k{/Meta}");
  await screen.findByPlaceholderText(PLACEHOLDER);
  return screen.getByRole("listbox", { name: "Results" });
}

const option = (list: HTMLElement, id: string) =>
  waitFor(() => {
    const found = within(list)
      .getAllByRole("option")
      .find((o) => o.id.endsWith(`-${id}`));
    if (!found) throw new Error(`No option ${id}`);
    return found;
  });

const optionIds = (list: HTMLElement) =>
  within(list)
    .getAllByRole("option")
    .map((o) => o.id.replace(/^.*?-(?=[a-z-]+:)/, ""));

describe("the command palette", () => {
  it("offers the pages, actions and switches the person may use in a project", async () => {
    signedIn();
    projectData();
    const { user } = await renderApp(LIMITS);
    const list = await openPalette(user);
    const ids = optionIds(list);
    expect(ids).toEqual(expect.arrayContaining(["page:keys", "org:providers", "org-settings:members", "account:profile", "instance:system"]));
    expect(ids).toEqual(
      expect.arrayContaining(["create:page", "create:key", "create:project", "create:invite", "switch:project", "theme:dark", "help:shortcuts"]),
    );
    expect(ids).not.toContain("create:limit");
    expect(ids).not.toContain("switch:org");
    expect(within(list).getByRole("option", { name: /Add limit/ })).toHaveTextContent("C");
  });

  it("goes to a page and closes", async () => {
    signedIn();
    projectData();
    const { user, router } = await renderApp(LIMITS);
    const list = await openPalette(user);
    await user.click(await option(list, "page:keys"));
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
    expect(await option(screen.getByRole("listbox", { name: "Results" }), "page:keys")).toBeInTheDocument();
    expect(optionIds(screen.getByRole("listbox", { name: "Results" }))).not.toContain("page:limits");
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
    const list = await openPalette(user);
    await user.click(within(list).getByRole("option", { name: /Switch organization/ }));
    await user.click(await screen.findByRole("option", { name: /Globex/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/orgs/org_globex"));
  });

  it("switches to another project", async () => {
    signedIn();
    projectData();
    const { user, router } = await renderApp(LIMITS);
    const list = await openPalette(user);
    await user.click(within(list).getByRole("option", { name: /Switch project/ }));
    await user.click(await screen.findByRole("option", { name: /Support triage/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${OTHER_PROJECT}`));
  });

  it("runs the page's own create action and links to the others", async () => {
    signedIn();
    const { user, router } = await renderApp(`/orgs/${ORG}/projects`);
    let list = await openPalette(user);
    expect(optionIds(list)).not.toContain("create:project");
    await user.click(within(list).getByRole("option", { name: /Create project/ }));
    expect(await screen.findByRole("dialog", { name: "Create project" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Create project" })).not.toBeInTheDocument());
    list = await openPalette(user);
    await user.click(within(list).getByRole("option", { name: /Create key/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/keys`));
    expect(router.state.location.search).toMatchObject({ create: true });
  });

  it("invites a member from anywhere in the organization", async () => {
    signedIn();
    const { user, router } = await renderApp(`/orgs/${ORG}/projects`);
    const list = await openPalette(user);
    await user.click(within(list).getByRole("option", { name: /Invite member/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/settings/orgs/${ORG}/members`));
    expect(router.state.location.search).toMatchObject({ tab: "invites", invite: true });
  });

  it("leaves out the instance for people who don't run it", async () => {
    signedIn({ session: whoAmI({ user: person({ is_instance_admin: false }) }) });
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    const list = await openPalette(user);
    expect(optionIds(list).some((id) => id.startsWith("instance:"))).toBe(false);
    expect(optionIds(list)).toContain("account:profile");
  });

  it("changes the theme", async () => {
    signedIn();
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    let list = await openPalette(user);
    await user.click(within(list).getByRole("option", { name: /Use the dark theme/ }));
    await waitFor(() => expect(document.documentElement).toHaveAttribute("data-theme", "dark"));
    list = await openPalette(user);
    await user.click(within(list).getByRole("option", { name: /Use the light theme/ }));
    await waitFor(() => expect(document.documentElement).toHaveAttribute("data-theme", "light"));
    list = await openPalette(user);
    await user.click(within(list).getByRole("option", { name: /Use the system theme/ }));
    await waitFor(() => expect(document.documentElement).not.toHaveAttribute("data-theme"));
  });

  it("opens the list of keyboard shortcuts", async () => {
    signedIn();
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    const list = await openPalette(user);
    await user.click(within(list).getByRole("option", { name: /Keyboard shortcuts/ }));
    const sheet = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(within(sheet).getByText("Single keys work when no field has focus.")).toBeInTheDocument();
    expect(within(sheet).getByText("Go to request logs")).toBeInTheDocument();
  });

  it("offers only the account and the theme to someone outside every organization", async () => {
    signedIn({ session: whoAmI({ memberships: [] }), memberships: [] });
    const { user } = await renderApp("/");
    const list = await openPaletteIn(user, "Console");
    const ids = optionIds(list);
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
    const list = await openPaletteIn(user, "Settings");
    await user.click(await option(list, "page:keys"));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${OTHER_PROJECT}/keys`));
  });

  it("works in the first organization's default project from settings when there is no last place", async () => {
    const globex = { org_id: "org_globex", org_name: "Globex" };
    signedIn({
      session: whoAmI({ memberships: [membership(), membership(globex)] }),
      memberships: [myMembership(), myMembership({ ...globex, projects: [] })],
    });
    projectData();
    on("GET", "me", me());
    const { user, router } = await renderApp("/settings/account/profile");
    const list = await openPaletteIn(user, "Settings");
    await user.click(await option(list, "page:limits"));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/limits`));
  });

  it("uses the organization's default project when the last place was in another organization", async () => {
    const globex = { org_id: "org_globex", org_name: "Globex" };
    signedIn({
      session: whoAmI({ memberships: [membership(globex), membership()] }),
      memberships: [myMembership({ ...globex, projects: [] }), myMembership()],
    });
    projectData();
    on("GET", "me", me());
    window.localStorage.setItem("kaeso:last", JSON.stringify({ org: "org_globex", project: "proj_globex" }));
    const { user, router } = await renderApp(`/settings/orgs/${ORG}/general`);
    const list = await openPaletteIn(user, "Settings");
    await user.click(await option(list, "page:keys"));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/keys`));
  });
});
