import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { AuditEventView, InstanceUserView } from "~/api/generated/types";
import { USER, signedIn, whoAmI } from "~/test/fixtures";
import { instanceOrg, instanceUser } from "~/test/fixtures/instance-account";
import { paged } from "~/test/fixtures/org";
import { auditEvent } from "~/test/fixtures/shell";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = "/settings/instance/audit-log";

const EVENTS = [
  auditEvent({ id: "evt_self", action: "user.deactivated", actor: { kind: "user", id: USER }, target_kind: "user", target_id: "usr_grace" }),
  auditEvent({
    id: "evt_other",
    action: "org.renamed",
    actor: { kind: "user", id: "usr_grace" },
    target_kind: "org",
    target_id: "org_contoso",
    ip: "198.51.100.4",
  }),
  auditEvent({ id: "evt_system", action: "settings.updated", actor: { kind: "system" }, target_kind: "settings", target_id: "instance", ip: "198.51.100.5" }),
  auditEvent({
    id: "evt_gone",
    action: "org.deleted",
    actor: { kind: "user", id: "usr_gone" },
    target_kind: "user",
    target_id: "usr_unknown",
    ip: "198.51.100.6",
  }),
  auditEvent({ id: "evt_anon", action: "user.created", actor: { kind: "user", id: null }, target_kind: "org", target_id: "org_unknown", ip: "198.51.100.7" }),
  auditEvent({ id: "evt_key", action: "key.used", actor: { kind: "api_key", id: "key_ci" }, target_kind: null, target_id: null, ip: "198.51.100.8" }),
  auditEvent({ id: "evt_kind", action: "key.used", actor: { kind: "service_account", id: null }, target_kind: null, target_id: null, ip: "198.51.100.9" }),
];

const USERS = [instanceUser({ id: USER, name: "Ada Okafor", email: "owner@example.com" }), instanceUser()];
const ORGS = [instanceOrg({ id: "org_contoso", name: "Contoso" })];

function instanceAudit(events: AuditEventView[] = EVENTS, { listed = USERS, users = USERS }: { listed?: InstanceUserView[]; users?: InstanceUserView[] } = {}) {
  on("GET", "instance/audit-log", paged(events));
  on("GET", "instance/audit-log/:event", ({ params }: { params: Record<string, string> }) => events.find((e) => e.id === params.event));
  on("GET", "instance/users", ({ query }: { query: URLSearchParams }) =>
    paged(query.get("search") ? users.filter((u) => u.name.toLowerCase().includes(query.get("search")?.toLowerCase() ?? "")) : listed),
  );
  on(
    "GET",
    "instance/users/:user",
    ({ params }: { params: Record<string, string> }) => users.find((u) => u.id === params.user) ?? apiError(404, "not_found", "No such user."),
  );
  on(
    "GET",
    "instance/orgs/:org",
    ({ params }: { params: Record<string, string> }) => ORGS.find((o) => o.id === params.org) ?? apiError(404, "not_found", "No such organization."),
  );
}

const table = () => screen.getByRole("table", { name: "Audit log" });
const row = (ip: string) => within(table()).getByText(ip).closest('[role="row"]') as HTMLElement;

