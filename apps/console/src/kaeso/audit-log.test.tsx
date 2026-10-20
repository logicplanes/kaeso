import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ALL_ORG_RIGHTS, ORG, membership, signedIn, whoAmI } from "~/test/fixtures";
import { OTHER_USER, member, notifications, paged } from "~/test/fixtures/org";
import { projectData, projectMember, projectPath } from "~/test/fixtures/project";
import { auditEvent } from "~/test/fixtures/shell";
import { apiError, on, requests } from "~/test/network";
import { showExportProgress } from "~/shell/export-job";
import { renderApp } from "~/test/render";

const ORG_LOG = "orgs/:org/audit-log";
const PATH = `/settings/orgs/${ORG}/audit-log`;
const values = (v: object) => v as Record<string, never>;
const logQuery = () => requests("GET", ORG_LOG).map((r) => Object.fromEntries(r.query));

const grace = member({ user: { id: OTHER_USER, name: "Grace Hopper", email: "grace@example.com" } });
const created = auditEvent({ target_kind: "user", target_id: OTHER_USER });
const old = auditEvent({
  id: "evt_old",
  action: "org.updated",
  at: "2024-03-02T10:00:00Z",
  actor: { kind: "system" },
  target_kind: null,
  target_id: null,
  ip: null,
  user_agent: null,
  request_id: null,
});

function orgLog(events = [created, old], rights: string[] = ALL_ORG_RIGHTS) {
  signedIn({ session: whoAmI({ memberships: [membership({ rights })] }) });
  on("GET", "orgs/:org/members", paged([member(), grace]));
  on("GET", "orgs/:org/credentials", { data: [] });
  on("GET", "orgs/:org/platform-keys", paged([]));
  on("GET", ORG_LOG, paged(events));
}

const table = () => screen.findByRole("table", { name: "Audit log" });

function installPicker() {
  const picker = vi.fn(async () => ({
    name: "audit.jsonl",
    createWritable: async () => new WritableStream<Uint8Array>(),
    remove: async () => undefined,
  }));
  Object.defineProperty(window, "showSaveFilePicker", { value: picker, configurable: true, writable: true });
}

function streamedExport() {
  let controller: ReadableStreamDefaultController<Uint8Array> | undefined;
  const body = new ReadableStream<Uint8Array>({
    start(c) {
      controller = c;
    },
  });
  on("GET", `${ORG_LOG}/export`, () => new Response(body, { status: 200, headers: { "content-type": "application/x-ndjson" } }));
  return {
    push: (bytes: number) => controller?.enqueue(new Uint8Array(bytes)),
    end: () => controller?.close(),
  };
}

afterEach(async () => {
  await waitFor(() => expect(showExportProgress()).toBe(false));
  Reflect.deleteProperty(window, "showSaveFilePicker");
});

