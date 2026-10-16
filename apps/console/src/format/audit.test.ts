import { describe, expect, it } from "vitest";
import { apply, load } from "~/i18n";
import { AUDIT_ACTIONS, auditActionOptions, rightsChangeSummary, targetKindLabel } from "./audit";

const names: Record<string, string> = { "logs.export": "Export logs", "limits.manage": "Manage limits", "logs.metadata.view": "View request logs" };
const name = (id: string) => names[id] ?? id;

describe("rights change summary", () => {
  it("says what a role change added and removed, in the org and in every project", () => {
    expect(
      rightsChangeSummary(
        { name: "Support lead", rights: ["limits.manage"], project_rights: [] },
        { name: "Support lead", rights: ["logs.export"], project_rights: ["logs.metadata.view"] },
        name,
      ),
    ).toBe("Support lead: + Export logs, − Manage limits; in every project + View request logs");
  });
  it("says a member's new role", () => {
    expect(rightsChangeSummary({ role: { name: "Viewer" }, rights: ["a"] }, { role: { name: "Developer" }, rights: ["a"] }, name)).toBe("Viewer → Developer");
  });
  it("is null when no rights changed", () => {
    expect(rightsChangeSummary({ name: "x" }, { name: "x" }, name)).toBeNull();
  });

  it("names a role by its preset when it has no name, and says the change of role beside the rights", () => {
    expect(rightsChangeSummary({ preset: "viewer", rights: [] }, { preset: "developer", rights: ["logs.export"] }, name)).toBe(
      "viewer → developer: + Export logs",
    );
  });

  it("lists the rights alone when no role is named", () => {
    expect(rightsChangeSummary({ project_rights: ["logs.metadata.view"] }, { project_rights: [] }, name)).toBe("in every project − View request logs");
    expect(rightsChangeSummary(null, { rights: ["limits.manage"] }, name)).toBe("+ Manage limits");
  });

  it("names the role once a member was created or removed", () => {
    expect(rightsChangeSummary(undefined, { role: { name: "Auditor" }, rights: ["logs.export"] }, name)).toBe("Auditor: + Export logs");
    expect(rightsChangeSummary({ name: "Billing", rights: ["logs.export"] }, null, name)).toBe("Billing: − Export logs");
    expect(rightsChangeSummary(undefined, { role: { name: "Auditor" } }, name)).toBeNull();
  });

  it("ignores what isn't a list of rights", () => {
    expect(rightsChangeSummary(["rights"], "rights", name)).toBeNull();
    expect(rightsChangeSummary({ rights: "logs.export" }, { rights: [1, "logs.export", null] }, name)).toBe("+ Export logs");
  });

  it("is written in German", async () => {
    apply(await load("de"));
    expect(
      rightsChangeSummary({ name: "Lead", rights: ["a"], project_rights: [] }, { name: "Lead", rights: ["b"], project_rights: ["c"] }, (r) => r.toUpperCase()),
    ).toBe("Lead: + B, − A; in jedem Projekt + C");
  });
});

describe("audit actions", () => {
  it("offer every action, each kind first with a choice for all its actions", () => {
    const options = auditActionOptions();
    expect(options.slice(0, 4)).toEqual([
      { value: "alias.", label: "alias.*", description: "Every action of this kind" },
      { value: "alias.create", label: "alias.create" },
      { value: "alias.update", label: "alias.update" },
      { value: "alias.delete", label: "alias.delete" },
    ]);
    const kinds = Object.keys(AUDIT_ACTIONS);
    const actions = Object.values(AUDIT_ACTIONS).flat();
    expect(options).toHaveLength(kinds.length + actions.length);
    expect(options.filter((o) => o.description).map((o) => o.value)).toEqual(kinds.map((k) => `${k}.`));
    expect(actions.every((a) => kinds.some((k) => a.startsWith(`${k}.`)))).toBe(true);
  });

  it("describe the choice for all actions in German", async () => {
    apply(await load("de"));
    expect(auditActionOptions()[0]?.description).toBe("Jede Aktion dieser Art");
  });
});

describe("target kinds", () => {
  it("are named in English and German, and an unknown kind stays as written", async () => {
    const kinds = ["user", "org", "invite", "project", "service_account", "limit", "credential", "catalogue_model", "alias", "platform_key", "key"];
    expect(kinds.map(targetKindLabel)).toEqual([
      "User",
      "Organization",
      "Invitation",
      "Project",
      "Service account",
      "Limit",
      "Provider key",
      "Model",
      "Alias",
      "Platform key",
      "Key",
    ]);
    expect(targetKindLabel("webhook")).toBe("webhook");
    apply(await load("de"));
    expect(kinds.map(targetKindLabel)).toEqual([
      "Benutzer",
      "Organisation",
      "Einladung",
      "Projekt",
      "Dienstkonto",
      "Limit",
      "Anbieter-Schlüssel",
      "Modell",
      "Alias",
      "Plattform-Schlüssel",
      "Schlüssel",
    ]);
  });
});
