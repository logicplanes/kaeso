import { describe, expect, it } from "vitest";
import { copyName } from "./route-editor-page";

describe("copyName", () => {
  it("adds -copy, then counts up past the names taken, within 200 characters", () => {
    expect(copyName("fast", new Set())).toBe("fast-copy");
    expect(copyName("fast", new Set(["fast-copy", "fast-copy-2"]))).toBe("fast-copy-3");
    const long = "a".repeat(200);
    expect(copyName(long, new Set())).toHaveLength(200);
    expect(copyName(long, new Set())).toMatch(/a-copy$/);
    expect(copyName(long, new Set([`${"a".repeat(195)}-copy`]))).toBe(`${"a".repeat(193)}-copy-2`);
  });
});
