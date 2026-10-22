import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
  ALL_PROJECT_RIGHTS,
  ORG,
  OTHER_PROJECT,
  PROJECT,
  membership,
  myMembership,
  project,
  signedIn,
  signedOut,
  user as person,
  whoAmI,
} from "~/test/fixtures";
import { notifications } from "~/test/fixtures/org";
import { apiError, noContent, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { atFixtureTime } from "~/test/clock";

atFixtureTime();

const sidebar = () => screen.getByRole("navigation", { name: "Console" });

describe("the sidebar in a project", () => {
  it("lists the project's pages the person holds rights for and links back to the organization", async () => {
    signedIn({ projectRights: ALL_PROJECT_RIGHTS.filter((r) => r !== "keys.all.view" && r !== "project.audit_log.view") });
    on("GET", "orgs/:org/projects/:project/limits", page([]));
    await renderApp(`/orgs/${ORG}/projects/${PROJECT}/limits`);
    const nav = await screen.findByRole("navigation", { name: "Console" });
    await within(nav).findByRole("link", { name: "Limits" });
    expect(within(nav).getByRole("link", { name: "Northwind Labs" })).toHaveAttribute("href", `/orgs/${ORG}`);
    expect(within(nav).getByRole("link", { name: "Request logs" })).toHaveAttribute("href", `/orgs/${ORG}/projects/${PROJECT}/logs`);
    expect(within(nav).getByRole("link", { name: "Overview" })).toHaveAttribute("href", `/orgs/${ORG}/projects/${PROJECT}`);
    expect(within(nav).getByRole("link", { name: "Limits" })).toHaveAttribute("aria-current", "page");
    expect(within(nav).getByRole("link", { name: "Overview" })).not.toHaveAttribute("aria-current");
    expect(within(nav).getByRole("link", { name: "Northwind Labs" })).not.toHaveAttribute("aria-current");
    expect(within(nav).queryByRole("link", { name: "API keys" })).not.toBeInTheDocument();
    expect(within(nav).queryByRole("link", { name: "Audit log" })).not.toBeInTheDocument();
    expect(await within(nav).findByRole("button", { name: "Checkout assistant", description: "Switch project" })).toBeInTheDocument();
  });

  it("switches to another project on the same page", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/limits", page([]));
    const { user, router } = await renderApp(`/orgs/${ORG}/projects/${PROJECT}/limits`);
    await user.click(await within(sidebar()).findByRole("button", { name: "Checkout assistant", description: "Switch project" }));
    const menu = await screen.findByRole("menu");
    expect(within(menu).getByRole("menuitemradio", { name: /Checkout assistant/ })).toHaveAttribute("aria-checked", "true");
    expect(within(menu).getByRole("menuitem", { name: "All projects" })).toHaveAttribute("href", `/orgs/${ORG}/projects`);
    await user.click(within(menu).getByRole("menuitemradio", { name: /Support triage/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${OTHER_PROJECT}/limits`));
  });

  it("keeps the chosen period when switching from the overview", async () => {
    signedIn({ projectRights: ["usage.view", "project.view"] });
    on("GET", "orgs/:org/projects/:project/usage/span", { first: null, last: null });
    on("GET", "orgs/:org/projects/:project/usage", { data: [] });
    on("GET", "orgs/:org/projects/:project/models", { data: [] });
    on("GET", "orgs/:org/projects/:project/logs", page([]));
    const { user, router } = await renderApp(`/orgs/${ORG}/projects/${PROJECT}?from=2026-10-01&to=2026-10-07`);
    await user.click(await within(sidebar()).findByRole("button", { name: "Checkout assistant", description: "Switch project" }));
    await user.click(await screen.findByRole("menuitemradio", { name: /Support triage/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${OTHER_PROJECT}`));
    expect(router.state.location.search).toMatchObject({ from: "2026-10-01", to: "2026-10-07" });
  });

  it("switches from an overview without a chosen period to the other project's overview", async () => {
    signedIn({ projectRights: ["usage.view", "project.view"] });
    on("GET", "orgs/:org/projects/:project/usage/span", { first: null, last: null });
    on("GET", "orgs/:org/projects/:project/usage", { data: [] });
    on("GET", "orgs/:org/projects/:project/models", { data: [] });
    on("GET", "orgs/:org/projects/:project/logs", page([]));
    const { user, router } = await renderApp(`/orgs/${ORG}/projects/${PROJECT}`);
    await user.click(await within(sidebar()).findByRole("button", { name: "Checkout assistant", description: "Switch project" }));
    await user.click(await screen.findByRole("menuitemradio", { name: /Support triage/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${OTHER_PROJECT}`));
    expect(router.state.location.searchStr).toBe("");
  });

  it("still names and offers the project it is in when the organization's projects can't load", async () => {
    signedIn();
    on("GET", "orgs/:org/projects", apiError(403, "permission_denied", "You can't list this organization's projects."));
    on("GET", "orgs/:org/projects/:project/limits", page([]));
    const { user } = await renderApp(`/orgs/${ORG}/projects/${PROJECT}/limits`);
    await user.click(await within(sidebar()).findByRole("button", { name: "Checkout assistant", description: "Switch project" }));
    const menu = await screen.findByRole("menu");
    expect(within(menu).getAllByRole("menuitemradio")).toHaveLength(1);
    expect(within(menu).getByRole("menuitemradio", { name: /Checkout assistant/ })).toHaveAttribute("aria-checked", "true");
  });

  it("leads back to an organization the person isn't in by a plain name, then leaves it", async () => {
    signedIn();
    let release = () => {};
    const held = new Promise<void>((resolve) => (release = resolve));
    let calls = 0;
    on("GET", "auth/session", async () => {
      calls += 1;
      if (calls > 1) await held;
      return whoAmI();
    });
    on("GET", "orgs/:org/projects/:project/limits", page([]));
    const { router } = await renderApp(`/orgs/org_other/projects/${PROJECT}/limits`);
    const nav = await screen.findByRole("navigation", { name: "Console" });
    expect(await within(nav).findByRole("link", { name: "Organization" })).toHaveAttribute("href", "/orgs/org_other");
    release();
    expect(await notifications().findByText("That organization doesn't exist, or you don't have access to it")).toBeInTheDocument();
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}`));
  });

  it("names the project it is in even when the organization's list leaves it out", async () => {
    signedIn({ projects: [project({ id: OTHER_PROJECT, name: "Support triage", is_default: false })] });
    on("GET", "orgs/:org/projects/:project", project());
    on("GET", "orgs/:org/projects/:project/limits", page([]));
    const { user } = await renderApp(`/orgs/${ORG}/projects/${PROJECT}/limits`);
    await user.click(await within(sidebar()).findByRole("button", { name: "Checkout assistant", description: "Switch project" }));
    const menu = await screen.findByRole("menu");
    expect(within(menu).getAllByRole("menuitemradio")).toHaveLength(2);
    expect(within(menu).getByRole("menuitemradio", { name: /Checkout assistant/ })).toHaveTextContent("Default");
    expect(within(menu).getByRole("menuitemradio", { name: /Support triage/ })).toBeInTheDocument();
  });
});

describe("the sidebar in an organization", () => {
  it("shows the organization's pages the person may see and its projects", async () => {
    const rights = membership().rights.filter((r) => r !== "provider_credentials.view");
    signedIn({ session: whoAmI({ memberships: [membership({ rights })] }) });
    await renderApp(`/orgs/${ORG}/projects`);
    const nav = await screen.findByRole("navigation", { name: "Console" });
    expect(within(nav).getByRole("link", { name: "Overview" })).toHaveAttribute("href", `/orgs/${ORG}`);
    expect(within(nav).getAllByRole("link", { name: "Projects" })[0]).toHaveAttribute("href", `/orgs/${ORG}/projects`);
    expect(within(nav).queryByRole("link", { name: "Providers" })).not.toBeInTheDocument();
    expect(await within(nav).findByRole("link", { name: "Support triage" })).toHaveAttribute("href", `/orgs/${ORG}/projects/${OTHER_PROJECT}`);
    expect(within(nav).getByRole("link", { name: "Checkout assistant" })).toHaveAttribute("href", `/orgs/${ORG}/projects/${PROJECT}`);
  });

  it("lists projects past the first page", async () => {
    signedIn();
    on("GET", "orgs/:org/projects", ({ query }: { query: URLSearchParams }) =>
      query.get("after") === "cur_2" ? page([project({ id: "proj_late", name: "Late project", is_default: false })]) : page([project()], "cur_2"),
    );
    await renderApp(`/orgs/${ORG}/projects`);
    const nav = await screen.findByRole("navigation", { name: "Console" });
    expect(await within(nav).findByRole("link", { name: "Late project" })).toHaveAttribute("href", `/orgs/${ORG}/projects/proj_late`);
    expect(within(nav).getByRole("link", { name: "Checkout assistant" })).toBeInTheDocument();
  });

  it("opens the command palette from the search button", async () => {
    signedIn();
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    await user.click(await within(await screen.findByRole("navigation", { name: "Console" })).findByRole("button", { name: "Search" }));
    expect(await screen.findByPlaceholderText("Search pages, actions, projects, members and keys")).toBeInTheDocument();
  });
});

describe("the organization switcher", () => {
  it("switches organization and links to settings, the account and the instance", async () => {
    const second = membership({ org_id: "org_globex", org_name: "Globex" });
    signedIn({
      session: whoAmI({ memberships: [membership(), second] }),
      memberships: [myMembership(), myMembership({ org_id: "org_globex", org_name: "Globex" })],
    });
    const { user, router } = await renderApp(`/orgs/${ORG}/projects`);
    await user.click(await screen.findByRole("button", { name: "Northwind Labs", description: "Switch or open settings" }));
    const menu = await screen.findByRole("menu");
    expect(screen.getByText("Ada Okafor")).toBeInTheDocument();
    expect(screen.getByText("owner@example.com")).toBeInTheDocument();
    expect(within(menu).getByRole("menuitem", { name: "Organization settings" })).toHaveAttribute("href", `/settings/orgs/${ORG}/general`);
    expect(within(menu).getByRole("menuitem", { name: "Account" })).toHaveAttribute("href", "/settings/account/profile");
    expect(within(menu).getByRole("menuitem", { name: "Instance" })).toHaveAttribute("href", "/settings/instance/general");
    await user.click(within(menu).getByRole("menuitemradio", { name: "Globex" }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/orgs/org_globex"));
  });

  it("shows when an organization is scheduled for deletion", async () => {
    const doomed = membership({ org_state: "scheduled_for_deletion", org_deletion_due_at: "2026-10-20T12:00:00Z" });
    signedIn({
      session: whoAmI({ memberships: [doomed, membership({ org_id: "org_globex", org_name: "Globex" })], can_create_orgs: false }),
      memberships: [
        myMembership({ org_state: "scheduled_for_deletion", org_deletion_due_at: "2026-10-20T12:00:00Z" }),
        myMembership({ org_id: "org_globex", org_name: "Globex" }),
      ],
    });
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    const switcher = await screen.findByRole("button", { name: "Northwind Labs Deletion on 20 Oct", description: "Switch or open settings" });
    expect(switcher).toHaveTextContent("Deletion on 20 Oct");
    await user.click(switcher);
    const menu = await screen.findByRole("menu");
    expect(within(menu).getByRole("menuitemradio", { name: /Northwind Labs/ })).toHaveTextContent("Deletion on 20 Oct");
    expect(within(menu).queryByRole("menuitem", { name: "Create organization" })).not.toBeInTheDocument();
  });

  it("leaves out the organization list for a person in one organization who can't create more", async () => {
    signedIn({ session: whoAmI({ can_create_orgs: false, user: { ...whoAmI().user, is_instance_admin: false } }) });
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    await user.click(await screen.findByRole("button", { name: "Northwind Labs", description: "Switch or open settings" }));
    const menu = await screen.findByRole("menu");
    expect(within(menu).queryByRole("menuitemradio")).not.toBeInTheDocument();
    expect(within(menu).queryByRole("menuitem", { name: "Instance" })).not.toBeInTheDocument();
    expect(within(menu).getByRole("menuitem", { name: "Sign out" })).toBeInTheDocument();
  });

  it("creates an organization and opens it", async () => {
    let created = false;
    signedIn();
    const initech = membership({ org_id: "org_new", org_name: "Initech" });
    on("GET", "auth/session", () => whoAmI({ memberships: created ? [membership(), initech] : [membership()] }));
    on("GET", "me/memberships", () => ({ data: created ? [myMembership(), myMembership({ org_id: "org_new", org_name: "Initech" })] : [myMembership()] }));
    on("POST", "me/orgs", () => {
      created = true;
      return { id: "org_new", name: "Initech" };
    });
    const { user, router } = await renderApp(`/orgs/${ORG}/projects`);
    await user.click(await screen.findByRole("button", { name: "Northwind Labs", description: "Switch or open settings" }));
    await user.click(await screen.findByRole("menuitem", { name: "Create organization" }));
    const dialog = await screen.findByRole("dialog", { name: "Create organization" });
    const create = within(dialog).getByRole("button", { name: "Create organization" });
    expect(create).toBeDisabled();
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "  Initech  ");
    await user.click(create);
    expect(await within(screen.getByRole("region", { name: "Notifications" })).findByText("Organization created")).toBeInTheDocument();
    expect(requests("POST", "me/orgs")[0]?.body).toEqual({ name: "Initech" });
    await waitFor(() => expect(router.state.location.pathname).toBe("/orgs/org_new"));
  });

  it("shows the gateway's word on the name under the field until the name changes", async () => {
    signedIn();
    on("POST", "me/orgs", apiError(400, "invalid_request", "A name has at most 100 characters.", { param: "name" }));
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    await user.click(await screen.findByRole("button", { name: "Northwind Labs", description: "Switch or open settings" }));
    await user.click(await screen.findByRole("menuitem", { name: "Create organization" }));
    const dialog = await screen.findByRole("dialog", { name: "Create organization" });
    const name = within(dialog).getByRole("textbox", { name: "Name" });
    await user.type(name, "Initech");
    await user.click(within(dialog).getByRole("button", { name: "Create organization" }));
    await waitFor(() => expect(name).toHaveAccessibleDescription("A name has at most 100 characters."));
    expect(name).toHaveAttribute("aria-invalid", "true");
    await user.type(name, "s");
    expect(name).not.toHaveAttribute("aria-invalid");
    expect(within(dialog).queryByText("A name has at most 100 characters.")).not.toBeInTheDocument();
  });

  it("closes the dialog when the gateway refuses to create an organization", async () => {
    signedIn();
    on("POST", "me/orgs", apiError(403, "permission_denied", "You can't create organizations on this instance."));
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    await user.click(await screen.findByRole("button", { name: "Northwind Labs", description: "Switch or open settings" }));
    await user.click(await screen.findByRole("menuitem", { name: "Create organization" }));
    const dialog = await screen.findByRole("dialog", { name: "Create organization" });
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Initech");
    await user.click(within(dialog).getByRole("button", { name: "Create organization" }));
    expect((await screen.findAllByText("You can't create organizations on this instance.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Create organization" })).not.toBeInTheDocument());
  });

  it("goes to the sign-in page when the session had already ended", async () => {
    signedIn();
    const { user, router } = await renderApp(`/orgs/${ORG}/projects`);
    signedOut();
    on("POST", "auth/logout", apiError(401, "authentication", "Sign in to continue."));
    await user.click(await screen.findByRole("button", { name: "Northwind Labs", description: "Switch or open settings" }));
    await user.click(await screen.findByRole("menuitem", { name: "Sign out" }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/sign-in"));
  });

  it("forgets the person's recent items and drafts on sign-out and keeps the theme", async () => {
    signedIn();
    on("POST", "auth/logout", () => {
      signedOut();
      return noContent();
    });
    window.localStorage.setItem("kaeso:recent", JSON.stringify([{ kind: "key", id: "key_web", label: "checkout-web", href: "/" }]));
    window.localStorage.setItem("kaeso:playground-draft:proj_checkout", "Where is order 4471?");
    window.localStorage.setItem("kaeso:theme", "dark");
    const { user, router } = await renderApp(`/orgs/${ORG}/projects`);
    await user.click(await screen.findByRole("button", { name: "Northwind Labs", description: "Switch or open settings" }));
    await user.click(await screen.findByRole("menuitem", { name: "Sign out" }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/sign-in"));
    expect(window.localStorage.getItem("kaeso:recent")).toBeNull();
    expect(window.localStorage.getItem("kaeso:playground-draft:proj_checkout")).toBeNull();
    expect(window.localStorage.getItem("kaeso:theme")).toBe("dark");
  });

  it("stays signed in and says the gateway can't be reached when something else answers", async () => {
    signedIn();
    on("POST", "auth/logout", () => new Response("<!doctype html><title>Portal</title>", { status: 200, headers: { "content-type": "text/html" } }));
    const { user, router } = await renderApp(`/orgs/${ORG}/projects`);
    await user.click(await screen.findByRole("button", { name: "Northwind Labs", description: "Switch or open settings" }));
    await user.click(await screen.findByRole("menuitem", { name: "Sign out" }));
    expect((await notifications().findAllByText("Can't reach the gateway. Try again.")).length).toBeGreaterThan(0);
    expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects`);
  });

  it("signs out and goes to the sign-in page", async () => {
    signedIn();
    on("POST", "auth/logout", () => {
      signedOut();
      return noContent();
    });
    const { user, router } = await renderApp(`/orgs/${ORG}/projects`);
    await user.click(await screen.findByRole("button", { name: "Northwind Labs", description: "Switch or open settings" }));
    await user.click(await screen.findByRole("menuitem", { name: "Sign out" }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/sign-in"));
    expect(requests("POST", "auth/logout")).toHaveLength(1);
    expect(await notifications().findByText("You're signed out")).toBeInTheDocument();
  });

  it("stays signed in and says why when signing out fails", async () => {
    signedIn();
    on("POST", "auth/logout", apiError(500, "internal", "The gateway hit an error."));
    const { user, router } = await renderApp(`/orgs/${ORG}/projects`);
    await user.click(await screen.findByRole("button", { name: "Northwind Labs", description: "Switch or open settings" }));
    await user.click(await screen.findByRole("menuitem", { name: "Sign out" }));
    expect((await screen.findAllByText("The gateway hit an error.")).length).toBeGreaterThan(0);
    expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects`);
  });
});

describe("pictures in the sidebar", () => {
  it("shows the organization's and the projects' pictures", async () => {
    signedIn({
      session: whoAmI({ memberships: [membership({ org_avatar_url: "/avatars/org.webp" })], user: { ...whoAmI().user, avatar_url: "/avatars/ada.webp" } }),
      projects: [project({ avatar_url: "/avatars/checkout.webp" }), project({ id: OTHER_PROJECT, name: "Support triage", is_default: false })],
    });
    on("GET", "orgs/:org/projects/:project/limits", page([]));
    const { container } = await renderApp(`/orgs/${ORG}/projects/${PROJECT}/limits`);
    await within(sidebar()).findByRole("button", { name: "Checkout assistant", description: "Switch project" });
    await waitFor(() => expect(container.querySelector('img[src="/avatars/checkout.webp"]')).not.toBeNull());
    expect(container.querySelector('img[src="/avatars/org.webp"]')).not.toBeNull();
  });
});

describe("the switcher for a person outside every organization", () => {
  it("shows the person's picture and offers only to create an organization", async () => {
    signedIn({ session: whoAmI({ memberships: [], user: person({ avatar_url: "/avatars/ada.webp" }) }), memberships: [] });
    const { user } = await renderApp("/");
    const switcher = await screen.findByRole("button", { name: "Ada Okafor", description: "Account and sign out" });
    await waitFor(() => expect(switcher.querySelector('img[src="/avatars/ada.webp"]')).not.toBeNull());
    await user.click(switcher);
    const menu = await screen.findByRole("menu");
    expect(within(menu).getByRole("menuitem", { name: "Create organization" })).toBeInTheDocument();
    expect(within(menu).queryByRole("menuitemradio")).not.toBeInTheDocument();
    expect(within(menu).queryByRole("menuitem", { name: "Organization settings" })).not.toBeInTheDocument();
    expect(within(menu).getByRole("menuitem", { name: "Account" })).toHaveAttribute("href", "/settings/account/profile");
  });
});
