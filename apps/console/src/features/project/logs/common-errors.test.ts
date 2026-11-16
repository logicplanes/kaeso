import { describe, expect, it } from "vitest";
import { errorGroup } from "~/test/fixtures/project-logs";
import { filtersOf, messagePrefix } from "./common-errors";

describe("the most common errors", () => {
  it("keeps a message up to its first number or id", () => {
    expect(messagePrefix("Rate limit reached for gpt-5.4-mini; retry after 12 s.")).toBe("Rate limit reached for gpt");
    expect(messagePrefix("Quota used up (req_01abc).")).toBe("Quota used up");
    expect(messagePrefix("The provider failed.")).toBe("The provider failed");
    expect(messagePrefix("429 Too many requests")).toBeUndefined();
    expect(messagePrefix("x".repeat(300))).toHaveLength(200);
  });

  it("filters to what a row counts, leaving out what it doesn't know", () => {
    expect(filtersOf(errorGroup())).toEqual({ error: "rate_limited", status: 429, provider: "openai", error_message: "Rate limit reached for gpt" });
    expect(filtersOf(errorGroup({ error_kind: "", provider: "", example: "" }))).toEqual({
      error: undefined,
      status: 429,
      provider: undefined,
      error_message: undefined,
    });
  });
});
