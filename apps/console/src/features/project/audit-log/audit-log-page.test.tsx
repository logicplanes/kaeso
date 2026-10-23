import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { AuditEventView } from "~/api/generated/types";
import { ALL_PROJECT_RIGHTS, USER, signedIn, whoAmI } from "~/test/fixtures";
import { paged } from "~/test/fixtures/org";
import { projectData, projectMember, projectPath } from "~/test/fixtures/project";
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

const table = () => screen.getByRole("table", { name: "Audit log" });
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

  it("shows ids instead of names without View members", async () => {
    signedIn({ projectRights: ALL_PROJECT_RIGHTS.filter((r) => r !== "project.members.view") });
    projectData({ members: [projectMember()] });
    projectAudit();
    await renderApp(PATH);
    expect(await within(table()).findByText("Ada Okafor (you)")).toBeInTheDocument();
    expect(within(row("198.51.100.1")).getByRole("link", { name: "usr_grace" })).toBeInTheDocument();
    expect(within(row("198.51.100.2")).getAllByText("usr_grace").length).toBeGreaterThan(0);
    expect(requests("GET", "orgs/:org/projects/:project/members")).toHaveLength(0);
  });
});
