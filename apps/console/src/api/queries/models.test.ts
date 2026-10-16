import { describe, expect, it } from "vitest";
import { queryClient } from "~/query";
import { ORG, PROJECT } from "~/test/fixtures";
import { preview, searchHit } from "~/test/fixtures/project";
import { apiError, on, page, requests } from "~/test/network";
import { modelPreviewQuery, modelSearchQuery } from "./models";

describe("searching models.dev", () => {
  it("sends the words, provider and deprecated choice, and leaves out empty words", async () => {
    on("GET", "orgs/:org/projects/:project/models/search", page([searchHit()]));
    expect(await queryClient.fetchQuery(modelSearchQuery(ORG, PROJECT, { q: "sonnet", provider: "anthropic", include_deprecated: true }))).toHaveLength(1);
    await queryClient.fetchQuery(modelSearchQuery(ORG, PROJECT, { q: "", provider: "openai", include_deprecated: false }));
    expect(requests("GET", "orgs/:org/projects/:project/models/search").map((r) => Object.fromEntries(r.query))).toEqual([
      { q: "sonnet", provider: "anthropic", include_deprecated: "true", limit: "50" },
      { provider: "openai", include_deprecated: "false", limit: "50" },
    ]);
  });
});

describe("previewing a model", () => {
  it("has no preview for a model models.dev doesn't list", async () => {
    on("GET", "orgs/:org/projects/:project/models/preview", apiError(404, "not_found", "models.dev doesn't list it."));
    expect(await queryClient.fetchQuery(modelPreviewQuery(ORG, PROJECT, "openai", "gpt-unknown"))).toBeNull();
  });

  it("returns the preview, and fails with the API's message otherwise", async () => {
    on("GET", "orgs/:org/projects/:project/models/preview", preview());
    expect(await queryClient.fetchQuery(modelPreviewQuery(ORG, PROJECT, "anthropic", "claude-sonnet-4-5"))).toEqual(preview());
    on("GET", "orgs/:org/projects/:project/models/preview", apiError(400, "invalid_request", "Name a provider."));
    await expect(queryClient.fetchQuery(modelPreviewQuery(ORG, PROJECT, "", "x"))).rejects.toThrow("Name a provider.");
  });
});
