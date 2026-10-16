import { afterEach, describe, expect, it, vi } from "vitest";
import { LOCALE_STORAGE_KEY } from "./locales";
import { followPreferredLocale, preferredLocale, preferredPath, rememberLocale, storedLocale } from "./preference";

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

const fakeWindow = (pathname: string, languages: string[]) => {
  const replaceState = vi.fn();
  return { win: { location: { pathname }, navigator: { languages }, history: { state: { key: "k" }, replaceState } } as unknown as Window, replaceState };
};

describe("the language preference", () => {
  it("remembers a choice and ignores anything that isn't a language", () => {
    expect(storedLocale()).toBeUndefined();
    rememberLocale("de");
    expect(localStorage.getItem(LOCALE_STORAGE_KEY)).toBe("de");
    expect(storedLocale()).toBe("de");
    localStorage.setItem(LOCALE_STORAGE_KEY, "klingon");
    expect(storedLocale()).toBeUndefined();
  });

  it("prefers the remembered choice over the browser's languages", () => {
    expect(preferredLocale(["zh-TW"])).toBe("zh-CN");
    rememberLocale("en");
    expect(preferredLocale(["zh-TW"])).toBe("en");
  });

  it("keeps working when storage is unavailable", () => {
    const blocked = () => {
      throw new Error("blocked");
    };
    vi.stubGlobal("localStorage", { getItem: blocked, setItem: blocked });
    expect(() => rememberLocale("de")).not.toThrow();
    expect(storedLocale()).toBeUndefined();
    expect(preferredLocale(["de"])).toBe("de");
  });

  it("moves an English address to the preferred language, and leaves others alone", () => {
    expect(preferredPath("/api/errors/", "de")).toBe("/de/api/errors");
    expect(preferredPath("/", "zh-CN")).toBe("/zh-CN");
    expect(preferredPath("/api", "en")).toBeNull();
    expect(preferredPath("/de/api", "zh-CN")).toBeNull();
  });

  it("rewrites the first address without a navigation", () => {
    const german = fakeWindow("/self-hosting", ["de-DE"]);
    followPreferredLocale(german.win);
    expect(german.replaceState).toHaveBeenCalledWith({ key: "k" }, "", "/de/self-hosting");
    const english = fakeWindow("/self-hosting", ["en-US"]);
    followPreferredLocale(english.win);
    expect(english.replaceState).not.toHaveBeenCalled();
  });
});
