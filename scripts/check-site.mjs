import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

export const LANGUAGES = ["en", "de", "zh-CN"];
const HREFLANGS = [...LANGUAGES, "x-default"];
const OG_LOCALES = { en: "en_US", de: "de_DE", "zh-CN": "zh_CN" };
const ENTITIES = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'" };

const decode = (s) =>
  s.replace(/&(#\d+|#x[\da-f]+|\w+);/gi, (whole, name) => {
    if (name[0] !== "#") return ENTITIES[name] ?? whole;
    return String.fromCodePoint(name[1] === "x" || name[1] === "X" ? parseInt(name.slice(2), 16) : Number(name.slice(1)));
  });

function filesIn(dir, ext, base = dir) {
  return readdirSync(dir).flatMap((name) => {
    const full = join(dir, name);
    if (statSync(full).isDirectory()) return filesIn(full, ext, base);
    return name.endsWith(ext) ? [relative(base, full).split(sep).join("/")] : [];
  });
}

export function addressOf(file) {
  const path = file.replace(/\.html$/, "").replace(/(^|\/)index$/, "");
  return path ? `/${path}` : "/";
}

export function languageOf(address) {
  const first = address.split("/")[1] ?? "";
  return LANGUAGES.find((l) => l !== "en" && l === first) ?? "en";
}

export function inLanguage(address, language) {
  const prefix = languageOf(address) === "en" ? "" : `/${languageOf(address)}`;
  const within = address.slice(prefix.length) || "/";
  if (language === "en") return within;
  return within === "/" ? `/${language}` : `/${language}${within}`;
}

const url = (origin, address) => `${origin}${address}`;

function attributes(tag) {
  const out = {};
  for (const m of tag.matchAll(/([\w:-]+)(?:\s*=\s*"([^"]*)")?/g)) out[m[1].toLowerCase()] = decode(m[2] ?? "");
  return out;
}

export function readHead(html) {
  const end = html.indexOf("</head>");
  const head = end === -1 ? html : html.slice(0, end);
  const tag = (name) => [...head.matchAll(new RegExp(`<${name}\\b([^>]*)>`, "gi"))].map((m) => attributes(m[1]));
  const metas = tag("meta");
  const links = tag("link");
  const meta = (key) => metas.filter((m) => (m.name ?? m.property) === key).map((m) => m.content ?? "");
  const rel = (name) => links.filter((l) => (l.rel ?? "").split(/\s+/).includes(name));
  const title = /<title>([^<]*)<\/title>/i.exec(head);
  return {
    lang: /<html\b[^>]*\blang="([^"]*)"/i.exec(html)?.[1] ?? "",
    title: title ? decode(title[1]).trim() : "",
    meta,
    metas,
    canonical: rel("canonical").map((l) => l.href ?? ""),
    alternates: rel("alternate")
      .filter((l) => l.hreflang)
      .map((l) => ({ hreflang: l.hreflang, href: l.href ?? "" })),
    rel,
    structured: [...head.matchAll(/<script\b[^>]*type="application\/ld\+json"[^>]*>([\s\S]*?)<\/script>/gi)].map((m) => m[1]),
    noindex: meta("robots").some((c) => /\bnoindex\b/i.test(c)),
  };
}

