import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ORG, signedIn } from "~/test/fixtures";
import { credential, paged } from "~/test/fixtures/org";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const groups = () => requests("GET", "orgs/:org/usage").map((r) => r.query.get("group_by"));

function usageApi() {
  on("GET", "orgs/:org/usage/span", { first_day: "2026-09-01", last_day: "2026-10-15" });
  on("GET", "orgs/:org/usage", { data: [] });
}

describe("the organization overview address", () => {
  it("breaks usage down by member over the period it names", async () => {
    signedIn();
    usageApi();
    await renderApp(`/orgs/${ORG}?by=member&period=90d`);
    expect(await screen.findByRole("heading", { name: "Overview", level: 1 })).toBeInTheDocument();
    await waitFor(() => expect(groups()).toContain("member"));
    expect(screen.getAllByText("Last 90 days").length).toBeGreaterThan(0);
  });

  it("falls back to projects and the last 30 days for values it doesn't know", async () => {
    signedIn();
    usageApi();
    await renderApp(`/orgs/${ORG}?by=planet&period=bogus`);
    await waitFor(() => expect(groups()).toContain("project"));
    expect(groups()).not.toContain("planet");
    expect(screen.getAllByText("Last 30 days").length).toBeGreaterThan(0);
  });

  it("reads a custom range of days", async () => {
    signedIn();
    usageApi();
    await renderApp(`/orgs/${ORG}?from=2026-10-01&to=2026-10-07`);
    await waitFor(() =>
      expect(requests("GET", "orgs/:org/usage").map((r) => [r.query.get("from"), r.query.get("to")])).toContainEqual(["2026-10-01", "2026-10-07"]),
    );
  });
});

describe("the organization's projects and providers", () => {
  it("opens the create dialog from the address", async () => {
    signedIn();
    await renderApp(`/orgs/${ORG}/projects?create=true`);
    expect(await screen.findByRole("dialog", { name: "Create project" })).toBeInTheDocument();
  });

  it("lists the projects without a dialog otherwise", async () => {
    signedIn();
    await renderApp(`/orgs/${ORG}/projects?create=soon`);
    expect(await screen.findByRole("heading", { name: "Projects", level: 1 })).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Create project" })).not.toBeInTheDocument();
  });

  it("shows the provider keys", async () => {
    signedIn();
    on("GET", "orgs/:org/credentials", { data: [credential()] });
    on("GET", "orgs/:org/members", paged([]));
    on("GET", "orgs/:org/platform-keys", paged([]));
    await renderApp(`/orgs/${ORG}/providers`);
    expect(await screen.findByRole("heading", { name: "Providers", level: 1 })).toBeInTheDocument();
    expect(await screen.findByText("OpenAI production")).toBeInTheDocument();
  });
});
