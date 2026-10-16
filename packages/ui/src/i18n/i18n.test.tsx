import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, sep } from "node:path";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { formatMessage } from "./message.js";
import { LocaleProvider, loadUiMessages, matchUiLocale, preferredUiLocale, resolveUiLocale, useUiText } from "./index.js";
import { formatDate, formatNumber } from "../format/index.js";
import en from "./locales/en.js";
import de from "./locales/de.js";
import zhCN from "./locales/zh-CN.js";

describe("formatMessage", () => {
  it("fills values and leaves unknown placeholders visible", () => {
    expect(formatMessage("Hello {name}", { name: "Ada" }, "en")).toBe("Hello Ada");
    expect(formatMessage("Hello {name}", {}, "en")).toBe("Hello {name}");
    expect(formatMessage("{count} keys", { count: 1234 }, "de")).toBe("1.234 keys");
  });

  it("chooses plural branches by the locale's rules, with exact matches first", () => {
    const m = "{count, plural, =0 {No keys} one {# key} other {# keys}}";
    expect(formatMessage(m, { count: 0 }, "en")).toBe("No keys");
    expect(formatMessage(m, { count: 1 }, "en")).toBe("1 key");
    expect(formatMessage(m, { count: 2500 }, "en")).toBe("2,500 keys");
    expect(formatMessage("{count, plural, other {# 个密钥}}", { count: 1 }, "zh-CN")).toBe("1 个密钥");
  });

  it("selects branches and nests placeholders inside them", () => {
    const m = "{role, select, owner {{name} owns it} other {{name} is a member}}";
    expect(formatMessage(m, { role: "owner", name: "Ada" }, "en")).toBe("Ada owns it");
    expect(formatMessage(m, { role: "viewer", name: "Lin" }, "en")).toBe("Lin is a member");
  });
});

describe("LocaleProvider", () => {
  function Close() {
    const t = useUiText();
    return <span>{t("test.close", "Close")}</span>;
  }

  it("shows the English beside a key without messages, and the translation with them", () => {
    const { rerender } = render(
      <LocaleProvider locale="en">
        <Close />
      </LocaleProvider>,
    );
    expect(screen.getByText("Close")).toBeInTheDocument();
    rerender(
      <LocaleProvider locale="de" messages={{ "test.close": "Schließen" }}>
        <Close />
      </LocaleProvider>,
    );
    expect(screen.getByText("Schließen")).toBeInTheDocument();
  });

  it("makes the formatters write in its language", () => {
    render(<LocaleProvider locale="de">{null}</LocaleProvider>);
    expect(formatNumber(1234.5)).toBe("1.234,5");
    expect(formatDate(new Date(2026, 9, 14))).toBe("14. Okt. 2026");
    render(<LocaleProvider locale="en">{null}</LocaleProvider>);
    expect(formatNumber(1234.5)).toBe("1,234.5");
    expect(formatDate(new Date(2026, 9, 14))).toBe("14 Oct 2026");
  });

  it("resolves regional and script tags to the shipped languages", async () => {
    expect(resolveUiLocale("de-AT")).toBe("de");
    expect(resolveUiLocale("zh-Hans-CN")).toBe("zh-CN");
    expect(resolveUiLocale("fr")).toBe("en");
    expect(await loadUiMessages("en")).toEqual({});
  });

  it("matches a browser's language tags to a shipped language, or none", () => {
    expect(matchUiLocale(" EN-gb ")).toBe("en");
    expect(matchUiLocale("zh")).toBe("zh-CN");
    expect(matchUiLocale("fr-FR")).toBeUndefined();
    expect(preferredUiLocale(["fr-FR", "de-CH", "en"])).toBe("de");
    expect(preferredUiLocale(["ja", "fr"])).toBe("en");
    expect(preferredUiLocale([])).toBe("en");
  });
});

function withoutComments(src: string): string {
  let out = "";
  for (let i = 0; i < src.length; i++) {
    const c = src[i];
    if (c === '"' || c === "'" || c === "`") {
      const start = i;
      for (i++; i < src.length && src[i] !== c; i++) if (src[i] === "\\") i++;
      out += src.slice(start, i + 1);
    } else if (c === "/" && src[i + 1] === "/") {
      while (i < src.length && src[i] !== "\n") i++;
      out += "\n";
    } else if (c === "/" && src[i + 1] === "*") {
      const end = src.indexOf("*/", i + 2);
      i = end < 0 ? src.length : end + 1;
    } else out += c;
  }
  return out;
}

