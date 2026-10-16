import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, PROJECT, membership, myMembership, signedIn, whoAmI } from "~/test/fixtures";
import { memberUser } from "~/test/fixtures/public";
import { apiError, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { queryClient } from "~/query";
import { projectGate } from "./route-gates";

const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));
const BASE = `/orgs/${ORG}/projects/${PROJECT}`;

describe("the start page", () => {
  it("returns to the page kept before signing in again", async () => {
    signedIn();
    on("GET", "me/sessions", { data: [] });
    window.sessionStorage.setItem("kaeso:return", "/settings/account/sessions");
    await renderApp("/");
    await waitFor(() => expect(window.location.pathname).toBe("/settings/account/sessions"));
    expect(window.sessionStorage.getItem("kaeso:return")).toBeNull();
  });

  it("ignores a kept page on another site", async () => {
    signedIn();
    window.sessionStorage.setItem("kaeso:return", "//elsewhere.test/x");
    await renderApp("/");
    await waitFor(() => expect(window.location.pathname).toBe(`/orgs/${ORG}`));
  });

  it("opens the last project the person was in", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/limits", page([]));
    window.localStorage.setItem("kaeso:last", JSON.stringify({ org: ORG, project: PROJECT }));
    await renderApp("/");
    await waitFor(() => expect(window.location.pathname).toBe(BASE));
  });

  it("stays on the welcome page without an organization", async () => {
    signedIn({ session: whoAmI({ memberships: [] }), memberships: [] });
    await renderApp("/");
    expect(await screen.findByText("You're not in an organization yet")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/");
  });
});

describe("a project page the person can't open", () => {
  it("moves to the first page they can open and says who can give the right", async () => {
    signedIn({ projectRights: ["limits.view", "project.view"] });
    on("GET", "orgs/:org/projects/:project/limits", page([]));
    on("GET", "orgs/:org/projects/:project/rights/:right/grantors", { data: [memberUser("Grace Hopper")] });
    await renderApp(`${BASE}/keys`);
    await waitFor(() => expect(window.location.pathname).toBe(`${BASE}/limits`));
    expect(await (await notifications()).findByText("You need View all keys; ask Grace Hopper for it.")).toBeInTheDocument();
    expect(requests("GET", "orgs/:org/projects/:project/rights/:right/grantors").map((r) => r.params.right)).toContain("keys.all.view");
    expect(JSON.parse(window.localStorage.getItem("kaeso:last") ?? "null")).toEqual({ org: ORG, project: PROJECT });
  });

  it("moves from the overview without a notice", async () => {
    signedIn({ projectRights: ["limits.view"] });
    on("GET", "orgs/:org/projects/:project/limits", page([]));
    await renderApp(BASE);
    await waitFor(() => expect(window.location.pathname).toBe(`${BASE}/limits`));
    expect(requests("GET", "orgs/:org/projects/:project/rights/:right/grantors")).toHaveLength(0);
  });

  it("keeps only viewing in an organization scheduled for deletion", async () => {
    signedIn({
      session: whoAmI({ memberships: [membership({ org_state: "scheduled_for_deletion" })] }),
      memberships: [myMembership({ org_state: "scheduled_for_deletion" })],
    });
    on("GET", "orgs/:org/projects/:project/usage/span", { first: null, last: null });
    on("GET", "orgs/:org/projects/:project/usage", { data: [] });
    on("GET", "orgs/:org/projects/:project/models", { data: [] });
    on("GET", "orgs/:org/projects/:project/logs", page([]));
    await renderApp(`${BASE}/playground`);
    await waitFor(() => expect(window.location.pathname).toBe(BASE));
  });

  it("falls back to the organization's project rights when the project's own can't be read", async () => {
    signedIn({ session: whoAmI({ memberships: [membership({ project_rights: ["limits.view"] })] }) });
    on("GET", "orgs/:org/projects/:project/my-rights", apiError(404, "not_found", "No such project."));
    on("GET", "orgs/:org/projects/:project/limits", page([]));
    await renderApp(`${BASE}/keys`);
    await waitFor(() => expect(window.location.pathname).toBe(`${BASE}/limits`));
  });

  it("leaves a project in an organization the person isn't in", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/my-rights", apiError(404, "not_found", "No such project."));
    await renderApp(`/orgs/org_contoso/projects/${PROJECT}/keys`);
    expect(await (await notifications()).findByText("You don't belong to that organization")).toBeInTheDocument();
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
  });

  it("shows the page's error when the rights can't be read", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/my-rights", apiError(422, "invalid_request", "That project id is malformed."));
    await renderApp(`${BASE}/keys`);
    expect(await screen.findByText("This page couldn't load")).toBeInTheDocument();
    expect((await (await notifications()).findAllByText("That project id is malformed.")).length).toBeGreaterThan(0);
  });
});

describe("a project page that opens", () => {
  it("opens even when the organization's project list can't be read", async () => {
    signedIn();
    on("GET", "orgs/:org/projects", apiError(403, "permission_denied", "You can't list this organization's projects."));
    on("GET", "orgs/:org/projects/:project/limits", page([]));
    await renderApp(`${BASE}/limits`);
    expect(await screen.findByRole("heading", { name: "Limits", level: 1 })).toBeInTheDocument();
    expect(window.location.pathname).toBe(`${BASE}/limits`);
  });

  it("isn't remembered as the last place when it's only preloaded", async () => {
    signedIn();
    await projectGate({ context: { queryClient }, params: { org: ORG, project: PROJECT }, location: { pathname: `${BASE}/keys` }, preload: true });
    expect(window.localStorage.getItem("kaeso:last")).toBeNull();
    await projectGate({ context: { queryClient }, params: { org: ORG, project: PROJECT }, location: { pathname: `${BASE}/keys` }, preload: false });
    expect(JSON.parse(window.localStorage.getItem("kaeso:last") ?? "null")).toEqual({ org: ORG, project: PROJECT });
  });
});
