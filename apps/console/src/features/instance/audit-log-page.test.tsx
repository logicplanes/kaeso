import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { AuditEventView } from "~/api/generated/types";
import { USER, signedIn } from "~/test/fixtures";
import { instanceOrg, instanceUser } from "~/test/fixtures/instance-account";
import { paged } from "~/test/fixtures/org";
import { auditEvent } from "~/test/fixtures/shell";
import { on, requests } from "~/test/network";
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

function instanceAudit(events: AuditEventView[] = EVENTS) {
  on("GET", "instance/audit-log", paged(events));
  on("GET", "instance/audit-log/:event", ({ params }: { params: Record<string, string> }) => events.find((e) => e.id === params.event));
  on("GET", "instance/users", paged([instanceUser({ id: USER, name: "Ada Okafor", email: "owner@example.com" }), instanceUser()]));
  on("GET", "instance/orgs", paged([instanceOrg({ id: "org_contoso", name: "Contoso" })]));
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
