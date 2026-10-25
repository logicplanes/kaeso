import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { signedIn, without } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { apiKey, grant, listed, projectData, projectMember, projectPath, serviceAccount } from "~/test/fixtures/project";
import { createdKey } from "~/test/fixtures/shell";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const ACCOUNTS = "orgs/:org/projects/:project/service-accounts";
const ACCOUNT = `${ACCOUNTS}/:account`;
const archived = serviceAccount({ id: "sa_old", name: "old-batch", archived_at: "2026-10-01T12:00:00Z" });

describe("ServiceAccountsPage", () => {
  it("drops ?create=true from the address when the dialog closes", async () => {
    signedIn();
    projectData();
    const { user, router } = await renderApp(projectPath("service-accounts?create=true"));
    await screen.findByRole("dialog", { name: /Create service account/ });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("says why the service accounts couldn't load", async () => {
    signedIn();
    projectData();
    on("GET", ACCOUNTS, apiError(400, "invalid_request", "That project can't list service accounts."));
    await renderApp(projectPath("service-accounts"));
    expect((await notifications().findAllByText("That project can't list service accounts.")).length).toBeGreaterThan(0);
  });

  it("lists the active service accounts by name with their keys and role", async () => {
    signedIn();
    projectData({
      members: [projectMember({ user: { id: "usr_ada", name: "Ada Okafor" } })],
      accounts: [
        serviceAccount({ id: "sa_zeta", name: "zeta-sync", created_by: null, grant: grant("viewer", { rights_removed: ["usage.view"] }) }),
        serviceAccount(),
        archived,
      ],
      keys: [
        apiKey({ principal: { kind: "service_account", id: "sa_deploy" } }),
        apiKey({ id: "key_two", principal: { kind: "service_account", id: "sa_deploy" } }),
        apiKey({ id: "key_mine" }),
      ],
    });
    await renderApp(projectPath("service-accounts"));

    const grid = await screen.findByRole("grid", { name: "Service accounts" });
    const rows = within(grid).getAllByRole("gridcell");
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveTextContent("ci-deploy");
    expect(rows[0]).toHaveTextContent("Created by Ada Okafor");
    await waitFor(() => expect(rows[0]).toHaveTextContent("2 keys"));
    expect(rows[0]).toHaveTextContent("Developer");
    expect(within(rows[0] as HTMLElement).getByText("Developer")).toHaveClass("bg-fill-2");
    expect(rows[1]).toHaveTextContent("zeta-sync");
    expect(rows[1]).toHaveTextContent("0 keys");
    expect(rows[1]).toHaveTextContent("Viewer · customized");
    expect(requests("GET", ACCOUNTS).map((r) => r.query.get("include_archived"))).toEqual(["true"]);
  });

  it("shows archived service accounts on the All tab", async () => {
    signedIn();
    projectData({ accounts: [serviceAccount(), archived] });
    const { user, router } = await renderApp(projectPath("service-accounts"));

    await screen.findByRole("gridcell", { name: /ci-deploy/ });
    await user.click(screen.getByRole("tab", { name: "All" }));
    expect(await screen.findByRole("gridcell", { name: /old-batch/ })).toHaveTextContent("Archived");
    expect(router.state.location.search).toEqual({ scope: "all" });
    await user.click(screen.getByRole("tab", { name: "Active" }));
    await waitFor(() => expect(screen.queryByRole("gridcell", { name: /old-batch/ })).not.toBeInTheDocument());
    expect(router.state.location.search).toEqual({});
  });

  it("lists only the archived service accounts under Archived, and says when there are none", async () => {
    signedIn();
    projectData({ accounts: [serviceAccount(), archived] });
    const { router } = await renderApp(projectPath("service-accounts?scope=archived"));
    expect(await screen.findByRole("gridcell", { name: /old-batch/ })).toBeInTheDocument();
    expect(screen.queryByRole("gridcell", { name: /ci-deploy/ })).not.toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Archived" })).toHaveAttribute("aria-selected", "true");
    expect(router.state.location.search).toEqual({ scope: "archived" });
  });

  it("says when no service account is archived", async () => {
    signedIn();
    projectData({ accounts: [serviceAccount()] });
    await renderApp(projectPath("service-accounts?scope=archived"));
    expect(await screen.findByText("No archived service accounts")).toBeInTheDocument();
  });

  it("offers to create the first service account", async () => {
    signedIn();
    projectData();
    const { user } = await renderApp(projectPath("service-accounts"));

    expect(await screen.findByText("No service accounts yet")).toBeInTheDocument();
    expect(screen.getByText("Give a deployment or a pipeline keys that outlast the people who made them.")).toBeInTheDocument();
    const buttons = screen.getAllByRole("button", { name: "Create service account" });
    expect(buttons).toHaveLength(2);
    await user.click(buttons[1] as HTMLElement);
    expect(await screen.findByRole("dialog", { name: /Create service account/ })).toBeInTheDocument();
  });

  it("opens a service account from the list and the create dialog from the header", async () => {
    signedIn();
    projectData({ accounts: [serviceAccount()] });
    on("GET", ACCOUNT, serviceAccount());
    const { user, router } = await renderApp(projectPath("service-accounts"));

    await user.click(await screen.findByRole("gridcell", { name: /ci-deploy/ }));
    expect(await screen.findByRole("dialog", { name: "ci-deploy" })).toBeInTheDocument();
    await waitFor(() => expect(router.state.location.search).toEqual({ account: "sa_deploy" }));
    await user.click(within(screen.getByRole("main")).getByRole("button", { name: "Create service account" }));
    expect(await screen.findByRole("dialog", { name: /Create service account/ })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /Create service account/ })).not.toBeInTheDocument());
  });

  it("opens the create dialog from the c shortcut", async () => {
    signedIn();
    projectData({ accounts: [serviceAccount()] });
    const { user } = await renderApp(projectPath("service-accounts"));

    await screen.findByRole("gridcell", { name: /ci-deploy/ });
    await user.keyboard("c");
    expect(await screen.findByRole("dialog", { name: /Create service account/ })).toBeInTheDocument();
  });

  it("creates a service account in two steps and opens it", async () => {
    signedIn();
    projectData();
    on("POST", ACCOUNTS, serviceAccount({ id: "sa_new", name: "checkout-production", grant: grant("viewer") }));
    on("GET", ACCOUNT, serviceAccount({ id: "sa_new", name: "checkout-production", grant: grant("viewer") }));
    const { user, router } = await renderApp(projectPath("service-accounts?create=true"));

    const dialog = await screen.findByRole("dialog", { name: /Create service account/ });
    expect(within(dialog).getByText("What uses it?")).toBeInTheDocument();
    const name = within(dialog).getByRole("textbox", { name: "Name" });
    await user.type(name, "{Enter}");
    expect(within(dialog).getByText("What uses it?")).toBeInTheDocument();
    await user.type(name, "  checkout-production  {Enter}");
    expect(await within(dialog).findByText("What can its keys do?")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("combobox", { name: "Role" }));
    await user.click(await screen.findByRole("option", { name: /Viewer/ }));
    await user.click(within(dialog).getByRole("button", { name: "Create service account" }));

    expect(await notifications().findByText("Service account checkout-production created")).toBeInTheDocument();
    expect(requests("POST", ACCOUNTS).map((r) => r.body)).toEqual([{ name: "checkout-production", role: "viewer", rights_added: [], rights_removed: [] }]);
    await waitFor(() => expect(router.state.location.search).toEqual({ account: "sa_new" }));
    expect(await screen.findByRole("dialog", { name: "checkout-production" })).toBeInTheDocument();
  });

  it("disables creating without the right and says who can give it", async () => {
    signedIn({ projectRights: without("service_accounts.create", "keys.all.view") });
    projectData();
    const { user } = await renderApp(projectPath("service-accounts"));

    expect(await screen.findByText("No service accounts yet")).toBeInTheDocument();
    const create = screen.getAllByRole("button", { name: "Create service account" });
    expect(create).toHaveLength(1);
    expect(create[0]).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, create[0] as HTMLElement, "You need Create service accounts; ask Lin Wei for it.");
    expect(requests("GET", "orgs/:org/projects/:project/keys")).toHaveLength(0);
  });
});

