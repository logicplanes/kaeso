import { describe, expect, it } from "vitest";
import { httpsHref } from "./links";

describe("httpsHref", () => {
  it("keeps https links", () => {
    expect(httpsHref(" https://github.com/logicplanes/kaeso/releases/tag/v1.2.3 ")).toBe("https://github.com/logicplanes/kaeso/releases/tag/v1.2.3");
  });
  it("drops every other scheme, credentials and what isn't a URL", () => {
    for (const bad of ["http://github.com/x", "javascript:alert(1)", "data:text/html,x", "//github.com/x", "https://user:pass@github.com/x", "", "releases"]) {
      expect(httpsHref(bad)).toBeNull();
    }
  });
});
