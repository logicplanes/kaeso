import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { project, signedIn } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { logSettings, projectData, projectPath } from "~/test/fixtures/project";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import type { ModelLifecycleSettings } from "~/api/generated/types";

const LIFECYCLE = "orgs/:org/projects/:project/model-lifecycle";
const MOVE = "Move aliases on shutdown";
const SERVE = "Answer retired models with the replacement";

function setUp({ rights, archived = false }: { rights?: string[]; archived?: boolean } = {}) {
  signedIn({ ...(rights ? { projectRights: rights } : {}), projects: [project(archived ? { archived_at: "2026-11-01T00:00:00Z" } : {})] });
  projectData();
  on("GET", "orgs/:org/projects/:project/logs/settings", logSettings());
}

describe("model retirement settings", () => {
  it("turn each switch on and off", async () => {
    setUp();
    let current: ModelLifecycleSettings = { move_on_shutdown: false, serve_replacement: false };
    on("GET", LIFECYCLE, () => current);
    on("PUT", LIFECYCLE, ({ body }: { body: ModelLifecycleSettings }) => (current = body));
    const { user } = await renderApp(projectPath("settings"));
    expect(await screen.findByText("Aliases keep the model; move them yourself.")).toBeInTheDocument();
    expect(screen.getByText("Requests to a retired model fail with 410.")).toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: MOVE }));
    expect(await notifications().findByText("Model retirement settings saved")).toBeInTheDocument();
    expect(await screen.findByText("On the shutdown day, aliases move to the replacement.")).toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: SERVE }));
    expect(await screen.findByText("Apps that name a retired model get its replacement.")).toBeInTheDocument();
    expect(requests("PUT", LIFECYCLE).map((r) => r.body)).toEqual([
      { move_on_shutdown: true, serve_replacement: false },
      { move_on_shutdown: true, serve_replacement: true },
    ]);
  });

  it("show the switch being saved as busy", async () => {
    setUp();
    on("GET", LIFECYCLE, { move_on_shutdown: false, serve_replacement: false });
    on("PUT", LIFECYCLE, () => new Promise(() => undefined));
    const { user } = await renderApp(projectPath("settings"));
    const move = await screen.findByRole("switch", { name: MOVE });
    await user.click(move);
    await waitFor(() => expect(move).toHaveAttribute("aria-busy", "true"));
    expect(screen.getByRole("switch", { name: SERVE })).not.toHaveAttribute("aria-busy");
  });

  it("can't change in an archived project", async () => {
    setUp({ archived: true });
    const { user } = await renderApp(projectPath("settings"));
    await hoverReason(user, await screen.findByRole("switch", { name: MOVE }), "An archived project can't change");
  });

  it("can't change without the right to manage models", async () => {
    setUp({ rights: ["project.view", "models.view"] });
    await renderApp(projectPath("settings"));
    const toggle = await screen.findByRole("switch", { name: SERVE });
    await waitFor(() => expect(toggle).toHaveAttribute("aria-disabled", "true"));
  });

  it("is left out without the right to see the models", async () => {
    setUp({ rights: ["project.view"] });
    await renderApp(projectPath("settings"));
    expect(await screen.findByRole("switch", { name: "Allow responses stored at the provider" })).toBeInTheDocument();
    expect(screen.queryByRole("switch", { name: MOVE })).not.toBeInTheDocument();
  });
});
