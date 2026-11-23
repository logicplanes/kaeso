import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { signedIn, without } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { externalSource, projectData, projectMember, projectPath, serviceAccount } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const SOURCES = "orgs/:org/projects/:project/sources";
const SOURCE = `${SOURCES}/:source`;
const KEY = "sk-kso-src-NeWkEyNeWkEyNeWkEyNeWkEyNeWkEyNeWkEyNeWkEyNeWk";
const claude = externalSource({
  activity: [
    { day: "2026-10-14", requests: 12, tool_calls: 3, refused: 0, duplicates: 1 },
    { day: "2026-10-15", requests: 5, tool_calls: 0, refused: 2, duplicates: 0 },
  ],
  first_event_at: "2026-10-14T09:00:00Z",
  last_event_at: "2026-10-15T11:58:00Z",
  last_error: "too_old",
  last_error_at: "2026-10-15T11:00:00Z",
});

function source(s = claude) {
  projectData({ sources: [s], accounts: [serviceAccount()], members: [projectMember({ user: { id: "usr_ada", name: "Ada Okafor" } })] });
  on("GET", SOURCE, s);
}

describe("SourceSheet", () => {
  it("shows what the source is, whom it counts for, what it reported and how to set it up", async () => {
    signedIn();
    source();
    await renderApp(projectPath("sources?source=src_claude"));

    const sheet = await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" });
    expect(within(sheet).getByText("Kind of source").nextElementSibling ?? sheet).toHaveTextContent("Claude Code");
    expect(sheet).toHaveTextContent("Connected");
    expect(sheet).toHaveTextContent("sk-kso-src-AbC1…wXyZ");
    await waitFor(() => expect(sheet).toHaveTextContent("Ada Okafor"));
    expect(sheet).toHaveTextContent("1 duplicate left out, 2 refused.");
    expect(within(sheet).getByRole("status")).toHaveTextContent("An event was more than 7 days old.");
    expect(within(sheet).getByText("~/.claude/settings.json")).toBeInTheDocument();
    expect(sheet).toHaveTextContent("Bearer <your source key>");
    expect(within(sheet).getByRole("link", { name: "See requests" })).toHaveAttribute("href", expect.stringContaining("key=source%3Asrc_claude"));
    expect(sheet).toHaveTextContent("limits alert but can't stop them");
  });

  it("names a service account or nobody as whom it counts for", async () => {
    signedIn();
    source(externalSource({ user_id: null, service_account_id: "sa_deploy" }));
    await renderApp(projectPath("sources?source=src_claude"));
    const sheet = await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" });
    await waitFor(() => expect(sheet).toHaveTextContent("ci-deploy (service account)"));
  });

  it("says nobody when the source counts for no one", async () => {
    signedIn();
    source(externalSource({ user_id: null, created_by: null, status: "idle", last_event_at: null }));
    await renderApp(projectPath("sources?source=src_claude"));
    const sheet = await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" });
    expect(sheet).toHaveTextContent("Nobody");
    expect(sheet).toHaveTextContent("No event yet");
    expect(sheet).toHaveTextContent("Idle");
  });

  it("renames the source", async () => {
    signedIn();
    source();
    on("PATCH", SOURCE, externalSource({ name: "Claude Code, studio" }));
    const { user } = await renderApp(projectPath("sources?source=src_claude"));

    const sheet = await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" });
    await user.click(await within(sheet).findByRole("button", { name: "Rename" }));
    await hoverReason(user, within(sheet).getByRole("button", { name: "Save" }), "Type a new name first.");
    await user.click(within(sheet).getByRole("button", { name: "Cancel" }));
    await user.click(within(sheet).getByRole("button", { name: "Rename" }));
    const name = within(sheet).getByRole("textbox", { name: "Name" });
    await user.clear(name);
    await user.type(name, " Claude Code, studio ");
    await user.click(within(sheet).getByRole("button", { name: "Save" }));
    expect(await notifications().findByText("Name saved")).toBeInTheDocument();
    expect(requests("PATCH", SOURCE).map((r) => r.body)).toEqual([{ name: "Claude Code, studio" }]);
  });

  it("says under the name why a new name is refused", async () => {
    signedIn();
    source();
    on("PATCH", SOURCE, apiError(400, "invalid_request", "A name can't contain control characters.", { param: "name" }));
    const { user } = await renderApp(projectPath("sources?source=src_claude"));
    const sheet = await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" });
    await user.click(await within(sheet).findByRole("button", { name: "Rename" }));
    const name = within(sheet).getByRole("textbox", { name: "Name" });
    await user.type(name, "x");
    await user.click(within(sheet).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(name).toHaveAccessibleDescription("A name can't contain control characters."));
    await user.type(name, "y");
    expect(within(sheet).queryByText("A name can't contain control characters.")).not.toBeInTheDocument();
  });

  it("turns its settings on and off, and stops accepting its events", async () => {
    signedIn();
    source();
    let body: Record<string, unknown> = {};
    on("PATCH", SOURCE, ({ body: b }: { body: Record<string, unknown> }) => {
      body = b;
      return externalSource({ ...(b.disabled ? { disabled_at: "2026-10-15T12:00:00Z", status: "disabled" } : {}), ...(b.content ? { content: true } : {}) });
    });
    const { user } = await renderApp(projectPath("sources?source=src_claude"));
    const sheet = await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" });

    await user.click(await within(sheet).findByRole("switch", { name: "Keep prompts and replies" }));
    expect(await notifications().findByText("Source saved")).toBeInTheDocument();
    expect(body).toEqual({ content: true });
    await user.click(within(sheet).getByRole("switch", { name: "Match members by email" }));
    await waitFor(() => expect(body).toEqual({ match_members: false }));
    await user.click(within(sheet).getByRole("switch", { name: "Accept its events" }));
    expect(await notifications().findByText("Source disabled")).toBeInTheDocument();
    expect(body).toEqual({ disabled: true });
  });

  it("shows the setting being saved while its change is on the way", async () => {
    signedIn();
    source();
    on("PATCH", SOURCE, () => new Promise(() => undefined));
    const { user } = await renderApp(projectPath("sources?source=src_claude"));
    const sheet = await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" });
    await user.click(await within(sheet).findByRole("switch", { name: "Keep prompts and replies" }));
    await waitFor(() => expect(within(sheet).getByRole("switch", { name: "Keep prompts and replies" })).toHaveAttribute("aria-busy", "true"));
    expect(within(sheet).getByRole("switch", { name: "Match members by email" })).not.toHaveAttribute("aria-busy");
    expect(within(sheet).getByRole("switch", { name: "Accept its events" })).not.toHaveAttribute("aria-busy");
    await user.click(within(sheet).getByRole("button", { name: "Rename" }));
    expect(within(sheet).getByRole("button", { name: "Save" })).not.toHaveAttribute("aria-busy");
  });

  it("names a service account it can't find by its id", async () => {
    signedIn();
    source(externalSource({ user_id: null, service_account_id: "sa_gone" }));
    await renderApp(projectPath("sources?source=src_claude"));
    const sheet = await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" });
    expect(await within(sheet).findByText("sa_gone (service account)")).toBeInTheDocument();
  });

  it("enables a disabled source again", async () => {
    signedIn();
    source(externalSource({ disabled_at: "2026-10-15T10:00:00Z", status: "disabled" }));
    on("PATCH", SOURCE, externalSource());
    const { user } = await renderApp(projectPath("sources?source=src_claude"));
    const sheet = await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" });
    expect(sheet).toHaveTextContent("Disabled: Kaeso refuses what it sends until you enable it again.");
    await user.click(await within(sheet).findByRole("switch", { name: "Accept its events" }));
    expect(await notifications().findByText("Source enabled")).toBeInTheDocument();
    expect(requests("PATCH", SOURCE).map((r) => r.body)).toEqual([{ disabled: false }]);
  });

  it("gives the source a new key and shows it once with the setup", async () => {
    signedIn();
    source();
    on("POST", `${SOURCE}/rotate`, { source: claude, key: KEY });
    const { user } = await renderApp(projectPath("sources?source=src_claude"));
    const sheet = await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" });
    await user.click(await within(sheet).findByRole("button", { name: "Give it a new key" }));
    const dialog = await screen.findByRole("dialog", { name: /Give Claude Code, Ada's laptop a new key\?/ });
    expect(dialog).toHaveTextContent("Its logged requests and settings stay as they are.");
    await user.click(within(dialog).getByRole("button", { name: "Give it a new key" }));
    const result = await screen.findByRole("dialog", { name: /New key for Claude Code, Ada's laptop/ });
    expect(within(result).getByRole("textbox", { name: "Key for Claude Code, Ada's laptop" })).toHaveTextContent(KEY);
    expect(result).toHaveTextContent(`Authorization=Bearer ${KEY}`);
    expect(requests("POST", `${SOURCE}/rotate`)).toHaveLength(1);
    await user.click(within(result).getByRole("button", { name: "Finish" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /New key for/ })).not.toBeInTheDocument());
  });

  it("keeps its key when the new-key question is dismissed", async () => {
    signedIn();
    source();
    const { user } = await renderApp(projectPath("sources?source=src_claude"));
    const sheet = await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" });
    await user.click(await within(sheet).findByRole("button", { name: "Give it a new key" }));
    await screen.findByRole("dialog", { name: /Give Claude Code, Ada's laptop a new key\?/ });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /a new key\?/ })).not.toBeInTheDocument());
    expect(requests("POST", `${SOURCE}/rotate`)).toHaveLength(0);
  });

  it("archives the source after confirming", async () => {
    signedIn();
    source();
    on("POST", `${SOURCE}/archive`, externalSource({ archived_at: "2026-10-15T12:00:00Z", status: "archived" }));
    const { user, router } = await renderApp(projectPath("sources?source=src_claude"));
    const sheet = await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" });
    await user.click(await within(sheet).findByRole("button", { name: "Archive source" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Archive Claude Code, Ada's laptop?" });
    await user.click(within(confirm).getByRole("button", { name: "Archive source" }));
    expect(await notifications().findByText("Source archived")).toBeInTheDocument();
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("shows an archived source read-only", async () => {
    signedIn();
    source(externalSource({ archived_at: "2026-10-01T12:00:00Z", status: "archived" }));
    await renderApp(projectPath("sources?source=src_claude"));
    const sheet = await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" });
    expect(await within(sheet).findByText(/Its key no longer works; its requests stay\./)).toBeInTheDocument();
    expect(within(sheet).queryByRole("button", { name: "Rename" })).not.toBeInTheDocument();
    expect(within(sheet).queryByRole("switch")).not.toBeInTheDocument();
    expect(within(sheet).queryByRole("button", { name: "Archive source" })).not.toBeInTheDocument();
  });

  it("asks for the right to change the source", async () => {
    signedIn({ projectRights: without("sources.manage") });
    source();
    const { user } = await renderApp(projectPath("sources?source=src_claude"));
    const sheet = await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" });
    await hoverReason(user, await within(sheet).findByRole("button", { name: "Rename" }), /Manage sources/);
    expect(within(sheet).getByRole("switch", { name: "Accept its events" })).toHaveAttribute("aria-disabled", "true");
    expect(within(sheet).getByRole("button", { name: "Give it a new key" })).toHaveAttribute("aria-disabled", "true");
    expect(within(sheet).getByRole("button", { name: "Archive source" })).toHaveAttribute("aria-disabled", "true");
  });

  it("changes whom its requests count for", async () => {
    signedIn();
    source(externalSource({ user_id: "usr_grace" }));
    on("PATCH", SOURCE, externalSource());
    const { user } = await renderApp(projectPath("sources?source=src_claude"));
    const sheet = await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" });
    await user.click(await within(sheet).findByRole("combobox", { name: "Counts for" }));
    await user.click(await screen.findByRole("option", { name: "ci-deploy (service account)" }));
    await waitFor(() => expect(requests("PATCH", SOURCE).map((r) => r.body)).toEqual([{ service_account_id: "sa_deploy" }]));
    await user.click(within(sheet).getByRole("combobox", { name: "Counts for" }));
    await user.click(await screen.findByRole("option", { name: "You" }));
    await waitFor(() => expect(requests("PATCH", SOURCE).map((r) => r.body)).toContainEqual({ user_id: "usr_ada" }));
  });

  it("lists the service accounts it can count for by name, and closes with Escape", async () => {
    signedIn();
    projectData({
      sources: [claude],
      accounts: [
        serviceAccount(),
        serviceAccount({ id: "sa_old", name: "batch", archived_at: "2026-10-01T12:00:00Z" }),
        serviceAccount({ id: "sa_audit", name: "audit" }),
      ],
    });
    on("GET", SOURCE, claude);
    const { user, router } = await renderApp(projectPath("sources?source=src_claude"));
    const sheet = await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" });
    await user.click(await within(sheet).findByRole("combobox", { name: "Counts for" }));
    const options = (await screen.findAllByRole("option")).map((o) => o.textContent);
    expect(options).toEqual(["You", "audit (service account)", "ci-deploy (service account)"]);
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("option")).not.toBeInTheDocument());
    await user.keyboard("{Escape}");
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("says when the source doesn't exist, and closes", async () => {
    signedIn();
    projectData();
    on("GET", SOURCE, apiError(404, "not_found", "No such source."));
    const { user, router } = await renderApp(projectPath("sources?source=src_gone"));
    const sheet = await screen.findByRole("dialog", { name: "Source" });
    expect(await within(sheet).findByText("This source doesn't exist or was removed.")).toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Back to sources" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("says when the source couldn't load", async () => {
    signedIn();
    projectData();
    on("GET", SOURCE, apiError(500, "internal", "Something broke."));
    await renderApp(projectPath("sources?source=src_claude"));
    expect(await screen.findByText("Couldn't load this source.")).toBeInTheDocument();
  });
});
