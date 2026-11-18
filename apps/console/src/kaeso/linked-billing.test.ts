import { describe, expect, it } from "vitest";
import { rememberLink, takeLink } from "./linked-billing";

describe("a link waiting for its callback", () => {
  it("is kept for the tab and taken once", () => {
    expect(rememberLink({ org: "org_1", attempt: "att_1", credential: "cred_1" })).toBe(true);
    expect(takeLink()).toEqual({ org: "org_1", attempt: "att_1", credential: "cred_1" });
    expect(takeLink()).toBeNull();
  });

  it("forgets what isn't a link", () => {
    for (const stored of ["{", "null", '{"org":1,"attempt":"a"}', '{"org":"o"}', '{"org":"o","attempt":"a","credential":2}']) {
      window.sessionStorage.setItem("kaeso.linked-billing", stored);
      expect(takeLink()).toBeNull();
    }
  });
});
