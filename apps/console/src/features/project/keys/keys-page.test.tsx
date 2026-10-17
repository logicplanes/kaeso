import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { NOW, PROJECT, signedIn, whoAmI, without } from "~/test/fixtures";
import { apiKey, listed, projectData, projectMember, projectPath, serviceAccount } from "~/test/fixtures/project";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const KEYS = "orgs/:org/projects/:project/keys";
const keyRequests = () => requests("GET", KEYS).map((r) => Object.fromEntries(r.query));

describe("KeysPage", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("groups every key by state under All and shows each key's expiry, owner and last use", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(NOW);
    signedIn();
    projectData({
      members: [projectMember()],
      accounts: [serviceAccount()],
      keys: [
        apiKey({ last_used_at: "2026-10-15T11:00:00Z" }),
        apiKey({ id: "key_batch", name: "nightly-batch", state: "disabled", principal: { kind: "service_account", id: "sa_deploy" }, access: "both" }),
        apiKey({ id: "key_soon", name: "expiring-soon", expires_at: "2026-10-17T12:00:00Z", principal: { kind: "user", id: "usr_grace" } }),
        apiKey({ id: "key_old", name: "old-key", state: "expired", expires_at: "2026-09-01T12:00:00Z" }),
      ],
    });
    await renderApp(projectPath("keys?archived=true"));

    await screen.findByRole("gridcell", { name: /checkout-web/ });
    const grid = screen.getByRole("grid", { name: "Keys" });
    expect(within(grid).getByRole("button", { name: /Active/ })).toHaveTextContent("2");
    expect(within(grid).getByRole("button", { name: /Disabled/ })).toHaveTextContent("1");
    expect(within(grid).getByRole("button", { name: /Expired/ })).toHaveTextContent("1");
    const web = within(grid).getByRole("gridcell", { name: /checkout-web/ });
    expect(web).toHaveTextContent("Never expires");
    expect(within(web).getByRole("img", { name: "Ada Okafor" })).toBeInTheDocument();
    expect(within(grid).getByRole("gridcell", { name: /expiring-soon/ })).toHaveTextContent("Never used");
    expect(within(grid).getByRole("gridcell", { name: /expiring-soon/ })).toHaveTextContent(/Expires in \d d/);
    expect(within(grid).getByRole("gridcell", { name: /expiring-soon/ })).toHaveTextContent(/Grace Hopper|G/);
    expect(within(grid).getByRole("gridcell", { name: /old-key/ })).toHaveTextContent(/Expired/);
    expect(within(within(grid).getByRole("gridcell", { name: /nightly-batch/ })).getByRole("img", { name: "ci-deploy" })).toBeInTheDocument();
  });

  it("shows an owner's picture on their keys, or their initial without one", async () => {
    signedIn({ session: whoAmI({ user: { ...whoAmI().user, avatar_url: "/avatars/ada.webp" } }) });
    projectData({
      members: [projectMember({ user: { avatar_url: "/avatars/grace.webp" } }), projectMember({ user: { id: "usr_alan", name: "Alan Turing" } })],
      keys: [
        apiKey(),
        apiKey({ id: "key_grace", name: "grace-key", principal: { kind: "user", id: "usr_grace" } }),
        apiKey({ id: "key_alan", name: "alan-key", principal: { kind: "user", id: "usr_alan" } }),
      ],
    });
    await renderApp(projectPath("keys"));

    const grid = await screen.findByRole("grid", { name: "Keys" });
    const cell = (name: RegExp) => within(grid).getByRole("gridcell", { name });
    await waitFor(() => expect(cell(/grace-key/).querySelector('img[src="/avatars/grace.webp"]')).not.toBeNull());
    expect(cell(/checkout-web/).querySelector('img[src="/avatars/ada.webp"]')).not.toBeNull();
    expect(within(cell(/alan-key/)).getByRole("img", { name: "Alan Turing" })).toHaveTextContent("A");
    expect(cell(/alan-key/).querySelector("img[src]")).toBeNull();
  });

  it("shows only active keys under Active", async () => {
    signedIn();
    projectData({
      keys: [
        apiKey(),
        apiKey({ id: "key_batch", name: "nightly-batch", state: "disabled" }),
        apiKey({ id: "key_old", name: "old-key", state: "expired", expires_at: "2026-09-01T12:00:00Z" }),
      ],
    });
    await renderApp(projectPath("keys"));

    await screen.findByRole("gridcell", { name: /checkout-web/ });
    const grid = screen.getByRole("grid", { name: "Keys" });
    expect(screen.getByRole("tab", { name: "Active" })).toHaveAttribute("aria-selected", "true");
    expect(within(grid).getAllByRole("gridcell")).toHaveLength(1);
    expect(within(grid).queryByRole("gridcell", { name: /nightly-batch/ })).not.toBeInTheDocument();
    expect(within(grid).queryByRole("gridcell", { name: /old-key/ })).not.toBeInTheDocument();
    expect(within(grid).queryByRole("button", { name: /Disabled/ })).not.toBeInTheDocument();
  });

  it("says where the other keys are when none is active", async () => {
    signedIn();
    projectData({ keys: [apiKey({ id: "key_batch", name: "nightly-batch", state: "disabled" })] });
    const { user, router } = await renderApp(projectPath("keys"));

    expect(await screen.findByText("No active keys")).toBeInTheDocument();
    expect(screen.getByText("Disabled, expired and archived keys are under All.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Show all keys" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ archived: true }));
    expect(await screen.findByRole("gridcell", { name: /nightly-batch/ })).toBeInTheDocument();
  });

  it("offers to create the first key when the project has none", async () => {
    signedIn();
    projectData();
    const { user } = await renderApp(projectPath("keys"));

    expect(await screen.findByText("No keys yet")).toBeInTheDocument();
    expect(screen.getByText("Create a key, then send it with every request to the gateway.")).toBeInTheDocument();
    const buttons = screen.getAllByRole("button", { name: "Create key" });
    await user.click(buttons[buttons.length - 1] as HTMLElement);
    expect(await screen.findByRole("dialog", { name: /Create key/ })).toBeInTheDocument();
  });

  it("opens the create dialog straight away from ?create=true", async () => {
    signedIn();
    projectData();
    const { user, router } = await renderApp(projectPath("keys?create=true"));
    expect(await screen.findByRole("dialog", { name: /Create key/ })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("filters by owner from the URL and clears the filter", async () => {
    signedIn();
    projectData({ members: [projectMember()], accounts: [serviceAccount()] });
    const { user } = await renderApp(projectPath("keys?owner=user:usr_grace"));

    expect(await screen.findByText("No keys for this filter")).toBeInTheDocument();
    expect(keyRequests()).toContainEqual({ user: "usr_grace", limit: "100" });
    await user.click(screen.getByRole("button", { name: "Clear filter" }));
    expect(await screen.findByText("No keys yet")).toBeInTheDocument();
    expect(keyRequests()).toContainEqual({ limit: "100" });
  });

  it("asks for a service account's keys and for archived keys", async () => {
    signedIn();
    projectData({ accounts: [serviceAccount()] });
    await renderApp(projectPath("keys?owner=sa:sa_deploy&archived=true"));

    expect(await screen.findByText("No keys for this filter")).toBeInTheDocument();
    expect(keyRequests()).toContainEqual({ service_account: "sa_deploy", include_archived: "true", limit: "100" });
    expect(screen.getByRole("tab", { name: "All" })).toHaveAttribute("aria-selected", "true");
  });

  it("includes archived keys when the All tab is chosen", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    const { user } = await renderApp(projectPath("keys"));

    await screen.findByRole("gridcell", { name: /checkout-web/ });
    await user.click(screen.getByRole("tab", { name: "All" }));
    await waitFor(() => expect(keyRequests()).toContainEqual({ include_archived: "true", limit: "100" }));
  });

  it("filters by a member chosen in the owner picker", async () => {
    signedIn();
    projectData({ keys: [apiKey()], members: [projectMember()], accounts: [serviceAccount()] });
    const { user } = await renderApp(projectPath("keys"));

    await screen.findByRole("gridcell", { name: /checkout-web/ });
    await user.click(screen.getByRole("combobox", { name: "Whose keys" }));
    expect(await screen.findByRole("option", { name: "ci-deploy (service account)" })).toBeInTheDocument();
    await user.click(screen.getByRole("option", { name: "Grace Hopper" }));
    await waitFor(() => expect(keyRequests()).toContainEqual({ user: "usr_grace", limit: "100" }));
  });

  it("reads every page of keys", async () => {
    signedIn();
    projectData();
    on("GET", KEYS, ({ query }: { query: URLSearchParams }) =>
      query.get("after") === "cur_2" ? listed([apiKey({ id: "key_two", name: "second-page" })]) : listed([apiKey()], "cur_2"),
    );
    await renderApp(projectPath("keys"));

    expect(await screen.findByRole("gridcell", { name: /second-page/ })).toBeInTheDocument();
    expect(screen.getByRole("gridcell", { name: /checkout-web/ })).toBeInTheDocument();
  });

  it("explains why a key can't be created without the right", async () => {
    signedIn({ projectRights: without("keys.own.create") });
    projectData();
    await renderApp(projectPath("keys"));

    expect(await screen.findByText("No keys yet")).toBeInTheDocument();
    const create = screen.getAllByRole("button", { name: "Create key" });
    expect(create).toHaveLength(1);
    expect(create[0]).toHaveAttribute("aria-disabled", "true");
  });

  it("hides the owner picker when members and service accounts can't be seen", async () => {
    signedIn({ projectRights: without("project.members.view", "service_accounts.view") });
    projectData({ keys: [apiKey({ principal: { kind: "user", id: "usr_grace" } })] });
    await renderApp(projectPath("keys"));

    expect(await screen.findByRole("gridcell", { name: /checkout-web/ })).toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: "Whose keys" })).not.toBeInTheDocument();
    expect(requests("GET", "orgs/:org/projects/:project/members")).toHaveLength(0);
    expect(requests("GET", `orgs/:org/projects/${PROJECT}/service-accounts`)).toHaveLength(0);
  });

  it("opens the create dialog from the header and from the c shortcut", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    const { user } = await renderApp(projectPath("keys"));

    await screen.findByRole("gridcell", { name: /checkout-web/ });
    await user.click(screen.getByRole("button", { name: "Create key" }));
    const dialog = await screen.findByRole("dialog", { name: /Create key/ });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(dialog).not.toBeInTheDocument());
    await user.keyboard("c");
    expect(await screen.findByRole("dialog", { name: /Create key/ })).toBeInTheDocument();
  });

  it("goes back to the active keys from the All tab", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    const { user, router } = await renderApp(projectPath("keys?archived=true"));

    await screen.findByRole("gridcell", { name: /checkout-web/ });
    await user.click(screen.getByRole("tab", { name: "Active" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
    expect(screen.getByRole("tab", { name: "Active" })).toHaveAttribute("aria-selected", "true");
  });

  it("shows everyone's keys again when the owner picker is cleared", async () => {
    signedIn();
    projectData({ keys: [apiKey({ principal: { kind: "user", id: "usr_grace" } })], members: [projectMember()] });
    const { user, router } = await renderApp(projectPath("keys?owner=user:usr_grace"));

    await screen.findByRole("gridcell", { name: /checkout-web/ });
    const picker = screen.getByRole("combobox", { name: "Whose keys" });
    await user.clear(picker);
    await user.tab();
    await waitFor(() => expect(router.state.location.search).toEqual({}));
    await waitFor(() => expect(keyRequests()).toContainEqual({ limit: "100" }));
  });
});
