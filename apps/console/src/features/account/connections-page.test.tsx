import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PersonConnectionView } from "~/api/generated/types";
import { ORG, signedIn } from "~/test/fixtures";
import { notifications } from "~/test/fixtures/org";
import { mcpConnection } from "~/test/fixtures/mcp";
import { apiError, noContent, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const MINE = "me/mcp-connections";
const ONE = `${MINE}/:server`;
const PATH = "/settings/account/connections";

function person(o: Partial<PersonConnectionView> = {}): PersonConnectionView {
  return { server: "mcp_x", server_name: "X", server_handle: "x", org: ORG, auth_method: "oauth", connection: null, ...o };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("Account › Connections", () => {
  it("lists the servers that ask for your own account, with how you're connected", async () => {
    signedIn();
    on(
      "GET",
      MINE,
      page([
        person(),
        person({
          server: "mcp_github",
          server_name: "GitHub",
          auth_method: "header",
          connection: mcpConnection({ kind: "header", header_name: "X-Api-Key", account: null }),
        }),
        person({ server: "mcp_drive", server_name: "Drive", connection: mcpConnection({ signed_out: true }) }),
        person({ server: "mcp_mail", server_name: "Mail", org: "org_gone", connection: mcpConnection() }),
      ]),
    );
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Connections" })).toBeInTheDocument();
    expect(screen.getByText("Not connected")).toBeInTheDocument();
    expect(screen.getByText("Connected")).toBeInTheDocument();
    expect(screen.getByText("Signed out")).toBeInTheDocument();
    expect(screen.getByText("Connected as @kaesoapp")).toBeInTheDocument();
    expect(screen.getAllByText("Northwind Labs").length).toBeGreaterThan(0);
    expect(screen.getByText("org_gone")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Replace secret" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Sign in again" })).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Disconnect" })).toHaveLength(3);
  });

  it("connects a secret header", async () => {
    signedIn();
    on("GET", MINE, page([person({ auth_method: "header" })]));
    on("POST", ONE, { redirect_url: null, connection: person({ auth_method: "header", connection: mcpConnection({ kind: "header" }) }) });
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Connect" }));
    const dialog = await screen.findByRole("dialog", { name: "Connect to X" });
    expect(within(dialog).getByRole("textbox", { name: "Header" })).toHaveValue("Authorization");
    await user.clear(within(dialog).getByRole("textbox", { name: "Header" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Header" }), "X-Api-Key");
    await user.type(within(dialog).getByLabelText("Value"), "k-1");
    await user.click(within(dialog).getByRole("button", { name: "Connect" }));
    expect(await notifications().findByText("Connected to X")).toBeInTheDocument();
    expect(requests("POST", ONE)[0]?.body).toEqual({ header: { name: "X-Api-Key", value: "k-1" } });
  });

  it("asks for the header before connecting, and closes without connecting", async () => {
    signedIn();
    on("GET", MINE, page([person({ auth_method: "header", connection: mcpConnection({ kind: "header", header_name: "X-Api-Key" }) })]));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Replace secret" }));
    const dialog = await screen.findByRole("dialog", { name: "Connect to X" });
    expect(within(dialog).getByRole("textbox", { name: "Header" })).toHaveValue("X-Api-Key");
    expect(within(dialog).getByRole("button", { name: "Connect" })).toHaveAttribute("aria-disabled", "true");
    await user.clear(within(dialog).getByRole("textbox", { name: "Header" }));
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(requests("POST", ONE)).toHaveLength(0);
  });

  it("signs in with OAuth at the server", async () => {
    signedIn();
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    on("GET", MINE, page([person(), person({ server: "mcp_drive", server_name: "Drive" })]));
    let answer: (v: unknown) => void = () => undefined;
    on("POST", ONE, () => new Promise((resolve) => (answer = resolve)));
    const { user } = await renderApp(PATH);
    await user.click((await screen.findAllByRole("button", { name: "Connect" }))[0] as HTMLElement);
    await waitFor(() => expect(requests("POST", ONE)).toHaveLength(1));
    answer({ redirect_url: "https://auth.x.example/authorize?state=s", connection: person() });
    await waitFor(() => expect(assign).toHaveBeenCalledWith("https://auth.x.example/authorize?state=s"));
    expect(requests("POST", ONE)[0]?.body).toEqual({ oauth: true });
    on("POST", ONE, { redirect_url: null, connection: person() });
    await user.click(screen.getAllByRole("button", { name: "Connect" })[1] as HTMLElement);
    await waitFor(() => expect(requests("POST", ONE)).toHaveLength(2));
    expect(assign).toHaveBeenCalledTimes(1);
  });

  it("disconnects, and closes the confirmation when that fails", async () => {
    signedIn();
    on("GET", MINE, page([person({ connection: mcpConnection() })]));
    on("DELETE", ONE, noContent);
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Disconnect" }));
    let alert = await screen.findByRole("alertdialog", { name: "Disconnect from X?" });
    await user.click(within(alert).getByRole("button", { name: "Disconnect" }));
    expect(await notifications().findByText("Disconnected from X")).toBeInTheDocument();
    on("DELETE", ONE, apiError(404, "not_found", "No such connection, or you cannot see it."));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Disconnect" }));
    alert = await screen.findByRole("alertdialog", { name: "Disconnect from X?" });
    await user.click(within(alert).getByRole("button", { name: "Disconnect" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("says how a sign-in ended and forgets it", async () => {
    signedIn();
    on("GET", MINE, page([person({ connection: mcpConnection() })]));
    await renderApp(`${PATH}?server=mcp_x&result=connected`);
    expect(await notifications().findByText("Signed in to X")).toBeInTheDocument();
    await waitFor(() => expect(window.location.search).toBe(""));
  });

  it("explains an account no server asks for", async () => {
    signedIn();
    on("GET", MINE, page([]));
    await renderApp(PATH);
    expect(await screen.findByText("No servers ask for your own account")).toBeInTheDocument();
  });

  it("says when your connections couldn't load", async () => {
    signedIn();
    on("GET", MINE, apiError(403, "permission_denied", "You can't see these connections."));
    await renderApp(PATH);
    expect(await screen.findByText("Your connections couldn't load")).toBeInTheDocument();
  });
});
