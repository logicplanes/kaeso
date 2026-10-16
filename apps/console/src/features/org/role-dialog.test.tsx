import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { RoleView } from "~/api/generated/types";
import { ORG, asAdmin, roles, signedIn } from "~/test/fixtures";
import { hoverReason, member, notifications, paged, fill } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/settings/orgs/${ORG}/members?tab=roles&create=true`;

type RoleBody = { scope: string; name: string; description: string; rights: string[]; project_rights?: string[] };

function serveRoles() {
  const list: RoleView[] = [...roles];
  on("GET", "orgs/:org/roles", () => ({ data: list }));
  on("POST", "orgs/:org/roles", ({ body }: { body: RoleBody }) => {
    const created = {
      ...(roles[0] as RoleView),
      id: "role_new",
      kind: "custom",
      scope: body.scope,
      name: body.name,
      description: body.description,
      rights: body.rights,
      project_rights: body.project_rights ?? [],
      holders: 0,
    } as RoleView;
    list.push(created);
    return created;
  });
}

const matrix = (scope: HTMLElement, label: string) =>
  within(scope)
    .getAllByLabelText(label)
    .find((el) => within(el).queryAllByRole("checkbox").length > 0) as HTMLElement;

async function openDialog() {
  const view = await renderApp(PATH);
  const dialog = await screen.findByRole("dialog", { name: "Create role" });
  return { ...view, dialog };
}

describe("creating a role", () => {
  it("creates a project role from another role's rights and opens it", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    serveRoles();
    const { user, dialog, router } = await openDialog();
    const next = () => within(dialog).getByRole("button", { name: "Continue" });
    await hoverReason(user, next(), "Name the role");
    const name = within(dialog).getByRole("textbox", { name: "Name" });
    await fill(user, name, "viewer");
    await hoverReason(user, next(), "There is a project role named viewer already");
    await user.clear(name);
    await fill(user, name, " Support agent ");
    await fill(user, within(dialog).getByRole("textbox", { name: /Description/ }), " Answers tickets ");
    await user.click(next());
    expect(await within(dialog).findByText("What can it do in a project?")).toBeInTheDocument();
    const create = within(dialog).getByRole("button", { name: "Create role" });
    await hoverReason(user, create, "Give the role at least one right");
    await user.click(within(dialog).getByRole("combobox", { name: "Start from" }));
    await user.click(await screen.findByRole("option", { name: /Viewer/ }));
    await user.click(within(matrix(dialog, "Rights of the role")).getByRole("checkbox", { name: /^View costs/ }));
    await user.click(within(dialog).getByRole("button", { name: "Create role" }));
    expect(await notifications().findByText("Support agent created")).toBeInTheDocument();
    const viewer = roles.find((r) => r.scope === "project" && r.id === "viewer") as RoleView;
    expect(requests("POST", "orgs/:org/roles").map((r) => r.body)).toEqual([
      { scope: "project", name: "Support agent", description: "Answers tickets", rights: viewer.rights.filter((r) => r !== "costs.view") },
    ]);
    await waitFor(() => expect(router.state.location.search).toEqual({ tab: "roles", role: "project:role_new" }));
    expect(await screen.findByRole("dialog", { name: "Support agent" })).toBeInTheDocument();
  });

  it("creates an organization role with rights in every project", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    serveRoles();
    const { user, dialog } = await openDialog();
    await fill(user, within(dialog).getByRole("textbox", { name: "Name" }), "Admin");
    await user.click(within(dialog).getByRole("radio", { name: /Organization role/ }));
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "There is an organization role named Admin already");
    await fill(user, within(dialog).getByRole("textbox", { name: "Name" }), " lite");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("What can it do in the organization?")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("combobox", { name: "Start from" }));
    await user.click(await screen.findByRole("option", { name: /Member/ }));
    await user.click(within(matrix(dialog, "Rights of the role")).getByRole("checkbox", { name: /^Edit organization/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("And in every project?")).toBeInTheDocument();
    await user.click(within(matrix(dialog, "Rights of the role in every project")).getByRole("checkbox", { name: "View project" }));
    await user.click(within(dialog).getByRole("button", { name: "Create role" }));
    expect(await notifications().findByText("Admin lite created")).toBeInTheDocument();
    const body = requests("POST", "orgs/:org/roles")[0]?.body as RoleBody;
    expect(body).toMatchObject({ scope: "org", name: "Admin lite", description: "", project_rights: ["project.view"] });
    expect([...body.rights].sort()).toEqual(["members.view", "org.update", "org.view"]);
  });

  it("takes a name before the existing roles have loaded", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("GET", "orgs/:org/roles", () => new Promise(() => {}));
    const { user, dialog } = await openDialog();
    await fill(user, within(dialog).getByRole("textbox", { name: "Name" }), "Admin");
    expect(within(dialog).getByRole("button", { name: "Continue" })).toBeEnabled();
  });

  it("starts over when the scope changes", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    serveRoles();
    const { user, dialog } = await openDialog();
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Ops");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("combobox", { name: "Start from" }));
    await user.click(await screen.findByRole("option", { name: /Viewer/ }));
    await user.click(within(dialog).getByRole("button", { name: "Back" }));
    await user.click(await within(dialog).findByRole("radio", { name: /Organization role/ }));
    await user.click(within(dialog).getByRole("radio", { name: /Organization role/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("What can it do in the organization?")).toBeInTheDocument();
    expect(within(dialog).getByRole("combobox", { name: "Start from" })).toHaveTextContent("No rights");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Give the role at least one right");
    await user.click(within(dialog).getByRole("button", { name: "Back" }));
    await user.click(await within(dialog).findByRole("radio", { name: /Project role/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("What can it do in a project?")).toBeInTheDocument();
    expect(within(dialog).getByRole("combobox", { name: "Start from" })).toHaveTextContent("No rights");
  });

  it("gives only the rights an admin holds", async () => {
    asAdmin();
    on("GET", "orgs/:org/members", paged([member()]));
    serveRoles();
    const { user, dialog } = await openDialog();
    await fill(user, within(dialog).getByRole("textbox", { name: "Name" }), "Payments");
    await user.click(within(dialog).getByRole("radio", { name: /Organization role/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("combobox", { name: "Start from" }));
    await user.click(await screen.findByRole("option", { name: /Billing/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("button", { name: "Create role" }));
    expect(await notifications().findByText("Payments created")).toBeInTheDocument();
    const billing = roles.find((r) => r.scope === "org" && r.id === "billing") as RoleView;
    const body = requests("POST", "orgs/:org/roles")[0]?.body as RoleBody;
    expect(body.rights).toEqual(billing.rights.filter((r) => r !== "billing.manage"));
  });

  it("stays open and says why when the role is refused", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("POST", "orgs/:org/roles", apiError(422, "invalid_request", "The organization has too many roles."));
    const { user, dialog } = await openDialog();
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Ops");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("combobox", { name: "Start from" }));
    await user.click(await screen.findByRole("option", { name: /Viewer/ }));
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect((await notifications().findAllByText("The organization has too many roles.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("dialog", { name: "Create role" })).toBeInTheDocument();
  });
});
