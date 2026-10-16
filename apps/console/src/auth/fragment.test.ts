import { describe, expect, it } from "vitest";
import { takeFragment } from "./fragment";

describe("a token in the address fragment", () => {
  it("is taken out of the address and kept for the page", () => {
    window.history.replaceState(null, "", "/auth/magic-link?next=%2Fkeys#token=mgl_1");
    expect(takeFragment("token")).toBe("mgl_1");
    expect(window.location.hash).toBe("");
    expect(window.location.search).toBe("?next=%2Fkeys");
    expect(takeFragment("token")).toBe("mgl_1");
  });

  it("is missing on a page opened without one", () => {
    window.history.replaceState(null, "", "/auth/email-confirm");
    expect(takeFragment("token")).toBeNull();
  });
});
