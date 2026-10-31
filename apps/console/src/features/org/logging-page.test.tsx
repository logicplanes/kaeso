import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, OTHER_PROJECT, PROJECT, project, signedIn, user, whoAmI, withRights } from "~/test/fixtures";
import { hoverReason, logJob, logSettings, notifications, storage, fill } from "~/test/fixtures/org";
import { apiError, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/settings/orgs/${ORG}/logging`;

describe("log settings", () => {
  it("shows content logging and how long entries are kept", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/settings", logSettings());
    const { user } = await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Logging", level: 1 })).toBeInTheDocument();
    expect(await screen.findByRole("switch", { name: "Log request content" })).toBeChecked();
    expect(screen.getByRole("spinbutton", { name: "Metadata retention" })).toHaveValue("90");
    expect(screen.getByRole("spinbutton", { name: "Content retention" })).toHaveValue("30");
    expect(screen.getByText("Timing, tokens, cost and errors; up to 365 days.")).toBeInTheDocument();
    expect(screen.getByText("Bodies, up to 180 days, no longer than metadata.")).toBeInTheDocument();
    await hoverReason(user, screen.getByRole("button", { name: "Save metadata retention" }), "No change to save");
    await hoverReason(user, screen.getByRole("button", { name: "Save content retention" }), "No change to save");
  });

  it("turns content logging off and on", async () => {
    signedIn();
    let current = logSettings();
    on("GET", "orgs/:org/logs/settings", () => current);
    on("PATCH", "orgs/:org/logs/settings", ({ body }: { body: { content_logging: boolean } }) => (current = logSettings(body)));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("switch", { name: "Log request content" }));
    expect(await notifications().findByText("Content logging turned off")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("switch", { name: "Log request content" })).not.toBeChecked());
    await user.click(screen.getByRole("switch", { name: "Log request content" }));
    expect(await notifications().findByText("Content logging turned on")).toBeInTheDocument();
    expect(requests("PATCH", "orgs/:org/logs/settings").map((r) => r.body)).toEqual([{ content_logging: false }, { content_logging: true }]);
  });

  it("marks only the setting that is saving as busy", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/settings", logSettings());
    const held: (() => void)[] = [];
    on(
      "PATCH",
      "orgs/:org/logs/settings",
      ({ body }: { body: Record<string, unknown> }) => new Promise((resolve) => held.push(() => resolve(logSettings(body)))),
    );
    const { user } = await renderApp(PATH);
    const toggle = () => screen.getByRole("switch", { name: "Log request content" });
    const save = () => screen.getByRole("button", { name: "Save metadata retention" });
    await user.click(await screen.findByRole("switch", { name: "Log request content" }));
    await waitFor(() => expect(toggle()).toHaveAttribute("aria-busy", "true"));
    expect(save()).not.toHaveAttribute("aria-busy");
    held.shift()?.();
    expect(await notifications().findByText("Content logging turned off")).toBeInTheDocument();
    await waitFor(() => expect(toggle()).not.toHaveAttribute("aria-busy"));
    const metadata = screen.getByRole("spinbutton", { name: "Metadata retention" });
    await user.clear(metadata);
    await user.type(metadata, "60");
    await user.click(save());
    await waitFor(() => expect(save()).toHaveAttribute("aria-busy", "true"));
    expect(toggle()).not.toHaveAttribute("aria-busy");
    held.shift()?.();
    expect(await notifications().findByText("Retention saved")).toBeInTheDocument();
  });

  it("checks the retention before saving it", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/settings", logSettings());
    on("PATCH", "orgs/:org/logs/settings", logSettings({ metadata_retention_days: 60, content_retention_days: 14 }));
    const { user } = await renderApp(PATH);
    const metadata = await screen.findByRole("spinbutton", { name: "Metadata retention" });
    const content = screen.getByRole("spinbutton", { name: "Content retention" });
    const saveMetadata = () => screen.getByRole("button", { name: "Save metadata retention" });
    const saveContent = () => screen.getByRole("button", { name: "Save content retention" });
    await user.clear(metadata);
    await user.type(metadata, "400");
    await hoverReason(user, saveMetadata(), "Metadata can be kept up to 365 days on this install");
    await user.clear(metadata);
    await user.type(metadata, "0");
    await hoverReason(user, saveMetadata(), "Enter whole days, at least 1");
    await user.clear(metadata);
    await user.type(metadata, "20");
    await hoverReason(user, saveMetadata(), "Metadata can't be kept shorter than content; shorten content first");
    await user.clear(content);
    await user.type(content, "200");
    await hoverReason(user, saveContent(), "Content can be kept up to 180 days on this install");
    await user.clear(content);
    await user.type(content, "120");
    await hoverReason(user, saveContent(), "Content can't be kept longer than metadata");
    await user.clear(content);
    await user.type(content, "14");
    await user.click(saveContent());
    expect(await notifications().findByText("Retention saved")).toBeInTheDocument();
    expect(requests("PATCH", "orgs/:org/logs/settings").map((r) => r.body)).toEqual([{ content_retention_days: 14 }]);
  });

  it("locks the settings for people who can't manage them", async () => {
    withRights(["org.view"], { role: "admin" });
    on("GET", "orgs/:org/logs/settings", logSettings());
    const { user } = await renderApp(`${PATH}?tab=storage`);
    const toggle = await screen.findByRole("switch", { name: "Log request content" });
    await hoverReason(user, toggle, "You need Manage log settings; ask an owner or admin for it.");
    expect(screen.getByRole("spinbutton", { name: "Metadata retention" })).toBeDisabled();
    expect(screen.getByRole("spinbutton", { name: "Content retention" })).toBeDisabled();
    await hoverReason(user, screen.getByRole("button", { name: "Save metadata retention" }), "You need Manage log settings; ask an owner or admin for it.");
    expect(screen.getByRole("tab", { name: "Storage" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("tab", { name: "Deletions" })).toHaveAttribute("aria-disabled", "true");
    expect(requests("GET", "orgs/:org/logs/storage")).toHaveLength(0);
  });

  it("says when the log settings couldn't load", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/settings", apiError(403, "forbidden", "The gateway failed."));
    await renderApp(PATH);
    expect(await screen.findByText("The log settings couldn't load")).toBeInTheDocument();
  });

  it("hides the page from people who can't view the organization", async () => {
    withRights([], { role: "admin" });
    await renderApp(PATH);
    expect(await screen.findByText("You can't see the logging settings here")).toBeInTheDocument();
  });

  it("switches tabs through the URL", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/settings", logSettings());
    on("GET", "orgs/:org/logs/storage", storage());
    on("GET", "orgs/:org/logs/deletions", { data: [] });
    const { user, router } = await renderApp(PATH);
    await user.click(await screen.findByRole("tab", { name: "Storage" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ tab: "storage" }));
    expect(await screen.findByRole("table", { name: "Storage by project" })).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Deletions" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ tab: "deletions" }));
    expect(await screen.findByText("No deletions yet.")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Settings" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("sends the old logs address to the logging page", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/deletions", { data: [] });
    const { router } = await renderApp(`/settings/orgs/${ORG}/logs?tab=deletions`);
    expect(await screen.findByText("No deletions yet.")).toBeInTheDocument();
    expect(router.state.location.pathname).toBe(PATH);
  });
});

describe("log storage", () => {
  it("shows the totals and each project's share, largest first", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/storage", storage());
    await renderApp(`${PATH}?tab=storage`);
    const totals = await screen.findByLabelText("Log storage in total");
    await within(totals).findByText("1,500");
    expect(within(totals).getByText("900 with content")).toBeInTheDocument();
    expect(within(totals).getByText("As received")).toBeInTheDocument();
    expect(within(totals).getByText("Estimated")).toBeInTheDocument();
    const table = screen.getByRole("table", { name: "Storage by project" });
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows.map((r) => within(r).getAllByRole("cell")[0]?.textContent)).toEqual(["Checkout assistant", "proj_gone"]);
    expect(rows[0]).toHaveTextContent("1,200");
    expect(rows[0]).toHaveTextContent("800");
    expect(screen.getByText("Back up the master keys")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /^Backups/ })).toHaveAttribute("href", "https://docs.kaeso.app/self-hosting/backups");
  });

  it("leaves the master keys to instance admins", async () => {
    signedIn({ session: whoAmI({ user: user({ is_instance_admin: false }) }) });
    on("GET", "orgs/:org/logs/storage", storage());
    await renderApp(`${PATH}?tab=storage`);
    await screen.findByRole("table", { name: "Storage by project" });
    expect(screen.queryByText("Back up the master keys")).not.toBeInTheDocument();
  });

  it("says when there are no request logs yet", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/storage", storage({ entries: 0, logical_bytes: 0, estimated_disk_bytes: 0, projects: [] }));
    await renderApp(`${PATH}?tab=storage`);
    expect(await screen.findByText("No request logs yet")).toBeInTheDocument();
    expect(screen.getByText("0 with content")).toBeInTheDocument();
  });

  it("says when the log storage couldn't load", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/storage", apiError(403, "forbidden", "ClickHouse is down."));
    await renderApp(`${PATH}?tab=storage`);
    expect(await screen.findByText("The log storage couldn't load")).toBeInTheDocument();
    expect(screen.getByText("Back up the master keys")).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: "Storage by project" })).not.toBeInTheDocument();
  });
});

describe("log deletions", () => {
  it("lists past and queued deletions", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/deletions", {
      data: [
        logJob(),
        logJob({ id: "job_2", kind: "shrink_metadata", state: "queued", project_id: null, done_at: null, last_error: "ClickHouse timed out" }),
        logJob({ id: "job_3", kind: "shrink_content", project_id: "proj_gone" }),
        logJob({ id: "job_4", kind: "compaction", project_id: OTHER_PROJECT }),
      ],
    });
    await renderApp(`${PATH}?tab=deletions`);
    const table = await screen.findByRole("table", { name: "Log deletions" });
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows).toHaveLength(4);
    expect(rows[0]).toHaveTextContent("Deletion");
    expect(rows[0]).toHaveTextContent("1 Oct 2026 – 2 Oct 2026");
    expect(rows[0]).toHaveTextContent("Checkout assistant");
    expect(rows[0]).toHaveTextContent("Done");
    expect(rows[0]).toHaveTextContent("15 Oct 2026");
    expect(rows[1]).toHaveTextContent("Shorter metadata retention");
    expect(rows[1]).toHaveTextContent("Every project");
    expect(rows[1]).toHaveTextContent("Queued");
    expect(rows[1]).toHaveTextContent("Retrying: ClickHouse timed out");
    expect(rows[1]).toHaveTextContent("Not yet");
    expect(rows[2]).toHaveTextContent("Shorter content retention");
    expect(rows[2]).toHaveTextContent("proj_gone");
    expect(rows[3]).toHaveTextContent("compaction");
    expect(rows[3]).toHaveTextContent("Support triage");
  });

  it("says when the deletions couldn't load", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/deletions", apiError(403, "forbidden", "The gateway failed."));
    await renderApp(`${PATH}?tab=deletions`);
    expect(await screen.findByText("The deletions couldn't load")).toBeInTheDocument();
  });

  it("explains that deleting logs needs a right", async () => {
    withRights(["org.view", "logs.storage.view"], { role: "admin" });
    on("GET", "orgs/:org/logs/deletions", { data: [] });
    const { user } = await renderApp(`${PATH}?tab=deletions`);
    await hoverReason(user, await screen.findByRole("button", { name: "Delete logs" }), "You need Delete logs; ask an owner or admin for it.");
  });

  it("deletes a period's logs in one project once the word is typed", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/deletions", { data: [] });
    on("POST", "orgs/:org/logs/deletions", logJob({ state: "queued", done_at: null }));
    const { user } = await renderApp(`${PATH}?tab=deletions`);
    await user.click(await screen.findByRole("button", { name: "Delete logs" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete request logs" });
    expect(within(dialog).getByText("Whole UTC days; usage and costs stay.")).toBeInTheDocument();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Choose the period");
    await user.click(within(dialog).getByRole("button", { description: "Period" }));
    await user.click(await screen.findByRole("option", { name: "Today" }));
    const today = new Date().toISOString().slice(0, 10);
    expect(await within(dialog).findByText(/whole UTC days\.$/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("From which projects?")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("combobox", { name: "Project" }));
    await user.click(await screen.findByRole("option", { name: "Checkout assistant" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("Deleted log entries cannot be recovered.")).toBeInTheDocument();
    const typed = within(dialog).getByRole("textbox", { name: "Type delete to confirm" });
    await fill(user, typed, "remove");
    expect(within(dialog).getAllByText("Type delete to confirm")).toHaveLength(1);
    await user.hover(within(dialog).getByRole("button", { name: "Delete logs" }));
    await waitFor(() => expect(screen.getAllByText("Type delete to confirm")).toHaveLength(2));
    await user.unhover(within(dialog).getByRole("button", { name: "Delete logs" }));
    await user.clear(typed);
    await fill(user, typed, " delete ");
    await user.click(within(dialog).getByRole("button", { name: "Delete logs" }));
    expect(await notifications().findByText("Deletion queued; it runs in the background")).toBeInTheDocument();
    const next = new Date(Date.parse(`${today}T00:00:00Z`) + 86_400_000).toISOString();
    expect(requests("POST", "orgs/:org/logs/deletions").map((r) => r.body)).toEqual([{ from: `${today}T00:00:00Z`, to: next, project: PROJECT }]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Delete request logs" })).not.toBeInTheDocument());
  });

  it("offers every project while the projects are still loading", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/deletions", { data: [] });
    on("GET", "orgs/:org/projects", ({ query }: { query: URLSearchParams }) =>
      query.get("include_archived") === "true" ? new Promise(() => undefined) : page([project()]),
    );
    const { user } = await renderApp(`${PATH}?tab=deletions`);
    await user.click(await screen.findByRole("button", { name: "Delete logs" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete request logs" });
    await user.click(within(dialog).getByRole("button", { description: "Period" }));
    await user.click(await screen.findByRole("option", { name: "Today" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("combobox", { name: "Project" }));
    expect((await screen.findAllByRole("option")).map((o) => o.textContent)).toEqual(["Every project"]);
  });

  it("deletes from every project and stays open when refused", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/deletions", { data: [] });
    on("POST", "orgs/:org/logs/deletions", apiError(409, "conflict", "A deletion is already running."));
    const { user } = await renderApp(`${PATH}?tab=deletions`);
    await user.click(await screen.findByRole("button", { name: "Delete logs" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete request logs" });
    await user.click(within(dialog).getByRole("button", { description: "Period" }));
    await user.click(await screen.findByRole("option", { name: "Last 7 days" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("button", { name: "Continue" }));
    await fill(user, await within(dialog).findByRole("textbox", { name: "Type delete to confirm" }), "delete");
    await user.click(within(dialog).getByRole("button", { name: "Delete logs" }));
    expect((await notifications().findAllByText("A deletion is already running.")).length).toBeGreaterThan(0);
    expect(requests("POST", "orgs/:org/logs/deletions").map((r) => (r.body as { project?: string }).project)).toEqual([undefined]);
    expect(screen.getByRole("dialog", { name: "Delete request logs" })).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Delete request logs" })).not.toBeInTheDocument());
  });
});

describe("logging on an install without request logs", () => {
  it("says logs are off instead of showing settings and storage", async () => {
    signedIn({ session: whoAmI({ request_logs: false }) });
    await renderApp(PATH);
    expect(await screen.findByText("Request logs are off on this install")).toBeInTheDocument();
    expect(screen.getByText("Usage, limits and budgets still count every request; instance admins can turn logs on.")).toBeInTheDocument();
    expect(screen.queryByRole("switch", { name: "Log request content" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Storage" })).not.toBeInTheDocument();
    expect(requests("GET", "orgs/:org/logs/settings")).toHaveLength(0);
    expect(requests("GET", "orgs/:org/logs/storage")).toHaveLength(0);
  });
});
