import { describe, expect, it } from "vitest";
import { homePath, isLocale, localeOfPath, localePrefix, localizedPath, pathWithinLocale, switchLocale } from "./locales";

describe("locales", () => {
  it("knows its three languages and nothing else", () => {
    expect(isLocale("en")).toBe(true);
    expect(isLocale("de")).toBe(true);
    expect(isLocale("zh-CN")).toBe(true);
    expect(isLocale("fr")).toBe(false);
    expect(isLocale("zh-cn")).toBe(false);
  });

  it("puts English at the root and every other language under its own folder", () => {
    expect(localePrefix("en")).toBe("");
    expect(localePrefix("de")).toBe("/de");
    expect(homePath("en")).toBe("/");
    expect(homePath("zh-CN")).toBe("/zh-CN");
  });

  it("reads the language of an address, case-insensitively", () => {
    expect(localeOfPath("/")).toBe("en");
    expect(localeOfPath("")).toBe("en");
    expect(localeOfPath("/get-started/quick-start")).toBe("en");
    expect(localeOfPath("/de/get-started")).toBe("de");
    expect(localeOfPath("/zh-cn/api")).toBe("zh-CN");
    expect(localeOfPath("/deutsch")).toBe("en");
  });

  it("finds the page within a language and moves it to another", () => {
    expect(pathWithinLocale("/de/api/errors/")).toBe("/api/errors");
    expect(pathWithinLocale("/de")).toBe("/");
    expect(pathWithinLocale("/api")).toBe("/api");
    expect(localizedPath("/", "de")).toBe("/de");
    expect(localizedPath("/api", "en")).toBe("/api");
    expect(switchLocale("/de/self-hosting/backups", "zh-CN")).toBe("/zh-CN/self-hosting/backups");
    expect(switchLocale("/zh-CN", "en")).toBe("/");
    expect(switchLocale("/console/api-keys", "de")).toBe("/de/console/api-keys");
  });
});
