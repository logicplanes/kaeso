import { describe, expect, it } from "vitest";
import { screen } from "@testing-library/react";
import type { SsoGroups } from "~/api/generated/types";
import { ORG, PROJECT, signedIn, withRights } from "~/test/fixtures";
import { hoverReason, notifications, security, sso } from "~/test/fixtures/org";
import { on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const nth = (name: string, i: number) => screen.getAllByRole("button", { name })[i] as HTMLElement;

const PATH = `/settings/orgs/${ORG}/security`;
const GROUPS = "orgs/:org/security/sso/groups";

const groups = (over: Partial<SsoGroups> = {}): SsoGroups => ({
  claim: "groups",
  scopes: [],
  mode: "add",
  rules: [{ group: "kaeso-admins", role: "admin", projects: [{ project: PROJECT, role: "developer" }] }],
  claim_missing_at: null,
  your_last_groups: null,
  ...over,
});

describe("single sign-on groups", () => {
  it("edits the claim, the mode and the rules", async () => {
    signedIn();
    on("GET", "orgs/:org/security", security({ sso: sso() }));
    on("GET", GROUPS, groups());
    on("PUT", GROUPS, ({ body }: { body: SsoGroups }) => groups(body));
    const { user } = await renderApp(PATH);
    expect(await screen.findByDisplayValue("kaeso-admins")).toBeInTheDocument();
    expect(screen.getByText("Never takes anything away.")).toBeInTheDocument();
    expect(screen.getByText("None yet; sign in through the provider to see yours.")).toBeInTheDocument();

    const claim = screen.getByRole("textbox", { name: "Groups claim" });
    await user.clear(claim);
    await user.type(claim, "roles");
    await user.type(screen.getByRole("textbox", { name: "Extra scopes" }), "groups  offline");
    await user.click(screen.getByRole("radio", { name: "Keep in sync" }));
    expect(screen.getByText("Takes away what a rule gave once no rule gives it.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Add rule" }));
    await user.type(screen.getByRole("textbox", { name: "Group of rule 2" }), "everyone");
    await user.click(screen.getByRole("combobox", { name: "Role of rule 2" }));
    await user.click(await screen.findByRole("option", { name: "Developer" }));
    await user.click(nth("Add project", 1));
    await user.click(nth("Add project", 1));
    await user.click(screen.getByRole("combobox", { name: "Role in project 2 of rule 2" }));
    await user.click(await screen.findByRole("option", { name: "Viewer" }));
    await user.click(screen.getByRole("combobox", { name: "Project 1 of rule 2" }));
    await user.click(await screen.findByRole("option", { name: "Checkout assistant" }));
    await user.click(nth("Remove project", 2));
    await user.click(screen.getByRole("combobox", { name: "Role of rule 1" }));
    await user.click(await screen.findByRole("option", { name: "No organization role" }));

    await user.click(screen.getByRole("button", { name: "Save group rules" }));
    expect(await notifications().findByText("Group rules saved")).toBeInTheDocument();
    expect(requests("PUT", GROUPS).map((r) => r.body)).toEqual([
      {
        claim: "roles",
        scopes: ["groups", "offline"],
        mode: "sync",
        rules: [
          { group: "kaeso-admins", role: null, projects: [{ project: PROJECT, role: "developer" }] },
          { group: "everyone", role: "developer", projects: [{ project: PROJECT, role: "developer" }] },
        ],
      },
    ]);
  });

  it("shows the groups of your last sign-in, a missing claim, and removes a rule", async () => {
    signedIn();
    on("GET", "orgs/:org/security", security({ sso: sso() }));
    on("GET", GROUPS, groups({ your_last_groups: ["kaeso-admins", "everyone"], claim_missing_at: "2026-11-01T00:00:00Z", mode: "sync" }));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("kaeso-admins, everyone")).toBeInTheDocument();
    expect(screen.getByText(/lacked the claim; nothing changed/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Remove rule" }));
    expect(screen.queryByDisplayValue("kaeso-admins")).not.toBeInTheDocument();
  });

  it("needs the right to manage single sign-on, and isn't shown without it connected", async () => {
    withRights(["org.view"], { role: "admin" });
    on("GET", "orgs/:org/projects", page([]));
    on("GET", "orgs/:org/security", security({ sso: sso() }));
    on("GET", GROUPS, groups({ rules: [{ group: "x", role: null }] }));
    const { user } = await renderApp(PATH);
    await hoverReason(user, await screen.findByRole("button", { name: "Save group rules" }), /Manage SSO/);
    expect(screen.getByRole("button", { name: "Add project" })).toBeDisabled();
  });

  it("isn't shown without a connection", async () => {
    signedIn();
    on("GET", "orgs/:org/security", security({ sso: null }));
    await renderApp(PATH);
    expect(await screen.findByText("Require single sign-on")).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "Groups claim" })).not.toBeInTheDocument();
    expect(requests("GET", GROUPS)).toHaveLength(0);
  });
});
