import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ADMIN_ORG_RIGHTS, PROJECT, asAdmin, project, signedIn } from "~/test/fixtures";
import { hoverReason, notifications, paged } from "~/test/fixtures/org";
import { logSettings, projectData, projectPath } from "~/test/fixtures/project";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import type { GuardrailView, ProjectGuardrailView } from "~/api/generated/types";

const USED = "orgs/:org/projects/:project/guardrails";
const ALL = "orgs/:org/guardrails";
const SET = "orgs/:org/guardrails/projects";

const used = (over: Partial<ProjectGuardrailView> = {}): ProjectGuardrailView => ({
  id: "grd_pii",
  name: "PII check",
  phases: ["request"],
  on_error: "block",
  streams: "skip",
  timeout_ms: 2000,
  ...over,
});

const org = (over: Partial<GuardrailView> = {}): GuardrailView =>
  ({
    ...used(),
    url: "https://guardrail.example.com/check",
    projects: [],
    created_by: { kind: "user", id: "usr_ada", name: "Ada Okafor" },
    created_at: "2026-11-01T10:00:00Z",
    updated_at: "2026-11-01T10:00:00Z",
    revision: 1,
    ...over,
  }) as GuardrailView;

function setUp(list: ProjectGuardrailView[], orgRights?: string[]) {
  if (orgRights) asAdmin(orgRights);
  else signedIn({ projects: [project()] });
  projectData();
  on("GET", "orgs/:org/projects/:project/logs/settings", logSettings());
  on("GET", "orgs/:org/projects/:project/response-cache", { enabled: false, ttl_seconds: 3600, org_allowed: true, available: true, effective: false });
  on("GET", USED, paged(list));
}

describe("project guardrails", () => {
  it("lists the project's guardrails in the order they run", async () => {
    setUp([used(), used({ id: "grd_out", name: "Output check", phases: ["reply"], on_error: "allow", streams: "hold" })]);
    await renderApp(projectPath("settings"));
    const list = await screen.findByRole("grid", { name: "Guardrails" });
    const pii = await within(list).findByRole("gridcell", { name: /PII check/ });
    expect(pii).toHaveTextContent("Requests");
    expect(pii).toHaveTextContent("Refuses requests when it fails");
    expect(pii).toHaveTextContent("1 of 2");
    const out = within(list).getByRole("gridcell", { name: /Output check/ });
    expect(out).toHaveTextContent("Replies, streams held");
    expect(out).toHaveTextContent("Lets requests through when it fails");
    expect(out).toHaveTextContent("2 of 2");
  });

  it("says when the project uses none", async () => {
    setUp([]);
    await renderApp(projectPath("settings"));
    expect(await screen.findByText("Requests go to models unchecked.")).toBeInTheDocument();
  });

  it("is hidden from people who can't choose guardrails while the project has none", async () => {
    setUp(
      [],
      ADMIN_ORG_RIGHTS.filter((r) => r !== "guardrails.manage"),
    );
    await renderApp(projectPath("settings"));
    await screen.findByRole("heading", { name: "Response cache" });
    expect(requests("GET", USED)).toHaveLength(1);
    expect(screen.queryByRole("heading", { name: "Guardrails" })).not.toBeInTheDocument();
  });

  it("can't be changed by people without Manage guardrails", async () => {
    setUp(
      [used()],
      ADMIN_ORG_RIGHTS.filter((r) => r !== "guardrails.manage"),
    );
    const { user } = await renderApp(projectPath("settings"));
    await screen.findByRole("grid", { name: "Guardrails" });
    const choose = screen.getByRole("button", { name: "Choose guardrails" });
    expect(choose).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, choose, /Manage guardrails/);
  });

  it("chooses the guardrails and their order", async () => {
    setUp([used(), used({ id: "grd_gone", name: "Deleted meanwhile" })]);
    on("GET", ALL, paged([org(), org({ id: "grd_out", name: "Output check", phases: ["reply"] }), org({ id: "grd_jb", name: "Jailbreak" })]));
    on("PUT", SET, paged([]));
    const { user } = await renderApp(projectPath("settings"));
    await screen.findByRole("grid", { name: "Guardrails" });
    await user.click(screen.getByRole("button", { name: "Choose guardrails" }));
    const dialog = await screen.findByRole("dialog", { name: "Choose guardrails" });
    expect(await within(dialog).findByRole("checkbox", { name: /PII check/ })).toBeChecked();
    expect(within(dialog).getByRole("button", { name: "Move PII check up" })).toBeDisabled();
    expect(within(dialog).queryByRole("checkbox", { name: /Deleted meanwhile/ })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("checkbox", { name: /Output check/ }));
    await user.click(within(dialog).getByRole("checkbox", { name: /Jailbreak/ }));
    await user.click(within(dialog).getByRole("button", { name: "Move Jailbreak up" }));
    await user.click(within(dialog).getByRole("button", { name: "Move PII check down" }));
    await user.click(within(dialog).getByRole("checkbox", { name: /Output check/ }));
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(await notifications().findByText("Guardrails saved")).toBeInTheDocument();
    expect(requests("PUT", SET).map((r) => r.body)).toEqual([{ project: PROJECT, guardrails: ["grd_jb", "grd_pii"] }]);
    expect(screen.queryByRole("dialog", { name: "Choose guardrails" })).not.toBeInTheDocument();
  });

  it("points to the organization's settings when it has no guardrails", async () => {
    setUp([]);
    on("GET", ALL, paged([]));
    const { user } = await renderApp(projectPath("settings"));
    await screen.findByText("Requests go to models unchecked.");
    await user.click(screen.getByRole("button", { name: "Choose guardrails" }));
    const dialog = await screen.findByRole("dialog", { name: "Choose guardrails" });
    expect(await within(dialog).findByText("Add one under Organization settings › Guardrails first.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog", { name: "Choose guardrails" })).not.toBeInTheDocument();
  });
});
