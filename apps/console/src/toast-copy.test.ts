import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const root = join(__dirname, "..");
const TOAST = /toast(\.(?:success|error|warning))?\(\s*t\(\s*"([^"]+)",\s*"((?:[^"\\]|\\.)*)"/g;

const catalog = (locale: string): Record<string, string> =>
  Object.assign(
    {},
    ...readdirSync(join(root, "src/i18n/locales", locale)).map((f) => JSON.parse(readFileSync(join(root, "src/i18n/locales", locale, f), "utf8"))),
  );

const toasts = readdirSync(join(root, "src"), { recursive: true })
  .map(String)
  .filter((f) => /\.tsx?$/.test(f) && !/\.test\.tsx?$/.test(f) && !f.startsWith("api/generated"))
  .flatMap((file) =>
    [...readFileSync(join(root, "src", file), "utf8").matchAll(TOAST)].map((m) => ({ file, error: m[1] === ".error", key: m[2] ?? "", english: m[3] ?? "" })),
  );

describe("toasts", () => {
  const de = catalog("de");
  const zh = catalog("zh-CN");

  it("are found in the sources", () => {
    expect(toasts.length).toBeGreaterThan(50);
  });

  it.each(toasts.filter((x) => !/^\{\w+\}: \{message\}$/.test(x.english)))("$key ends with a period only when it is an error", ({ error, key, english }) => {
    expect(english.endsWith("."), english).toBe(error);
    expect((de[key] ?? "").endsWith("."), de[key]).toBe(error);
    expect((zh[key] ?? "").endsWith("。"), zh[key]).toBe(error);
  });
});