describe("ServiceAccountSheet", () => {
  function account(view = serviceAccount(), keys = [apiKey({ id: "key_sa", name: "deploy-key", principal: { kind: "service_account", id: view.id } })]) {
    projectData({ accounts: [view], members: [projectMember({ user: { id: "usr_ada", name: "Ada Okafor" } })] });
    on("GET", ACCOUNT, view);
    on("GET", "orgs/:org/projects/:project/keys", ({ query }: { query: URLSearchParams }) => listed(query.get("service_account") === view.id ? keys : []));
  }

  it("shows the account, its keys and its limits", async () => {
    signedIn();
    account(serviceAccount(), [
      apiKey({ id: "key_sa", name: "deploy-key", principal: { kind: "service_account", id: "sa_deploy" }, last_used_at: "2026-10-15T11:55:00Z" }),
      apiKey({ id: "key_plat", name: "platform-key", access: "both", principal: { kind: "service_account", id: "sa_deploy" } }),
      apiKey({ id: "key_gone", name: "gone-key", state: "archived", principal: { kind: "service_account", id: "sa_deploy" } }),
    ]);
    await renderApp(projectPath("service-accounts?account=sa_deploy"));

    const sheet = await screen.findByRole("dialog", { name: "ci-deploy" });
    expect(await within(sheet).findByText("sa_deploy")).toBeInTheDocument();
    expect(within(sheet).getByText(/15 Oct 2026 by/)).toBeInTheDocument();
    const keys = await within(sheet).findByRole("grid", { name: "Keys of ci-deploy" });
    expect(within(keys).getAllByRole("gridcell")).toHaveLength(2);
    expect(within(keys).getByRole("gridcell", { name: /platform-key/ })).toHaveTextContent("Never used");
    expect(within(sheet).getByText("No limits on this service account; the project's limits apply.")).toBeInTheDocument();
    expect(within(sheet).getByText("Archiving is permanent; its history stays.")).toBeInTheDocument();
  });

  it("shows only the creation date when nobody is named as its creator", async () => {
    signedIn();
    account(serviceAccount({ created_by: null }));
    await renderApp(projectPath("service-accounts?account=sa_deploy"));

    const sheet = await screen.findByRole("dialog", { name: "ci-deploy" });
    expect(await within(sheet).findByText("15 Oct 2026")).toBeInTheDocument();
    expect(within(sheet).queryByText(/15 Oct 2026 by/)).not.toBeInTheDocument();
  });

  it("says its keys are loading until they arrive", async () => {
    signedIn();
    account(serviceAccount(), []);
    const waiting: (() => void)[] = [];
    on("GET", "orgs/:org/projects/:project/keys", () => new Promise((resolve) => waiting.push(() => resolve(listed([])))));
    await renderApp(projectPath("service-accounts?account=sa_deploy"));

    const sheet = await screen.findByRole("dialog", { name: "ci-deploy" });
    expect(await within(sheet).findByText("Loading keys…")).toBeInTheDocument();
    waiting.forEach((go) => go());
    expect(await within(sheet).findByText("No keys yet.")).toBeInTheDocument();
  });

  it("renames the service account", async () => {
    signedIn();
    account();
    on("PATCH", ACCOUNT, serviceAccount({ name: "ci-release" }));
    const { user } = await renderApp(projectPath("service-accounts?account=sa_deploy"));

    const sheet = await screen.findByRole("dialog", { name: "ci-deploy" });
    await user.click(await within(sheet).findByRole("button", { name: "Rename" }));
    const input = within(sheet).getByRole("textbox", { name: "Name" });
    expect(within(sheet).getByRole("button", { name: "Save" })).toBeDisabled();
    await user.click(within(sheet).getAllByRole("button", { name: "Cancel" })[0] as HTMLElement);
    expect(within(sheet).queryByRole("textbox", { name: "Name" })).not.toBeInTheDocument();

    await user.click(within(sheet).getByRole("button", { name: "Rename" }));
    await user.clear(within(sheet).getByRole("textbox", { name: "Name" }));
    expect(input).not.toBeInTheDocument();
    await user.type(within(sheet).getByRole("textbox", { name: "Name" }), " ci-release ");
    await user.click(within(sheet).getByRole("button", { name: "Save" }));
    expect(await notifications().findByText("Name saved")).toBeInTheDocument();
    expect(requests("PATCH", ACCOUNT).map((r) => r.body)).toEqual([{ name: "ci-release" }]);
  });

  it("changes the role of the service account", async () => {
    signedIn();
    account();
    on("PATCH", ACCOUNT, serviceAccount({ grant: grant("viewer") }));
    const { user } = await renderApp(projectPath("service-accounts?account=sa_deploy"));

    const sheet = await screen.findByRole("dialog", { name: "ci-deploy" });
    await user.click(await within(sheet).findByRole("combobox", { name: "Role" }));
    await user.click(await screen.findByRole("option", { name: /Viewer/ }));
    await user.click(within(sheet).getByRole("button", { name: "Cancel" }));
    expect(within(sheet).getByRole("combobox", { name: "Role" })).toHaveTextContent("Developer");
    await user.click(within(sheet).getByRole("combobox", { name: "Role" }));
    await user.click(await screen.findByRole("option", { name: /Viewer/ }));
    await user.click(within(sheet).getByRole("button", { name: "Save role" }));
    expect(await notifications().findByText("Rights saved")).toBeInTheDocument();
    expect(requests("PATCH", ACCOUNT).map((r) => r.body)).toEqual([{ grant: { role: "viewer", rights_added: [], rights_removed: [] } }]);
  });

  it("shows the save it is waiting for as busy, the name's or the role's", async () => {
    signedIn();
    account();
    on("PATCH", ACCOUNT, () => new Promise(() => undefined));
    const { user } = await renderApp(projectPath("service-accounts?account=sa_deploy"));

    const sheet = await screen.findByRole("dialog", { name: "ci-deploy" });
    await user.click(await within(sheet).findByRole("combobox", { name: "Role" }));
    await user.click(await screen.findByRole("option", { name: /Viewer/ }));
    await user.click(within(sheet).getByRole("button", { name: "Save role" }));
    await waitFor(() => expect(within(sheet).getByRole("button", { name: "Save role" })).toHaveAttribute("aria-busy", "true"));
    await user.click(within(sheet).getByRole("button", { name: "Rename" }));
    expect(within(sheet).getByRole("button", { name: "Save" })).not.toHaveAttribute("aria-busy");
  });

  it("archives the service account after confirming", async () => {
    signedIn();
    account();
    on("POST", `${ACCOUNT}/archive`, serviceAccount({ archived_at: "2026-10-15T12:00:00Z" }));
    const { user, router } = await renderApp(projectPath("service-accounts?account=sa_deploy"));

    const sheet = await screen.findByRole("dialog", { name: "ci-deploy" });
    await user.click(await within(sheet).findByRole("button", { name: "Archive service account" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Archive ci-deploy?" });
    await user.click(within(confirm).getByRole("button", { name: "Archive service account" }));
    expect(await notifications().findByText("Service account archived")).toBeInTheDocument();
    expect(requests("POST", `${ACCOUNT}/archive`)).toHaveLength(1);
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("shows an archived service account read-only", async () => {
    signedIn();
    account(archived, []);
    const { user } = await renderApp(projectPath("service-accounts?account=sa_old&scope=all"));

    const sheet = await screen.findByRole("dialog", { name: "old-batch" });
    expect(await within(sheet).findByText("Archived 1 Oct 2026. Its keys were archived with it.")).toBeInTheDocument();
    expect(within(sheet).getByRole("combobox", { name: "Role" })).toBeDisabled();
    expect(within(sheet).queryByRole("button", { name: "Rename" })).not.toBeInTheDocument();
    expect(within(sheet).queryByRole("button", { name: "Save role" })).not.toBeInTheDocument();
    expect(within(sheet).queryByRole("button", { name: "Create key" })).not.toBeInTheDocument();
    expect(within(sheet).queryByRole("button", { name: "Archive service account" })).not.toBeInTheDocument();
    expect(await within(sheet).findByText("No keys yet.")).toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Close" }));
  });

  it("creates a key for the service account and shows its secret once", async () => {
    signedIn();
    account(serviceAccount(), []);
    on("POST", `${ACCOUNT}/keys`, createdKey({ name: "deploy-key", principal: { kind: "service_account", id: "sa_deploy" } }));
    const { user } = await renderApp(projectPath("service-accounts?account=sa_deploy"));

    const sheet = await screen.findByRole("dialog", { name: "ci-deploy" });
    await user.click(await within(sheet).findByRole("button", { name: "Create key" }));
    const dialog = await screen.findByRole("dialog", { name: /Create key/ });
    expect(within(dialog).getByText("A key for ci-deploy")).toBeInTheDocument();
    expect(within(dialog).queryByRole("combobox", { name: "For" })).not.toBeInTheDocument();
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "deploy-key{Enter}");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("button", { name: "Create key" }));
    await waitFor(() => expect(requests("POST", `${ACCOUNT}/keys`)).toHaveLength(1));
    expect(requests("POST", `${ACCOUNT}/keys`)[0]?.body).toEqual({ name: "deploy-key", access: "inference", expires_in_days: 90 });
    const reveal = await screen.findByRole("dialog", { name: "Key created" });
    await user.click(within(reveal).getByRole("button", { name: "Finish" }));
    await user.click(within(await screen.findByRole("alertdialog", { name: "Did you copy the key?" })).getByRole("button", { name: "Close without copying" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Key created" })).not.toBeInTheDocument());
    expect(screen.getByRole("dialog", { name: "ci-deploy" })).toBeInTheDocument();
  });

  it("hides keys, limits and archiving without their rights and says what renaming needs", async () => {
    signedIn({ projectRights: without("keys.all.view", "limits.view", "service_accounts.archive", "service_accounts.rights.update") });
    account();
    const { user } = await renderApp(projectPath("service-accounts?account=sa_deploy"));

    const sheet = await screen.findByRole("dialog", { name: "ci-deploy" });
    expect(await within(sheet).findByRole("combobox", { name: "Role" })).toBeDisabled();
    expect(within(sheet).queryByRole("heading", { name: "Keys" })).not.toBeInTheDocument();
    expect(within(sheet).queryByRole("heading", { name: "Limits" })).not.toBeInTheDocument();
    expect(within(sheet).queryByRole("heading", { name: "Archive" })).not.toBeInTheDocument();
    const rename = within(sheet).getByRole("button", { name: "Rename" });
    expect(rename).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, rename, /^You need Change service accounts; ask .+ for it\.$/);
    await user.click(rename);
    expect(within(sheet).queryByRole("textbox", { name: "Name" })).not.toBeInTheDocument();
  });

  it("says which right creating a key for the service account needs", async () => {
    signedIn({ projectRights: without("service_accounts.keys.create") });
    account(serviceAccount(), []);
    const { user } = await renderApp(projectPath("service-accounts?account=sa_deploy"));

    const sheet = await screen.findByRole("dialog", { name: "ci-deploy" });
    const create = await within(sheet).findByRole("button", { name: "Create key" });
    expect(create).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, create, /^You need Create service account keys; ask .+ for it\.$/);
  });

  it("says when the service account doesn't exist", async () => {
    signedIn();
    projectData();
    on("GET", ACCOUNT, apiError(404, "not_found", "No such service account."));
    const { user, router } = await renderApp(projectPath("service-accounts?account=sa_gone"));

    expect(await screen.findByText("This service account doesn't exist or was removed.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Back to service accounts" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("says when the service account couldn't load", async () => {
    signedIn();
    projectData();
    on("GET", ACCOUNT, apiError(403, "permission_denied", "You can't see this service account."));
    await renderApp(projectPath("service-accounts?account=sa_deploy"));
    expect(await screen.findByText("Couldn't load this service account.")).toBeInTheDocument();
    expect((await notifications().findAllByText("You can't see this service account.")).length).toBeGreaterThan(0);
  });
});
