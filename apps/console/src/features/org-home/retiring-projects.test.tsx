import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ORG, signedIn } from "~/test/fixtures";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

describe("the overview's retiring models", () => {
  it("lists the projects that use them, with their first shutdown", async () => {
    signedIn();
    on("GET", "orgs/:org/deprecations", {
      data: [
        { project_id: "proj_support", name: "Support bot", models: 2, retired: 0, next_shutdown_on: "2026-11-05" },
        { project_id: "proj_batch", name: "Batch jobs", models: 1, retired: 1, next_shutdown_on: null },
      ],
    });
    await renderApp(`/orgs/${ORG}`);
    const banner = (await screen.findByText("2 projects use retiring models")).closest("[role]") ?? document.body;
    expect(within(banner as HTMLElement).getByRole("link", { name: "Support bot" })).toHaveAttribute(
      "href",
      `/orgs/${ORG}/projects/proj_support/models?tab=retiring`,
    );
    expect(screen.getByText(/2 models, the first shuts down 5 Nov 2026/)).toBeInTheDocument();
    expect(screen.getByText(/1 model, 1 already retired/)).toBeInTheDocument();
  });

  it("says when no shutdown date is known yet, and counts the projects past the first five", async () => {
    signedIn();
    on("GET", "orgs/:org/deprecations", {
      data: Array.from({ length: 7 }, (_, i) => ({ project_id: `proj_${i}`, name: `Project ${i}`, models: 2, retired: 0, next_shutdown_on: null })),
    });
    await renderApp(`/orgs/${ORG}`);
    const banner = (await screen.findByText("7 projects use retiring models")).closest("[role]") as HTMLElement;
    expect(within(banner).getAllByText(/2 models, no shutdown date yet/)).toHaveLength(5);
    expect(within(banner).queryByText(/already retired/)).not.toBeInTheDocument();
    expect(within(banner).queryByRole("link", { name: "Project 5" })).not.toBeInTheDocument();
    expect(within(banner).getByText("2 more projects")).toBeInTheDocument();
  });

  it("stays away while no project uses one", async () => {
    signedIn();
    await renderApp(`/orgs/${ORG}`);
    expect(await screen.findByRole("heading", { name: "Overview" })).toBeInTheDocument();
    expect(screen.queryByText(/use retiring models/)).not.toBeInTheDocument();
  });

  it("stays away while they can't load", async () => {
    signedIn();
    on("GET", "orgs/:org/deprecations", () => apiError(503, "unavailable", "Try again in a minute."));
    await renderApp(`/orgs/${ORG}`);
    expect(await screen.findByRole("heading", { name: "Overview" })).toBeInTheDocument();
    await waitFor(() => expect(requests("GET", "orgs/:org/deprecations").length).toBeGreaterThan(0));
    expect(screen.queryByText(/use retiring models/)).not.toBeInTheDocument();
  });
});