export function xmlProblems(xml) {
  const problems = [];
  const body = xml.replace(/^﻿?<\?xml[^?]*\?>\s*/, "");
  if (body === xml && !xml.startsWith("<?xml")) problems.push("does not start with an XML declaration");
  const stack = [];
  let last = 0;
  for (const m of body.matchAll(/<(\/?)([\w:.-]+)((?:\s+[\w:.-]+\s*=\s*"[^"<]*")*)\s*(\/?)>/g)) {
    const text = body.slice(last, m.index);
    if (/[<>]/.test(text)) problems.push(`has a stray < or > near "${text.trim().slice(0, 40)}"`);
    if (/&(?!(amp|lt|gt|quot|apos|#\d+|#x[\da-f]+);)/i.test(text + m[3])) problems.push(`has an unescaped & near "${(text + m[3]).trim().slice(0, 40)}"`);
    last = m.index + m[0].length;
    const [, closing, name, , selfClosing] = m;
    if (selfClosing) continue;
    if (!closing) stack.push(name);
    else if (stack.pop() !== name) problems.push(`closes <${name}> that is not open`);
  }
  if (body.slice(last).trim()) problems.push("has text after the root element");
  if (stack.length) problems.push(`leaves <${stack.join(">, <")}> open`);
  return problems;
}

export function readSitemap(xml) {
  const urls = [...xml.matchAll(/<url>([\s\S]*?)<\/url>/g)].map((m) => ({
    loc: decode(/<loc>([^<]*)<\/loc>/.exec(m[1])?.[1] ?? "").trim(),
    lastmod: /<lastmod>([^<]*)<\/lastmod>/.exec(m[1])?.[1],
    alternates: [...m[1].matchAll(/<xhtml:link\b([^>]*)\/?>/g)].map((l) => {
      const a = attributes(l[1]);
      return { rel: a.rel, hreflang: a.hreflang, href: a.href ?? "" };
    }),
  }));
  return urls;
}

function checkRobots(dir, origin, problems) {
  const file = join(dir, "robots.txt");
  if (!existsSync(file)) return problems.push("robots.txt is missing");
  const lines = readFileSync(file, "utf8")
    .split("\n")
    .map((l) => l.trim());
  if (!lines.includes("User-agent: *")) problems.push("robots.txt has no `User-agent: *` group");
  if (lines.some((l) => /^Disallow:\s*\/\s*$/i.test(l))) problems.push("robots.txt disallows the whole site");
  if (!lines.includes(`Sitemap: ${origin}/sitemap.xml`)) problems.push(`robots.txt does not point to ${origin}/sitemap.xml`);
}

function checkSitemap(dir, origin, problems) {
  const file = join(dir, "sitemap.xml");
  if (!existsSync(file)) {
    problems.push("sitemap.xml is missing");
    return [];
  }
  const xml = readFileSync(file, "utf8");
  for (const p of xmlProblems(xml)) problems.push(`sitemap.xml ${p}`);
  if (!/<urlset\b[^>]*xmlns="http:\/\/www\.sitemaps\.org\/schemas\/sitemap\/0\.9"/.test(xml))
    problems.push("sitemap.xml has no <urlset> in the sitemaps.org namespace");
  if (!/<urlset\b[^>]*xmlns:xhtml="http:\/\/www\.w3\.org\/1999\/xhtml"/.test(xml))
    problems.push("sitemap.xml does not declare the xhtml namespace for its alternates");
  const urls = readSitemap(xml);
  if (!urls.length) problems.push("sitemap.xml lists no pages");
  const locs = new Set();
  for (const u of urls) {
    if (!u.loc.startsWith(`${origin}/`)) problems.push(`sitemap.xml lists ${u.loc || "an empty <loc>"}, which is not on ${origin}`);
    if (locs.has(u.loc)) problems.push(`sitemap.xml lists ${u.loc} twice`);
    locs.add(u.loc);
    if (u.lastmod !== undefined && !/^\d{4}-\d{2}-\d{2}(T\d{2}:\d{2}(:\d{2}(\.\d+)?)?(Z|[+-]\d{2}:\d{2}))?$/.test(u.lastmod))
      problems.push(`sitemap.xml gives ${u.loc} a <lastmod> that is not a W3C date: ${u.lastmod}`);
    const langs = u.alternates.map((a) => a.hreflang);
    if (JSON.stringify([...langs].sort()) !== JSON.stringify([...HREFLANGS].sort()))
      problems.push(`sitemap.xml gives ${u.loc} the alternates ${langs.join(", ") || "none"} instead of ${HREFLANGS.join(", ")}`);
    if (u.alternates.some((a) => a.rel !== "alternate")) problems.push(`sitemap.xml has an xhtml:link for ${u.loc} that is not rel="alternate"`);
    const address = u.loc.slice(origin.length) || "/";
    for (const a of u.alternates.filter((a) => HREFLANGS.includes(a.hreflang))) {
      const expected = url(origin, inLanguage(address, a.hreflang === "x-default" ? "en" : a.hreflang));
      if (a.href !== expected) problems.push(`sitemap.xml gives ${u.loc} the ${a.hreflang} alternate ${a.href} instead of ${expected}`);
    }
  }
  for (const u of urls)
    for (const a of u.alternates) if (!locs.has(a.href)) problems.push(`sitemap.xml names ${a.href} as an alternate of ${u.loc} but does not list it`);
  return urls;
}

function checkManifest(dir, href, problems) {
  const file = join(dir, href.replace(/^\//, ""));
  if (!existsSync(file)) return problems.push(`the web manifest ${href} is missing`);
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(file, "utf8"));
  } catch {
    return problems.push(`the web manifest ${href} is not JSON`);
  }
  if (!manifest.name) problems.push(`the web manifest ${href} has no name`);
  if (!Array.isArray(manifest.icons) || !manifest.icons.length) return problems.push(`the web manifest ${href} has no icons`);
  for (const icon of manifest.icons)
    if (!existsSync(join(dir, String(icon.src).replace(/^\//, "")))) problems.push(`the web manifest's icon ${icon.src} is missing`);
}

function checkPage(dir, origin, file, address, head, problems) {
  const at = `${file}:`;
  const language = languageOf(address);
  const own = url(origin, address);
  if (head.lang !== language) problems.push(`${at} <html lang> is "${head.lang}", not "${language}"`);
  if (!head.title) problems.push(`${at} has no <title>`);
  const [description = ""] = head.meta("description");
  if (!description.trim()) problems.push(`${at} has no description`);
  if (head.canonical.length !== 1 || head.canonical[0] !== own)
    problems.push(`${at} has the canonical address ${head.canonical.join(", ") || "none"} instead of ${own}`);
  const expected = HREFLANGS.map((h) => ({ hreflang: h, href: url(origin, inLanguage(address, h === "x-default" ? "en" : h)) }));
  for (const e of expected) {
    const found = head.alternates.filter((a) => a.hreflang === e.hreflang);
    if (found.length !== 1 || found[0].href !== e.href)
      problems.push(`${at} has the ${e.hreflang} alternate ${found.map((f) => f.href).join(", ") || "none"} instead of ${e.href}`);
  }
  const one = (key, test, what) => {
    const values = head.meta(key);
    if (values.length !== 1 || !test(values[0])) problems.push(`${at} has ${key} ${values.map((v) => `"${v}"`).join(", ") || "none"}; ${what}`);
  };
  one("og:title", (v) => v.trim() !== "", "it needs the page's title");
  one("og:description", (v) => v.trim() !== "", "it needs the page's description");
  one("og:url", (v) => v === own, `it must be ${own}`);
  one("og:site_name", (v) => v.trim() !== "", "it needs the site's name");
  one("og:type", (v) => v.trim() !== "", "it needs a type");
  one("og:locale", (v) => v === OG_LOCALES[language], `it must be ${OG_LOCALES[language]}`);
  const others = LANGUAGES.filter((l) => l !== language).map((l) => OG_LOCALES[l]);
  if (JSON.stringify(head.meta("og:locale:alternate").sort()) !== JSON.stringify(others.sort()))
    problems.push(`${at} must name ${others.join(" and ")} as og:locale:alternate`);
  one(
    "og:image",
    (v) => v.startsWith(`${origin}/`) && existsSync(join(dir, v.slice(origin.length + 1))),
    `it must be an image of ${origin} that is in the build`,
  );
  one("og:image:alt", (v) => v.trim() !== "", "it needs a text");
  one("twitter:card", (v) => v === "summary_large_image", "it must be summary_large_image");
  const themes = head.metas.filter((m) => m.name === "theme-color");
  for (const scheme of ["light", "dark"])
    if (!themes.some((m) => m.media === `(prefers-color-scheme: ${scheme})` && m.content)) problems.push(`${at} has no theme-color for the ${scheme} scheme`);
  const icons = head.rel("icon");
  if (!icons.some((l) => l.type === "image/svg+xml")) problems.push(`${at} has no SVG favicon`);
  if (!icons.some((l) => l.type === "image/png")) problems.push(`${at} has no PNG favicon`);
  for (const link of [...icons, ...head.rel("apple-touch-icon"), ...head.rel("manifest")])
    if (!link.href?.startsWith("/") || !existsSync(join(dir, link.href.slice(1)))) problems.push(`${at} links to ${link.href}, which is not in the build`);
  if (!head.rel("apple-touch-icon").length) problems.push(`${at} has no apple-touch-icon`);
  if (!head.rel("manifest").length) problems.push(`${at} has no web manifest`);
  for (const json of head.structured) {
    try {
      const data = JSON.parse(json);
      if (data["@context"] !== "https://schema.org" || typeof data["@type"] !== "string")
        problems.push(`${at} has structured data without a schema.org @context and @type`);
      if (data.url !== undefined && !String(data.url).startsWith(`${origin}/`))
        problems.push(`${at} has structured data for ${data.url}, which is not on ${origin}`);
      if (data.inLanguage !== undefined && data.inLanguage !== language) problems.push(`${at} has structured data in ${data.inLanguage}, not ${language}`);
    } catch {
      problems.push(`${at} has structured data that is not JSON`);
    }
  }
  return description;
}

export function checkSite(dir, origin) {
  const problems = [];
  const root = resolve(dir);
  if (!existsSync(root)) return { problems: [`${dir} does not exist; build the site first`], pages: 0, listed: 0 };
  checkRobots(root, origin, problems);
  const urls = checkSitemap(root, origin, problems);
  const listed = new Set(urls.map((u) => u.loc));
  const indexed = new Map();
  const titles = new Map();
  const descriptions = new Map();
  let manifest = "";
  for (const file of filesIn(root, ".html").sort()) {
    const address = addressOf(file);
    const own = url(origin, address);
    const head = readHead(readFileSync(join(root, file), "utf8"));
    if (head.noindex) {
      if (listed.has(own)) problems.push(`${file}: is noindex, but sitemap.xml lists ${own}`);
      continue;
    }
    if (!listed.has(own)) problems.push(`${file}: ${own} is missing from sitemap.xml`);
    const description = checkPage(root, origin, file, address, head, problems);
    manifest ||= head.rel("manifest")[0]?.href ?? "";
    if (indexed.has(address)) continue;
    indexed.set(address, file);
    for (const [seen, key, value, what] of [
      [titles, `${languageOf(address)} ${head.title}`, head.title, "title as the page in its language"],
      [descriptions, description, description, "description as"],
    ]) {
      if (!value) continue;
      const other = seen.get(key);
      if (other) problems.push(`${file}: has the same ${what} ${other}: "${value}"`);
      else seen.set(key, file);
    }
  }
  for (const loc of listed) {
    const address = loc.slice(origin.length) || "/";
    if (!indexed.has(address)) problems.push(`sitemap.xml lists ${loc}, but the build has no indexable page for it`);
  }
  if (manifest) checkManifest(root, manifest, problems);
  for (const language of LANGUAGES) {
    const file = language === "en" ? "404.html" : `${language}/404.html`;
    if (!existsSync(join(root, file))) problems.push(`${file} is missing`);
    else if (!readHead(readFileSync(join(root, file), "utf8")).noindex) problems.push(`${file}: a not-found page must be noindex`);
  }
  return { problems, pages: indexed.size, listed: listed.size };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [dir, origin] = process.argv.slice(2);
  if (!dir || !origin) {
    console.error("usage: node scripts/check-site.mjs <dist> <origin>");
    process.exit(2);
  }
  const { problems, pages, listed } = checkSite(dir, origin.replace(/\/+$/, ""));
  if (problems.length) {
    console.error(`site: ${problems.length} to fix in ${dir}\n  ${problems.join("\n  ")}`);
    process.exit(1);
  }
  console.log(`site: ${pages} pages, all ${listed} in sitemap.xml, with their alternates and link previews`);
}
