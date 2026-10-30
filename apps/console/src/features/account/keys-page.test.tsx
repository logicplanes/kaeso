import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, OTHER_PROJECT, PROJECT, membership, myMembership, myProject, signedIn, whoAmI } from "~/test/fixtures";
import { key } from "~/test/fixtures/instance-account";
import { paged } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = "/settings/account/keys";
const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));
const grid = () => screen.getByRole("grid", { name: "Your keys" });
const keyQueries = () => requests("GET", "me/keys").map((r) => Object.fromEntries(r.query));

function keys(list = [key()]) {
  on("GET", "me/keys", paged(list));
}

function act(action: "disable" | "enable" | "archive") {
  on("POST", `me/keys/:key/${action}`, ({ params }: { params: Record<string, string> }) =>
    key({ id: params.key ?? "", state: action === "enable" ? "active" : action === "disable" ? "disabled" : "archived" }),
  );
}

describe("your API keys", () => {
  it("groups the keys by project with their expiry and last use", async () => {
    signedIn();
    keys([
      key({ last_used_at: "2026-10-15T11:00:00Z" }),
      key({ id: "key_soon", name: "triage-bot", project_id: OTHER_PROJECT, expires_at: new Date(Date.now() + 3 * 86_400_000).toISOString(), access: "both" }),
      key({ id: "key_gone", name: "orphan", project_id: "proj_gone" }),
    ]);
    await renderApp(PATH);
    await within(await screen.findByRole("grid", { name: "Your keys" })).findByRole("gridcell", { name: /support-bot/ });
    expect(within(grid()).getByRole("button", { name: /Checkout assistant/ })).toBeInTheDocument();
    expect(within(grid()).getByRole("button", { name: /Support triage/ })).toBeInTheDocument();
    expect(within(grid()).getByRole("button", { name: /proj_gone/ })).toBeInTheDocument();
    expect(within(grid()).getByRole("gridcell", { name: /support-bot/ })).toHaveTextContent(/Used /);
    expect(within(grid()).getByRole("gridcell", { name: /support-bot/ })).toHaveTextContent("Never expires");
    expect(within(grid()).getByRole("gridcell", { name: /triage-bot/ })).toHaveTextContent("Expires in 3 days");
    expect(within(grid()).getByRole("gridcell", { name: /triage-bot/ })).toHaveTextContent("Never used");
    expect(keyQueries()).toEqual([{ limit: "100", include_archived: "false" }]);
  });

  it("names the organization of each project when there are several", async () => {
    signedIn({
      session: whoAmI({ memberships: [membership(), membership({ org_id: "org_contoso", org_name: "Contoso" })] }),
      memberships: [myMembership(), myMembership({ org_id: "org_contoso", org_name: "Contoso", projects: [myProject({ id: "proj_c", name: "Billing" })] })],
    });
    keys([key(), key({ id: "key_c", name: "billing-bot", project_id: "proj_c" })]);
    await renderApp(PATH);
    await within(await screen.findByRole("grid", { name: "Your keys" })).findByRole("gridcell", { name: /billing-bot/ });
    expect(within(grid()).getByRole("button", { name: /Northwind Labs \/ Checkout assistant/ })).toBeInTheDocument();
    expect(within(grid()).getByRole("button", { name: /Contoso \/ Billing/ })).toBeInTheDocument();
  });

  it("reads every page of keys", async () => {
    signedIn();
    on("GET", "me/keys", ({ query }: { query: URLSearchParams }) =>
      query.get("after") === "cur_2" ? paged([key({ id: "key_two", name: "second-page" })]) : paged([key()], "cur_2"),
    );
    await renderApp(PATH);
    expect(await within(await screen.findByRole("grid", { name: "Your keys" })).findByRole("gridcell", { name: /second-page/ })).toBeInTheDocument();
    expect(keyQueries()).toEqual([
      { limit: "100", include_archived: "false" },
      { limit: "100", include_archived: "false", after: "cur_2" },
    ]);
  });

  it("says when there are no active keys, then none at all", async () => {
    signedIn();
    keys([]);
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("No active keys")).toBeInTheDocument();
    expect(screen.getByText("Archived keys are under Archived; create new ones on a project's API keys page.")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "All" }));
    expect(await screen.findByText("No keys yet")).toBeInTheDocument();
    expect(keyQueries()).toContainEqual({ limit: "100", include_archived: "true" });
  });

  it("lists only active keys under Active and every key under All", async () => {
    signedIn();
    keys([
      key(),
      key({ id: "key_off", name: "paused-bot", state: "disabled" }),
      key({ id: "key_old", name: "old-bot", state: "expired", expires_at: "2026-09-01T12:00:00Z" }),
    ]);
    const { user } = await renderApp(PATH);
    await within(await screen.findByRole("grid", { name: "Your keys" })).findByRole("gridcell", { name: /support-bot/ });
    expect(within(grid()).queryByRole("gridcell", { name: /paused-bot/ })).not.toBeInTheDocument();
    expect(within(grid()).queryByRole("gridcell", { name: /old-bot/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "All" }));
    expect(await within(grid()).findByRole("gridcell", { name: /paused-bot/ })).toBeInTheDocument();
    expect(within(grid()).getByRole("gridcell", { name: /old-bot/ })).toBeInTheDocument();
  });

  it("lists only archived keys under Archived, and says when there are none", async () => {
    signedIn();
    keys([key(), key({ id: "key_gone", name: "gone-bot", state: "archived" })]);
    const { user } = await renderApp(PATH);
    await within(await screen.findByRole("grid", { name: "Your keys" })).findByRole("gridcell", { name: /support-bot/ });
    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Active", "Archived", "All"]);
    await user.click(screen.getByRole("tab", { name: "Archived" }));
    expect(await within(grid()).findByRole("gridcell", { name: /gone-bot/ })).toBeInTheDocument();
    expect(within(grid()).queryByRole("gridcell", { name: /support-bot/ })).not.toBeInTheDocument();
  });

  it("says when no key of yours is archived", async () => {
    signedIn();
    keys([key()]);
    const { user } = await renderApp(PATH);
    await within(await screen.findByRole("grid", { name: "Your keys" })).findByRole("gridcell", { name: /support-bot/ });
    await user.click(screen.getByRole("tab", { name: "Archived" }));
    expect(await screen.findByText("No archived keys")).toBeInTheDocument();
  });

  it("says where the other keys are when none is active", async () => {
    signedIn();
    keys([key({ state: "disabled" })]);
    await renderApp(PATH);
    expect(await screen.findByText("No active keys")).toBeInTheDocument();
    expect(screen.getByText("Disabled and expired keys are under All.")).toBeInTheDocument();
  });

  it("says when the keys couldn't load", async () => {
    signedIn();
    on("GET", "me/keys", apiError(429, "rate_limited", "Too many requests; try again in a minute."));
    await renderApp(PATH);
    expect(await screen.findByText("Your keys couldn't load")).toBeInTheDocument();
    expect(screen.queryByText("No active keys")).not.toBeInTheDocument();
  });

  it("opens a key in its project", async () => {
    signedIn();
    keys();
    const { user } = await renderApp(PATH);
    await user.click(await within(await screen.findByRole("grid", { name: "Your keys" })).findByRole("gridcell", { name: /support-bot/ }));
    await waitFor(() => expect(window.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/keys`));
    expect(window.location.search).toBe("?key=key_support");
  });

  it("stays put when the key's project isn't one of the person's", async () => {
    signedIn();
    keys([key({ project_id: "proj_gone" })]);
    const { user } = await renderApp(PATH);
    await user.click(await within(await screen.findByRole("grid", { name: "Your keys" })).findByRole("gridcell", { name: /support-bot/ }));
    expect(window.location.pathname).toBe(PATH);
  });

  it("disables an active key", async () => {
    signedIn();
    keys();
    act("disable");
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for support-bot" }));
    expect(screen.queryByRole("menuitem", { name: "Enable" })).not.toBeInTheDocument();
    await user.click(await screen.findByRole("menuitem", { name: "Disable" }));
    expect(await (await notifications()).findByText("support-bot disabled")).toBeInTheDocument();
    expect(requests("POST", "me/keys/:key/disable").map((r) => r.params.key)).toEqual(["key_support"]);
  });

  it("undoes a disable from its toast", async () => {
    signedIn();
    keys();
    act("disable");
    act("enable");
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for support-bot" }));
    await user.click(await screen.findByRole("menuitem", { name: "Disable" }));
    const toasts = await notifications();
    await user.click(await toasts.findByRole("button", { name: "Undo" }));
    await waitFor(() => expect(requests("POST", "me/keys/:key/enable").map((r) => r.params.key)).toEqual(["key_support"]));
  });

  it("keeps one toast per key, so a key toggled again shows only its newest state and Undo", async () => {
    signedIn();
    keys();
    act("disable");
    act("enable");
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for support-bot" }));
    await user.click(await screen.findByRole("menuitem", { name: "Disable" }));
    const toasts = await notifications();
    await user.click(await toasts.findByRole("button", { name: "Undo" }));
    expect(await toasts.findByText("support-bot enabled")).toBeInTheDocument();
    await waitFor(() => expect(toasts.queryByText("support-bot disabled")).not.toBeInTheDocument());
    expect(toasts.getAllByRole("button", { name: "Undo" })).toHaveLength(1);
  });

  it("undoes an enable from its toast", async () => {
    signedIn();
    keys([key({ state: "disabled" })]);
    act("enable");
    act("disable");
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("tab", { name: "All" }));
    await user.click(await screen.findByRole("button", { name: "Actions for support-bot" }));
    await user.click(await screen.findByRole("menuitem", { name: "Enable" }));
    const toasts = await notifications();
    await user.click(await toasts.findByRole("button", { name: "Undo" }));
    await waitFor(() => expect(requests("POST", "me/keys/:key/disable").map((r) => r.params.key)).toEqual(["key_support"]));
  });

  it("enables a disabled key", async () => {
    signedIn();
    keys([key({ state: "disabled" })]);
    act("enable");
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("tab", { name: "All" }));
    await user.click(await screen.findByRole("button", { name: "Actions for support-bot" }));
    await user.click(await screen.findByRole("menuitem", { name: "Enable" }));
    expect(await (await notifications()).findByText("support-bot enabled")).toBeInTheDocument();
  });

  it("offers only archiving for an expired key", async () => {
    signedIn();
    keys([key({ state: "expired", expires_at: "2026-09-01T12:00:00Z" })]);
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("tab", { name: "All" }));
    await user.click(await screen.findByRole("button", { name: "Actions for support-bot" }));
    expect(await screen.findByRole("menuitem", { name: "Archive" })).toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "Enable" })).not.toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "Disable" })).not.toBeInTheDocument();
  });

  it("archives a key after confirming", async () => {
    signedIn();
    keys();
    act("archive");
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for support-bot" }));
    await user.click(await screen.findByRole("menuitem", { name: "Archive" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Archive support-bot?" });
    await user.click(within(confirm).getByRole("button", { name: "Archive key" }));
    expect(await (await notifications()).findByText("support-bot archived")).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(requests("POST", "me/keys/:key/archive")).toHaveLength(1);
  });

  it("closes the archive confirmation without archiving", async () => {
    signedIn();
    keys();
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for support-bot" }));
    await user.click(await screen.findByRole("menuitem", { name: "Archive" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Archive support-bot?" });
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(requests("POST", "me/keys/:key/archive")).toHaveLength(0);
  });

  it("keeps a key that is being disabled from making another key's archive look busy", async () => {
    signedIn();
    keys([key(), key({ id: "key_triage", name: "triage-bot" })]);
    on("POST", "me/keys/:key/disable", () => new Promise(() => undefined));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for support-bot" }));
    await user.click(await screen.findByRole("menuitem", { name: "Disable" }));
    await waitFor(() => expect(requests("POST", "me/keys/:key/disable")).toHaveLength(1));

    await user.click(screen.getByRole("button", { name: "Actions for triage-bot" }));
    await user.click(await screen.findByRole("menuitem", { name: "Archive" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Archive triage-bot?" });
    expect(within(confirm).getByRole("button", { name: "Archive key" })).not.toHaveAttribute("aria-busy");
    expect(within(confirm).getByRole("button", { name: "Cancel" })).toBeEnabled();
  });

  it("opens the archive confirmation fresh after a refused archive", async () => {
    signedIn();
    keys();
    on("POST", "me/keys/:key/archive", apiError(409, "conflict", "That key is in use by a running export."));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for support-bot" }));
    await user.click(await screen.findByRole("menuitem", { name: "Archive" }));
    let confirm = await screen.findByRole("alertdialog", { name: "Archive support-bot?" });
    await user.click(within(confirm).getByRole("button", { name: "Archive key" }));
    expect((await (await notifications()).findAllByText("That key is in use by a running export.")).length).toBeGreaterThan(0);
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());

    act("archive");
    await user.click(screen.getByRole("button", { name: "Actions for support-bot" }));
    await user.click(await screen.findByRole("menuitem", { name: "Archive" }));
    confirm = await screen.findByRole("alertdialog", { name: "Archive support-bot?" });
    expect(within(confirm).getByRole("button", { name: "Archive key" })).not.toHaveAttribute("aria-busy");
    await user.click(within(confirm).getByRole("button", { name: "Archive key" }));
    expect(await (await notifications()).findByText("support-bot archived")).toBeInTheDocument();
    expect(requests("POST", "me/keys/:key/archive")).toHaveLength(2);
  });

  it("shows why a change was refused", async () => {
    signedIn();
    keys();
    on("POST", "me/keys/:key/disable", apiError(409, "conflict", "That key is already disabled."));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for support-bot" }));
    await user.click(await screen.findByRole("menuitem", { name: "Disable" }));
    expect((await (await notifications()).findAllByText("That key is already disabled.")).length).toBeGreaterThan(0);
  });

  it("offers no actions for archived keys or keys in a read-only organization", async () => {
    signedIn({
      session: whoAmI({ memberships: [membership({ org_state: "scheduled_for_deletion" }), membership({ org_id: "org_contoso", org_name: "Contoso" })] }),
      memberships: [
        myMembership({ org_state: "scheduled_for_deletion" }),
        myMembership({ org_id: "org_contoso", org_name: "Contoso", projects: [myProject({ id: "proj_c", name: "Billing" })] }),
      ],
    });
    keys([key(), key({ id: "key_old", name: "old-bot", project_id: "proj_c", state: "archived" })]);
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("tab", { name: "All" }));
    await within(await screen.findByRole("grid", { name: "Your keys" })).findByRole("gridcell", { name: /old-bot/ });
    expect(screen.queryByRole("button", { name: "Actions for support-bot" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Actions for old-bot" })).not.toBeInTheDocument();
  });
});
