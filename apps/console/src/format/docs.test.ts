import { afterEach, describe, expect, it } from "vitest";
import { apply } from "~/i18n";
import { docsPage } from "./docs";

afterEach(() => apply({ locale: "en", messages: {}, ui: {}, provider: undefined }));

describe("docsPage", () => {
  it("links to the documentation in the console's language", () => {
    expect(docsPage("/console/provider-keys")).toBe("https://docs.kaeso.app/console/provider-keys");
    apply({ locale: "de", messages: {}, ui: {}, provider: undefined });
    expect(docsPage("/console/provider-keys")).toBe("https://docs.kaeso.app/de/console/provider-keys");
    apply({ locale: "zh-CN", messages: {}, ui: {}, provider: undefined });
    expect(docsPage("/console/provider-keys")).toBe("https://docs.kaeso.app/zh-CN/console/provider-keys");
  });
});
