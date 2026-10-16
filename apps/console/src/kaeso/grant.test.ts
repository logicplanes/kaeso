import { describe, expect, it } from "vitest";
import type { RightsCatalogue, RoleView } from "~/api/generated/types";
import { indexCatalogue, rightName, rightNames, rightsGroups, sourceChip, withNeeds } from "./catalogue";
import { applyChanges, effectiveRights, grantLabel, sameGrant, type Grant } from "./grant";
import { cantGive, holdsAll, roleOptions } from "./roles";

const right = (id: string, name: string, needs: string[] = [], scope = "project") => ({
  id,
  scope,
  name,
  description: `${name}.`,
  sensitive: id === "logs.export",
  recent_sign_in: false,
  cloud_only: false,
  needs,
});
const catalogue: RightsCatalogue = {
  org: {
    categories: [
      {
        id: "members",
        name: "Members & access",
        rights: [right("org.view", "View organization", [], "org"), right("members.view", "View members", ["org.view"], "org")],
      },
    ],
  },
  project: {
    categories: [
      { id: "project", name: "Project", rights: [right("project.view", "View project")] },
      {
        id: "logs",
        name: "Logs & data",
        rights: [
          right("logs.metadata.view", "View request logs", ["project.view"]),
          right("logs.export", "Export logs", ["project.view", "logs.metadata.view"]),
        ],
      },
    ],
  },
};
const cat = indexCatalogue(catalogue);
const role = (id: string, scope: string, rights: string[], project_rights: string[] = []): RoleView => ({
  id,
  scope,
  kind: id.startsWith("role_") ? "custom" : "builtin",
  name: id === "role_1" ? "Support lead" : id.charAt(0).toUpperCase() + id.slice(1),
  description: "",
  rights,
  project_rights,
  holders: 0,
});
const viewer = new Set(["project.view", "logs.metadata.view"]);
const plain: Grant = { role: "viewer", rights_added: [], rights_removed: [] };

describe("grants", () => {
  it("adds a right outside the role and takes it back", () => {
    const g = applyChanges(viewer, plain, [{ code: "logs.export", granted: true }], cat.order.project);
    expect(g.rights_added).toEqual(["logs.export"]);
    expect(effectiveRights(viewer, g).has("logs.export")).toBe(true);
    expect(applyChanges(viewer, g, [{ code: "logs.export", granted: false }], cat.order.project)).toEqual(plain);
  });
  it("removes a role's right and restores it", () => {
    const g = applyChanges(viewer, plain, [{ code: "logs.metadata.view", granted: false }], cat.order.project);
    expect(g.rights_removed).toEqual(["logs.metadata.view"]);
    expect(applyChanges(viewer, g, [{ code: "logs.metadata.view", granted: true }], cat.order.project)).toEqual(plain);
  });
  it("says a customized role", () => {
    expect(grantLabel({ role: { id: "viewer", kind: "builtin", name: "Viewer" }, rights_added: ["logs.export"], rights_removed: [] })).toBe(
      "Viewer · customized",
    );
  });
  it("tells the same grant from a changed one, whatever the order of its rights", () => {
    const custom: Grant = { role: "viewer", rights_added: ["logs.export", "project.view"], rights_removed: ["logs.metadata.view"] };
    expect(sameGrant(custom, { ...custom, rights_added: ["project.view", "logs.export"] })).toBe(true);
    expect(sameGrant(custom, { ...custom, role: "developer" })).toBe(false);
    expect(sameGrant(custom, { ...custom, rights_added: ["logs.export"] })).toBe(false);
    expect(sameGrant(custom, { ...custom, rights_added: ["logs.export", "logs.metadata.view"] })).toBe(false);
    expect(sameGrant(custom, { ...custom, rights_removed: [] })).toBe(false);
  });
});

describe("the catalogue", () => {
  it("groups rights by category with names, needs and origins, and disables what the editor doesn't hold", () => {
    const groups = rightsGroups(cat, "project", { granted: new Set(["project.view"]), base: viewer, held: viewer });
    expect(groups.map((g) => g.label)).toEqual(["Project", "Logs & data"]);
    const logs = groups[1]?.rights ?? [];
    expect(logs[0]).toMatchObject({ code: "logs.metadata.view", label: "View request logs", granted: false, origin: "removed", requires: ["project.view"] });
    expect(logs[0]?.disabled).toBeUndefined();
    expect(logs[1]).toMatchObject({ code: "logs.export", sensitive: true, disabled: true });
  });
  it("names rights and follows what they need", () => {
    expect(rightNames(cat, ["logs.export", "project.view", "logs.metadata.view"])).toBe("Export logs, View project and 1 more");
    expect([...withNeeds(cat, ["logs.export"])].sort()).toEqual(["logs.export", "logs.metadata.view", "project.view"]);
  });
  it("falls back to the id of a right the catalogue doesn't know", () => {
    expect(rightName(cat, "billing.view")).toBe("billing.view");
    expect(rightName(undefined, "logs.export")).toBe("logs.export");
    expect([...withNeeds(cat, ["billing.view"])]).toEqual(["billing.view"]);
    expect(sourceChip(cat, { kind: "org_right" }, "project").label).toBe("");
    expect(sourceChip(cat, { kind: "org_right", right: "org.view" }, "project").label).toBe("View organization");
  });
});

describe("who may give a role", () => {
  const developer = { held: new Set(["org.view", "members.view"]), everyProject: new Set<string>(), isOwner: false };
  it("keeps owner for owners", () => {
    expect(cantGive(cat, role("owner", "org", ["org.view"]), developer)).toBe("Only owners can make someone an owner");
    expect(cantGive(cat, role("owner", "org", ["org.view"]), { ...developer, isOwner: true })).toBeUndefined();
  });
  it("names the rights the person doesn't hold, in the org and in every project", () => {
    expect(cantGive(cat, role("role_1", "org", ["org.view"], ["logs.export"]), developer)).toBe("You can't give it: you don't hold Export logs");
    const options = roleOptions(cat, [role("member", "org", ["org.view"]), role("role_1", "org", ["org.view"], ["logs.export"])], developer);
    expect(options.map((o) => [o.value, !!o.disabled, o.custom])).toEqual([
      ["member", false, false],
      ["role_1", true, true],
    ]);
  });
  it("holds a peer's rights", () => {
    expect(holdsAll(viewer, ["project.view"])).toBe(true);
    expect(holdsAll(viewer, ["logs.export"])).toBe(false);
  });
});
