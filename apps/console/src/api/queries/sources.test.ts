import { describe, expect, it } from "vitest";
import { queryClient } from "~/query";
import { ORG, PROJECT } from "~/test/fixtures";
import { on, page, requests } from "~/test/network";
import { sourcesQuery } from "./sources";

describe("a project's sources", () => {
  it("reads every page of them", async () => {
    on("GET", "orgs/:org/projects/:project/sources", ({ query }: { query: URLSearchParams }) =>
      query.get("after") ? page([{ id: "src_2" }]) : page([{ id: "src_1" }], "cur_2"),
    );
    const sources = await queryClient.fetchQuery(sourcesQuery(ORG, PROJECT));
    expect(sources.map((s) => s.id)).toEqual(["src_1", "src_2"]);
    expect(requests("GET", "orgs/:org/projects/:project/sources").map((r) => r.query.get("after"))).toEqual([null, "cur_2"]);
  });
});
