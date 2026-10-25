import { describe, expect, it } from "vitest";
import type { GrantBody } from "~/api/generated/types";
import { ORG, OTHER_PROJECT, PROJECT } from "~/test/fixtures";
import { agents, grantBody, grantable } from "~/test/fixtures/agents";
import { fromGrant, gainedSensitive, presetDraft, summary, toGrant, withRequested } from "./agent-grant";

describe("an agent's rights in the console", () => {
  it("start from the preset, narrowed to what the person holds and each org allows", () => {
    const g = grantable();
    const d = presetDraft(agents, g, "builder");
    expect(d.orgs[ORG]?.included).toBe(true);
    expect(d.orgs[ORG]?.rights).toContain("projects.create");
    expect(d.orgs.org_contoso?.included).toBe(false);
    expect(d.account).toEqual(["account.view", "account.keys.view"]);
    expect(d.instance).toEqual([]);
    const admin = presetDraft(
      agents,
      grantable({ instance: ["instance.system.view", "instance.users.view", "instance.orgs.view", "instance.audit_log.view"] }),
      "observer",
    );
    expect(admin.instance).toEqual(["instance.system.view", "instance.users.view", "instance.orgs.view", "instance.audit_log.view"]);
  });

  it("survive the round trip through the API", () => {
    const g = grantable();
    const body = grantBody({ future_orgs: { rights: ["org.view"], project_rights: ["project.view"] } });
    expect(toGrant(fromGrant(body, g))).toEqual(body);
    const selected = grantBody({
      customized: true,
      orgs: [
        { org: ORG, included: true, rights: ["org.view"], projects: { all: false, rights: [], selected: [{ project: PROJECT, rights: ["project.view"] }] } },
      ],
    });
    expect(toGrant(fromGrant(selected, g)).orgs?.[0]).toEqual(selected.orgs?.[0]);
  });

  it("name the sensitive rights a change adds, once", () => {
    const g = grantable();
    const before = fromGrant(grantBody(), g);
    expect(gainedSensitive(agents, before, before)).toEqual([]);
    const org = before.orgs[ORG];
    if (!org) throw new Error("no org");
    const after = { ...before, orgs: { ...before.orgs, [ORG]: { ...org, projectRights: [...org.projectRights, "logs.content.view"] } } };
    expect(gainedSensitive(agents, before, after)).toEqual(["logs.content.view"]);
    expect(gainedSensitive(agents, after, before)).toEqual([]);
    expect(gainedSensitive(agents, null, presetDraft(agents, g, "full"))).toContain("org.delete");
  });

  it("sum up what the agent can do in plain words", () => {
    const g = grantable();
    expect(summary(agents, g, presetDraft(agents, g, "observer"))).toBe(
      "Can read settings, members, projects, logs, usage and costs, audit logs, models, and limits in Northwind.",
    );
    expect(summary(agents, g, presetDraft(agents, g, "builder"))).toMatch(/^Can manage projects, keys, and models and read .* in Northwind\.$/);
    const none = { ...presetDraft(agents, g, "builder"), orgs: {} };
    expect(summary(agents, g, none)).toBe("Can read your profile, nothing in your organizations.");
    expect(summary(agents, g, { ...none, future: { on: true, rights: [], projectRights: [] } })).toBe("Acts only in organizations you join later.");
    expect(summary(agents, g, { ...presetDraft(agents, g, "observer"), instance: ["instance.system.view", "instance.settings.manage"] })).toMatch(
      /Also helps administer the instance\.$/,
    );
  });

  it("add a right an agent asks for, with what it needs, where the person holds it", () => {
    const g = grantable();
    const d = fromGrant(grantBody(), g);
    const asked = withRequested(agents, g, d, "logs.export", ORG, PROJECT);
    expect(asked.held).toBe(true);
    expect(asked.draft.orgs[ORG]?.projectRights).toEqual(expect.arrayContaining(["logs.export", "logs.metadata.view", "project.view"]));
    expect(asked.draft.customized).toBe(true);
    const missing = withRequested(agents, g, d, "logs.export", ORG, OTHER_PROJECT);
    expect(missing).toEqual({ draft: d, held: false });
    expect(withRequested(agents, g, d, "account.keys.manage").draft.account).toEqual(["account.view", "account.keys.view", "account.keys.manage"]);
    expect(withRequested(agents, g, d, "instance.system.view").held).toBe(false);
  });

  it("fill what the API leaves out and keep closed or picked projects as they are", () => {
    const g = grantable();
    const sparse = fromGrant({ preset: "custom", orgs: [{ org: ORG, included: true }], future_orgs: {} } as GrantBody, g);
    expect(sparse.preset).toBe("builder");
    expect(sparse.account).toEqual(["account.view"]);
    expect(sparse.orgs[ORG]).toEqual({ included: true, rights: [], all: true, projectRights: [], selected: {} });
    expect(sparse.future).toEqual({ on: true, rights: [], projectRights: [] });
    const closed = {
      ...sparse,
      orgs: { [ORG]: { included: false, rights: ["org.view"], all: false, projectRights: [], selected: { [PROJECT]: ["project.view"] } } },
    };
    expect(toGrant(closed).orgs).toEqual([{ org: ORG, included: false, rights: [], projects: { all: false, rights: [], selected: [] } }]);
  });

  it("count a sensitive right in one project as known when every project had it", () => {
    const g = grantable();
    const every = fromGrant(grantBody(), g);
    const org = every.orgs[ORG];
    if (!org) throw new Error("no org");
    const before = {
      ...every,
      orgs: { [ORG]: { ...org, projectRights: [...org.projectRights, "logs.content.view"] } },
      future: { on: true, rights: [], projectRights: [] },
    };
    const after = { ...before, orgs: { [ORG]: { ...org, all: false, selected: { [PROJECT]: ["project.view", "logs.metadata.view", "logs.content.view"] } } } };
    expect(gainedSensitive(agents, before, after)).toEqual([]);
    const future = { ...after, instance: ["instance.updates.apply"], future: { on: true, rights: ["org.delete"], projectRights: ["logs.export"] } };
    expect(gainedSensitive(agents, after, future).sort()).toEqual(["instance.updates.apply", "logs.export", "org.delete"]);
  });

  it("count many organizations and say when an agent only changes things", () => {
    const base = grantable();
    const org = base.orgs[0];
    if (!org) throw new Error("no org");
    const g = grantable({ orgs: [org, { ...org, id: "org_a", name: "Alpha" }, { ...org, id: "org_b", name: "Beta" }] });
    const d = presetDraft(agents, g, "observer");
    expect(summary(agents, g, d)).toMatch(/ in 3 organizations\.$/);
    const managing = {
      ...d,
      orgs: { [org.id]: { included: true, rights: ["projects.create", "security.2fa.require", "made.up"], all: false, projectRights: [], selected: {} } },
    };
    expect(summary(agents, g, managing)).toBe("Can manage projects and security in Northwind.");
  });

  it("find where a requested right belongs, or say the person lacks it", () => {
    const g = grantable({ instance: ["instance.system.view"] });
    const d = fromGrant(grantBody(), g);
    expect(withRequested(agents, g, d, "instance.system.view").draft.instance).toEqual(["instance.system.view"]);
    expect(withRequested(agents, g, d, "org.update", ORG).draft.orgs[ORG]?.rights).toContain("org.update");
    expect(withRequested(agents, g, d, "org.update", "org_gone").held).toBe(false);
    expect(withRequested(agents, g, d, "limits.manage", ORG).draft.orgs[ORG]?.projectRights).toContain("limits.manage");
    expect(withRequested(agents, g, d, "nothing.real", ORG).held).toBe(false);
    expect(withRequested(agents, g, { ...d, orgs: {} }, "org.update", ORG).draft.orgs[ORG]?.included).toBe(true);
    expect(fromGrant({ preset: "observer" } as GrantBody, g).orgs[ORG]?.included).toBe(false);
    const picked = { ...d, orgs: { [ORG]: { included: false, rights: [], all: false, projectRights: [], selected: {} } } };
    const asked = withRequested(agents, g, picked, "limits.view", ORG, PROJECT).draft.orgs[ORG];
    expect(asked?.included).toBe(true);
    expect(asked?.rights).toEqual(["org.view"]);
    expect(asked?.selected[PROJECT]).toEqual(["project.view", "limits.view"]);
  });
});
