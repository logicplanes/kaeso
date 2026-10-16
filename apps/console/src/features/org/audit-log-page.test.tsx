import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { AuditEventView } from "~/api/generated/types";
import { ORG, PROJECT, USER, signedIn, withRights } from "~/test/fixtures";
import { OTHER_USER, credential, fill, member, paged, platformKey } from "~/test/fixtures/org";
import { auditEvent } from "~/test/fixtures/shell";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/settings/orgs/${ORG}/audit-log`;
const BASE = `/settings/orgs/${ORG}`;

function orgData(events: AuditEventView[]) {
  on("GET", "orgs/:org/members", paged([member(), member({ user: { id: OTHER_USER, name: "Grace Hopper", email: "grace@example.com" } })]));
  on("GET", "orgs/:org/platform-keys", paged([platformKey()]));
  on("GET", "orgs/:org/credentials", { data: [credential()] });
  on("GET", "orgs/:org/audit-log", paged(events));
  on(
    "GET",
    "orgs/:org/audit-log/:event",
    ({ params }: { params: Record<string, string> }) => events.find((e) => e.id === params.event) ?? apiError(404, "not_found", "No such event."),
  );
}

const event = (id: string, actor: AuditEventView["actor"], target_kind: string | null, target_id: string | null) =>
  auditEvent({ id, actor, target_kind, target_id, action: `test.${id}` });

async function rowOf(action: string) {
  const code = await screen.findByText(action);
  return code.closest('[role="row"]') as HTMLElement;
}

describe("organization audit log", () => {
  it("names who made each change", async () => {
    signedIn();
    orgData([
      event("me", { kind: "user", id: USER }, null, null),
      event("grace", { kind: "user", id: OTHER_USER }, null, null),
      event("gone", { kind: "user", id: "usr_gone" }, null, null),
      event("anon", { kind: "user", id: null }, null, null),
      event("system", { kind: "system" }, null, null),
      event("pk", { kind: "org_platform_key", id: "opk_terraform" }, null, null),
      event("pkgone", { kind: "org_platform_key", id: "opk_gone" }, null, null),
      event("pknone", { kind: "org_platform_key" }, null, null),
      event("key", { kind: "api_key", id: "key_web" }, null, null),
      event("keynone", { kind: "api_key" }, null, null),
      event("sa", { kind: "service_account", id: "sa_bot" }, null, null),
      event("sanone", { kind: "service_account" }, null, null),
      event("other", { kind: "robot", id: "rb_1" }, null, null),
      event("othernone", { kind: "robot" }, null, null),
    ]);
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Audit log", level: 1 })).toBeInTheDocument();
    expect(await rowOf("test.me")).toHaveTextContent("Ada Okafor (you)");
    expect(await rowOf("test.grace")).toHaveTextContent("Grace Hopper");
    expect(await rowOf("test.gone")).toHaveTextContent("usr_gone");
    expect(await rowOf("test.anon")).toHaveTextContent("A member");
    expect(await rowOf("test.system")).toHaveTextContent("System");
    expect(await rowOf("test.pk")).toHaveTextContent("Platform key Terraform");
    expect(await rowOf("test.pkgone")).toHaveTextContent("Platform key opk_gone");
    expect(await rowOf("test.pknone")).toHaveTextContent("Platform key");
    expect(await rowOf("test.key")).toHaveTextContent("API key key_web");
    expect(await rowOf("test.keynone")).toHaveTextContent("API key");
    expect(await rowOf("test.sa")).toHaveTextContent("Service account sa_bot");
    expect(await rowOf("test.sanone")).toHaveTextContent("Service account");
    expect(await rowOf("test.other")).toHaveTextContent("rb_1");
    expect(await rowOf("test.othernone")).toHaveTextContent("robot");
  });

  it("names and links what each change was made to", async () => {
    signedIn();
    const system = { kind: "system" };
    orgData([
      event("project", system, "project", PROJECT),
      event("user", system, "user", OTHER_USER),
      event("invite", system, "invite", "inv_1"),
      event("credential", system, "credential", "cred_openai"),
      event("platformkey", system, "platform_key", "opk_terraform"),
      event("org", system, "org", ORG),
      event("otherorg", system, "org", "org_other"),
      event("key", system, "key", "key_web"),
    ]);
    await renderApp(PATH);
    const link = async (action: string) => within(await rowOf(action)).getByRole("link");
    expect(await link("test.project")).toHaveAttribute("href", `/orgs/${ORG}/projects/${PROJECT}`);
    expect(await link("test.project")).toHaveTextContent("Checkout assistant");
    expect(await link("test.user")).toHaveAttribute("href", `${BASE}/members?member=${OTHER_USER}`);
    expect(await link("test.user")).toHaveTextContent("Grace Hopper");
    expect(await link("test.invite")).toHaveAttribute("href", `${BASE}/members?tab=invites`);
    expect(await link("test.invite")).toHaveTextContent("inv_1");
    expect(await link("test.credential")).toHaveAttribute("href", `/orgs/${ORG}/providers`);
    expect(await link("test.credential")).toHaveTextContent("OpenAI production");
    expect(await link("test.platformkey")).toHaveAttribute("href", `${BASE}/platform-keys`);
    expect(await link("test.platformkey")).toHaveTextContent("Terraform");
    expect(await link("test.org")).toHaveAttribute("href", `${BASE}/general`);
    expect(await link("test.org")).toHaveTextContent("Northwind Labs");
    expect(await link("test.otherorg")).toHaveTextContent("org_other");
    expect(within(await rowOf("test.key")).queryByRole("link")).not.toBeInTheDocument();
    expect(await rowOf("test.key")).toHaveTextContent("key_web");
  });

  it("opens an event from the list and keeps it in the URL", async () => {
    signedIn();
    orgData([event("grace", { kind: "user", id: OTHER_USER }, "user", OTHER_USER)]);
    const { user, router } = await renderApp(PATH);
    await user.click(await screen.findByText("test.grace"));
    await waitFor(() => expect(router.state.location.search).toEqual({ event: "grace" }));
    const sheet = await screen.findByRole("dialog", { name: "test.grace" });
    expect(await within(sheet).findByText("Grace Hopper", { selector: "a *, a" })).toBeInTheDocument();
    expect(requests("GET", "orgs/:org/audit-log/grace")).toHaveLength(1);
  });

  it("filters by target id through the URL", async () => {
    signedIn();
    orgData([]);
    const { user, router } = await renderApp(PATH);
    expect(await screen.findByText("No events yet")).toBeInTheDocument();
    await fill(user, screen.getByRole("searchbox", { name: "Target id" }), "key_web");
    await user.keyboard("{Enter}");
    await waitFor(() => expect(router.state.location.search).toEqual({ target: "key_web" }));
    await waitFor(() => expect(requests("GET", "orgs/:org/audit-log").map((r) => r.query.get("target"))).toContain("key_web"));
    expect(await screen.findByText("No events match")).toBeInTheDocument();
  });

  it("offers an export to people who may export the log", async () => {
    signedIn();
    orgData([]);
    await renderApp(`${PATH}?actor=${USER}&event=evt_1`);
    expect(await screen.findByRole("button", { name: "Export" })).toBeInTheDocument();
  });

  it("reads the log without the other rights", async () => {
    withRights(["org.view", "audit_log.view"], { role: "auditor" });
    orgData([event("grace", { kind: "user", id: OTHER_USER }, "credential", "cred_openai")]);
    await renderApp(PATH);
    const row = await rowOf("test.grace");
    expect(row).toHaveTextContent(OTHER_USER);
    expect(row).toHaveTextContent("cred_openai");
    expect(screen.queryByRole("button", { name: "Export" })).not.toBeInTheDocument();
    expect(requests("GET", "orgs/:org/members")).toHaveLength(0);
    expect(requests("GET", "orgs/:org/platform-keys")).toHaveLength(0);
    expect(requests("GET", "orgs/:org/credentials")).toHaveLength(0);
  });

  it("hides the page from people who can't view the audit log", async () => {
    withRights(["org.view"], { role: "auditor" });
    await renderApp(PATH);
    expect(await screen.findByText("You can't see the audit log here")).toBeInTheDocument();
    expect(screen.getByText("You need View audit log; ask an owner or admin for it.")).toBeInTheDocument();
    expect(requests("GET", "orgs/:org/audit-log")).toHaveLength(0);
  });
});
