import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { LANGUAGES, inLanguage, languageOf, readHead } from "../../../scripts/check-site.mjs";

const escapeXml = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

function htmlOf(dir, pathname) {
  const path = pathname.replace(/^\/+|\/+$/g, "");
  const file = [path ? `${path}.html` : "index.html", path ? `${path}/index.html` : ""].find((f) => f && existsSync(join(dir, f)));
  return file ? readFileSync(join(dir, file), "utf8") : null;
}

export function sitemapXml(origin, addresses) {
  const english = (address) => inLanguage(address, "en");
  const sorted = [...new Set(addresses)].sort(
    (a, b) => english(a).localeCompare(english(b)) || LANGUAGES.indexOf(languageOf(a)) - LANGUAGES.indexOf(languageOf(b)),
  );
  const urls = sorted.map((address) => {
    const links = [...LANGUAGES, "x-default"].map((hreflang) => {
      const href = `${origin}${inLanguage(address, hreflang === "x-default" ? "en" : hreflang)}`;
      return `    <xhtml:link rel="alternate" hreflang="${hreflang}" href="${escapeXml(href)}" />`;
    });
    return ["  <url>", `    <loc>${escapeXml(`${origin}${address}`)}</loc>`, ...links, "  </url>"].join("\n");
  });
  return [
    `<?xml version="1.0" encoding="UTF-8"?>`,
    `<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">`,
    ...urls,
    `</urlset>`,
    "",
  ].join("\n");
}

export function sitemap() {
  let origin = "";
  return {
    name: "kaeso-sitemap",
    hooks: {
      "astro:config:done": ({ config }) => {
        origin = new URL(config.site).origin;
      },
      "astro:build:done": ({ dir, pages }) => {
        const out = fileURLToPath(dir);
        const addresses = pages.flatMap(({ pathname }) => {
          const html = htmlOf(out, pathname);
          if (html === null || readHead(html).noindex) return [];
          const path = pathname.replace(/^\/+|\/+$/g, "");
          return [path ? `/${path}` : "/"];
        });
        writeFileSync(join(out, "sitemap.xml"), sitemapXml(origin, addresses));
      },
    },
  };
}
