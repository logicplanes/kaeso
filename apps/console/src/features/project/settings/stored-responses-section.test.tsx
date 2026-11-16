import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { project, signedIn } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { logSettings, projectData, projectPath } from "~/test/fixtures/project";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import type { ProjectStoredResponses } from "~/api/generated/types";

const STORED = "orgs/:org/projects/:project/stored-responses";
const NAME = "Allow responses stored at the provider";

const settings = (over: Partial<ProjectStoredResponses> = {}): ProjectStoredResponses => ({
  enabled: false,
  org_allowed: true,
  effective: false,
  ...over,
});

function setUp({ rights, current = settings(), archived = false }: { rights?: string[]; current?: ProjectStoredResponses; archived?: boolean } = {}) {
  signedIn({ ...(rights ? { projectRights: rights } : {}), projects: [project(archived ? { archived_at: "2026-11-01T00:00:00Z" } : {})] });
  projectData();
  on("GET", "orgs/:org/projects/:project/logs/settings", logSettings());
  on("GET", STORED, current);
}

describe("project stored responses", () => {
  it("turns stored responses on and off", async () => {
    setUp();
    let current = settings();
    on("GET", STORED, () => current);
    on("PATCH", STORED, ({ body }: { body: { enabled: boolean } }) => (current = settings({ enabled: body.enabled, effective: body.enabled })));
    const { user } = await renderApp(projectPath("settings"));

    const toggle = await screen.findByRole("switch", { name: NAME });
    expect(await screen.findByText("Apps must send each conversation whole.")).toBeInTheDocument();
    await user.click(toggle);
    expect(await notifications().findByText("Stored responses turned on")).toBeInTheDocument();
    expect(await screen.findByText("Apps may use store and previous_response_id.")).toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: NAME }));
    expect(await notifications().findByText("Stored responses turned off")).toBeInTheDocument();
    expect(requests("PATCH", STORED).map((r) => r.body)).toEqual([{ enabled: true }, { enabled: false }]);
  });

  it("says when the organization keeps it off", async () => {
    setUp({ current: settings({ org_allowed: false }) });
    const { user } = await renderApp(projectPath("settings"));
    const toggle = await screen.findByRole("switch", { name: NAME });
    expect(await screen.findByText("Off for the organization; it can't be turned on here.")).toBeInTheDocument();
    await hoverReason(user, toggle, "Off for the organization");
  });

  it("can't change in an archived project", async () => {
    setUp({ archived: true });
    const { user } = await renderApp(projectPath("settings"));
    await hoverReason(user, await screen.findByRole("switch", { name: NAME }), "An archived project can't change");
  });

  it("can't change without the right to edit the project", async () => {
    setUp({ rights: ["project.view"], current: settings({ enabled: true, effective: true }) });
    await renderApp(projectPath("settings"));
    const toggle = await screen.findByRole("switch", { name: NAME });
    await waitFor(() => expect(toggle).toBeChecked());
    expect(toggle).toHaveAttribute("aria-disabled", "true");
  });
});
