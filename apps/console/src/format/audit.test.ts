import { describe, expect, it } from "vitest";
import { apply, load } from "~/i18n";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { AUDIT_ACTIONS, auditActionLabel, auditActionOptions, rightsChangeSummary, snapshotName, targetKindLabel } from "./audit";

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
  const gatewayActions = () => {
    const src = readFileSync(join(__dirname, "../../../../crates/kaeso-server/src/platform/audit.rs"), "utf8");
    const start = src.indexOf("pub const ACTIONS");
    return [...src.slice(start, src.indexOf("];", start)).matchAll(/"([a-z0-9_]+\.[a-z0-9_]+)"/g)].map((m) => m[1] ?? "");
  };

  it("name every action the gateway writes, and no other", () => {
    const gateway = gatewayActions();
    expect(gateway.length).toBeGreaterThan(90);
    expect(Object.values(AUDIT_ACTIONS).flat().sort()).toEqual([...gateway].sort());
    for (const action of gateway) expect(auditActionLabel(action), action).not.toBe(action);
    expect(new Set(gateway.map(auditActionLabel)).size).toBe(gateway.length);
  });

  it("keep an action they don't know as it was written", () => {
    expect(auditActionLabel("webhook.create")).toBe("webhook.create");
  });

  it("offer every action by its name, each kind first with a choice for all its actions", () => {
    const options = auditActionOptions();
    expect(options.slice(0, 4)).toEqual([
      { value: "alias.", label: "Aliases: every action", description: "alias.*" },
      { value: "alias.create", label: "Alias created", description: "alias.create" },
      { value: "alias.update", label: "Alias changed", description: "alias.update" },
      { value: "alias.delete", label: "Alias deleted", description: "alias.delete" },
    ]);
    const kinds = Object.keys(AUDIT_ACTIONS);
    const actions = Object.values(AUDIT_ACTIONS).flat();
    expect(options).toHaveLength(kinds.length + actions.length);
    expect(options.filter((o) => o.value.endsWith(".")).map((o) => o.value)).toEqual(kinds.map((k) => `${k}.`));
    for (const kind of options.filter((o) => o.value.endsWith("."))) expect(kind.label, kind.value).not.toMatch(/^[a-z_]+: /);
  });

  it("are named in German and Chinese", async () => {
    apply(await load("de"));
    expect(auditActionLabel("security.sso_delete")).toBe("Single Sign-on getrennt");
    expect(auditActionOptions()[0]?.label).toBe("Aliasse: jede Aktion");
    apply(await load("zh-CN"));
    expect(auditActionLabel("key.archive")).toBe("已归档密钥");
  });
});

describe("target names in snapshots", () => {
  it("come from the event's values, by the kind of target", () => {
    expect(snapshotName({ target_kind: "key", after: { name: "checkout-web" } })).toBe("checkout-web");
    expect(snapshotName({ target_kind: "catalogue_model", after: { provider: "deepseek", model_id: "deepseek-flash" } })).toBe("deepseek/deepseek-flash");
    expect(snapshotName({ target_kind: "invite", after: { email: "sam@example.com" } })).toBe("sam@example.com");
    expect(snapshotName({ target_kind: "limit", after: { description: "daily token budget of 20000000 tokens" } })).toBe(
      "daily token budget of 20000000 tokens",
    );
    expect(snapshotName({ target_kind: "org", after: { org: { id: "org_1", name: "Northwind Labs" } } })).toBe("Northwind Labs");
    expect(snapshotName({ target_kind: "org", before: { name: "Northwind" } })).toBe("Northwind");
    expect(snapshotName({ target_kind: "role", after: null, before: { name: "Support lead" } })).toBe("Support lead");
    expect(snapshotName({ target_kind: "catalogue_model", after: { model_id: "gpt-5" } })).toBe("gpt-5");
    expect(snapshotName({ target_kind: "user", after: { email: "grace@example.com" } })).toBe("grace@example.com");
    expect(snapshotName({ target_kind: "project", after: { email: "grace@example.com" } })).toBeNull();
    expect(snapshotName({ target_kind: "key", after: null, before: null })).toBeNull();
  });
});

describe("target kinds", () => {
  it("are named in English and German, and an unknown kind stays as written", async () => {
    expect([targetKindLabel("role"), targetKindLabel("update")]).toEqual(["Role", "Update"]);
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