function sourceFiles(): string[] {
  const out: string[] = [];
  const walk = (dir: string) => {
    for (const name of readdirSync(dir)) {
      const path = join(dir, name);
      if (statSync(path).isDirectory()) walk(path);
      else if (/\.tsx?$/.test(name) && !/\.(test|stories)\./.test(name) && !path.includes(`${sep}i18n${sep}`)) out.push(path);
    }
  };
  walk(join(__dirname, ".."));
  return out;
}

function sourceMessages(): Map<string, string> {
  const out = new Map<string, string>();
  for (const path of sourceFiles()) {
    for (const m of withoutComments(readFileSync(path, "utf8")).matchAll(/\bt\(\s*"([a-z][a-zA-Z0-9]*\.[a-zA-Z0-9.]+)",\s*"((?:[^"\\]|\\.)*)"/g)) {
      const key = m[1] ?? "";
      const english = JSON.parse(`"${m[2] ?? ""}"`) as string;
      if (out.has(key) && out.get(key) !== english) throw new Error(`${key} has two English texts: "${out.get(key)}" and "${english}"`);
      out.set(key, english);
    }
  }
  return out;
}

function placeholders(message: string): string[] {
  const out: string[] = [];
  const close = (from: number) => {
    let depth = 0;
    for (let i = from; i < message.length; i++) {
      if (message[i] === "{") depth++;
      else if (message[i] === "}" && --depth === 0) return i;
    }
    return message.length;
  };
  for (let i = 0; i < message.length; i++) {
    if (message[i] !== "{") continue;
    const end = close(i);
    const [name = "", kind, ...rest] = message.slice(i + 1, end).split(",");
    if (kind === undefined) out.push(name.trim());
    else {
      const body = rest.join(",");
      const keys: string[] = [];
      for (const m of body.matchAll(/([=\w]+)\s*\{/g)) keys.push(m[1] ?? "");
      if (kind.trim() === "select") out.push(`${name.trim()}:select:${keys.sort().join(",")}`);
      else out.push(name.trim());
      let depth = 0;
      let start = -1;
      for (let j = 0; j < body.length; j++) {
        if (body[j] === "{" && depth++ === 0) start = j + 1;
        else if (body[j] === "}" && --depth === 0) out.push(...placeholders(body.slice(start, j)));
      }
    }
    i = end;
  }
  return [...new Set(out)].sort();
}

describe("the library's catalogs", () => {
  const source = sourceMessages();

  it("find the English beside every key: a plain string literal in the same call", () => {
    for (const path of sourceFiles()) {
      const src = withoutComments(readFileSync(path, "utf8"));
      for (const m of src.matchAll(/\bt\(\s*"([a-z][a-zA-Z0-9]*\.[a-zA-Z0-9.]+)",\s*(\S)/g)) expect(m[2], `${path}: ${m[1]}`).toBe('"');
    }
  });

  it("are read through the context module, so a component carries none of the provider's code", () => {
    for (const path of sourceFiles()) expect(readFileSync(path, "utf8"), path).not.toMatch(/from "(\.\.\/)+i18n(\/index)?"/);
  });

  it("list every key the components use, with the same English", () => {
    for (const [key, english] of source) expect({ key, english: en[key] }).toEqual({ key, english });
    for (const key of Object.keys(en)) expect(source.has(key), `${key} is in en.ts but no component uses it`).toBe(true);
  });

  it.each([
    ["de", de],
    ["zh-CN", zhCN],
  ])("translate every key into %s, keeping each placeholder", (_locale, catalog) => {
    for (const [key, english] of Object.entries(en)) {
      const translated = catalog[key];
      expect(translated, `${key} has no translation`).toBeTypeOf("string");
      expect(placeholders(translated ?? ""), key).toEqual(placeholders(english));
    }
    for (const key of Object.keys(catalog)) expect(key in en, `${key} isn't a library key`).toBe(true);
  });
});