describe("instance audit log", () => {
  it("names actors and targets from the install's users and organizations", async () => {
    signedIn();
    instanceAudit();
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Audit log", level: 1 })).toBeInTheDocument();
    expect(await within(table()).findByText("Ada Okafor (you)")).toBeInTheDocument();
    await waitFor(() => expect(within(row("198.51.100.4")).getByText("Grace Hopper")).toBeInTheDocument());
    expect(within(row("198.51.100.4")).getByRole("link", { name: "Contoso" })).toHaveAttribute("href", "/settings/instance/orgs/org_contoso");
    expect(within(row("198.51.100.5")).getByText("System")).toBeInTheDocument();
    expect(within(row("198.51.100.5")).getByText("instance")).toBeInTheDocument();
    expect(within(row("198.51.100.5")).queryByRole("link")).not.toBeInTheDocument();
    expect(within(row("198.51.100.6")).getByText("usr_gone")).toBeInTheDocument();
    expect(within(row("198.51.100.6")).getByRole("link", { name: "usr_unknown" })).toHaveAttribute("href", "/settings/instance/users?user=usr_unknown");
    expect(within(row("198.51.100.7")).getByText("A user")).toBeInTheDocument();
    expect(within(row("198.51.100.7")).getByRole("link", { name: "org_unknown" })).toHaveAttribute("href", "/settings/instance/orgs/org_unknown");
    expect(within(row("198.51.100.8")).getByText("key_ci")).toBeInTheDocument();
    expect(within(row("198.51.100.9")).getByText("service_account")).toBeInTheDocument();
    const self = within(table()).getByText("Ada Okafor (you)").closest('[role="row"]') as HTMLElement;
    expect(within(self).getByRole("link", { name: "Grace Hopper" })).toHaveAttribute("href", "/settings/instance/users?user=usr_grace");
  });

  it("looks up only the people and organizations the user list doesn't already name", async () => {
    signedIn();
    instanceAudit();
    await renderApp(PATH);
    await waitFor(() => expect(within(row("198.51.100.4")).getByText("Grace Hopper")).toBeInTheDocument());
    await waitFor(() =>
      expect(
        requests("GET", "instance/orgs/:org")
          .map((r) => r.params.org)
          .sort(),
      ).toEqual(["org_contoso", "org_unknown"]),
    );
    expect(
      requests("GET", "instance/users/:user")
        .map((r) => r.params.user)
        .sort(),
    ).toEqual(["usr_gone", "usr_unknown"]);
  });

  it("filters by actor through the URL", async () => {
    signedIn();
    instanceAudit();
    const { user } = await renderApp(PATH);
    await within(table()).findByText("Ada Okafor (you)");
    await user.click(screen.getByRole("combobox", { name: "Actor" }));
    await user.click(await screen.findByRole("option", { name: /Grace Hopper/ }));
    await waitFor(() => expect(new URLSearchParams(window.location.search).get("actor")).toBe("usr_grace"));
    await waitFor(() => expect(requests("GET", "instance/audit-log").at(-1)?.query.get("actor")).toBe("usr_grace"));
  });

  it("names actors and filters by people beyond the first page of users", async () => {
    signedIn();
    const linus = instanceUser({ id: "usr_linus", name: "Linus Pauling", email: "linus@example.com" });
    instanceAudit([auditEvent({ id: "evt_far", action: "org.renamed", actor: { kind: "user", id: "usr_linus" }, ip: "198.51.100.10" })], {
      listed: [USERS[0] as InstanceUserView],
      users: [...USERS, linus],
    });
    const { user } = await renderApp(PATH);
    await waitFor(() => expect(within(row("198.51.100.10")).getByText("Linus Pauling")).toBeInTheDocument());
    await user.type(screen.getByRole("combobox", { name: "Actor" }), "lin");
    await user.click(await screen.findByRole("option", { name: /linus@example\.com/ }));
    await waitFor(() => expect(new URLSearchParams(window.location.search).get("actor")).toBe("usr_linus"));
    expect(requests("GET", "instance/users").some((r) => r.query.get("search") === "lin")).toBe(true);
  });

  it("keeps an actor from the URL that the user search doesn't list", async () => {
    signedIn();
    const linus = instanceUser({ id: "usr_linus", name: "Linus Pauling", email: "linus@example.com" });
    instanceAudit([auditEvent({ id: "evt_far", action: "org.renamed", actor: { kind: "user", id: "usr_linus" }, ip: "198.51.100.10" })], {
      listed: [USERS[0] as InstanceUserView],
      users: [...USERS, linus],
    });
    await renderApp(`${PATH}?actor=usr_linus`);
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Actor" })).toHaveValue("Linus Pauling"));
    expect(requests("GET", "instance/audit-log").at(-1)?.query.get("actor")).toBe("usr_linus");
  });

  it("shows each actor's picture, or their initial without one", async () => {
    signedIn({ session: whoAmI({ user: { ...whoAmI().user, avatar_url: "/avatars/ada.webp" } }) });
    const everyone = [instanceUser({ id: USER, name: "Ada Okafor", email: "owner@example.com" }), instanceUser({ avatar_url: "/avatars/grace.webp" })];
    instanceAudit(EVENTS, { listed: everyone, users: everyone });
    const { user } = await renderApp(PATH);
    const self = (await within(table()).findByText("Ada Okafor (you)")).closest('[role="row"]') as HTMLElement;
    expect(self.querySelector('img[src="/avatars/ada.webp"]')).not.toBeNull();
    await waitFor(() => expect(row("198.51.100.4").querySelector('img[src="/avatars/grace.webp"]')).not.toBeNull());
    expect(within(row("198.51.100.6")).getByText("U")).toBeInTheDocument();
    expect(row("198.51.100.6").querySelector("img")).toBeNull();
    expect(row("198.51.100.5").querySelector("img")).toBeNull();

    await user.click(screen.getByRole("combobox", { name: "Actor" }));
    const option = await screen.findByRole("option", { name: /Grace Hopper/ });
    expect(option.querySelector('img[src="/avatars/grace.webp"]')).not.toBeNull();
  });

  it("shows the initial for an actor without a picture", async () => {
    signedIn();
    instanceAudit();
    const { user } = await renderApp(PATH);
    await within(table()).findByText("Ada Okafor (you)");
    await waitFor(() => expect(within(row("198.51.100.4")).getByText("G")).toBeInTheDocument());
    expect(table().querySelector("img")).toBeNull();

    await user.click(screen.getByRole("combobox", { name: "Actor" }));
    const option = await screen.findByRole("option", { name: /Grace Hopper/ });
    expect(within(option).getByText("G")).toBeInTheDocument();
  });

  it("opens an event in a sheet", async () => {
    signedIn();
    instanceAudit();
    const { user } = await renderApp(PATH);
    await user.click(await within(table()).findByText("198.51.100.4"));
    const sheet = await screen.findByRole("dialog", { name: "org.renamed" });
    expect(await within(sheet).findByText("Grace Hopper")).toBeInTheDocument();
    expect(new URLSearchParams(window.location.search).get("event")).toBe("evt_other");
    expect(requests("GET", "instance/audit-log/:event").map((r) => r.params)).toEqual([{ event: "evt_other" }]);
  });
});
