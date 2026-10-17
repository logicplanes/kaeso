import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { InstanceOrgUsage, InstanceOrgView, MemberView, ProjectView } from "~/api/generated/types";
import { ORG, USER, project, signedIn, whoAmI } from "~/test/fixtures";
import { instanceOrg, instanceUser, member, orgUsage } from "~/test/fixtures/instance-account";
import { hoverReason, notifications, paged } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { atFixtureTime } from "~/test/clock";

atFixtureTime();

const OTHER = "org_contoso";
const path = (org = ORG) => `/settings/instance/orgs/${org}`;
const ADA = member({ id: USER, name: "Ada Okafor", email: "owner@example.com", role: "owner" });
const GRACE = member();
const LINUS = member({ id: "usr_linus", name: "Linus Park", email: null, state: "deactivated" });

interface OrgData {
  view?: InstanceOrgView;
  usage?: InstanceOrgUsage;
  members?: MemberView[];
  projects?: ProjectView[];
}

function orgData({ view = instanceOrg(), usage = orgUsage(), members = [ADA, GRACE, LINUS], projects = [] }: OrgData = {}) {
  on("GET", "instance/orgs/:org", view);
  on("GET", "instance/orgs/:org/usage", usage);
  on("GET", "instance/orgs/:org/members", paged(members));
  on("GET", "instance/orgs/:org/projects", paged(projects));
}

