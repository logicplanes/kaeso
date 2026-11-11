import { describe, expect, it } from "vitest";
import { askOf, outcomeOf, safeUrl } from "./data";
import { ok, refused, stats } from "./test/fixtures";

describe("results", () => {
  it("reads the stats a successful call carries", () => {
    expect(outcomeOf(ok(stats()))).toEqual({ stats: stats() });
  });

  it("turns a missing right into its approve link", () => {
    expect(outcomeOf(refused)).toEqual({
      failure: {
        kind: "rights",
        message: "This agent doesn't have `usage.org.view`; its user can grant it in the console.",
        approveUrl: "https://kaeso.example.com/settings/account/agents/agt_1?add=usage.org.view",
      },
    });
  });

  it("keeps the message of any other failure and of a result it can't draw", () => {
    expect(outcomeOf({ isError: true, content: [{ type: "text", text: "Invalid arguments." }] })).toEqual({
      failure: { kind: "error", message: "Invalid arguments." },
    });
    expect(outcomeOf({ content: [{ type: "text", text: "Done." }], structuredContent: { metric: "money" } })).toEqual({
      failure: { kind: "error", message: "Done." },
    });
  });

  it("opens only web links", () => {
    expect(safeUrl("https://kaeso.example.com/orgs")).toBe("https://kaeso.example.com/orgs");
    expect(safeUrl("javascript:alert(1)")).toBeNull();
    expect(safeUrl("data:text/html,x")).toBeNull();
    expect(safeUrl(42)).toBeNull();
    expect(safeUrl("not a url")).toBeNull();
    expect(outcomeOf({ ...refused, structuredContent: { error: { type: "insufficient_rights", message: "x", approve_url: "javascript:x" } } })).toEqual({
      failure: { kind: "rights", message: "x", approveUrl: null },
    });
  });

  it("takes only the inputs it knows from the tool's arguments", () => {
    expect(askOf({ metric: "latency", range: "24h", group_by: "key", org: "org_1" })).toEqual({ metric: "latency", range: "24h", group_by: "key" });
    expect(askOf({ metric: "money" })).toEqual({ metric: undefined, range: undefined, group_by: undefined });
    expect(askOf(undefined)).toEqual({ metric: undefined, range: undefined, group_by: undefined });
  });
});
