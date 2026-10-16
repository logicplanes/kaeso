import { describe, expect, it } from "vitest";
import { queryClient } from "~/query";
import { ORG, PROJECT } from "~/test/fixtures";
import { access } from "~/test/fixtures/project";
import { on, requests } from "~/test/network";
import { modelAccessQuery } from "./members";

describe("the models a principal can use", () => {
  it("asks the endpoint of a member, a service account or a key", async () => {
    on("GET", "orgs/:org/projects/:project/members/:user/models", access({ all: false }));
    on("GET", "orgs/:org/projects/:project/service-accounts/:account/models", access());
    on("GET", "orgs/:org/projects/:project/keys/:key/models", access());
    expect((await queryClient.fetchQuery(modelAccessQuery(ORG, PROJECT, { kind: "user", id: "usr_grace" }))).all).toBe(false);
    await queryClient.fetchQuery(modelAccessQuery(ORG, PROJECT, { kind: "service_account", id: "sa_deploy" }));
    await queryClient.fetchQuery(modelAccessQuery(ORG, PROJECT, { kind: "key", id: "key_web" }));
    expect(requests().map((r) => r.path)).toEqual([
      `/platform/v1/orgs/${ORG}/projects/${PROJECT}/members/usr_grace/models`,
      `/platform/v1/orgs/${ORG}/projects/${PROJECT}/service-accounts/sa_deploy/models`,
      `/platform/v1/orgs/${ORG}/projects/${PROJECT}/keys/key_web/models`,
    ]);
  });
});
