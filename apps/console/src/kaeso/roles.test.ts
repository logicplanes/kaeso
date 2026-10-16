import { describe, expect, it } from "vitest";
import { catalogue, roles } from "~/test/fixtures";
import { indexCatalogue } from "./catalogue";
import { cantGive, roleOptions, roleSize, rolesOfScope } from "./roles";

const cat = indexCatalogue(catalogue);
const role = (scope: "org" | "project", id: string) => roles.find((r) => r.scope === scope && r.id === id) as (typeof roles)[number];

describe("roles", () => {
  it("sizes a role by its rights and its rights in every project", () => {
    const viewer = role("project", "viewer");
    expect(roleSize(viewer)).toBe(`${viewer.rights.length} rights`);
    const admin = role("org", "admin");
    expect(roleSize({ ...admin, project_rights: ["project.view"] })).toBe(`${admin.rights.length} rights, 1 in projects`);
    expect(roleSize({ ...admin, project_rights: [] })).toBe(`${admin.rights.length} rights`);
  });

  it("refuses an organization role whose project rights the giver doesn't hold everywhere", () => {
    const member = { ...role("org", "member"), project_rights: ["project.view", "usage.view"] };
    const giver = { held: new Set(member.rights), everyProject: new Set(["project.view"]), isOwner: false };
    expect(cantGive(cat, member, giver)).toBe("You can't give it: you don't hold View usage");
    expect(cantGive(cat, member, { held: new Set(member.rights), isOwner: false })).toBe("You can't give it: you don't hold View project and View usage");
    expect(cantGive(cat, role("org", "owner"), giver)).toBe("Only owners can make someone an owner");
    expect(cantGive(cat, role("org", "owner"), { ...giver, isOwner: true })).toBeUndefined();
  });

  it("adds each role's size as a hint when asked", () => {
    const options = roleOptions(cat, rolesOfScope(roles, "project"), null, { hint: true });
    expect(options.find((o) => o.value === "viewer")).toMatchObject({
      label: "Viewer",
      hint: `${role("project", "viewer").rights.length} rights`,
      custom: false,
    });
    expect(options.find((o) => o.label === "Support lead")?.custom).toBe(true);
    expect(rolesOfScope(undefined, "org")).toEqual([]);
  });
});