describe("an organization seen from the instance", () => {
  it("shows its usage, facts, members and projects", async () => {
    signedIn();
    orgData({
      members: [ADA, GRACE, LINUS],
      projects: [
        project(),
        project({ id: "proj_old", name: "Old pilot", is_default: false, archived_at: "2026-09-01T12:00:00Z" }),
        project({ id: "proj_new", name: "Search", is_default: false }),
      ],
      view: instanceOrg({ members: 3, projects: 3 }),
    });
    await renderApp(path());
    expect(await screen.findByRole("heading", { name: "Northwind Labs", level: 1 })).toBeInTheDocument();
    const usage = screen.getByRole("list", { name: "Usage in the last 30 days" });
    expect(await within(usage).findByText("3,000")).toBeInTheDocument();
    expect(within(usage).getByText("100 a day on average")).toBeInTheDocument();
    expect(within(usage).getByText("1.5M")).toBeInTheDocument();
    expect(within(usage).getByText("500 per request")).toBeInTheDocument();
    expect(within(usage).getByText("$12.50")).toBeInTheDocument();
    expect(within(usage).getByText("$0.0042 per request")).toBeInTheDocument();
    expect(within(usage).getByText("5.0 MB")).toBeInTheDocument();
    expect(within(usage).getByText("3,000 entries")).toBeInTheDocument();
    expect(screen.getByText(ORG)).toBeInTheDocument();
    expect(screen.getByText("None")).toBeInTheDocument();
    expect(screen.getByText("With content")).toBeInTheDocument();
    expect(screen.getByText("Metadata kept 90 days, content 30 days.")).toBeInTheDocument();
    expect(screen.getByText("1 owner")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Open organization" })).toHaveAttribute("href", `/orgs/${ORG}`);
    const members = await screen.findByRole("grid", { name: "Members" });
    expect(within(members).getByText("Grace Hopper")).toBeInTheDocument();
    expect(within(members).getByText("grace@example.com")).toBeInTheDocument();
    expect(within(members).getByText("Owner")).toBeInTheDocument();
    expect(within(members).getAllByText("Member")).toHaveLength(2);
    expect(within(members).getByText("Deactivated")).toBeInTheDocument();
    expect(screen.getByText("3 members, with their role and the day they joined")).toBeInTheDocument();
    const projects = await screen.findByRole("grid", { name: "Projects" });
    expect(within(projects).getByText("Default")).toBeInTheDocument();
    expect(within(projects).getByText("Archived")).toBeInTheDocument();
    expect(within(projects).getByText("Search")).toBeInTheDocument();
    expect(screen.getByText("3 projects, with the day each was created")).toBeInTheDocument();
  });

  it("shows the pictures of members, projects and users", async () => {
    signedIn();
    orgData({
      members: [{ ...GRACE, user: { ...GRACE.user, avatar_url: "/avatars/grace.webp" } }],
      projects: [project({ avatar_url: "/avatars/checkout.webp" })],
    });
    on("GET", "instance/users", paged([instanceUser({ avatar_url: "/avatars/grace.webp" })]));
    const { user } = await renderApp(path());
    const members = await screen.findByRole("grid", { name: "Members" });
    await waitFor(() => expect(members.querySelector('img[src="/avatars/grace.webp"]')).not.toBeNull());
    const projects = await screen.findByRole("grid", { name: "Projects" });
    await waitFor(() => expect(projects.querySelector('img[src="/avatars/checkout.webp"]')).not.toBeNull());
    await user.click(screen.getByRole("button", { name: "Add owner" }));
    const dialog = await screen.findByRole("dialog", { name: "Add an owner" });
    await user.click(within(dialog).getByRole("combobox", { name: "User" }));
    const option = await screen.findByRole("option", { name: /Grace Hopper/ });
    expect(option.querySelector('img[src="/avatars/grace.webp"]')).not.toBeNull();
  });

  it("says when nothing happened in the last 30 days and it has no members or projects", async () => {
    signedIn();
    orgData({
      usage: orgUsage({ requests_30d: 0, tokens_30d: 0, cost_30d: "0", log_entries: 1, log_logical_bytes: 0 }),
      members: [],
      projects: [],
      view: instanceOrg({ members: 0, projects: 0 }),
    });
    await renderApp(path());
    expect(await screen.findByText("None in this period")).toBeInTheDocument();
    expect(screen.getByText("1 entry")).toBeInTheDocument();
    expect(screen.queryByText(/per request/)).not.toBeInTheDocument();
    expect(await screen.findByText("No members.")).toBeInTheDocument();
    expect(await screen.findByText("No projects.")).toBeInTheDocument();
  });

  it("says when it shows only the first members and projects", async () => {
    signedIn();
    orgData({ members: [GRACE], projects: [project()], view: instanceOrg({ members: 250, projects: 120 }) });
    await renderApp(path());
    expect(await screen.findByText("The first 1 of 250 members, with their role and the day they joined")).toBeInTheDocument();
    expect(await screen.findByText("The first 1 of 120 projects, with the day each was created")).toBeInTheDocument();
  });

  it.each([
    [{ require_sso: true, require_2fa: true }, "Single sign-on and two-factor"],
    [{ require_sso: true }, "Single sign-on"],
    [{ require_2fa: true }, "Two-factor authentication"],
  ])("names what sign-in requires (%o)", async (overrides, label) => {
    signedIn();
    orgData({ view: instanceOrg(overrides) });
    await renderApp(path());
    expect(await screen.findByText(label)).toBeInTheDocument();
  });

  it("says when it keeps metadata only", async () => {
    signedIn();
    orgData({ view: instanceOrg({ content_logging: false, metadata_retention_days: 14 }) });
    await renderApp(path());
    expect(await screen.findByText("Metadata only")).toBeInTheDocument();
    expect(screen.getByText("Metadata kept 14 days.")).toBeInTheDocument();
  });

  it("offers no link into an organization you're not a member of", async () => {
    signedIn();
    orgData({ view: instanceOrg({ id: OTHER, name: "Contoso" }) });
    await renderApp(path(OTHER));
    expect(await screen.findByRole("heading", { name: "Contoso", level: 1 })).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Open organization" })).not.toBeInTheDocument();
  });

  it("says when the organization doesn't exist", async () => {
    signedIn();
    on("GET", "instance/orgs/:org", apiError(404, "not_found", "No such organization."));
    await renderApp(path("org_gone"));
    expect(await screen.findByText("This organization doesn't exist or was removed.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Back to organizations" })).toHaveAttribute("href", "/settings/instance/orgs");
  });

  it("says when the organization can't be shown", async () => {
    signedIn();
    on("GET", "instance/orgs/:org", apiError(403, "permission_denied", "Not allowed."));
    await renderApp(path());
    expect(await screen.findByText("This organization can't be shown right now.")).toBeInTheDocument();
  });
});

