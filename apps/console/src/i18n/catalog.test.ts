import { describe, expect, it } from "vitest";
import { check, placeholders, styleProblems } from "../../scripts/i18n.mjs";
import { formatMessage } from "@logicplanes/kaeso-ui/i18n";
import { resolve } from "./index";

describe("the console's catalogs", () => {
  it("have every key of the source in German and Chinese, with the same placeholders", () => {
    expect(check().problems).toEqual([]);
  });

  it("refuse English left in a translation, informal German, 你 and ASCII punctuation in Chinese", () => {
    expect(styleProblems("de", "keys.create", "Create key", "Create key")).toEqual(["is still the English"]);
    expect(styleProblems("zh-CN", "common.cancel", "Cancel", "Cancel")).toEqual(["is still the English"]);
    expect(styleProblems("de", "common.status", "Status", "Status")).toEqual([]);
    expect(styleProblems("zh-CN", "kaeso.audit.ip", "IP", "IP")).toEqual([]);
    expect(styleProblems("de", "org.invite.failed", "{email}: {message}", "{email}: {message}")).toEqual([]);
    expect(styleProblems("zh-CN", "org.sso.domainExample", "example.com", "example.com")).toEqual([]);
    expect(styleProblems("de", "logs.param.topK", "Top k", "Top k")).toEqual([]);
    expect(styleProblems("de", "a.b", "Ask {name}", "Fragen Sie {name}")).toEqual([]);
    expect(styleProblems("de", "a.b", "Your keys", "Deine Schlüssel")).toEqual(["says „du“; address the reader with „Sie“"]);
    expect(styleProblems("de", "a.b", "Use `du -sh`", "`du -sh` verwenden")).toEqual([]);
    expect(styleProblems("de", "a.b", 'Type "delete"', 'Zur Bestätigung "löschen" eingeben')).toEqual(["uses straight quotes; write „…“"]);
    expect(styleProblems("zh-CN", "a.b", "Your keys", "你的密钥")).toEqual(["says 你; address the reader with 您"]);
    expect(styleProblems("zh-CN", "a.b", "Saved, done", "已保存,完成")).toEqual(["uses ASCII punctuation in Chinese text; use full-width punctuation"]);
    expect(styleProblems("zh-CN", "a.b", "Send `a, b`", "请发送 `a, b`。")).toEqual([]);
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
    expect(resolve("auto", ["en-GB", "de"])).toBe("en");
  });

  it("writes plurals in each language's way", () => {
    const de = "{count, plural, one {# Schlüssel} other {# Schlüssel}} in {project}";
    expect(formatMessage(de, { count: 1200, project: "Atlas" }, "de")).toBe("1.200 Schlüssel in Atlas");
  });
});
