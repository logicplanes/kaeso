import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ORG, signedIn } from "~/test/fixtures";
import { on } from "~/test/network";
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

  it("stays away while no project uses one", async () => {
    signedIn();
    await renderApp(`/orgs/${ORG}`);
    expect(await screen.findByRole("heading", { name: "Overview" })).toBeInTheDocument();
    expect(screen.queryByText(/use retiring models/)).not.toBeInTheDocument();
  });
});
