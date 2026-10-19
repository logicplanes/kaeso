import { screen, within } from "@testing-library/react";
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

  it("says on the overview when the limits or the recent errors couldn't load", async () => {
    signedIn();
    projectData();
    on("GET", "orgs/:org/projects/:project/usage/span", { first_day: "2026-09-01", last_day: "2026-10-14" });
    on("GET", "orgs/:org/projects/:project/usage", { data: [] });
    on("GET", "orgs/:org/projects/:project/limits", refused);
    on("GET", "orgs/:org/projects/:project/logs", refused);
    on("GET", "orgs/:org/projects/:project/keys", listed([]));
    await renderApp(projectPath("?from=2026-10-01&to=2026-10-14"));
    const aside = (await screen.findByText("The limits couldn't load; the notification says why.")).closest("aside") ?? document.body;
    expect(within(aside).getByText("The recent errors couldn't load; the notification says why.")).toBeInTheDocument();
    expect(within(aside).queryByText("No project limits. Keys and members can still have their own.")).not.toBeInTheDocument();
    expect(within(aside).queryByText("No failed requests.")).not.toBeInTheDocument();
  });
});
