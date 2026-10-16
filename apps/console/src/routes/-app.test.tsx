import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ALL_PROJECT_RIGHTS, ORG, PROJECT, project, signedIn, signedOut, whoAmI } from "~/test/fixtures";
import { methods } from "~/test/fixtures/public";
import { listed } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const notifications = () => within(screen.getByRole("region", { name: "Notifications" }));
const last = () => JSON.parse(window.localStorage.getItem("kaeso:last") ?? "null") as unknown;

describe("opening the app", () => {
  it("sends a person without a session to sign in, and back afterwards", async () => {
    signedOut();
    on("GET", "auth/methods", methods());
    const { router } = await renderApp(`/orgs/${ORG}/projects?create=true`);
    await waitFor(() => expect(router.state.location.pathname).toBe("/sign-in"));
    expect(router.state.location.search).toEqual({ next: `/orgs/${ORG}/projects?create=true` });
    expect(await screen.findByRole("heading", { name: "Sign in", level: 1 })).toBeInTheDocument();
  });

  it("says the gateway isn't answering when the session can't load", async () => {
    on("GET", "auth/session", apiError(429, "rate_limited", "Too many requests; try again in a minute."));
    await renderApp("/settings/account/appearance");
    expect(await screen.findByRole("heading", { name: "Kaeso isn't answering" })).toBeInTheDocument();
    expect((await notifications().findAllByText("Too many requests; try again in a minute.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Retry now" })).toBeInTheDocument();
  });

  it("opens the person's default project from the start page", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/usage/span", { first_day: null, last_day: null });
    on("GET", "orgs/:org/projects/:project/usage", { data: [] });
    on("GET", "orgs/:org/projects/:project/models", { data: [] });
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    window.localStorage.setItem("kaeso:last", JSON.stringify({ org: ORG, project: PROJECT }));
    const { router } = await renderApp("/");
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}`));
    expect(await screen.findByRole("heading", { name: "Overview", level: 1 })).toBeInTheDocument();
  });

  it("welcomes a person who is in no organization", async () => {
    signedIn({ session: whoAmI({ memberships: [] }), memberships: [] });
    const { router } = await renderApp("/");
    expect(await screen.findByRole("heading", { name: "Welcome to Kaeso", level: 1 })).toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/");
    expect(screen.getByRole("button", { name: "Create organization" })).toBeInTheDocument();
  });

  it("says when a page doesn't exist", async () => {
    signedIn();
    await renderApp("/no/such/page");
    expect(await screen.findByRole("heading", { name: "Not found", level: 1 })).toBeInTheDocument();
  });
});

describe("remembering where the person was", () => {
  it("remembers the organization from its pages", async () => {
    signedIn();
    await renderApp(`/orgs/${ORG}/projects`);
    await screen.findByRole("heading", { name: "Projects", level: 1 });
    expect(last()).toEqual({ org: ORG });
  });

  it("remembers the project from its pages", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/logs/settings", { content_logging: true });
    await renderApp(`/orgs/${ORG}/projects/${PROJECT}/settings`);
    await screen.findByRole("heading", { name: "Settings", level: 1 });
    expect(last()).toEqual({ org: ORG, project: PROJECT });
  });
});

describe("a project that can't open", () => {
  it("says it doesn't exist with the API's message", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project", apiError(404, "not_found", "No such project, or you cannot see it."));
    await renderApp(`/orgs/${ORG}/projects/proj_gone/settings`);
    expect(await screen.findByText("This doesn't exist or was removed.")).toBeInTheDocument();
    expect(screen.getByText("No such project, or you cannot see it.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Go to the start page" })).toHaveAttribute("href", "/");
  });

  it("says the person has no access", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project", apiError(403, "permission_denied", "You are not a member of this project."));
    await renderApp(`/orgs/${ORG}/projects/${PROJECT}/settings`);
    expect(await screen.findByText("You don't have access to this page.")).toBeInTheDocument();
    expect(within(screen.getByRole("main")).getByText("You are not a member of this project.")).toBeInTheDocument();
  });

  it("offers to try again after a failure, and loads once it works", async () => {
    signedIn();
    let broken = true;
    on("GET", "orgs/:org/projects/:project", () => (broken ? apiError(400, "invalid_request", "The project is being moved; try again.") : project()));
    on("GET", "orgs/:org/projects/:project/logs/settings", { content_logging: true });
    const { user } = await renderApp(`/orgs/${ORG}/projects/${PROJECT}/settings`);
    expect(await screen.findByText("This page couldn't load")).toBeInTheDocument();
    expect((await notifications().findAllByText("The project is being moved; try again.")).length).toBeGreaterThan(0);
    broken = false;
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("heading", { name: "Settings", level: 1 })).toBeInTheDocument();
  });

  it("moves a person to a page they hold rights for", async () => {
    signedIn({ projectRights: ALL_PROJECT_RIGHTS.filter((r) => r !== "keys.all.view") });
    on("GET", "orgs/:org/projects/:project/usage/span", { first_day: null, last_day: null });
    on("GET", "orgs/:org/projects/:project/usage", { data: [] });
    on("GET", "orgs/:org/projects/:project/models", { data: [] });
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    on("GET", "orgs/:org/projects/:project/rights/:right/grantors", { data: [] });
    const { router } = await renderApp(`/orgs/${ORG}/projects/${PROJECT}/keys`);
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}`));
    expect(requests("GET", "orgs/:org/projects/:project/keys")).toHaveLength(0);
  });
});