describe("renaming the organization", () => {
  it("renames it with the trimmed name", async () => {
    signedIn();
    orgData();
    on("PATCH", "instance/orgs/:org", ({ body }: { body: { name: string } }) => instanceOrg({ name: body.name }));
    const { user } = await renderApp(path());
    await user.click(await screen.findByRole("button", { name: "Rename" }));
    const dialog = await screen.findByRole("dialog", { name: "Rename the organization" });
    const input = within(dialog).getByRole("textbox", { name: "Name" });
    await hoverReason(user, within(dialog).getByRole("button", { name: "Rename" }), "Enter a new name");
    await user.clear(input);
    await hoverReason(user, within(dialog).getByRole("button", { name: "Rename" }), "Enter a name");
    await user.type(input, "x".repeat(101));
    await hoverReason(user, within(dialog).getByRole("button", { name: "Rename" }), "Use at most 100 characters");
    await user.clear(input);
    await user.type(input, "  Northwind Research ");
    await user.click(within(dialog).getByRole("button", { name: "Rename" }));
    expect(await notifications().findByText("Renamed to Northwind Research")).toBeInTheDocument();
    expect(requests("PATCH", "instance/orgs/:org").map((r) => r.body)).toEqual([{ name: "Northwind Research" }]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Rename the organization" })).not.toBeInTheDocument());
  });

  it("submits from the keyboard and closes on cancel", async () => {
    signedIn();
    orgData();
    on("PATCH", "instance/orgs/:org", apiError(409, "conflict", "That name is taken."));
    const { user } = await renderApp(path());
    await user.click(await screen.findByRole("button", { name: "Rename" }));
    const dialog = await screen.findByRole("dialog", { name: "Rename the organization" });
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(requests("PATCH", "instance/orgs/:org")).toHaveLength(0);
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), " Two");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect((await notifications().findAllByText("That name is taken.")).length).toBeGreaterThan(0);
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Rename the organization" })).not.toBeInTheDocument());
  });
});

