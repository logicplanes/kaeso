import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ORG, PROJECT, project, signedIn } from "~/test/fixtures";
import { notifications } from "~/test/fixtures/org";
import { apiError, on, page } from "~/test/network";
import { renderApp } from "~/test/render";

const LIMITS = `/orgs/${ORG}/projects/${PROJECT}/limits`;

describe("a page that fails to load", () => {
  it("says what doesn't exist and links to the start", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project", apiError(404, "not_found", "No project proj_checkout in this organization."));
    await renderApp(LIMITS);
    expect(await screen.findByRole("heading", { name: "Not found", level: 1 })).toBeInTheDocument();
    expect(screen.getByText("This doesn't exist or was removed")).toBeInTheDocument();
    expect(screen.getByText("No project proj_checkout in this organization.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Go to the start page" })).toHaveAttribute("href", "/");
    expect(notifications().queryByText("No project proj_checkout in this organization.")).not.toBeInTheDocument();
  });

  it("says when the person has no access", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project", apiError(403, "permission_denied", "You are not a member of this project."));
    await renderApp(LIMITS);
    expect(await screen.findByRole("heading", { name: "No access", level: 1 })).toBeInTheDocument();
    expect(screen.getByText("You don't have access to this page")).toBeInTheDocument();
    expect(within(screen.getByRole("main")).getByText("You are not a member of this project.")).toBeInTheDocument();
    expect(notifications().queryByText("You are not a member of this project.")).not.toBeInTheDocument();
  });

  it("shows the API's message and loads the page on a new try", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project", apiError(400, "invalid_request", "The project id is malformed."));
    const { user } = await renderApp(LIMITS);
    expect(await screen.findByRole("heading", { name: "Couldn't load this page", level: 1 })).toBeInTheDocument();
    expect(screen.getByText("Try again in a moment.")).toBeInTheDocument();
    expect(await notifications().findAllByText("The project id is malformed.")).not.toHaveLength(0);
    on("GET", "orgs/:org/projects/:project", project());
    on("GET", "orgs/:org/projects/:project/limits", page([]));
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("heading", { name: "Limits", level: 1 })).toBeInTheDocument();
  });

  it("says when the page itself broke", async () => {
    signedIn();
    on("GET", "orgs/:org/projects", ({ query }: { query: URLSearchParams }) =>
      query.get("include_archived") === "true" ? { data: [null] } : page([project()]),
    );
    await renderApp(`/orgs/${ORG}`);
    expect(await screen.findByRole("heading", { name: "Couldn't load this page", level: 1 })).toBeInTheDocument();
    await waitFor(() => expect(notifications().getAllByText("This page hit an unexpected error; reload to try again").length).toBeGreaterThan(0));
  });
});
