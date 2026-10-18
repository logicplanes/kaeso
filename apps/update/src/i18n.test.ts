import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { CATALOGS, locale, localeOf, setLocale, t } from "./i18n";

const src = resolve("src");
const sources = readdirSync(src)
  .filter((f) => /\.tsx?$/.test(f) && !f.includes(".test."))
  .map((f) => readFileSync(`${src}/${f}`, "utf8"))
  .join("\n");
const used = new Map([...sources.matchAll(/\bt\(\s*"([\w.]+)",\s*"([^"]*)"/g)].map((m) => [m[1] ?? "", m[2] ?? ""]));

const SAME = new Set(["de release.name", "zh-CN release.name", "de page.heading"]);

afterEach(() => setLocale("en"));

describe("the interface's words", () => {
  it("are all translated into German and Chinese, with no leftovers", () => {
    expect(used.size).toBeGreaterThan(10);
    for (const tag of ["de", "zh-CN"] as const) {
      expect(Object.keys(CATALOGS[tag]).sort()).toEqual([...used.keys()].sort());
      for (const [key, english] of used) {
        const translated = CATALOGS[tag][key] ?? "";
        expect(translated.match(/\{\w+\}/g) ?? [], `${tag} ${key}`).toEqual(english.match(/\{\w+\}/g) ?? []);
        if (!SAME.has(`${tag} ${key}`)) expect(translated, `${tag} ${key}`).not.toBe(english);
      }
    }
  });

  it("address the reader formally", () => {
    const german = Object.values(CATALOGS.de).join(" ");
    expect(german).toContain("Sie");
    expect(german).not.toMatch(/\b(du|dein|deine|dich|dir)\b/i);
    const chinese = Object.values(CATALOGS["zh-CN"]).join("");
    expect(chinese).toContain("您");
    expect(chinese).not.toContain("你");
  });

  it("follow the browser's language and fill in values", () => {
    expect(localeOf(["de-AT", "en"])).toBe("de");
    expect(localeOf(["zh-Hans-CN"])).toBe("zh-CN");
    expect(localeOf(["fr-FR"])).toBe("en");
    expect(t("release.name", "Kaeso {version}", { version: "3.1.0" })).toBe("Kaeso 3.1.0");
    setLocale("zh-CN");
    expect(locale()).toBe("zh-CN");
    expect(t("error.retry", "Try again")).toBe("重试");
    expect(t("missing.key", "Stays English")).toBe("Stays English");
  });
});
