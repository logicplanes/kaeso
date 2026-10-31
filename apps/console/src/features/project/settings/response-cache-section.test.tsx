import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { project, signedIn } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { logSettings, projectData, projectPath } from "~/test/fixtures/project";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import type { ProjectResponseCache } from "~/api/generated/types";

const CACHE = "orgs/:org/projects/:project/response-cache";
const CLEAR = "orgs/:org/projects/:project/response-cache/clear";

const cache = (over: Partial<ProjectResponseCache> = {}): ProjectResponseCache => ({
  enabled: false,
  ttl_seconds: 3600,
  org_allowed: true,
  available: true,
  effective: false,
  ...over,
});

const on_ = cache({ enabled: true, effective: true });

function setUp({ rights, settings = cache() }: { rights?: string[]; settings?: ProjectResponseCache } = {}) {
  signedIn({ ...(rights ? { projectRights: rights } : {}), projects: [project()] });
  projectData();
  on("GET", "orgs/:org/projects/:project/logs/settings", logSettings());
  on("GET", CACHE, settings);
}

describe("project response cache", () => {
  it("turns the cache on and then shows how long replies are kept", async () => {
    setUp();
    on("PATCH", CACHE, on_);
    const { user } = await renderApp(projectPath("settings"));

    const toggle = await screen.findByRole("switch", { name: "Cache replies" });
    expect(toggle).not.toBeChecked();
    expect(screen.getByText("Every request goes to the provider.")).toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: "Keep replies for" })).not.toBeInTheDocument();

    await user.click(toggle);
    expect(await notifications().findByText("Response cache turned on")).toBeInTheDocument();
    expect(requests("PATCH", CACHE).map((r) => r.body)).toEqual([{ enabled: true }]);
  });

  it("changes how long replies are kept and clears the cache", async () => {
    setUp({ settings: on_ });
    on("PATCH", CACHE, cache({ enabled: true, effective: true, ttl_seconds: 86400 }));
    on("POST", CLEAR, null);
    const { user } = await renderApp(projectPath("settings"));

    expect(await screen.findByText("Non-streamed requests sent twice get the stored reply.")).toBeInTheDocument();
    const ttl = screen.getByRole("combobox", { name: "Keep replies for" });
    expect(ttl).toHaveTextContent("1 hour");
    await user.click(ttl);
    await user.click(await screen.findByRole("option", { name: "1 day" }));
    expect(await notifications().findByText("Replies kept for 1 day")).toBeInTheDocument();
    expect(requests("PATCH", CACHE).map((r) => r.body)).toEqual([{ ttl_seconds: 86400 }]);

    await user.click(screen.getByRole("button", { name: "Clear cache" }));
    expect(await notifications().findByText("Response cache cleared")).toBeInTheDocument();
    expect(requests("POST", CLEAR)).toHaveLength(1);
  });

  it("offers a stored length that isn't a preset", async () => {
    setUp({ settings: cache({ enabled: true, effective: true, ttl_seconds: 120 }) });
    await renderApp(projectPath("settings"));
    expect(await screen.findByRole("combobox", { name: "Keep replies for" })).toHaveTextContent("2 minutes");
  });

  it("names a length set through the API in seconds", async () => {
    setUp({ settings: cache({ enabled: true, effective: true, ttl_seconds: 90 }) });
    await renderApp(projectPath("settings"));
    expect(await screen.findByRole("combobox", { name: "Keep replies for" })).toHaveTextContent("90 seconds");
  });

  it("can't be turned on while the organization has it off", async () => {
    setUp({ settings: cache({ org_allowed: false }) });
    const { user } = await renderApp(projectPath("settings"));
    const toggle = await screen.findByRole("switch", { name: "Cache replies" });
    expect(screen.getByText("Off for the organization; it can't be turned on here.")).toBeInTheDocument();
    await hoverReason(user, toggle, "Off for the organization");
  });

  it("says when the install doesn't offer it", async () => {
    setUp({ settings: cache({ available: false }) });
    const { user } = await renderApp(projectPath("settings"));
    const toggle = await screen.findByRole("switch", { name: "Cache replies" });
    expect(screen.getByText("This install doesn't offer the response cache.")).toBeInTheDocument();
    await hoverReason(user, toggle, "Off on this install");
  });

  it("is read-only without the right to edit the project", async () => {
    setUp({ rights: ["project.view"], settings: on_ });
    await renderApp(projectPath("settings"));
    const toggle = await screen.findByRole("switch", { name: "Cache replies" });
    expect(toggle).toBeChecked();
    expect(toggle).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("button", { name: "Clear cache" })).toHaveAttribute("aria-disabled", "true");
  });
});
