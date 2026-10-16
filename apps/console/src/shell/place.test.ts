import { describe, expect, it } from "vitest";
import { ORG, OTHER_PROJECT, PROJECT, myMembership, myProject } from "~/test/fixtures";
import { writeLast } from "./last";
import { defaultProjectOf, homePlace } from "./place";

const globex = myMembership({ org_id: "org_globex", org_name: "Globex", projects: [myProject({ id: "proj_globex", is_default: false })] });

describe("the place to start", () => {
  it("returns to the last project the person worked in", () => {
    writeLast({ org: ORG, project: OTHER_PROJECT });
    expect(homePlace([globex, myMembership()])).toEqual({ org: ORG, project: OTHER_PROJECT });
  });

  it("falls back to the organization when the last project is gone", () => {
    writeLast({ org: ORG, project: "proj_gone" });
    expect(homePlace([myMembership()])).toEqual({ org: ORG });
  });

  it("keeps an organization-only last place", () => {
    writeLast({ org: ORG });
    expect(homePlace([globex, myMembership()])).toEqual({ org: ORG });
  });

  it("starts in the first organization when the last one was left", () => {
    writeLast({ org: "org_left", project: PROJECT });
    expect(homePlace([globex, myMembership()])).toEqual({ org: "org_globex" });
  });

  it("has nowhere to start without memberships", () => {
    window.localStorage.setItem("kaeso:last", JSON.stringify({ org: "" }));
    expect(homePlace([])).toBeNull();
  });
});

describe("the default project", () => {
  it("prefers the marked default, then the first project", () => {
    expect(defaultProjectOf([myMembership()], ORG)).toBe(PROJECT);
    expect(defaultProjectOf([globex], "org_globex")).toBe("proj_globex");
    expect(defaultProjectOf([myMembership({ projects: [] })], ORG)).toBeNull();
    expect(defaultProjectOf([], ORG)).toBeNull();
  });
});
