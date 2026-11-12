// @vitest-environment node
import { Suspense, use, type ReactNode } from "react";
import { renderToPipeableStream } from "react-dom/server";
import { Writable } from "node:stream";
import { describe, expect, it } from "vitest";
import { LocaleProvider, useFormatters } from "./index.js";
import { formatLocale } from "../format/index.js";
import { UsageBreakdown } from "../components/kaeso/usage.js";
import { Reasoning } from "../components/playground/reasoning.js";
import zhCN from "./locales/zh-CN.js";
import de from "./locales/de.js";

const NOW = Date.UTC(2026, 10, 12, 12, 0, 0);

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((r) => (resolve = r));
  return { promise, resolve };
}

function After({ gate, children }: { gate: Promise<void>; children: ReactNode }) {
  use(gate);
  return children;
}

function Dates() {
  const { formatRelative, formatDuration } = useFormatters();
  return (
    <p>
      {formatRelative(NOW - 5 * 60_000, NOW)} · {formatDuration(3_200)}
    </p>
  );
}

function Page({ locale, gate }: { locale: string; gate: Promise<void> }) {
  const messages = locale === "zh-CN" ? zhCN : locale === "de" ? de : {};
  return (
    <LocaleProvider locale={locale} messages={messages}>
      <Suspense fallback={null}>
        <After gate={gate}>
          <Dates />
          <UsageBreakdown tokens={{ input: 793_640, output: 2_890_000 }} />
          <Reasoning duration={3_200} text="Checked the route." />
        </After>
      </Suspense>
    </LocaleProvider>
  );
}

function render(locale: string, gate: Promise<void>): Promise<string> {
  return new Promise((resolve, reject) => {
    let html = "";
    const sink = new Writable({
      write(chunk, _encoding, next) {
        html += String(chunk);
        next();
      },
      final(done) {
        resolve(html);
        done();
      },
    });
    const stream = renderToPipeableStream(<Page locale={locale} gate={gate} />, {
      onAllReady: () => stream.pipe(sink),
      onError: reject,
    });
  });
}

const LANGUAGE = {
  en: { says: ["5 min ago", "3.2 s", "Thought for"], never: ["分钟前", "vor 5 Min"] },
  "zh-CN": { says: ["5 分钟前", "3.2 秒", "思考了", "万"], never: ["min ago", "Thought for"] },
  de: { says: ["vor 5 Min.", "3,2 s", "nachgedacht", "Mio."], never: ["min ago", "分钟前"] },
} as const;

describe("rendering on a server", () => {
  it("formats each request in its own language while another request renders in between", async () => {
    const zh = deferred();
    const en = deferred();
    const zhPage = render("zh-CN", zh.promise);
    const enPage = render("en", en.promise);
    en.resolve();
    const enHtml = await enPage;
    zh.resolve();
    const zhHtml = await zhPage;

    for (const text of LANGUAGE["zh-CN"].says) expect(zhHtml).toContain(text);
    for (const text of LANGUAGE["zh-CN"].never) expect(zhHtml).not.toContain(text);
    for (const text of LANGUAGE.en.says) expect(enHtml).toContain(text);
    for (const text of LANGUAGE.en.never) expect(enHtml).not.toContain(text);
  });

  it("keeps every language apart across many requests finishing in any order", async () => {
    const locales = Array.from({ length: 60 }, (_, i) => (["en", "zh-CN", "de"] as const)[i % 3]!);
    const gates = locales.map(() => deferred());
    const pages = locales.map((locale, i) => render(locale, gates[i]!.promise));
    for (const i of locales.map((_, i) => (i * 37) % locales.length)) gates[i]!.resolve();
    const html = await Promise.all(pages);

    locales.forEach((locale, i) => {
      for (const text of LANGUAGE[locale].says) expect(html[i], `${locale} page ${i}`).toContain(text);
      for (const text of LANGUAGE[locale].never) expect(html[i], `${locale} page ${i}`).not.toContain(text);
    });
  });

  it("leaves the plain formatters' language alone", async () => {
    const gate = deferred();
    const page = render("zh-CN", gate.promise);
    gate.resolve();
    await page;
    expect(formatLocale()).toBe("en");
  });
});
