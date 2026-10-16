import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { KeyView } from "~/api/generated/types";
import { ALL_PROJECT_RIGHTS, signedIn } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { apiKey, projectData, projectMember, projectPath, serviceAccount } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const KEY = "orgs/:org/projects/:project/keys/:key";
const without = (...rights: string[]) => ALL_PROJECT_RIGHTS.filter((r) => !rights.includes(r));
const sheet = (name = "checkout-web") => screen.findByRole("dialog", { name });

function keyApi(key: KeyView): void {
  let current = key;
  on("GET", KEY, () => current);
  const act = (state: KeyView["state"]) => () => (current = { ...current, state });
  on("POST", `${KEY}/disable`, act("disabled"));
  on("POST", `${KEY}/enable`, act("active"));
  on("POST", `${KEY}/archive`, act("archived"));
  on("POST", "me/keys/:key/disable", act("disabled"));
  on("POST", "me/keys/:key/enable", act("active"));
  on("POST", "me/keys/:key/archive", act("archived"));
}

describe("KeySheet", () => {
  it("shows who a key is for, its access, dates and id", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    keyApi(apiKey({ last_used_at: "2026-10-15T11:00:00Z", expires_at: "2099-01-01T00:00:00Z", access: "both" }));
    await renderApp(projectPath("keys?key=key_web"));

    const dialog = await sheet();
    expect(dialog).toHaveTextContent("sk-kso-AbC123…wxyz");
    expect(dialog).toHaveTextContent("Ada Okafor (you)");
    expect(dialog).toHaveTextContent(/by Ada Okafor \(you\)/);
    expect(dialog).toHaveTextContent(/1 Jan 2099 \(expires in [\d,]+ d\)/);
    expect(dialog).toHaveTextContent("Inference + platform");
    expect(dialog).toHaveTextContent("key_web");
    expect(within(dialog).getByText("Limits")).toBeInTheDocument();
    expect(within(dialog).getByText("Models")).toBeInTheDocument();
    expect(requests("GET", "orgs/:org/projects/:project/limits").some((r) => r.query.get("key") === "key_web")).toBe(true);
  });

  it("names a service account's key and says when it expired", async () => {
    signedIn();
    projectData({ keys: [apiKey()], accounts: [serviceAccount()], members: [projectMember()] });
    keyApi(
      apiKey({
        name: "deploy-bot",
        principal: { kind: "service_account", id: "sa_deploy" },
        created_by: { kind: "user", id: "usr_grace" },
        expires_at: "2026-01-01T00:00:00Z",
        state: "expired",
      }),
    );
    await renderApp(projectPath("keys?key=key_web"));

    const dialog = await sheet("deploy-bot");
    expect(await within(dialog).findByText("ci-deploy")).toBeInTheDocument();
    expect(dialog).toHaveTextContent("by Grace Hopper");
    expect(dialog).toHaveTextContent("1 Jan 2026 (expired)");
    await screen.findByRole("button", { name: "Key actions" });
  });

  it("disables a key and undoes it from the toast", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    keyApi(apiKey());
    const { user } = await renderApp(projectPath("keys?key=key_web"));

    await sheet();
    await user.click(await screen.findByRole("button", { name: "Key actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Disable" }));
    expect(await notifications().findByText("Key disabled")).toBeInTheDocument();
    expect(requests("POST", `${KEY}/disable`)).toHaveLength(1);
    await user.click(within(screen.getByRole("region", { name: "Notifications" })).getByRole("button", { name: "Undo" }));
    expect(await notifications().findByText("Key enabled")).toBeInTheDocument();
    expect(requests("POST", `${KEY}/enable`)).toHaveLength(1);
  });

  it("enables a disabled key", async () => {
    signedIn();
    projectData({ keys: [apiKey({ state: "disabled" })] });
    keyApi(apiKey({ state: "disabled" }));
    const { user } = await renderApp(projectPath("keys?key=key_web"));

    await sheet();
    await user.click(await screen.findByRole("button", { name: "Key actions" }));
    expect(screen.queryByRole("menuitem", { name: "Disable" })).not.toBeInTheDocument();
    await user.click(await screen.findByRole("menuitem", { name: "Enable" }));
    expect(await notifications().findByText("Key enabled")).toBeInTheDocument();
    expect(requests("POST", `${KEY}/enable`)).toHaveLength(1);
  });

  it("undoes enabling a key from the toast", async () => {
    signedIn();
    projectData({ keys: [apiKey({ state: "disabled" })] });
    keyApi(apiKey({ state: "disabled" }));
    const { user } = await renderApp(projectPath("keys?key=key_web"));

    await sheet();
    await user.click(await screen.findByRole("button", { name: "Key actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Enable" }));
    expect(await notifications().findByText("Key enabled")).toBeInTheDocument();
    await user.click(within(screen.getByRole("region", { name: "Notifications" })).getByRole("button", { name: "Undo" }));
    expect(await notifications().findByText("Key disabled")).toBeInTheDocument();
    expect(requests("POST", `${KEY}/disable`)).toHaveLength(1);
  });

  it("names the parties it doesn't know and still reads the key's limits", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    keyApi(apiKey({ principal: { kind: "user", id: null }, created_by: { kind: "migration", id: null } }));
    await renderApp(projectPath("keys?key=key_web"));

    const dialog = await sheet();
    expect(dialog).toHaveTextContent("A member");
    expect(dialog).toHaveTextContent(/by migration/);
    expect(within(dialog).getByText("Limits")).toBeInTheDocument();
    await waitFor(() => expect(requests("GET", "orgs/:org/projects/:project/limits").some((r) => r.query.get("key") === "key_web")).toBe(true));
  });

  it("names a party of a kind it doesn't know by its id", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    keyApi(apiKey({ created_by: { kind: "migration", id: "mig_2026" } }));
    await renderApp(projectPath("keys?key=key_web"));

    expect(await sheet()).toHaveTextContent(/by mig_2026/);
  });

  it("keeps the key's state when disabling it fails", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    keyApi(apiKey());
    on("POST", `${KEY}/disable`, apiError(409, "conflict", "This key is in use by a deploy; try again later."));
    const { user } = await renderApp(projectPath("keys?key=key_web"));

    await sheet();
    await user.click(await screen.findByRole("button", { name: "Key actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Disable" }));
    expect((await notifications().findAllByText("This key is in use by a deploy; try again later.")).length).toBeGreaterThan(0);
    await user.click(await screen.findByRole("button", { name: "Key actions" }));
    expect(await screen.findByRole("menuitem", { name: "Disable" })).toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "Enable" })).not.toBeInTheDocument();
  });

  it("archives a key after confirming and closes the sheet", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    keyApi(apiKey());
    const { user, router } = await renderApp(projectPath("keys?key=key_web"));

    await sheet();
    await user.click(await screen.findByRole("button", { name: "Key actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Archive" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Archive checkout-web?" });
    expect(confirm).toHaveTextContent("Applications using it stop authenticating; archiving can't be undone, and its history stays.");
    await user.click(within(confirm).getByRole("button", { name: "Archive key" }));
    expect(await notifications().findByText("Key archived")).toBeInTheDocument();
    expect(requests("POST", `${KEY}/archive`)).toHaveLength(1);
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("manages an own key through the person's own endpoints", async () => {
    signedIn({ projectRights: without("keys.all.manage") });
    projectData({ keys: [apiKey()] });
    keyApi(apiKey());
    const { user } = await renderApp(projectPath("keys?key=key_web"));

    await sheet();
    await user.click(await screen.findByRole("button", { name: "Key actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Disable" }));
    expect(await notifications().findByText("Key disabled")).toBeInTheDocument();
    expect(requests("POST", "me/keys/:key/disable")).toHaveLength(1);
    expect(requests("POST", `${KEY}/disable`)).toHaveLength(0);

    await user.click(await screen.findByRole("button", { name: "Key actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Enable" }));
    expect(await notifications().findByText("Key enabled")).toBeInTheDocument();
    expect(requests("POST", "me/keys/:key/enable")).toHaveLength(1);

    await user.click(await screen.findByRole("button", { name: "Key actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Archive" }));
    await user.click(within(await screen.findByRole("alertdialog", { name: "Archive checkout-web?" })).getByRole("button", { name: "Archive key" }));
    expect(await notifications().findByText("Key archived")).toBeInTheDocument();
    expect(requests("POST", "me/keys/:key/archive")).toHaveLength(1);
  });

  it("explains who can manage someone else's key", async () => {
    signedIn({ projectRights: without("keys.all.manage", "limits.view") });
    projectData({ keys: [apiKey()], members: [projectMember()] });
    keyApi(apiKey({ principal: { kind: "user", id: "usr_grace" } }));
    const { user } = await renderApp(projectPath("keys?key=key_web"));

    const dialog = await sheet();
    expect(await within(dialog).findByText("Grace Hopper")).toBeInTheDocument();
    const actions = await within(dialog).findByRole("button", { name: "Key actions" });
    expect(actions).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, actions, "You need Manage all keys; ask Lin Wei for it.");
    expect(within(dialog).queryByText("Limits")).not.toBeInTheDocument();
    expect(requests("GET", "orgs/:org/projects/:project/keys/:key/models")).toHaveLength(0);
  });

  it("names the right to manage your own key when you hold neither", async () => {
    signedIn({ projectRights: without("keys.all.manage", "keys.own.manage", "limits.view") });
    projectData({ keys: [apiKey()] });
    keyApi(apiKey());
    const { user } = await renderApp(projectPath("keys?key=key_web"));

    const actions = await within(await sheet()).findByRole("button", { name: "Key actions" });
    expect(actions).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, actions, /^You need Manage own keys; ask .+ for it\.$/);
  });

  it("offers no actions on an archived key", async () => {
    signedIn();
    projectData({ keys: [] });
    keyApi(apiKey({ state: "archived" }));
    await renderApp(projectPath("keys?key=key_web"));

    const dialog = await sheet();
    expect(dialog).toHaveTextContent("sk-kso-AbC123…wxyz");
    expect(within(dialog).queryByRole("button", { name: "Key actions" })).not.toBeInTheDocument();
  });

  it("says when a key is gone and goes back to the keys", async () => {
    signedIn();
    projectData();
    on("GET", KEY, apiError(404, "not_found", "No such key."));
    const { user, router } = await renderApp(projectPath("keys?key=key_gone"));

    const dialog = await screen.findByRole("dialog", { name: "Key" });
    expect(await within(dialog).findByText("This key doesn't exist or was removed.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Back to keys" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("says when a key couldn't load", async () => {
    signedIn();
    projectData();
    on("GET", KEY, apiError(403, "permission_denied", "You can't see this key."));
    await renderApp(projectPath("keys?key=key_web"));

    const dialog = await screen.findByRole("dialog", { name: "Key" });
    expect(await within(dialog).findByText("Couldn't load this key.")).toBeInTheDocument();
  });

  it("opens a key from the list", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    keyApi(apiKey());
    const { user, router } = await renderApp(projectPath("keys"));

    await user.click(await screen.findByRole("gridcell", { name: /checkout-web/ }));
    await waitFor(() => expect(router.state.location.search).toEqual({ key: "key_web" }));
    expect(await sheet()).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it.each([
    [{ kind: "system" }, "by System"],
    [{ kind: "org_platform_key", id: "opk_terraform" }, "by Platform key opk_terraform"],
    [{ kind: "org_platform_key" }, "by Platform key"],
    [{ kind: "api_key", id: "key_ci" }, "by API key key_ci"],
    [{ kind: "api_key" }, "by API key"],
    [{ kind: "user" }, "by A member"],
    [{ kind: "service_account" }, "by A service account"],
    [{ kind: "user", id: "usr_left" }, "by usr_left"],
  ] as const)("names the creator %o", async (createdBy, text) => {
    signedIn();
    projectData({ keys: [apiKey()] });
    keyApi(apiKey({ created_by: createdBy as KeyView["created_by"] }));
    await renderApp(projectPath("keys?key=key_web"));
    expect(await sheet()).toHaveTextContent(text);
  });
});