describe("AuditLog", () => {
  it("lists events with their time, actor, action, target and client", async () => {
    orgLog();
    await renderApp(PATH);

    const rows = within(await table())
      .getAllByRole("row")
      .slice(1);
    expect(rows).toHaveLength(2);
    const [first, second] = rows as [HTMLElement, HTMLElement];
    expect(first.querySelector("time")).toHaveAttribute("datetime", created.at);
    expect(first).toHaveTextContent("Ada Okafor (you)");
    expect(first).toHaveTextContent("key.created");
    expect(await within(first).findByRole("link", { name: "Grace Hopper" })).toHaveAttribute("href", `/settings/orgs/${ORG}/members?member=${OTHER_USER}`);
    expect(first).toHaveTextContent("203.0.113.7");
    expect(first).toHaveTextContent("Mozilla/5.0");
    expect(second).toHaveTextContent("2024");
    expect(second).toHaveTextContent("System");
    expect(second).toHaveTextContent("None");
    expect(logQuery()).toEqual([{ limit: "100" }]);
  });

  it("names actions and targets in words, and says the times' zone", async () => {
    const createdKey = auditEvent({ id: "evt_key", action: "key.create", target_kind: "key", target_id: "key_web", after: values({ name: "checkout-web" }) });
    const archived = auditEvent({ id: "evt_arch", action: "key.archive", target_kind: "key", target_id: "key_web", after: null, before: null });
    const sso = auditEvent({ id: "evt_sso", action: "security.sso_delete", target_kind: "org", target_id: ORG, after: null, before: null });
    orgLog([sso, archived, createdKey]);
    on("GET", `${ORG_LOG}/:event`, sso);
    const { user } = await renderApp(PATH);

    const grid = await table();
    expect(
      within(grid)
        .getAllByRole("columnheader")
        .map((h) => h.textContent),
    ).toContain("Time (UTC)");
    const rows = within(grid).getAllByRole("row").slice(1);
    expect(rows.map((r) => r.textContent)).toEqual([
      expect.stringContaining("Single sign-on disconnected"),
      expect.stringContaining("Key archived"),
      expect.stringContaining("Key created"),
    ]);
    expect(rows[1]).toHaveTextContent("checkout-web");
    expect(rows[1]).not.toHaveTextContent("key_web");
    expect(grid).not.toHaveTextContent("security.sso_delete");

    await user.click(rows[0] as HTMLElement);
    const sheet = await screen.findByRole("dialog", { name: "Single sign-on disconnected" });
    expect(sheet).toHaveTextContent("security.sso_delete");
    expect(await within(sheet).findByText(/ UTC$/)).toBeInTheDocument();
    expect(within(sheet).getByText("No before or after values are kept for this kind of change.")).toBeInTheDocument();
  });

  it("on a phone, keeps who acted as a named picture and shortens the time", async () => {
    vi.stubGlobal("matchMedia", (query: string) => ({
      matches: query === "(max-width: 639px)",
      media: query,
      addEventListener() {},
      removeEventListener() {},
      addListener() {},
      removeListener() {},
    }));
    try {
      vi.setSystemTime(new Date("2026-10-15T12:00:00Z"));
      orgLog([
        auditEvent({ id: "evt_today", action: "key.create", at: "2026-10-15T00:05:00Z", target_kind: "key", target_id: "key_web" }),
        auditEvent({ id: "evt_jan", action: "key.archive", at: "2026-01-01T09:00:00Z" }),
        auditEvent({ id: "evt_old", action: "key.create", at: "2024-03-02T10:00:00Z", target_kind: "key", target_id: "key_web" }),
      ]);
      await renderApp(PATH);
      const rows = within(await table())
        .getAllByRole("row")
        .slice(1);
      expect(within(rows[0] as HTMLElement).getByRole("img", { name: "Ada Okafor (you)" })).toBeInTheDocument();
      expect(rows.map((r) => r.querySelector("time")?.textContent)).toEqual(["00:05", "1 Jan", "2 Mar 24"]);
      expect(rows[0]).toHaveTextContent("Key created");
    } finally {
      vi.unstubAllGlobals();
      vi.useRealTimers();
    }
  });

  it("shows each actor's picture, or their initial without one", async () => {
    const pictured = member({ user: { id: OTHER_USER, name: "Grace Hopper", email: "grace@example.com", avatar_url: "/avatars/grace.webp" } });
    signedIn({ session: whoAmI({ user: { ...whoAmI().user, avatar_url: "/avatars/ada.webp" }, memberships: [membership({ rights: ALL_ORG_RIGHTS })] }) });
    on("GET", "orgs/:org/members", paged([member(), pictured]));
    on("GET", "orgs/:org/credentials", { data: [] });
    on("GET", "orgs/:org/platform-keys", paged([]));
    on("GET", ORG_LOG, paged([created, auditEvent({ id: "evt_grace", actor: { kind: "user", id: OTHER_USER }, ip: "198.51.100.2" }), old]));
    const { user } = await renderApp(PATH);

    const rows = within(await table())
      .getAllByRole("row")
      .slice(1);
    expect(rows[0]?.querySelector('img[src="/avatars/ada.webp"]')).not.toBeNull();
    await waitFor(() => expect(rows[1]?.querySelector('img[src="/avatars/grace.webp"]')).not.toBeNull());
    expect(rows[2]?.querySelector("img")).toBeNull();

    await user.click(screen.getByRole("combobox", { name: "Actor" }));
    const option = await screen.findByRole("option", { name: /Grace Hopper/ });
    expect(option.querySelector('img[src="/avatars/grace.webp"]')).not.toBeNull();
  });

  it("shows the initial for an actor without a picture", async () => {
    orgLog();
    const { user } = await renderApp(PATH);

    const rows = within(await table())
      .getAllByRole("row")
      .slice(1);
    expect(within(rows[0] as HTMLElement).getByText("A")).toBeInTheDocument();
    expect((await table()).querySelector("img")).toBeNull();

    await user.click(screen.getByRole("combobox", { name: "Actor" }));
    const option = await screen.findByRole("option", { name: /Grace Hopper/ });
    expect(within(option).getByText("G")).toBeInTheDocument();
    expect(option.querySelector("img")).toBeNull();
  });

  it("says when there are no events yet", async () => {
    orgLog([]);
    await renderApp(PATH);
    expect(await screen.findByText("No events yet")).toBeInTheDocument();
    expect(screen.getByText("Every change is recorded here.")).toBeInTheDocument();
  });

  it("filters by actor, action, target and days in the URL", async () => {
    orgLog([]);
    const { user, router } = await renderApp(`${PATH}?from=2026-10-01&to=2026-10-03`);

    expect(await screen.findByText("No events match")).toBeInTheDocument();
    expect(screen.getByText("Remove a filter or widen the days.")).toBeInTheDocument();
    expect(logQuery()).toContainEqual({ from: "2026-10-01T00:00:00Z", to: "2026-10-04T00:00:00.000Z", limit: "100" });

    await user.click(screen.getByRole("combobox", { name: "Actor" }));
    await user.click(await screen.findByRole("option", { name: /Grace Hopper/ }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ actor: OTHER_USER }));

    await user.type(screen.getByRole("combobox", { name: "Action" }), "Key created");
    await user.click(await screen.findByRole("option", { name: /^Key created/ }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ actor: OTHER_USER, action: "key.create" }));

    const target = screen.getByRole("searchbox", { name: "Target id" });
    await user.type(target, "  key_web {Enter}");
    await waitFor(() => expect(router.state.location.search).toMatchObject({ target: "key_web" }));
    await waitFor(() => expect(logQuery()).toContainEqual(expect.objectContaining({ actor: OTHER_USER, action: "key.create", target: "key_web" })));

    await user.clear(target);
    await user.tab();
    await waitFor(() => expect(router.state.location.search).not.toHaveProperty("target"));

    await user.click(screen.getByRole("button", { name: "Days" }));
    await user.click(within(await screen.findByRole("dialog", { name: "Choose a date range" })).getByRole("option", { name: "Today" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ from: expect.stringMatching(/^\d{4}-\d{2}-\d{2}$/) }));
    const { from, to } = router.state.location.search as { from: string; to: string };
    expect(to).toBe(from);
  });

  it("loads the next page of events", async () => {
    orgLog();
    on("GET", ORG_LOG, ({ query }: { query: URLSearchParams }) =>
      query.get("after") === "cur_2" ? paged([auditEvent({ id: "evt_3", action: "project.created" })]) : paged([created], "cur_2"),
    );
    const { user } = await renderApp(PATH);

    await table();
    await user.click(screen.getByRole("button", { name: "Load more" }));
    expect(await screen.findByText("project.created")).toBeInTheDocument();
    expect(logQuery()).toContainEqual({ limit: "100", after: "cur_2" });
  });

  it("says when the audit log couldn't load", async () => {
    orgLog();
    on("GET", ORG_LOG, apiError(403, "permission_denied", "You can't read the audit log."));
    const { user } = await renderApp(PATH);
    expect((await notifications().findAllByText("You can't read the audit log.")).length).toBeGreaterThan(0);
    await user.click(within(await table()).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(logQuery()).toHaveLength(2));
  });

  it("opens an event with its details, rights change and diff", async () => {
    const changed = auditEvent({
      id: "evt_role",
      action: "member.rights_updated",
      target_kind: "user",
      target_id: OTHER_USER,
      before: values({ role: { name: "Member" }, rights: ["org.view"] }),
      after: values({ role: { name: "Admin" }, rights: ["org.view", "members.invite"] }),
    });
    orgLog([changed]);
    on("GET", `${ORG_LOG}/:event`, changed);
    const { user, router } = await renderApp(PATH);

    await user.click(within(await table()).getByText("member.rights_updated"));
    await waitFor(() => expect(router.state.location.search).toEqual({ event: "evt_role" }));
    const sheet = await screen.findByRole("dialog", { name: "member.rights_updated" });
    expect(await within(sheet).findByText("Member → Admin: + Invite members")).toBeInTheDocument();
    expect(within(sheet).getByText("Ada Okafor (you)")).toBeInTheDocument();
    expect(within(sheet).getByRole("link", { name: "Grace Hopper" })).toBeInTheDocument();
    expect(within(sheet).getByRole("button", { name: OTHER_USER })).toBeInTheDocument();
    expect(within(sheet).getByRole("button", { name: "req_123" })).toBeInTheDocument();
    expect(within(sheet).getByText("203.0.113.7")).toBeInTheDocument();
    expect(within(sheet).getByText("Before → after")).toBeInTheDocument();
    expect(requests("GET", `${ORG_LOG}/:event`).map((r) => r.params.event)).toEqual(["evt_role"]);

    await user.click(within(sheet).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("shows the values an event created", async () => {
    const added = auditEvent({ id: "evt_add", action: "project.created", target_kind: "project", target_id: "proj_new", after: values({ name: "New" }) });
    orgLog([added]);
    on("GET", `${ORG_LOG}/:event`, added);
    await renderApp(`${PATH}?event=evt_add`);
    const sheet = await screen.findByRole("dialog", { name: "project.created" });
    expect(await within(sheet).findByText("After")).toBeInTheDocument();
    expect(within(sheet).getByText("New")).toBeInTheDocument();
    expect(within(sheet).getByRole("button", { name: /proj_new/ })).toBeInTheDocument();
  });

  it("shows the values an event removed", async () => {
    const removed = auditEvent({ id: "evt_del", action: "project.deleted", target_kind: "project", target_id: "proj_old", before: values({ name: "Old" }) });
    orgLog([removed]);
    on("GET", `${ORG_LOG}/:event`, removed);
    await renderApp(`${PATH}?event=evt_del`);
    const sheet = await screen.findByRole("dialog", { name: "project.deleted" });
    expect(await within(sheet).findByText("Before")).toBeInTheDocument();
  });

  it("says when an event records no values", async () => {
    orgLog([old]);
    on("GET", `${ORG_LOG}/:event`, old);
    await renderApp(`${PATH}?event=evt_old`);
    const sheet = await screen.findByRole("dialog", { name: "org.updated" });
    expect(await within(sheet).findByText("No before or after values are kept for this kind of change.")).toBeInTheDocument();
    expect(within(sheet).queryByText("IP")).not.toBeInTheDocument();
    expect(within(sheet).queryByRole("heading", { name: "Rights" })).not.toBeInTheDocument();
  });

  it("says when an event doesn't exist and goes back", async () => {
    orgLog();
    on("GET", `${ORG_LOG}/:event`, apiError(404, "not_found", "No such event."));
    const { user, router } = await renderApp(`${PATH}?event=evt_gone`);
    expect(await screen.findByText("This event doesn't exist")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Back to the audit log" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("says when an event can't be shown", async () => {
    orgLog();
    on("GET", `${ORG_LOG}/:event`, apiError(403, "permission_denied", "You can't read this event."));
    await renderApp(`${PATH}?event=evt_secret`);
    expect(await screen.findByText("This event can't be shown right now")).toBeInTheDocument();
  });

  it("exports the filtered audit log through the browser", async () => {
    orgLog();
    on("GET", `${ORG_LOG}/export`, "");
    const { user } = await renderApp(`${PATH}?actor=${OTHER_USER}`);

    await table();
    await user.click(screen.getByRole("button", { name: "Export JSON Lines" }));
    expect(await notifications().findByText("Export started")).toBeInTheDocument();
    expect(logQuery()).toContainEqual({ actor: OTHER_USER, limit: "1" });
  });

  it("hides the export without Export audit log", async () => {
    orgLog(
      undefined,
      ALL_ORG_RIGHTS.filter((r) => r !== "audit_log.export"),
    );
    await renderApp(PATH);
    await table();
    expect(screen.queryByRole("button", { name: "Export JSON Lines" })).not.toBeInTheDocument();
  });

  it("opens a project event from the loaded rows", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    const event = auditEvent({ id: "evt_p", action: "member.added", actor: { kind: "user", id: "usr_grace" }, target_kind: "user", target_id: "usr_grace" });
    on("GET", "orgs/:org/projects/:project/audit-log", paged([event]));
    const { user } = await renderApp(projectPath("audit-log"));

    const row = within(await table()).getAllByRole("row")[1] as HTMLElement;
    expect(await within(row).findByRole("link", { name: "Grace Hopper" })).toHaveAttribute("href", `${projectPath("members")}?member=usr_grace`);
    await user.click(within(row).getByText("member.added"));
    const sheet = await screen.findByRole("dialog", { name: "member.added" });
    expect(within(sheet).getByText("User")).toBeInTheDocument();
    expect(requests("GET", "orgs/:org/audit-log/:event")).toHaveLength(0);
  });

  it("can't show a project event that isn't loaded", async () => {
    signedIn();
    projectData();
    on("GET", "orgs/:org/projects/:project/audit-log", paged([]));
    await renderApp(projectPath("audit-log?event=evt_elsewhere"));
    expect(await screen.findByText("This event can't be shown right now")).toBeInTheDocument();
  });

  it("waits for the project's events before opening the one the address names", async () => {
    signedIn();
    projectData();
    let release = () => {};
    const held = new Promise<void>((resolve) => (release = resolve));
    const event = auditEvent({ id: "evt_p", action: "member.added" });
    on("GET", "orgs/:org/projects/:project/audit-log", async () => {
      await held;
      return paged([event]);
    });
    await renderApp(projectPath("audit-log?event=evt_p"));
    const sheet = await screen.findByRole("dialog", { name: "Event" });
    expect(within(sheet).queryByText("This event can't be shown right now")).not.toBeInTheDocument();
    release();
    expect(await screen.findByRole("dialog", { name: "member.added" })).toBeInTheDocument();
    expect(screen.queryByText("This event can't be shown right now")).not.toBeInTheDocument();
  });

  it("reads the install's audit log", async () => {
    signedIn();
    on("GET", "instance/users", paged([]));
    on("GET", "instance/orgs", paged([]));
    const event = auditEvent({
      id: "evt_i",
      action: "settings.updated",
      target_kind: "settings",
      target_id: "instance",
      after: values({ org_creation: "everyone" }),
    });
    on("GET", "instance/audit-log", paged([event]));
    on("GET", "instance/audit-log/:event", event);
    await renderApp("/settings/instance/audit-log?event=evt_i");

    const sheet = await screen.findByRole("dialog", { name: "settings.updated" });
    expect(await within(sheet).findByText("After")).toBeInTheDocument();
    expect(requests("GET", "instance/audit-log/:event").map((r) => r.params.event)).toEqual(["evt_i"]);
  });

  it("shows this year's times in the reader's language", async () => {
    orgLog([created]);
    await renderApp(PATH, { locale: "de" });
    await waitFor(() => expect(document.querySelector(`time[datetime="${created.at}"]`)).toHaveTextContent(/Okt/));
  });

  it("offers actors without an email and clears the actor, action and target filters", async () => {
    orgLog([]);
    on("GET", "orgs/:org/members", paged([member(), member({ user: { id: OTHER_USER, name: "Grace Hopper", email: null } })]));
    const { user, router } = await renderApp(`${PATH}?actor=${OTHER_USER}&action=key.create`);

    expect(await screen.findByText("No events match")).toBeInTheDocument();
    const actor = screen.getByRole("combobox", { name: "Actor" });
    await waitFor(() => expect(actor).toHaveValue("Grace Hopper"));
    await user.clear(actor);
    await user.tab();
    await waitFor(() => expect(router.state.location.search).not.toHaveProperty("actor"));

    const action = screen.getByRole("combobox", { name: "Action" });
    await user.clear(action);
    await user.tab();
    await waitFor(() => expect(router.state.location.search).not.toHaveProperty("action"));

    const target = screen.getByRole("searchbox", { name: "Target id" });
    await user.type(target, "   {Enter}");
    expect(router.state.location.search).not.toHaveProperty("target");
    await user.clear(target);
    await user.type(target, "key_web");
    await user.tab();
    await waitFor(() => expect(router.state.location.search).toMatchObject({ target: "key_web" }));
  });

  it("shows the export's progress and doesn't start a second one", async () => {
    orgLog();
    installPicker();
    const stream = streamedExport();
    const { user } = await renderApp(PATH);

    await table();
    await user.click(screen.getByRole("button", { name: "Export JSON Lines" }));
    expect(await notifications().findAllByText("Exporting audit events")).not.toHaveLength(0);
    stream.push(2048);
    expect(await notifications().findAllByText("Exporting audit events · 2.0 KB")).not.toHaveLength(0);
    await user.click(screen.getByRole("button", { name: "Export JSON Lines" }));
    expect(requests("GET", `${ORG_LOG}/export`)).toHaveLength(1);
    stream.end();
    expect(await notifications().findByText("Audit log exported")).toBeInTheDocument();
  });
});