describe("appointing an owner", () => {
  it("adds a user who isn't an owner yet", async () => {
    signedIn();
    orgData();
    on(
      "GET",
      "instance/users",
      paged([instanceUser({ id: USER, name: "Ada Okafor" }), instanceUser(), instanceUser({ id: "usr_linus", name: "Linus Park", email: null })]),
    );
    on("POST", "instance/orgs/:org/owner", instanceOrg({ owners: 2 }));
    const { user } = await renderApp(path());
    await user.click(await screen.findByRole("button", { name: "Add owner" }));
    const dialog = await screen.findByRole("dialog", { name: "Add an owner" });
    expect(within(dialog).queryByRole("button", { name: "Add me" })).not.toBeInTheDocument();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Add owner" }), "Choose a user");
    await user.click(within(dialog).getByRole("combobox", { name: "User" }));
    expect(await screen.findByRole("option", { name: /Linus Park/ })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /Ada Okafor/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole("option", { name: /Grace Hopper/ }));
    await user.click(within(dialog).getByRole("button", { name: "Add owner" }));
    expect(await notifications().findByText("Owner added")).toBeInTheDocument();
    expect(requests("POST", "instance/orgs/:org/owner").map((r) => r.body)).toEqual([{ user_id: "usr_grace" }]);
    expect(requests("GET", "instance/users").map((r) => r.query.get("state"))).toContain("active");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add an owner" })).not.toBeInTheDocument());
  });

  it("opens with no user chosen after a choice was cancelled", async () => {
    signedIn();
    orgData();
    on("GET", "instance/users", paged([instanceUser(), instanceUser({ id: "usr_linus", name: "Linus Park", email: null })]));
    const { user } = await renderApp(path());
    await user.click(await screen.findByRole("button", { name: "Add owner" }));
    const dialog = await screen.findByRole("dialog", { name: "Add an owner" });
    await user.click(within(dialog).getByRole("combobox", { name: "User" }));
    await user.click(await screen.findByRole("option", { name: /Grace Hopper/ }));
    expect(within(dialog).getByRole("combobox", { name: "User" })).toHaveValue("Grace Hopper");
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add an owner" })).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Add owner" }));
    const again = await screen.findByRole("dialog", { name: "Add an owner" });
    expect(within(again).getByRole("combobox", { name: "User" })).toHaveValue("");
    expect(within(again).getByRole("button", { name: "Add owner" })).toHaveAttribute("aria-disabled", "true");
    expect(requests("POST", "instance/orgs/:org/owner")).toHaveLength(0);
  });

  it("makes you an owner of an organization without one", async () => {
    signedIn();
    orgData({ view: instanceOrg({ id: OTHER, name: "Contoso", owners: 0 }), members: [GRACE] });
    on("GET", "instance/users", paged([]));
    on("POST", "instance/orgs/:org/owner", instanceOrg({ id: OTHER, name: "Contoso", owners: 1 }));
    const { user } = await renderApp(path(OTHER));
    expect(await screen.findByText("Nobody can manage it until an owner is appointed.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Add owner" }));
    const dialog = await screen.findByRole("dialog", { name: "Add an owner" });
    await user.click(within(dialog).getByRole("button", { name: "Add me" }));
    expect(await notifications().findByText("You're an owner now")).toBeInTheDocument();
    expect(requests("POST", "instance/orgs/:org/owner").map((r) => [r.params, r.body])).toEqual([[{ org: OTHER }, { user_id: USER }]]);
  });

  it("marks only the choice being added as busy", async () => {
    signedIn();
    orgData({ view: instanceOrg({ id: OTHER, name: "Contoso", owners: 0 }), members: [GRACE] });
    on("GET", "instance/users", paged([instanceUser()]));
    const waiting: (() => void)[] = [];
    on("POST", "instance/orgs/:org/owner", () => new Promise((resolve) => waiting.push(() => resolve(instanceOrg({ id: OTHER, name: "Contoso", owners: 1 })))));
    const { user } = await renderApp(path(OTHER));
    await user.click(await screen.findByRole("button", { name: "Add owner" }));
    const dialog = await screen.findByRole("dialog", { name: "Add an owner" });
    await user.click(within(dialog).getByRole("combobox", { name: "User" }));
    await user.click(await screen.findByRole("option", { name: /Grace Hopper/ }));
    await user.click(within(dialog).getByRole("button", { name: "Add owner" }));
    await waitFor(() => expect(within(dialog).getByRole("button", { name: "Add owner" })).toHaveAttribute("aria-busy", "true"));
    expect(within(dialog).getByRole("button", { name: "Add me" })).not.toHaveAttribute("aria-busy");
    for (const release of waiting) release();
    expect(await notifications().findByText("Owner added")).toBeInTheDocument();
  });

  it("submits the chosen user from the keyboard and closes on cancel", async () => {
    signedIn();
    orgData();
    on("GET", "instance/users", paged([instanceUser()]));
    on("POST", "instance/orgs/:org/owner", apiError(409, "conflict", "Grace is deactivated."));
    const { user } = await renderApp(path());
    await user.click(await screen.findByRole("button", { name: "Add owner" }));
    const dialog = await screen.findByRole("dialog", { name: "Add an owner" });
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(requests("POST", "instance/orgs/:org/owner")).toHaveLength(0);
    await user.click(within(dialog).getByRole("combobox", { name: "User" }));
    await user.click(await screen.findByRole("option", { name: /Grace Hopper/ }));
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect((await notifications().findAllByText("Grace is deactivated.")).length).toBeGreaterThan(0);
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add an owner" })).not.toBeInTheDocument());
  });
});

