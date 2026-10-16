import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, roles, signedIn } from "~/test/fixtures";
import { member, paged } from "~/test/fixtures/org";
import { on } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/settings/orgs/${ORG}/members?tab=roles`;

describe("organization roles", () => {
  it("lists the organization and project roles with their size and holders", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    await renderApp(PATH);
    const orgRoles = await screen.findByRole("grid", { name: "Organization roles" });
    expect(screen.getByText("One per member: their rights in the organization, and in every project.")).toBeInTheDocument();
    expect(within(orgRoles).getByRole("gridcell", { name: /^Owner/ })).toHaveTextContent("Every right");
    expect(within(orgRoles).getByRole("gridcell", { name: /^Owner/ })).toHaveTextContent("Held by 1");
    expect(within(orgRoles).getByRole("gridcell", { name: /^Admin/ })).toHaveTextContent("30 rights, 31 in projects");
    expect(within(orgRoles).getByRole("gridcell", { name: /^Billing/ })).toHaveTextContent("Nobody yet");
    expect(within(orgRoles).getByRole("gridcell", { name: /^Member/ })).toHaveTextContent("2 rights");
    const projectRoles = screen.getByRole("grid", { name: "Project roles" });
    const lead = within(projectRoles).getByRole("gridcell", { name: /^Support lead/ });
    expect(lead).toHaveTextContent("Custom");
    expect(lead).toHaveTextContent("8 rights");
    expect(lead).toHaveTextContent("Calls models and reads request logs and usage to help customers");
  });

  it("says when the organization has no project roles", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("GET", "orgs/:org/roles", { data: roles.filter((r) => r.scope === "org") });
    await renderApp(PATH);
    expect(await screen.findByText("No project roles")).toBeInTheDocument();
  });

  it("opens a role's sheet from the list and closes it", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    const { user, router } = await renderApp(PATH);
    const projectRoles = await screen.findByRole("grid", { name: "Project roles" });
    await user.click(within(projectRoles).getByText("Viewer"));
    await waitFor(() => expect(router.state.location.search).toEqual({ tab: "roles", role: "project:viewer" }));
    const sheet = await screen.findByRole("dialog", { name: "Viewer" });
    await user.click(within(sheet).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ tab: "roles" }));
  });

  it("opens an organization role's sheet", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    const { user, router } = await renderApp(PATH);
    const orgRoles = await screen.findByRole("grid", { name: "Organization roles" });
    await user.click(within(orgRoles).getByText("Auditor"));
    await waitFor(() => expect(router.state.location.search).toEqual({ tab: "roles", role: "org:auditor" }));
    expect(await screen.findByRole("dialog", { name: "Auditor" })).toBeInTheDocument();
  });

  it("opens the role dialog from the create button", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    const { user } = await renderApp(PATH);
    await screen.findByRole("grid", { name: "Organization roles" });
    await user.click(screen.getByRole("button", { name: "Create role" }));
    const dialog = await screen.findByRole("dialog", { name: "Create role" });
    await user.click(within(dialog).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Create role" })).not.toBeInTheDocument());
  });
});
