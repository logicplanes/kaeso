import { describe, expect, it } from "vitest";
import { queryClient } from "~/query";
import { ORG, project } from "~/test/fixtures";
import { member } from "~/test/fixtures/instance-account";
import { on, page, requests } from "~/test/network";
import { instanceOrgMembersQuery, instanceOrgProjectsQuery } from "./instance";

describe("an organization seen by an instance admin", () => {
  it("lists every member and project, past the first page", async () => {
    on("GET", "instance/orgs/:org/members", ({ query }: { query: URLSearchParams }) =>
      query.get("after") ? page([member({ id: "usr_late", name: "Late member" })]) : page([member()], "cur_2"),
    );
    on("GET", "instance/orgs/:org/projects", ({ query }: { query: URLSearchParams }) =>
      query.get("after") ? page([project({ id: "proj_late", name: "Late project" })]) : page([project()], "cur_2"),
    );
    const members = await queryClient.fetchQuery(instanceOrgMembersQuery(ORG));
    const projects = await queryClient.fetchQuery(instanceOrgProjectsQuery(ORG));
    expect(members.map((m) => m.user.name)).toEqual(["Grace Hopper", "Late member"]);
    expect(projects.map((p) => p.name)).toEqual(["Checkout assistant", "Late project"]);
    expect(requests("GET", "instance/orgs/:org/projects").map((r) => r.query.get("after"))).toEqual([null, "cur_2"]);
  });
});
