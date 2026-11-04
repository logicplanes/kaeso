import { describe, expect, it } from "vitest";
import { ORG, OTHER_PROJECT, PROJECT, myMembership, myProject } from "~/test/fixtures";
import { writeLast } from "./last";
import { homeOrg } from "./place";

const globex = myMembership({ org_id: "org_globex", org_name: "Globex", projects: [myProject({ id: "proj_globex" })] });

describe("the place to start", () => {
  it("opens the organization of the last project, not the project", () => {
    writeLast({ org: ORG, project: OTHER_PROJECT });
    expect(homeOrg([globex, myMembership()])).toBe(ORG);
  });

  it("opens the last organization", () => {
    writeLast({ org: ORG });
    expect(homeOrg([globex, myMembership()])).toBe(ORG);
  });

  it("starts in the first organization when the last one was left", () => {
    writeLast({ org: "org_left", project: PROJECT });
    expect(homeOrg([globex, myMembership()])).toBe("org_globex");
  });

  it("opens an organization without projects", () => {
    expect(homeOrg([myMembership({ projects: [] })])).toBe(ORG);
  });

  it("has nowhere to start without memberships", () => {
    window.localStorage.setItem("kaeso:last", JSON.stringify({ org: "" }));
    expect(homeOrg([])).toBeNull();
  });
});
