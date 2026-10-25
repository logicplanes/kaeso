import { describe, expect, it } from "vitest";
import type { GrantBody } from "~/api/generated/types";
import { ORG, OTHER_PROJECT, PROJECT } from "~/test/fixtures";
import { agents, grantBody, grantable } from "~/test/fixtures/agents";
import { fromGrant, gainedSensitive, presetDraft, summary, summaryLines, toGrant, withRequested } from "./agent-grant";

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
    expect(summary(agents, g, presetDraft(agents, g, "observer"))).toBe("Reads settings, members, and 6 more in Northwind.");
    expect(summary(agents, g, presetDraft(agents, g, "builder"))).toBe("Manages projects, keys, and models and reads the rest in Northwind.");
    const none = { ...presetDraft(agents, g, "builder"), orgs: {}, future: { on: false, rights: [], projectRights: [] } };
    expect(summary(agents, g, none)).toBe("Reads your profile, nothing in your organizations.");
    expect(summary(agents, g, { ...none, future: { on: true, rights: [], projectRights: [] } })).toBe("Acts only in organizations you join later.");
    expect(summary(agents, g, { ...presetDraft(agents, g, "observer"), instance: ["instance.system.view", "instance.settings.manage"] })).toMatch(
      / Also helps run the instance\.$/,
    );
  });

  it("list what the agent can do, one short line each", () => {
    const g = grantable();
    expect(presetDraft(agents, g, "builder").future.on).toBe(true);
    expect(summaryLines(agents, g, presetDraft(agents, g, "observer"))).toEqual([
      "Reads all of Northwind except providers, security, and keys",
      "Also in organizations you join later",
    ]);
    expect(summaryLines(agents, g, presetDraft(agents, g, "builder"))).toEqual([
      "Manages projects, keys, and models in Northwind",
      "Reads the rest of Northwind",
      "Also in organizations you join later",
    ]);
    expect(summaryLines(agents, g, presetDraft(agents, g, "operator"))[0]).toMatch(/^Manages all of Northwind except /);
    expect(summaryLines(agents, g, presetDraft(agents, g, "full"))[0]).toBe("Manages all of Northwind");
    const only = (rights: string[]) => ({
      ...presetDraft(agents, g, "builder"),
      future: { on: false, rights: [], projectRights: [] },
      orgs: { [ORG]: { included: true, rights, all: true, projectRights: [], selected: {} } },
    });
    expect(summaryLines(agents, g, only(["org.view", "members.view"]))).toEqual(["Reads settings and members in Northwind"]);
    expect(summaryLines(agents, g, only(["projects.create"]))).toEqual(["Manages projects in Northwind"]);
    const views = (scope: "org" | "project") => agents[scope].categories.flatMap((c) => c.rights.map((r) => r.id)).filter((id) => /\.view(\.|$)/.test(id));
    expect(
      summaryLines(agents, g, {
        ...only(views("org")),
        orgs: { [ORG]: { included: true, rights: views("org"), all: true, projectRights: views("project"), selected: {} } },
      }),
    ).toEqual(["Reads all of Northwind except security"]);
    const none = { ...only([]), orgs: {} };
    expect(summaryLines(agents, g, none)).toEqual(["Reads your profile, nothing else"]);
    expect(summaryLines(agents, g, { ...none, instance: ["instance.system.view"] })).toEqual(["Reads the instance"]);
    expect(summaryLines(agents, g, { ...none, instance: ["instance.settings.manage"] })).toEqual(["Helps run the instance"]);
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

  it("show an organization the agent reaches as one joined later as open, and keep it open", () => {
    const g = grantable();
    const body = grantBody({
      orgs: [{ org: "org_contoso", included: false, rights: [], projects: { all: true, rights: [], selected: [] } }],
      future_orgs: { rights: ["org.view", "org.update"], project_rights: ["project.view", "made.up"] },
    });
    const d = fromGrant(body, g);
    expect(d.orgs[ORG]).toEqual({ included: true, rights: ["org.view", "org.update"], all: true, projectRights: ["project.view"], selected: {} });
    expect(d.orgs.org_contoso?.included).toBe(false);
    expect(toGrant(d).orgs?.find((o) => o.org === ORG)?.included).toBe(true);
    const refusing = g.orgs.find((o) => o.id === "org_contoso");
    if (!refusing) throw new Error("no org");
    expect(fromGrant({ ...body, orgs: [] }, grantable({ orgs: [{ ...refusing, id: ORG }] })).orgs[ORG]?.included).toBe(false);
  });

  it("open only the project a right was asked for in a closed organization", () => {
    const g = grantable();
    const d = fromGrant(grantBody({ orgs: [{ org: ORG, included: false, rights: [], projects: { all: true, rights: [], selected: [] } }] }), g);
    const asked = withRequested(agents, g, d, "limits.view", ORG, PROJECT).draft.orgs[ORG];
    expect(asked).toEqual({ included: true, rights: ["org.view"], all: false, projectRights: [], selected: { [PROJECT]: ["project.view", "limits.view"] } });
    const everywhere = withRequested(agents, g, d, "limits.view", ORG).draft.orgs[ORG];
    expect(everywhere?.all).toBe(true);
    expect(everywhere?.projectRights).toEqual(["project.view", "limits.view"]);
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
    expect(summary(agents, g, managing)).toBe("Manages projects and security in Northwind.");
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
