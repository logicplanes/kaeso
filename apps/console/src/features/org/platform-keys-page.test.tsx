import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, org, signedIn, withRights } from "~/test/fixtures";
import { createdPlatformKey, fill, hoverReason, notifications, paged, platformKey } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { cachedText } from "~/test/cache";

const PATH = `/settings/orgs/${ORG}/platform-keys`;
const DAY = 86_400_000;
const fromNow = (ms: number) => new Date(Date.now() + ms).toISOString();
const keyQueries = () => requests("GET", "orgs/:org/platform-keys").map((r) => r.query.get("include_archived"));

async function cellOf(name: string) {
  const grid = await screen.findByRole("grid", { name: "Platform keys" });
  return within(grid).findByRole("gridcell", { name: new RegExp(name) });
}

async function openCreate() {
  const view = await renderApp(PATH);
  await screen.findByText("No active platform keys");
  await view.user.click(screen.getAllByRole("button", { name: "Create platform key" })[0] as HTMLElement);
  const dialog = await screen.findByRole("dialog", { name: "Create platform key" });
  return { ...view, dialog };
}

describe("platform keys", () => {
  it("lists the active keys, and every key under All, with their expiry and last use", async () => {
    signedIn();
    on(
      "GET",
      "orgs/:org/platform-keys",
      paged([
        platformKey({ last_used_at: new Date(Date.now() - 5 * 60_000).toISOString() }),
        platformKey({ id: "opk_ci", name: "CI deploy", state: "disabled", expires_at: fromNow(3 * DAY) }),
      ]),
    );
    const { user } = await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Platform keys", level: 1 })).toBeInTheDocument();
    const terraform = await cellOf("Terraform");
    expect(terraform).toHaveTextContent("kso_org_…a1b2");
    expect(terraform).toHaveTextContent("Never expires");
    expect(terraform).toHaveTextContent("Used 5 min ago");
    expect(within(screen.getByRole("grid", { name: "Platform keys" })).queryByRole("gridcell", { name: /CI deploy/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "All" }));
    const ci = await cellOf("CI deploy");
    expect(ci).toHaveTextContent("Expires in 3 days");
    expect(ci).toHaveTextContent("Never used");
    expect(keyQueries()).toEqual(["false", "true"]);
  });

  it("lists only the archived keys under Archived", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", ({ query }: { query: URLSearchParams }) =>
      paged(
        query.get("include_archived") === "true" ? [platformKey(), platformKey({ id: "opk_old", name: "Old script", state: "archived" })] : [platformKey()],
      ),
    );
    const { user } = await renderApp(PATH);
    await cellOf("Terraform");
    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Active", "Archived", "All"]);
    await user.click(screen.getByRole("tab", { name: "Archived" }));
    expect(await cellOf("Old script")).toBeInTheDocument();
    expect(within(screen.getByRole("grid", { name: "Platform keys" })).queryByRole("gridcell", { name: /Terraform/ })).not.toBeInTheDocument();
  });

  it("says when no platform key is archived", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", paged([platformKey()]));
    const { user } = await renderApp(PATH);
    await cellOf("Terraform");
    await user.click(screen.getByRole("tab", { name: "Archived" }));
    expect(await screen.findByText("No archived platform keys")).toBeInTheDocument();
  });

  it("says when there are no active keys, and none at all under All", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", paged([]));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("No active platform keys")).toBeInTheDocument();
    expect(screen.getByText("Create one for scripts that manage the organization; archived keys are under Archived.")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "All" }));
    expect(await screen.findByText("No platform keys yet")).toBeInTheDocument();
    expect(screen.getByText("Create one for scripts that manage the organization through the Platform API.")).toBeInTheDocument();
    expect(keyQueries()).toEqual(["false", "true"]);
  });

  it("says where the other keys are when none is active", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", paged([platformKey({ state: "disabled" })]));
    await renderApp(PATH);
    expect(await screen.findByText("No active platform keys")).toBeInTheDocument();
    expect(screen.getByText("Disabled and expired keys are under All.")).toBeInTheDocument();
  });

  it("says when the keys couldn't load", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", apiError(403, "forbidden", "Platform keys are off on this install."));
    await renderApp(PATH);
    expect(await screen.findByText("The platform keys couldn't load")).toBeInTheDocument();
  });

  it("hides the page from people who can't view platform keys", async () => {
    withRights(["org.view"], { role: "auditor" });
    await renderApp(PATH);
    expect(await screen.findByText("You can't see the platform keys here")).toBeInTheDocument();
    expect(requests("GET", "orgs/:org/platform-keys")).toHaveLength(0);
  });

  it("offers no actions and no creating without the rights", async () => {
    withRights(["org.view", "platform_keys.view"], { role: "auditor" });
    on("GET", "orgs/:org/platform-keys", paged([platformKey()]));
    const { user } = await renderApp(PATH);
    expect(await cellOf("Terraform")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Actions for Terraform" })).not.toBeInTheDocument();
    const create = screen.getByRole("button", { name: "Create platform key" });
    expect(create).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, create, "You need Create platform keys; ask an owner or admin for it.");
  });

  it("offers no actions on an archived key", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", paged([platformKey({ state: "archived" })]));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("tab", { name: "All" }));
    expect(await cellOf("Terraform")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Actions for Terraform" })).not.toBeInTheDocument();
  });

  it("disables an active key", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", paged([platformKey()]));
    on("POST", "orgs/:org/platform-keys/:key/disable", platformKey({ state: "disabled" }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for Terraform" }));
    expect(screen.queryByRole("menuitem", { name: "Enable" })).not.toBeInTheDocument();
    await user.click(await screen.findByRole("menuitem", { name: "Disable" }));
    expect(await notifications().findByText("Terraform disabled")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/platform-keys/opk_terraform/disable")).toHaveLength(1);
  });

  it("enables a disabled key", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", paged([platformKey({ state: "disabled" })]));
    on("POST", "orgs/:org/platform-keys/:key/enable", platformKey());
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("tab", { name: "All" }));
    await user.click(await screen.findByRole("button", { name: "Actions for Terraform" }));
    await user.click(await screen.findByRole("menuitem", { name: "Enable" }));
    expect(await notifications().findByText("Terraform enabled")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/platform-keys/opk_terraform/enable")).toHaveLength(1);
  });

  it("offers only archiving for an expired key", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", paged([platformKey({ state: "expired", expires_at: fromNow(-DAY) })]));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("tab", { name: "All" }));
    expect(await cellOf("Terraform")).toHaveTextContent("Expired");
    await user.click(await screen.findByRole("button", { name: "Actions for Terraform" }));
    expect(await screen.findByRole("menuitem", { name: "Archive" })).toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "Disable" })).not.toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "Enable" })).not.toBeInTheDocument();
  });

  it("archives a key once confirmed", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", paged([platformKey()]));
    on("POST", "orgs/:org/platform-keys/:key/archive", platformKey({ state: "archived" }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for Terraform" }));
    await user.click(await screen.findByRole("menuitem", { name: "Archive" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Archive Terraform?" });
    expect(within(confirm).getByText("Scripts using it stop authenticating; archiving can't be undone, and its history stays.")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Archive key" }));
    expect(await notifications().findByText("Terraform archived")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/platform-keys/opk_terraform/archive")).toHaveLength(1);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("closes the confirmation and says why when archiving is refused", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", paged([platformKey()]));
    on("POST", "orgs/:org/platform-keys/:key/archive", apiError(409, "conflict", "The key is in use by a running job."));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for Terraform" }));
    await user.click(await screen.findByRole("menuitem", { name: "Archive" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Archive Terraform?" });
    await user.click(within(confirm).getByRole("button", { name: "Archive key" }));
    expect((await notifications().findAllByText("The key is in use by a running job.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("cancels archiving", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", paged([platformKey()]));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for Terraform" }));
    await user.click(await screen.findByRole("menuitem", { name: "Archive" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Archive Terraform?" });
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(requests("POST", "orgs/:org/platform-keys/:key/archive")).toHaveLength(0);
  });
});

describe("creating a platform key", () => {
  it("creates a key that expires in 90 days and shows its secret once", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", paged([]));
    on("POST", "orgs/:org/platform-keys", createdPlatformKey({ name: "Billing sync" }));
    const { user, dialog } = await openCreate();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Give the key a name");
    await fill(user, within(dialog).getByRole("textbox", { name: "Name" }), " Billing sync ");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("Choose the shortest time the key is needed.")).toBeInTheDocument();
    expect(within(dialog).getByRole("combobox", { name: "Expires" })).toHaveTextContent("In 90 days");
    await user.click(within(dialog).getByRole("button", { name: "Create key" }));
    const reveal = await screen.findByRole("dialog", { name: "Platform key created" });
    expect(reveal).toBe(dialog);
    expect(within(reveal).getByText("Copy the key for Billing sync now; it's shown once and Kaeso stores only a hash.")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/platform-keys").map((r) => r.body)).toEqual([{ name: "Billing sync", expires_in_days: 90 }]);
    await user.click(within(reveal).getByRole("button", { name: "Done" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Did you copy the key?" });
    await user.click(within(confirm).getByRole("button", { name: "Go back" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    await user.keyboard("{Escape}");
    await user.click(within(await screen.findByRole("alertdialog", { name: "Did you copy the key?" })).getByRole("button", { name: "Close without copying" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Platform key created" })).not.toBeInTheDocument());
  });

  it("closes the secret at once after it was copied", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", paged([]));
    on("POST", "orgs/:org/platform-keys", createdPlatformKey());
    const { user, dialog } = await openCreate();
    await fill(user, within(dialog).getByRole("textbox", { name: "Name" }), "Terraform");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("combobox", { name: "Expires" }));
    await user.click(await screen.findByRole("option", { name: "Never" }));
    await user.click(within(dialog).getByRole("button", { name: "Create key" }));
    const reveal = await screen.findByRole("dialog", { name: "Platform key created" });
    expect(requests("POST", "orgs/:org/platform-keys").map((r) => r.body)).toEqual([{ name: "Terraform" }]);
    expect(within(reveal).getAllByText(/kso_org_secret_value/).length).toBeGreaterThan(0);
    await user.click(within(reveal).getAllByRole("button", { name: /Copy/ })[0] as HTMLElement);
    expect(cachedText()).toContain("kso_org_secret_value");
    await user.click(within(reveal).getByRole("button", { name: "Done" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Platform key created" })).not.toBeInTheDocument());
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    await waitFor(() => expect(cachedText()).not.toContain("kso_org_secret_value"));
  });

  it("needs a date when the key expires on one, and keeps within the organization's limit", async () => {
    signedIn();
    on("GET", "orgs/:org", org({ max_key_lifetime_days: 30 }));
    on("GET", "orgs/:org/platform-keys", paged([]));
    const { user, dialog } = await openCreate();
    await fill(user, within(dialog).getByRole("textbox", { name: "Name" }), "Terraform");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("This organization allows keys for up to 30 days.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("combobox", { name: "Expires" }));
    expect(screen.queryByRole("option", { name: "Never" })).not.toBeInTheDocument();
    expect(screen.queryByRole("option", { name: "In 90 days" })).not.toBeInTheDocument();
    await user.click(await screen.findByRole("option", { name: "On a date" }));
    const create = within(dialog).getByRole("button", { name: "Create key" });
    await hoverReason(user, create, "Choose the expiry date");
    await user.click(create);
    expect(requests("POST", "orgs/:org/platform-keys")).toHaveLength(0);
  });

  it("creates a key from the c shortcut that expires on a chosen day", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", paged([]));
    on("POST", "orgs/:org/platform-keys", createdPlatformKey());
    const { user } = await renderApp(PATH);
    await screen.findByText("No active platform keys");
    await user.keyboard("c");
    const dialog = await screen.findByRole("dialog", { name: "Create platform key" });
    await fill(user, within(dialog).getByRole("textbox", { name: "Name" }), "Terraform");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("combobox", { name: "Expires" }));
    await user.click(await screen.findByRole("option", { name: "On a date" }));
    await user.click(within(dialog).getByRole("button", { name: /Expiry date/ }));
    const calendar = await screen.findByRole("dialog", { name: /date/i });
    const days = () =>
      within(calendar)
        .queryAllByRole("button")
        .filter((b) => /, \d{4}$/.test(b.getAttribute("aria-label") ?? "") && !b.hasAttribute("disabled"));
    if (days().length === 0) await user.click(within(calendar).getByRole("button", { name: /next month/i }));
    await user.click(days()[0] as HTMLElement);
    await waitFor(() => expect(within(dialog).getByRole("button", { name: "Create key" })).not.toHaveAttribute("aria-disabled", "true"));
    await user.click(within(dialog).getByRole("button", { name: "Create key" }));
    await screen.findByRole("dialog", { name: "Platform key created" });
    expect(requests("POST", "orgs/:org/platform-keys").map((r) => r.body)).toEqual([
      { name: "Terraform", expires_on: expect.stringMatching(/^\d{4}-\d{2}-\d{2}$/) },
    ]);
  });

  it("stays open and says why when the key is refused", async () => {
    signedIn();
    on("GET", "orgs/:org/platform-keys", paged([]));
    on("POST", "orgs/:org/platform-keys", apiError(422, "invalid_request", "That name is taken.", { param: "name" }));
    const { user, dialog } = await openCreate();
    await fill(user, within(dialog).getByRole("textbox", { name: "Name" }), "Terraform");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("button", { name: "Create key" }));
    expect((await notifications().findAllByText("That name is taken.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("dialog", { name: "Create platform key" })).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Create platform key" })).not.toBeInTheDocument());
  });
});
