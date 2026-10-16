import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ORG, PROJECT } from "~/test/fixtures";
import { memberRights } from "~/test/fixtures/org";
import { memberUser } from "~/test/fixtures/project";
import { apiError, on, page, requests } from "~/test/network";
import { renderUi } from "~/test/render";
import { queryClient } from "~/query";
import { grantorsQuery, memberRightsQuery, projectGrantorsQuery } from "./rights";

describe("who can give a right", () => {
  it("lists the organization's and the project's grantors", async () => {
    on("GET", "orgs/:org/rights/:right/grantors", page([memberUser({ name: "Lin Wei" })]));
    on("GET", "orgs/:org/projects/:project/rights/:right/grantors", page([memberUser({ name: "Grace Hopper" })]));
    expect((await queryClient.fetchQuery(grantorsQuery(ORG, "logs.export")))[0]?.name).toBe("Lin Wei");
    expect((await queryClient.fetchQuery(projectGrantorsQuery(ORG, PROJECT, "logs.export")))[0]?.name).toBe("Grace Hopper");
    expect(requests().map((r) => r.path)).toEqual([
      `/platform/v1/orgs/${ORG}/rights/logs.export/grantors`,
      `/platform/v1/orgs/${ORG}/projects/${PROJECT}/rights/logs.export/grantors`,
    ]);
  });

  it("stays quiet when the grantors can't load", async () => {
    on("GET", "orgs/:org/rights/:right/grantors", apiError(400, "invalid_request", "Grantors are hidden here."));
    await renderUi(<p>page</p>);
    await expect(queryClient.fetchQuery(grantorsQuery(ORG, "logs.export"))).rejects.toThrow("Grantors are hidden here.");
    expect(within(screen.getByRole("region", { name: "Notifications" })).queryByText("Grantors are hidden here.")).not.toBeInTheDocument();
    expect(requests("GET", "orgs/:org/rights/:right/grantors")).toHaveLength(1);
  });
});

describe("a member's rights", () => {
  it("reads them from the member's rights endpoint", async () => {
    on("GET", "orgs/:org/members/:user/rights", memberRights({ user_id: "usr_grace" }));
    expect((await queryClient.fetchQuery(memberRightsQuery(ORG, "usr_grace"))).user_id).toBe("usr_grace");
  });
});
