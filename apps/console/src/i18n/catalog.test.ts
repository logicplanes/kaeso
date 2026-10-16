import { describe, expect, it } from "vitest";
import { check, placeholders } from "../../scripts/i18n.mjs";
import { formatMessage } from "@logicplanes/kaeso-ui/i18n";
import { resolve, supported } from "./index";

describe("the console's catalogs", () => {
  it("have every key of the source in German and Chinese, with the same placeholders", () => {
    expect(check().problems).toEqual([]);
  });

  it("read placeholders through plural and select branches", () => {
    expect(placeholders("{count, plural, one {# key in {project}} other {# keys}}")).toBe("count:plural project");
    expect(placeholders("{kind, select, org {Organisation} other {Projekt}}")).toBe("kind:select");
  });
});

describe("the console's language", () => {
  it("follows the browser's first language it has, else English", () => {
    expect(resolve("auto", ["fr-FR", "de-AT", "en"])).toBe("de");
    expect(resolve("auto", ["zh-Hant-TW"])).toBe("zh-CN");
    expect(resolve("auto", ["fr", "es"])).toBe("en");
    expect(resolve("zh-CN", ["de"])).toBe("zh-CN");
    expect(supported("en-GB")).toBe("en");
  });

  it("writes plurals in each language's way", () => {
    const de = "{count, plural, one {# Schlüssel} other {# Schlüssel}} in {project}";
    expect(formatMessage(de, { count: 1200, project: "Atlas" }, "de")).toBe("1.200 Schlüssel in Atlas");
  });
});