describe("deleting the organization from the instance", () => {
  it("schedules the deletion once the name is typed", async () => {
    signedIn();
    orgData();
    on("POST", "instance/orgs/:org/schedule-deletion", instanceOrg({ state: "scheduled_for_deletion", deletion_due_at: "2026-10-22T12:00:00Z" }));
    const { user } = await renderApp(path());
    expect(await screen.findByText("Read-only for 7 days, then deleted for good.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Delete organization" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete Northwind Labs" });
    expect(within(dialog).getByText("Its keys stop at once; it stays read-only for 7 days, then goes for good.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    const typed = within(dialog).getByRole("textbox", { name: "Organization name" });
    await user.type(typed, "Northwind");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Delete organization" }), "Type Northwind Labs to confirm");
    await user.type(typed, " Labs{Enter}");
    expect(await notifications().findByText("Northwind Labs will be deleted on 22 Oct 2026")).toBeInTheDocument();
    expect(requests("POST", "instance/orgs/:org/schedule-deletion")).toHaveLength(1);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Delete Northwind Labs" })).not.toBeInTheDocument());
  });

  it("says it's scheduled when the API gives no date", async () => {
    signedIn();
    orgData();
    on("POST", "instance/orgs/:org/schedule-deletion", instanceOrg({ state: "scheduled_for_deletion" }));
    const { user } = await renderApp(path());
    await user.click(await screen.findByRole("button", { name: "Delete organization" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete Northwind Labs" });
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Organization name" }), "Northwind Labs{Enter}");
    expect(await notifications().findByText("Northwind Labs is scheduled for deletion")).toBeInTheDocument();
  });

  it("closes the dialog and says why when scheduling is refused", async () => {
    signedIn();
    orgData();
    on("POST", "instance/orgs/:org/schedule-deletion", apiError(409, "conflict", "Deleting organizations is off."));
    const { user } = await renderApp(path());
    await user.click(await screen.findByRole("button", { name: "Delete organization" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete Northwind Labs" });
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Organization name" }), "Northwind Labs{Enter}");
    expect((await notifications().findAllByText("Deleting organizations is off.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Delete Northwind Labs" })).not.toBeInTheDocument());
  });

  it("closes the dialog with Escape without scheduling", async () => {
    signedIn();
    orgData();
    const { user } = await renderApp(path());
    await user.click(await screen.findByRole("button", { name: "Delete organization" }));
    await screen.findByRole("dialog", { name: "Delete Northwind Labs" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Delete Northwind Labs" })).not.toBeInTheDocument());
    expect(requests("POST", "instance/orgs/:org/schedule-deletion")).toHaveLength(0);
  });

  it("isn't offered when the install doesn't allow deleting organizations", async () => {
    signedIn({ session: whoAmI({ org_deletion: { allowed: false, grace_days: 7 } }) });
    orgData();
    await renderApp(path());
    expect(await screen.findByRole("button", { name: "Rename" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Delete organization" })).not.toBeInTheDocument();
  });
});

describe("an organization scheduled for deletion", () => {
  const scheduled = instanceOrg({ state: "scheduled_for_deletion", deletion_due_at: "2026-10-22T12:00:00Z" });

  it("shows its date and restores it", async () => {
    signedIn();
    orgData({ view: scheduled });
    on("POST", "instance/orgs/:org/restore", instanceOrg());
    const { user } = await renderApp(path());
    expect(await screen.findByText("Deletion on 22 Oct 2026")).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Open organization" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Add owner" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Restore organization" }));
    expect(await notifications().findByText("Northwind Labs restored")).toBeInTheDocument();
    expect(requests("POST", "instance/orgs/:org/restore")).toHaveLength(1);
  });

  it("deletes it now and goes back to the list", async () => {
    signedIn();
    orgData({ view: scheduled });
    on("GET", "instance/orgs", paged([]));
    on("DELETE", "instance/orgs/:org", undefined);
    const { user } = await renderApp(path());
    await user.click(await screen.findByRole("button", { name: "Delete now" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete Northwind Labs now" });
    expect(within(dialog).getByText("Scheduled for")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Organization name" }), "Northwind Labs{Enter}");
    expect(await notifications().findByText("Northwind Labs deleted")).toBeInTheDocument();
    await waitFor(() => expect(window.location.pathname).toBe("/settings/instance/orgs"));
  });

  it("closes the delete dialog without deleting", async () => {
    signedIn();
    orgData({ view: instanceOrg({ state: "scheduled_for_deletion" }) });
    const { user } = await renderApp(path());
    expect(await screen.findByText("Scheduled for deletion")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Delete now" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete Northwind Labs now" });
    await user.click(within(dialog).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Delete Northwind Labs now" })).not.toBeInTheDocument());
    expect(requests("DELETE", "instance/orgs/:org")).toHaveLength(0);
  });

  it("offers only restoring when the install doesn't allow deleting organizations", async () => {
    signedIn({ session: whoAmI({ org_deletion: { allowed: false, grace_days: 7 } }) });
    orgData({ view: scheduled });
    await renderApp(path());
    expect(await screen.findByRole("button", { name: "Restore organization" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Delete now" })).not.toBeInTheDocument();
  });
});
