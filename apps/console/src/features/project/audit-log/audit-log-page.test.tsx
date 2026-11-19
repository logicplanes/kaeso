import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { AuditEventView } from "~/api/generated/types";
import { ALL_PROJECT_RIGHTS, PROJECT, USER, signedIn, whoAmI } from "~/test/fixtures";
import { paged } from "~/test/fixtures/org";
import { alias, apiKey, catalogueModel, limit, projectData, projectMember, projectPath, serviceAccount } from "~/test/fixtures/project";
import { auditEvent } from "~/test/fixtures/shell";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = projectPath("audit-log");

const EVENTS = [
  auditEvent({ id: "evt_self", action: "member.added", actor: { kind: "user", id: USER }, target_kind: "user", target_id: "usr_grace", ip: "198.51.100.1" }),
  auditEvent({
    id: "evt_other",
    action: "key.created",
    actor: { kind: "user", id: "usr_grace" },
    target_kind: "key",
    target_id: "key_web",
    ip: "198.51.100.2",
  }),
];

function projectAudit(events: AuditEventView[] = EVENTS) {
  on("GET", "orgs/:org/projects/:project/audit-log", paged(events));
}

const table = () => screen.getByRole("grid", { name: "Audit log" });
const row = (ip: string) => within(table()).getByText(ip).closest('[role="row"]') as HTMLElement;

describe("project audit log", () => {
  it("names actors and links member targets to the members page", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    projectAudit();
    await renderApp(PATH);
    expect(await within(table()).findByText("Ada Okafor (you)")).toBeInTheDocument();
    await waitFor(() =>
      expect(within(row("198.51.100.1")).getByRole("link", { name: "Grace Hopper" })).toHaveAttribute("href", `${projectPath("members")}?member=usr_grace`),
    );
    expect(within(row("198.51.100.2")).getByText("Grace Hopper")).toBeInTheDocument();
    expect(within(row("198.51.100.2")).getByText("key_web")).toBeInTheDocument();
    expect(within(row("198.51.100.2")).queryByRole("link")).not.toBeInTheDocument();
  });

  it("puts the filters where every other page's toolbar sits, with no padding above them", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    projectAudit();
    await renderApp(PATH);
    await within(table()).findByText("Ada Okafor (you)");
    const body = screen.getByRole("toolbar", { name: "Filter the audit log" }).closest(".overflow-y-auto");
    expect(body).not.toBeNull();
    expect(body).not.toHaveClass("pt-8");
  });

  it("shows each actor's picture, or their initial without one", async () => {
    signedIn({ session: whoAmI({ user: { ...whoAmI().user, avatar_url: "/avatars/ada.webp" } }) });
    projectData({ members: [projectMember({ user: { avatar_url: "/avatars/grace.webp" } })] });
    projectAudit();
    const { user } = await renderApp(PATH);
    await within(table()).findByText("Ada Okafor (you)");
    expect(row("198.51.100.1").querySelector('img[src="/avatars/ada.webp"]')).not.toBeNull();
    await waitFor(() => expect(row("198.51.100.2").querySelector('img[src="/avatars/grace.webp"]')).not.toBeNull());

    await user.click(screen.getByRole("combobox", { name: "Actor" }));
    const option = await screen.findByRole("option", { name: /Grace Hopper/ });
    expect(option.querySelector('img[src="/avatars/grace.webp"]')).not.toBeNull();
  });

  it("shows the initial for an actor without a picture", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    projectAudit();
    const { user } = await renderApp(PATH);
    await within(table()).findByText("Ada Okafor (you)");
    await waitFor(() => expect(within(row("198.51.100.2")).getByText("G")).toBeInTheDocument());
    expect(within(row("198.51.100.1")).getByText("A")).toBeInTheDocument();
    expect(table().querySelector("img")).toBeNull();

    await user.click(screen.getByRole("combobox", { name: "Actor" }));
    const option = await screen.findByRole("option", { name: /Grace Hopper/ });
    expect(within(option).getByText("G")).toBeInTheDocument();
    expect(option.querySelector("img")).toBeNull();
  });

  it("filters by a member through the URL and opens an event from its row", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    projectAudit();
    const { user } = await renderApp(PATH);
    await within(table()).findByText("Ada Okafor (you)");
    await user.click(screen.getByRole("combobox", { name: "Actor" }));
    await user.click(await screen.findByRole("option", { name: /Grace Hopper/ }));
    await waitFor(() => expect(new URLSearchParams(window.location.search).get("actor")).toBe("usr_grace"));
    await waitFor(() => expect(requests("GET", "orgs/:org/projects/:project/audit-log").at(-1)?.query.get("actor")).toBe("usr_grace"));
    await user.click(within(table()).getByText("198.51.100.2"));
    expect(await screen.findByRole("dialog", { name: "key.created" })).toBeInTheDocument();
    expect(window.location.pathname).toBe(PATH);
    expect(new URLSearchParams(window.location.search).get("event")).toBe("evt_other");
  });

  it("shows ids without links instead of names without View members", async () => {
    signedIn({ projectRights: ALL_PROJECT_RIGHTS.filter((r) => r !== "project.members.view") });
    projectData({ members: [projectMember()] });
    projectAudit();
    await renderApp(PATH);
    expect(await within(table()).findByText("Ada Okafor (you)")).toBeInTheDocument();
    expect(within(row("198.51.100.1")).getByText("usr_grace")).toBeInTheDocument();
    expect(within(row("198.51.100.1")).queryByRole("link")).not.toBeInTheDocument();
    expect(within(row("198.51.100.2")).getAllByText("usr_grace").length).toBeGreaterThan(0);
    expect(requests("GET", "orgs/:org/projects/:project/members")).toHaveLength(0);
  });
});

