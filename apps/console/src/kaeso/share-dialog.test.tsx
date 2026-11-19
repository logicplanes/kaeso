import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ALL_ORG_RIGHTS, ORG, USER, signedIn, withRights } from "~/test/fixtures";
import { hoverReason } from "~/test/fixtures/org";
import { projectData, projectPath } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { member, membersData } from "~/test/fixtures/org";
import { lastDays } from "./share";
import { defaultTitle } from "./share-dialog";

const PERIOD = "from=2026-10-01&to=2026-10-14";
const LINKS = "orgs/:org/shared-links";
const URL = "https://kaeso.example.com/share/9fQ2kX7mVb3pL8rT1wZ5yH4uJ6sD0gA2cE7nM1qR3tU";

function usageApi() {
  on("GET", "orgs/:org/projects/:project/usage/span", { first_day: "2026-09-01", last_day: "2026-10-14" });
  on("GET", "orgs/:org/projects/:project/usage", { data: [] });
  on("GET", "orgs/:org/projects/:project/usage/timeline", { data: [] });
}

function policy(allow: string, max_days: number | null) {
  on("GET", "orgs/:org/security", {
    require_2fa: false,
    require_sso: false,
    sso: null,
    sso_default_role: null,
    sso_redirect_url: "https://console.northwind.test/sso/callback",
    max_key_lifetime_days: null,
    shared_links: { allow, max_days },
  });
}

const created = (body: Record<string, unknown>) => ({
  link: {
    id: "shl_1",
    kind: body["kind"],
    title: body["title"],
    project: body["project"] ?? null,
    member: null,
    live: body["live"],
    days: body["days"] ?? null,
    from: null,
    to: null,
    metric: "requests",
    show_costs: body["show_costs"],
    hide_members: body["hide_members"],
    hide_keys: body["hide_keys"],
    hide_projects: body["hide_projects"],
    audience: body["audience"],
    created_by: "usr_ada",
    created_at: "2026-10-14T09:00:00Z",
    expires_at: null,
    views: 0,
    last_opened_at: null,
  },
  url: URL,
});

