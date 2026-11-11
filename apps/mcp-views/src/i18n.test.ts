import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import de from "./locales/de.json";
import zhCN from "./locales/zh-CN.json";
import { localeOf, setLocale, t } from "./i18n";

function used(): Map<string, string> {
  const out = new Map<string, string>();
  const dir = import.meta.dirname;
  for (const file of readdirSync(dir).filter((f) => /\.tsx?$/.test(f) && !f.includes(".test."))) {
    const source = readFileSync(join(dir, file), "utf8");
    for (const [, key = "", english = ""] of source.matchAll(/\bt\(\s*"([\w.]+)",\s*"((?:[^"\\]|\\.)*)"/g)) out.set(key, english);
  }
  return out;
}

const placeholders = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

describe("translations", () => {
  const keys = used();

  it.each([
    ["de", de as Record<string, string>],
    ["zh-CN", zhCN as Record<string, string>],
  ])("%s has every message the view writes, with the same placeholders, and nothing else", (_, catalog) => {
    expect(keys.size).toBeGreaterThan(30);
    expect(Object.keys(catalog).sort()).toEqual([...keys.keys()].sort());
    for (const [key, english] of keys) expect(placeholders(catalog[key] ?? ""), key).toEqual(placeholders(english));
  });

  it("follows the host's locale and falls back to English", () => {
    expect(localeOf("de-AT")).toBe("de");
    expect(localeOf("zh-Hans-CN")).toBe("zh-CN");
    expect(localeOf("fr-FR")).toBe("en");
    expect(localeOf(undefined)).toBe("en");
    setLocale("de-DE");
    expect(t("stats.title", "Usage")).toBe("Nutzung");
    expect(t("stats.ofRequests", "{requests} requests", { requests: 3 })).toBe("3 Anfragen");
    setLocale("en-US");
    expect(t("stats.ofRequests", "{requests} requests", { requests: 3 })).toBe("3 requests");
    expect(t("stats.ofRequests", "{requests} requests")).toBe("{requests} requests");
  });
});