// The generated type has no fields for a snapshot, which holds the target as it was.
const snapshot = (fields: Record<string, string>) => fields as unknown as Record<string, never>;

const TARGETS = [
  auditEvent({ id: "e1", action: "key.disabled", target_kind: "key", target_id: "key_web", ip: "198.51.100.11" }),
  auditEvent({ id: "e2", action: "key.archived", target_kind: "key", target_id: "key_old", ip: "198.51.100.12" }),
  auditEvent({ id: "e3", action: "catalogue_model.updated", target_kind: "catalogue_model", target_id: "cm_sonnet", ip: "198.51.100.13" }),
  auditEvent({ id: "e4", action: "alias.updated", target_kind: "alias", target_id: "al_fast", ip: "198.51.100.14" }),
  auditEvent({ id: "e5", action: "service_account.archived", target_kind: "service_account", target_id: "sa_gone", ip: "198.51.100.15" }),
  auditEvent({ id: "e6", action: "limit.updated", target_kind: "limit", target_id: "lim_key", ip: "198.51.100.16" }),
  auditEvent({ id: "e7", action: "limit.updated", target_kind: "limit", target_id: "lim_month", ip: "198.51.100.17" }),
  auditEvent({
    id: "e8",
    action: "portal_grant.created",
    target_kind: "portal_grant",
    target_id: "pg_1",
    ip: "198.51.100.18",
    after: snapshot({ name: "Docs search" }),
  }),
  auditEvent({ id: "e9", action: "project.updated", target_kind: "project", target_id: PROJECT, ip: "198.51.100.19" }),
  auditEvent({ id: "e10", action: "alias.deleted", target_kind: "alias", target_id: "al_gone", ip: "198.51.100.20", before: snapshot({ name: "cheap" }) }),
  auditEvent({ id: "e11", action: "project.updated", target_kind: "project", target_id: "prj_other", ip: "198.51.100.21" }),
  auditEvent({ id: "e12", action: "thing.done", target_kind: "thing", target_id: "thg_1", ip: "198.51.100.22" }),
];

const TARGET_DATA = {
  keys: [apiKey(), apiKey({ id: "key_old", name: "old-batch", state: "archived" })],
  models: [catalogueModel()],
  aliases: [alias()],
  accounts: [serviceAccount({ id: "sa_gone", name: "nightly", archived_at: "2026-10-01T00:00:00Z" })],
  limits: [limit(), limit({ id: "lim_key", level: "key", description: "" })],
};

const link = (ip: string) => within(row(ip)).getByRole("link");

describe("project audit log targets", () => {
  it("names keys, models, aliases, service accounts and limits and links each to where it opens", async () => {
    signedIn();
    projectData(TARGET_DATA);
    on("GET", "orgs/:org/projects/:project/audit-log", paged(TARGETS));
    await renderApp(PATH);
    await waitFor(() => expect(link("198.51.100.11")).toHaveTextContent("checkout-web"));
    expect(link("198.51.100.11")).toHaveAttribute("href", projectPath("keys?key=key_web"));
    await waitFor(() => expect(link("198.51.100.12")).toHaveAttribute("href", projectPath("keys?key=key_old")));
    expect(link("198.51.100.12")).toHaveTextContent("old-batch");
    await waitFor(() => expect(link("198.51.100.13")).toHaveAttribute("href", projectPath("models?model=cm_sonnet")));
    await waitFor(() => expect(link("198.51.100.14")).toHaveAttribute("href", projectPath("models/aliases/al_fast")));
    expect(link("198.51.100.14")).toHaveTextContent("fast");
    await waitFor(() => expect(link("198.51.100.15")).toHaveAttribute("href", projectPath("service-accounts?account=sa_gone")));
    expect(link("198.51.100.15")).toHaveTextContent("nightly");
    await waitFor(() => expect(link("198.51.100.16")).toHaveAttribute("href", projectPath("limits?level=key")));
    expect(link("198.51.100.16")).toHaveTextContent("lim_key");
    expect(link("198.51.100.17")).toHaveAttribute("href", projectPath("limits?level=project"));
    expect(link("198.51.100.17")).toHaveTextContent("monthly spend budget of 50 USD");
    expect(link("198.51.100.18")).toHaveAttribute("href", projectPath("tools"));
    expect(link("198.51.100.18")).toHaveTextContent("Docs search");
    expect(link("198.51.100.19")).toHaveAttribute("href", projectPath("settings"));
    // A deleted alias keeps its snapshot name, without a link; another project's and an unknown kind get no link.
    expect(within(row("198.51.100.20")).getByText("cheap")).toBeInTheDocument();
    for (const ip of ["198.51.100.20", "198.51.100.21", "198.51.100.22"]) expect(within(row(ip)).queryByRole("link")).not.toBeInTheDocument();
  });

  it("loads no list and links nothing the person can't view", async () => {
    const hidden = ["keys.all.view", "models.view", "service_accounts.view", "limits.view", "tools.view", "project.view"];
    signedIn({ projectRights: ALL_PROJECT_RIGHTS.filter((r) => !hidden.includes(r)) });
    projectData(TARGET_DATA);
    on("GET", "orgs/:org/projects/:project/audit-log", paged(TARGETS));
    await renderApp(PATH);
    expect(await within(table()).findByText("198.51.100.11")).toBeInTheDocument();
    expect(within(row("198.51.100.11")).getByText("key_web")).toBeInTheDocument();
    expect(within(table()).queryAllByRole("link")).toHaveLength(0);
    for (const path of ["keys", "models", "aliases", "service-accounts", "limits"]) {
      expect(requests("GET", `orgs/:org/projects/:project/${path}`)).toHaveLength(0);
    }
  });
});