describe("sharing a page as a read-only link", () => {
  it("makes a live link to the project's analytics and shows its address once", async () => {
    signedIn();
    projectData();
    usageApi();
    policy("anyone", null);
    on("POST", LINKS, ({ body }: { body: Record<string, unknown> }) => created(body));
    const { user } = await renderApp(projectPath(`analytics?${PERIOD}`));

    await user.click(await screen.findByRole("button", { name: "Share" }));
    const dialog = await screen.findByRole("dialog", { name: "Share a read-only link" });
    expect(within(dialog).getByRole("textbox", { name: "Title" })).toHaveValue("Checkout assistant usage");
    await user.click(within(dialog).getByRole("switch", { name: "Show costs" }));
    await user.click(within(dialog).getByRole("checkbox", { name: "Key names" }));
    await user.click(within(dialog).getByRole("checkbox", { name: "The project's name" }));
    expect(within(dialog).getByRole("textbox", { name: "Title" })).toHaveValue("Usage");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("combobox", { name: "Who" }));
    await user.click(await screen.findByRole("option", { name: /^Anyone with the link/ }));
    await user.click(within(dialog).getByRole("combobox", { name: "Expires" }));
    await user.click(await screen.findByRole("option", { name: "Never" }));
    await user.click(within(dialog).getByRole("button", { name: "Make link" }));

    expect(await within(dialog).findByText(URL)).toBeInTheDocument();
    expect(within(dialog).getByText("Copy the link now; Kaeso shows it only once and keeps only its hash.")).toBeInTheDocument();
    expect(requests("POST", LINKS).map((r) => r.body)).toEqual([
      {
        kind: "analytics",
        project: "proj_checkout",
        title: "Usage",
        live: true,
        days: 14,
        show_costs: true,
        hide_members: false,
        hide_keys: true,
        hide_projects: true,
        audience: "anyone",
      },
    ]);
  });

  it("offers members only and the organization's longest expiry when that's all it allows", async () => {
    signedIn();
    projectData();
    usageApi();
    policy("members", 7);
    on("POST", LINKS, ({ body }: { body: Record<string, unknown> }) => created(body));
    const { user } = await renderApp(projectPath(`analytics?${PERIOD}`));

    await user.click(await screen.findByRole("button", { name: "Share" }));
    const dialog = await screen.findByRole("dialog", { name: "Share a read-only link" });
    await user.click(within(dialog).getByRole("combobox", { name: "Numbers" }));
    await user.click(await screen.findByRole("option", { name: "Snapshot: this period, as it is now" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("combobox", { name: "Who" }));
    expect(await screen.findByRole("option", { name: /^Anyone with the link/ })).toHaveAttribute("aria-disabled", "true");
    await user.keyboard("{Escape}");
    await user.click(within(dialog).getByRole("combobox", { name: "Expires" }));
    expect((await screen.findAllByRole("option")).map((o) => o.textContent)).toEqual(["After 1 day", "After 7 days"]);
    await user.click(await screen.findByRole("option", { name: "After 1 day" }));
    await user.click(within(dialog).getByRole("button", { name: "Make link" }));
    await within(dialog).findByText(URL);
    expect(requests("POST", LINKS)[0]?.body).toMatchObject({ live: false, from: "2026-10-01", to: "2026-10-14", audience: "members", expires_in_days: 1 });
  });

  it("closes without making a link, and needs a title to go on", async () => {
    signedIn();
    projectData();
    usageApi();
    policy("members", 90);
    const { user } = await renderApp(projectPath(`analytics?${PERIOD}`));
    await user.click(await screen.findByRole("button", { name: "Share" }));
    const dialog = await screen.findByRole("dialog", { name: "Share a read-only link" });
    await user.clear(within(dialog).getByRole("textbox", { name: "Title" }));
    expect(within(dialog).getByRole("button", { name: "Continue" })).toHaveAttribute("aria-disabled", "true");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Share a read-only link" })).not.toBeInTheDocument());
    expect(requests("POST", LINKS)).toHaveLength(0);
  });

  it("can't share without the right or while the organization allows no links", async () => {
    withRights(ALL_ORG_RIGHTS.filter((r) => r !== "usage.share"));
    projectData();
    usageApi();
    const { user } = await renderApp(projectPath(`analytics?${PERIOD}`));
    const share = await screen.findByRole("button", { name: "Share" });
    await waitFor(() => expect(share).toHaveAttribute("aria-disabled", "true"));
    await hoverReason(user, share, /Share usage/);
  });

  it("says why a link can't be shared while links are off", async () => {
    signedIn();
    projectData();
    usageApi();
    policy("off", 90);
    const { user } = await renderApp(projectPath(`analytics?${PERIOD}`));
    const share = await screen.findByRole("button", { name: "Share" });
    await waitFor(() => expect(share).toHaveAttribute("aria-disabled", "true"));
    await hoverReason(user, share, "This organization doesn't allow shared links; ask one of its admins");
  });
});

describe("sharing from other pages", () => {
  async function shareFrom(path: string) {
    on("POST", LINKS, ({ body }: { body: Record<string, unknown> }) => created(body));
    const { user } = await renderApp(path);
    await user.click(await screen.findByRole("button", { name: "Share" }));
    const dialog = await screen.findByRole("dialog", { name: "Share a read-only link" });
    return { user, dialog };
  }

  it("shares the organization's analytics under its name", async () => {
    signedIn();
    on("GET", "orgs/:org/usage", { data: [] });
    on("GET", "orgs/:org/usage/span", { first_day: "2026-09-01", last_day: "2026-10-14" });
    on("GET", "orgs/:org/usage/timeline", { data: [] });
    policy("members", 90);
    const { user, dialog } = await shareFrom(`/orgs/${ORG}/analytics?${PERIOD}`);
    expect(within(dialog).getByRole("textbox", { name: "Title" })).toHaveValue("Northwind Labs usage");
    expect(within(dialog).getByRole("checkbox", { name: "Project names" })).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("button", { name: "Make link" }));
    await within(dialog).findByText(URL);
    expect(requests("POST", LINKS)[0]?.body).not.toHaveProperty("project");
  });

  it("shares a project's budgets for the day", async () => {
    signedIn();
    projectData();
    on("GET", "orgs/:org/projects/:project/limits", { data: [], has_more: false, next_cursor: null });
    policy("members", 90);
    const { user, dialog } = await shareFrom(projectPath("limits"));
    expect(within(dialog).getByRole("textbox", { name: "Title" })).toHaveValue("Checkout assistant budgets");
    expect(within(dialog).queryByRole("combobox", { name: "Numbers" })).not.toBeInTheDocument();
    expect(within(dialog).getByRole("checkbox", { name: "Key names" })).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("button", { name: "Make link" }));
    await within(dialog).findByText(URL);
    expect(requests("POST", LINKS)[0]?.body).toMatchObject({ kind: "budget", project: "proj_checkout", days: 1, expires_in_days: 30 });
  });

  it("leaves the name out of the title when it can't be read", async () => {
    signedIn();
    projectData();
    on("GET", "orgs/:org/projects/:project/limits", { data: [], has_more: false, next_cursor: null });
    on("GET", "orgs/:org/projects", apiError(503, "unavailable", "The gateway failed."));
    policy("members", 90);
    const { dialog } = await shareFrom(projectPath("limits"));
    expect(within(dialog).getByRole("textbox", { name: "Title" })).toHaveValue("Budgets");
  });

  it("shares a project's summary of the last 30 days without names of members or keys to hide", async () => {
    signedIn();
    projectData();
    on("GET", "orgs/:org/projects/:project/usage", { data: [] });
    on("GET", "orgs/:org/projects/:project/usage/timeline", { data: [] });
    policy("members", 90);
    const { dialog } = await shareFrom(projectPath(""));
    expect(within(dialog).getByRole("textbox", { name: "Title" })).toHaveValue("Checkout assistant summary");
    expect(within(dialog).queryByRole("checkbox", { name: "Member names" })).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("checkbox", { name: "Key names" })).not.toBeInTheDocument();
    expect(within(dialog).getByRole("checkbox", { name: "The project's name" })).toBeInTheDocument();
  });

  it("shares one's own stats, named until member names are hidden", async () => {
    signedIn();
    membersData([member()]);
    policy("members", 90);
    const { user, dialog } = await shareFrom(`/settings/orgs/${ORG}/members?member=${USER}`);
    const title = within(dialog).getByRole("textbox", { name: "Title" });
    expect(title).toHaveValue("Ada Okafor's usage");
    expect(within(dialog).getByRole("checkbox", { name: "Project names" })).toBeInTheDocument();
    await user.click(within(dialog).getByRole("checkbox", { name: "Member names" }));
    expect(title).toHaveValue("A member's usage");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("button", { name: "Make link" }));
    await within(dialog).findByText(URL);
    expect(requests("POST", LINKS)[0]?.body).toMatchObject({ kind: "member", member: USER, title: "A member's usage", hide_members: true, days: 30 });
  });
});

describe("defaultTitle", () => {
  it("names what the link shows, or only its kind when the name is hidden", () => {
    expect(defaultTitle("budget", "Checkout", true)).toBe("Budgets");
    expect(defaultTitle("project_card", "Checkout", true)).toBe("Project summary");
    expect(defaultTitle("analytics", "Northwind", false)).toBe("Northwind usage");
  });
});

describe("lastDays", () => {
  it("ends today, UTC, and counts today", () => {
    expect(lastDays(30, new Date("2026-10-14T23:30:00Z"))).toEqual({ from: "2026-09-15", to: "2026-10-14" });
    expect(lastDays(1, new Date("2026-10-14T00:00:00Z"))).toEqual({ from: "2026-10-14", to: "2026-10-14" });
  });
});
