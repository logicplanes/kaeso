import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, OTHER_PROJECT, PROJECT, membership, myMembership, myProject, signedIn, whoAmI } from "~/test/fixtures";
import { apiError, noContent, on, requests } from "~/test/network";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { renderApp } from "~/test/render";
import { atFixtureTime } from "~/test/clock";

atFixtureTime();

const PATH = "/settings/account/memberships";
const CONTOSO = "org_contoso";

function twoOrgs(contoso: Partial<Parameters<typeof myMembership>[0]> = {}, northwind: Partial<Parameters<typeof myMembership>[0]> = {}) {
  return signedIn({
    session: whoAmI({ memberships: [membership(), membership({ org_id: CONTOSO, org_name: "Contoso" })] }),
    memberships: [
      myMembership(northwind),
      myMembership({ org_id: CONTOSO, org_name: "Contoso", role: { id: "member", kind: "builtin", name: "Member" }, projects: [], ...contoso }),
    ],
  });
}

describe("memberships", () => {
  it("lists each organization with its role and projects", async () => {
    twoOrgs();
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Contoso" })).toBeInTheDocument();
    expect(screen.getByText("Owner of the organization")).toBeInTheDocument();
    expect(screen.getByText("Member of the organization")).toBeInTheDocument();
    const northwind = screen.getByRole("group", { name: "Projects in Northwind Labs" });
    expect(within(northwind).getByRole("link", { name: /Checkout assistant/ })).toHaveAttribute("href", `/orgs/${ORG}/projects/${PROJECT}`);
    expect(within(northwind).getByRole("link", { name: /Support triage/ })).toHaveAttribute("href", `/orgs/${ORG}/projects/${OTHER_PROJECT}`);
    expect(within(northwind).getByText("Default")).toBeInTheDocument();
    const contoso = screen.getByRole("group", { name: "Projects in Contoso" });
    expect(within(contoso).getByText("No projects")).toBeInTheDocument();
    expect(within(contoso).getByText("You belong to none of its projects.")).toBeInTheDocument();
  });

  it("marks an organization scheduled for deletion and offers no leaving", async () => {
    twoOrgs({
      org_state: "scheduled_for_deletion",
      org_deletion_due_at: "2026-10-22T12:00:00Z",
      projects: [myProject({ id: "proj_c", avatar_url: "/a.webp" })],
    });
    await renderApp(PATH);
    const contoso = await screen.findByRole("group", { name: "Projects in Contoso" });
    expect(within(contoso).getByText("Scheduled for deletion")).toBeInTheDocument();
    expect(within(contoso).getByText("Read-only until 22 Oct 2026; an owner can restore it.")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Leave organization" })).toHaveLength(1);
  });

  it("says how long a deleted organization stays read-only without a date", async () => {
    twoOrgs({ org_state: "scheduled_for_deletion", org_deletion_due_at: null });
    await renderApp(PATH);
    expect(await screen.findByText("Read-only until it's deleted; an owner can restore it.")).toBeInTheDocument();
  });

  it("says when the person belongs to no organization", async () => {
    signedIn({ session: whoAmI({ memberships: [] }), memberships: [] });
    await renderApp(PATH);
    expect(await screen.findByText("You don't belong to an organization.")).toBeInTheDocument();
  });

  it("leaves an organization after confirming", async () => {
    twoOrgs();
    on("DELETE", "me/memberships/:org", noContent);
    const { user } = await renderApp(PATH);
    await screen.findByRole("heading", { name: "Contoso" });
    const buttons = screen.getAllByRole("button", { name: "Leave organization" });
    await user.click(buttons[1] as HTMLElement);
    const confirm = await screen.findByRole("alertdialog", { name: "Leave Contoso?" });
    await user.click(within(confirm).getByRole("button", { name: "Leave organization" }));
    expect(await notifications().findByText("You left Contoso")).toBeInTheDocument();
    expect(requests("DELETE", "me/memberships/:org").map((r) => r.params.org)).toEqual([CONTOSO]);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("stays a member when leaving is refused", async () => {
    twoOrgs();
    on("DELETE", "me/memberships/:org", apiError(409, "conflict", "Transfer ownership of Northwind Labs first."));
    const { user } = await renderApp(PATH);
    await screen.findByRole("heading", { name: "Contoso" });
    await user.click(screen.getAllByRole("button", { name: "Leave organization" })[0] as HTMLElement);
    const confirm = await screen.findByRole("alertdialog", { name: "Leave Northwind Labs?" });
    await user.click(within(confirm).getByRole("button", { name: "Leave organization" }));
    expect((await notifications().findAllByText("Transfer ownership of Northwind Labs first.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(screen.getByRole("heading", { name: "Northwind Labs" })).toBeInTheDocument();
  });

  it("says why the only owner can't leave, and lets the others leave", async () => {
    twoOrgs({}, { last_owner: true });
    const { user } = await renderApp(PATH);
    await screen.findByRole("heading", { name: "Contoso" });
    const [northwind, contoso] = screen.getAllByRole("button", { name: "Leave organization" });
    await waitFor(() => expect(northwind).toHaveAttribute("aria-disabled", "true"));
    await hoverReason(user, northwind as HTMLElement, "You're its only owner; make someone else an owner first.");
    await user.click(northwind as HTMLElement);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(contoso).toBeEnabled();
  });

  it("closes the confirmation without leaving", async () => {
    twoOrgs();
    const { user } = await renderApp(PATH);
    await screen.findByRole("heading", { name: "Contoso" });
    await user.click(screen.getAllByRole("button", { name: "Leave organization" })[0] as HTMLElement);
    const confirm = await screen.findByRole("alertdialog", { name: "Leave Northwind Labs?" });
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(requests("DELETE", "me/memberships/:org")).toHaveLength(0);
  });
});
