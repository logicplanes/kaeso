import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { signedIn } from "~/test/fixtures";
import { notifications } from "~/test/fixtures/org";
import { listed, projectData, projectPath } from "~/test/fixtures/project";
import { apiError, on } from "~/test/network";
import { renderApp } from "~/test/render";

const refused = () => apiError(409, "conflict", "The project is being moved; try again.");

describe("a project page whose list couldn't load", () => {
  it.each([
    { page: "keys", path: "orgs/:org/projects/:project/keys", failed: "The keys couldn't load", empty: "No keys yet" },
    { page: "members", path: "orgs/:org/projects/:project/members", failed: "The members couldn't load", empty: "No members" },
    {
      page: "service-accounts",
      path: "orgs/:org/projects/:project/service-accounts",
      failed: "The service accounts couldn't load",
      empty: "No service accounts yet",
    },
    { page: "limits", path: "orgs/:org/projects/:project/limits", failed: "The limits couldn't load", empty: "No limits yet" },
  ])("says the $page couldn't load instead of showing an empty list", async ({ page, path, failed, empty }) => {
    signedIn();
    projectData();
    on("GET", path, refused);
    await renderApp(projectPath(page));
    expect(await screen.findByRole("heading", { name: failed })).toBeInTheDocument();
    expect(screen.getByText("The notification says why; try again from there.")).toBeInTheDocument();
    expect(screen.queryByText(empty)).not.toBeInTheDocument();
    expect((await notifications().findAllByText("The project is being moved; try again.")).length).toBeGreaterThan(0);
  });

  it("says on the overview when the limits or the latest requests couldn't load", async () => {
    signedIn();
    projectData();
    overviewUsage();
    on("GET", "orgs/:org/projects/:project/limits", refused);
    on("GET", "orgs/:org/projects/:project/logs", refused);
    on("GET", "orgs/:org/projects/:project/keys", listed([]));
    await renderApp(projectPath());
    expect(await screen.findByText("The limits couldn't load; the notification says why.")).toBeInTheDocument();
    expect(await screen.findByText("The latest requests couldn't load; the notification says why.")).toBeInTheDocument();
    expect(screen.queryByText("No project limits. Keys and members can still have their own.")).not.toBeInTheDocument();
    expect(screen.queryByText("No requests yet; they show here as they arrive")).not.toBeInTheDocument();
  });

  it("says on the overview why the latest requests are missing when request logs are unavailable, where no notification does", async () => {
    signedIn();
    projectData();
    overviewUsage();
    on("GET", "orgs/:org/projects/:project/logs", apiError(503, "unavailable", "Request logs are unavailable right now; try again shortly."));
    on("GET", "orgs/:org/projects/:project/keys", listed([]));
    await renderApp(projectPath());
    expect(await screen.findByText("Request logs are unavailable right now; try again shortly.")).toBeInTheDocument();
    expect(screen.queryByText("The latest requests couldn't load; the notification says why.")).not.toBeInTheDocument();
    expect(screen.getByText("No project limits. Keys and members can still have their own.")).toBeInTheDocument();
    expect(notifications().queryByText("Request logs are unavailable right now; try again shortly.")).not.toBeInTheDocument();
  });

  it("shows each part of the overview as soon as it has loaded, without waiting for the others", async () => {
    signedIn();
    projectData();
    overviewUsage();
    let answer: (rows: unknown) => void = () => {};
    on("GET", "orgs/:org/projects/:project/logs", () => new Promise((resolve) => (answer = resolve)));
    on("GET", "orgs/:org/projects/:project/keys", listed([]));
    await renderApp(projectPath());
    expect(await screen.findByText("No project limits. Keys and members can still have their own.")).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Latest requests" })).toHaveAttribute("aria-busy", "true");

    answer(listed([]));
    expect(await screen.findByText("No requests yet; they show here as they arrive")).toBeInTheDocument();
  });
});

function overviewUsage() {
  on("GET", "orgs/:org/projects/:project/usage/span", { first_day: "2026-09-01", last_day: "2026-10-14" });
  on("GET", "orgs/:org/projects/:project/usage", { data: [] });
}
