import { describe, expect, it } from "vitest";
import { Slugger, humanize, pathSegment, slugify } from "./slug.ts";

describe("slugify", () => {
  it("makes GitHub's anchors", () => {
    expect(slugify("Create a key")).toBe("create-a-key");
    expect(slugify("Keys & limits")).toBe("keys--limits");
    expect(slugify("What's `max_tokens`?")).toBe("whats-max_tokens");
    expect(slugify("  Rate limits (per key)  ")).toBe("rate-limits-per-key");
    expect(slugify("Über Modelle")).toBe("über-modelle");
    expect(slugify("v1.2.0 — notes")).toBe("v120--notes");
  });
});

describe("Slugger", () => {
  it("numbers repeated headings as GitHub does", () => {
    const s = new Slugger();
    expect([s.slug("Limits"), s.slug("Limits"), s.slug("Limits 1"), s.slug("Limits")]).toEqual(["limits", "limits-1", "limits-1-1", "limits-2"]);
  });

  it("gives a heading without letters an anchor", () => {
    const s = new Slugger();
    expect(s.slug("???")).toBe("section");
    expect(s.slug("!!!")).toBe("section-1");
  });
});

describe("pathSegment and humanize", () => {
  it("turns file names into addresses", () => {
    expect(pathSegment("Provider Keys.md")).toBe("provider-keys");
    expect(pathSegment("rate_limits.md")).toBe("rate-limits");
    expect(pathSegment("index.MD")).toBe("index");
    expect(pathSegment("--weird--name--.md")).toBe("weird-name");
  });

  it("turns folder names into labels", () => {
    expect(humanize("provider-keys")).toBe("Provider keys");
    expect(humanize("platform_api")).toBe("Platform api");
  });
});
