import { describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, signedIn, signedOut, user as person, whoAmI } from "~/test/fixtures";
import { agents, grantable } from "~/test/fixtures/agents";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PENDING = "kaeso:agent-request";

function request() {
  on("POST", "me/agent-requests/view", { client: "Claude Code", redirect_host: "127.0.0.1", expires_at: "2026-10-15T12:15:00Z" });
  on("GET", "me/agents/catalogue", agents);
  on("GET", "me/agents/grantable", grantable());
}

describe("connecting an agent", () => {
  it("asks for the agent's link when the page opens without one", async () => {
    signedIn();
    await renderApp("/authorize");
    expect(await screen.findByText("This sign-in link can't be used")).toBeInTheDocument();
    expect(screen.getByText("Start the sign-in from the agent again; it opens this page with its request.")).toBeInTheDocument();
  });

  it("explains a link the gateway refused", async () => {
    signedIn();
    await renderApp("/authorize#error=invalid_redirect_uri");
    expect(await screen.findByText("The agent asked to return to an address it didn't register; connect it again.")).toBeInTheDocument();
  });

  it("keeps the request while the person signs in, then asks them", async () => {
    signedOut();
    await renderApp("/authorize#request=req_tok");
    await waitFor(() => expect(window.location.pathname).toBe("/sign-in"));
    expect(window.sessionStorage.getItem(PENDING)).toBe("req_tok");
  });

  it("shows the agent, the default preset and its summary, and authorizes it", async () => {
    signedIn();
    request();
    on("POST", "me/agent-requests/approve", { redirect_to: "http://127.0.0.1:3333/callback?code=c&state=s", agent: "agt_new" });
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    const { user } = await renderApp("/authorize#request=req_tok");
    expect(await screen.findByRole("heading", { name: "Connect Claude Code" })).toBeInTheDocument();
    expect(window.location.hash).toBe("");
    expect(screen.getByText("Signed in as owner@example.com · returns to 127.0.0.1")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Builder" })).toBeChecked();
    expect(
      within(screen.getByRole("list", { name: "What it can do" }))
        .getAllByRole("listitem")
        .map((li) => li.textContent),
    ).toEqual(["Manages projects, keys, and models in Northwind", "Reads the rest of Northwind", "Also in organizations you join later"]);
    expect(screen.getByText("Until you revoke it")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Customize" }));
    const dialog = await screen.findByRole("dialog", { name: "Customize Claude Code" });
    expect(within(dialog).getByText("Nothing changes until you authorize.")).toBeInTheDocument();
    await user.clear(within(dialog).getByLabelText("Name"));
    await user.type(within(dialog).getByLabelText("Name"), "Release helper");
    expect(within(dialog).getByRole("heading", { name: "Customize Release helper" })).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Finish" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(screen.getByRole("heading", { name: "Connect Release helper" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Authorize" }));
    await waitFor(() => expect(assign).toHaveBeenCalledWith("http://127.0.0.1:3333/callback?code=c&state=s"));
    const [sent] = requests("POST", "me/agent-requests/approve").map((r) => r.body as Record<string, unknown>);
    expect(sent).toMatchObject({ request: "req_tok", name: "Release helper", grant: { future_orgs: expect.any(Object) } });
    expect(sent).not.toHaveProperty("expiry");
    expect((sent?.grant as { orgs: { org: string; included: boolean }[] }).orgs).toEqual(
      expect.arrayContaining([expect.objectContaining({ org: ORG, included: true }), expect.objectContaining({ org: "org_contoso", included: false })]),
    );
  });

  it("names the account by its name when it has no email address", async () => {
    signedIn({ session: whoAmI({ user: { ...person(), email: null } }) });
    request();
    await renderApp("/authorize#request=req_tok");
    expect(await screen.findByText(`Signed in as ${person().name} · returns to 127.0.0.1`)).toBeInTheDocument();
  });

  it("asks before giving full access", async () => {
    signedIn();
    request();
    on("POST", "me/agent-requests/approve", { redirect_to: "http://127.0.0.1:3333/callback?code=c", agent: "agt_new" });
    vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    const { user } = await renderApp("/authorize#request=req_tok");
    await user.click(await screen.findByRole("radio", { name: "Full access" }));
    await user.click(screen.getByRole("button", { name: "Authorize" }));
    expect(await screen.findByText("Give this agent sensitive rights?")).toBeInTheDocument();
    expect(requests("POST", "me/agent-requests/approve")).toHaveLength(0);
    await user.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "Authorize" }));
    await waitFor(() => expect(requests("POST", "me/agent-requests/approve")).toHaveLength(1));
  });

  it("denies and sends the browser back", async () => {
    signedIn();
    request();
    on("POST", "me/agent-requests/deny", { redirect_to: "http://127.0.0.1:3333/callback?error=access_denied", agent: null });
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    const { user } = await renderApp("/authorize#request=req_tok");
    await user.click(await screen.findByRole("button", { name: "Deny" }));
    await waitFor(() => expect(assign).toHaveBeenCalledWith("http://127.0.0.1:3333/callback?error=access_denied"));
    expect(window.sessionStorage.getItem(PENDING)).toBeNull();
  });

  it("says when the request has ended", async () => {
    signedIn();
    request();
    on("POST", "me/agent-requests/view", apiError(404, "not_found", "This sign-in request has ended or was used; connect the agent again."));
    await renderApp("/authorize#request=req_old");
    expect(await screen.findByText("This request has ended")).toBeInTheDocument();
  });

  it("treats an error it doesn't know as a missing link", async () => {
    signedIn();
    await renderApp("/authorize#error=constructor");
    expect(await screen.findByText("Start the sign-in from the agent again; it opens this page with its request.")).toBeInTheDocument();
  });

  it("names an agent Kaeso doesn't know", async () => {
    signedIn();
    await renderApp("/authorize#error=invalid_client");
    expect(await screen.findByText("Kaeso doesn't know this agent; connect it again from the agent.")).toBeInTheDocument();
  });

  it("offers to try again when the request couldn't be opened", async () => {
    signedIn();
    request();
    on("GET", "me/agents/grantable", apiError(503, "unavailable", "Kaeso cannot reach its database right now; try again shortly."));
    const { user } = await renderApp("/authorize#request=req_tok");
    expect(await screen.findByText("The request couldn't be opened")).toBeInTheDocument();
    expect(screen.getByText("Can't reach the gateway; try again.")).toBeInTheDocument();
    on("GET", "me/agents/grantable", grantable());
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("heading", { name: "Connect Claude Code" })).toBeInTheDocument();
  });

  it("shows the gateway's reason when the request can't be read", async () => {
    signedIn();
    request();
    on("POST", "me/agent-requests/view", apiError(503, "unavailable", "Kaeso cannot reach its database right now; try again shortly."));
    await renderApp("/authorize#request=req_tok");
    expect(await screen.findByText("The request couldn't be opened")).toBeInTheDocument();
    expect(screen.getAllByText("Kaeso cannot reach its database right now; try again shortly.").length).toBeGreaterThan(0);
  });

  it("authorizes a grant without sensitive rights without asking", async () => {
    signedIn();
    request();
    on("GET", "me/agents/grantable", grantable({ orgs: [] }));
    on("POST", "me/agent-requests/approve", { redirect_to: "http://127.0.0.1:3333/callback?code=c", agent: "agt_new" });
    vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    const { user } = await renderApp("/authorize#request=req_tok");
    await user.click(await screen.findByRole("radio", { name: "Observer" }));
    await user.click(screen.getByRole("button", { name: "Authorize" }));
    await waitFor(() => expect(requests("POST", "me/agent-requests/approve")).toHaveLength(1));
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });
});
