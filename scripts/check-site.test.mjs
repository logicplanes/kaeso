import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, test } from "node:test";
import { fileURLToPath } from "node:url";
import { addressOf, checkSite, inLanguage, readHead, readSitemap, xmlProblems } from "./check-site.mjs";

const ORIGIN = "https://docs.kaeso.example.com";
const script = join(dirname(fileURLToPath(import.meta.url)), "check-site.mjs");
const LOCALES = { en: "en_US", de: "de_DE", "zh-CN": "zh_CN" };
const TITLES = { "/": "Kaeso", "/guides": "Guides", "/guides/keys": "Keys" };
let dir = "";
afterEach(() => dir && rmSync(dir, { recursive: true, force: true }));

const at = (lang, path) => inLanguage(path, lang);

function head(lang, path, { noindex = false, extra = "", description, title } = {}) {
  const own = `${ORIGIN}${at(lang, path)}`;
  const text = description ?? `About ${TITLES[path] ?? path} in ${lang}.`;
  const name = title ?? `${TITLES[path] ?? path} (${lang})`;
  const others = Object.keys(LOCALES).filter((l) => l !== lang);
  return [
    `<!doctype html><html lang="${lang}"><head>`,
    `<title>${name}</title>`,
    `<meta name="description" content="${text}">`,
    noindex ? `<meta name="robots" content="noindex">` : `<link rel="canonical" href="${own}">`,
    ...(noindex
      ? []
      : [...Object.keys(LOCALES), "x-default"].map((l) => `<link rel="alternate" hreflang="${l}" href="${ORIGIN}${at(l === "x-default" ? "en" : l, path)}">`)),
    `<meta property="og:title" content="${name}">`,
    `<meta property="og:description" content="${text}">`,
    `<meta property="og:url" content="${own}">`,
    `<meta property="og:site_name" content="Kaeso">`,
    `<meta property="og:type" content="website">`,
    `<meta property="og:locale" content="${LOCALES[lang]}">`,
    ...others.map((l) => `<meta property="og:locale:alternate" content="${LOCALES[l]}">`),
    `<meta property="og:image" content="${ORIGIN}/og.png">`,
    `<meta property="og:image:alt" content="Kaeso">`,
    `<meta name="twitter:card" content="summary_large_image">`,
    `<meta name="theme-color" content="#ffffff" media="(prefers-color-scheme: light)">`,
    `<meta name="theme-color" content="#000000" media="(prefers-color-scheme: dark)">`,
    `<link rel="icon" href="/favicon.svg" type="image/svg+xml">`,
    `<link rel="icon" href="/favicon-32.png" type="image/png" sizes="32x32">`,
    `<link rel="apple-touch-icon" href="/apple-touch-icon.png">`,
    `<link rel="manifest" href="/site.webmanifest">`,
    extra,
    `</head><body></body></html>`,
  ].join("\n");
}

function sitemap(paths) {
  const urls = paths.flatMap((path) =>
    Object.keys(LOCALES).map((lang) => {
      const links = [...Object.keys(LOCALES), "x-default"].map(
        (l) => `<xhtml:link rel="alternate" hreflang="${l}" href="${ORIGIN}${at(l === "x-default" ? "en" : l, path)}"/>`,
      );
      return `<url><loc>${ORIGIN}${at(lang, path)}</loc>${links.join("")}</url>`;
    }),
  );
  return `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">${urls.join("\n")}</urlset>\n`;
}

function site(overrides = {}) {
  dir = mkdtempSync(join(tmpdir(), "kaeso-site-"));
  const files = {
    "robots.txt": `User-agent: *\nAllow: /\n\nSitemap: ${ORIGIN}/sitemap.xml\n`,
    "sitemap.xml": sitemap(["/", "/guides", "/guides/keys"]),
    "site.webmanifest": JSON.stringify({ name: "Kaeso", icons: [{ src: "/android-chrome-192.png" }] }),
    "og.png": "png",
    "favicon.svg": "<svg/>",
    "favicon-32.png": "png",
    "apple-touch-icon.png": "png",
    "android-chrome-192.png": "png",
  };
  for (const lang of Object.keys(LOCALES)) {
    const folder = lang === "en" ? "" : `${lang}/`;
    files[lang === "en" ? "index.html" : `${lang}.html`] = head(lang, "/");
    files[`${folder}guides.html`] = head(lang, "/guides");
    files[`${folder}guides/index.html`] = head(lang, "/guides");
    files[`${folder}guides/keys.html`] = head(lang, "/guides/keys");
    files[`${folder}404.html`] = head(lang, "/404", { noindex: true, title: "Not found", description: "Gone." });
  }
  for (const [name, text] of Object.entries({ ...files, ...overrides })) {
    const file = join(dir, name);
    if (text === null) continue;
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, text);
  }
  return dir;
}

test("reads a built file as its address", () => {
  assert.equal(addressOf("index.html"), "/");
  assert.equal(addressOf("de.html"), "/de");
  assert.equal(addressOf("de/index.html"), "/de");
  assert.equal(addressOf("guides/index.html"), "/guides");
  assert.equal(addressOf("zh-CN/guides/keys.html"), "/zh-CN/guides/keys");
  assert.equal(inLanguage("/de/guides", "zh-CN"), "/zh-CN/guides");
  assert.equal(inLanguage("/zh-CN", "en"), "/");
  assert.equal(inLanguage("/", "de"), "/de");
});

test("reads a page's head with its entities decoded", () => {
  const h = readHead(head("de", "/guides", { title: "Keys &amp; limits", extra: '<script type="application/ld+json">{"@type":"WebSite"}</script>' }));
  assert.equal(h.lang, "de");
  assert.equal(h.title, "Keys & limits");
  assert.deepEqual(h.canonical, [`${ORIGIN}/de/guides`]);
  assert.equal(h.alternates.length, 4);
  assert.deepEqual(h.meta("og:locale:alternate"), ["en_US", "zh_CN"]);
  assert.deepEqual(h.structured, ['{"@type":"WebSite"}']);
  assert.equal(h.noindex, false);
});

test("finds what makes a sitemap invalid XML", () => {
  assert.deepEqual(xmlProblems(sitemap(["/"])), []);
  assert.deepEqual(xmlProblems(`<?xml version="1.0"?><urlset><url><loc>a & b</loc></url></urlset>`), ['has an unescaped & near "a & b"']);
  assert.deepEqual(xmlProblems(`<?xml version="1.0"?><urlset><url></urlset>`), ["closes <urlset> that is not open", "leaves <urlset> open"]);
  assert.deepEqual(xmlProblems(`<urlset></urlset>`), ["does not start with an XML declaration"]);
  assert.equal(readSitemap(sitemap(["/guides"]))[1].loc, `${ORIGIN}/de/guides`);
});

test("passes a site whose pages are all in the sitemap, with their alternates and previews", () => {
  const result = checkSite(site(), ORIGIN);
  assert.deepEqual(result.problems, []);
  assert.equal(result.pages, 9);
  assert.equal(result.listed, 9);
});

test("names a built page that the sitemap leaves out, and a listed page that was never built", () => {
  const { problems } = checkSite(site({ "sitemap.xml": sitemap(["/", "/guides", "/changelog"]) }), ORIGIN);
  assert.ok(problems.includes(`guides/keys.html: ${ORIGIN}/guides/keys is missing from sitemap.xml`));
  assert.ok(problems.includes(`de/guides/keys.html: ${ORIGIN}/de/guides/keys is missing from sitemap.xml`));
  assert.ok(problems.includes(`sitemap.xml lists ${ORIGIN}/changelog, but the build has no indexable page for it`));
});

test("keeps noindex pages out of the sitemap and requires every not-found page to be noindex", () => {
  const { problems } = checkSite(
    site({ "sitemap.xml": sitemap(["/", "/guides", "/guides/keys", "/404"]), "de/404.html": head("de", "/404"), "zh-CN/404.html": null }),
    ORIGIN,
  );
  assert.ok(problems.includes(`404.html: is noindex, but sitemap.xml lists ${ORIGIN}/404`));
  assert.ok(problems.includes("de/404.html: a not-found page must be noindex"));
  assert.ok(problems.includes("zh-CN/404.html is missing"));
});

test("checks the head of every page", () => {
  const broken = head("de", "/guides/keys")
    .replace(`<link rel="canonical" href="${ORIGIN}/de/guides/keys">`, `<link rel="canonical" href="${ORIGIN}/de/guides/keys/">`)
    .replace(`hreflang="zh-CN" href="${ORIGIN}/zh-CN/guides/keys"`, `hreflang="zh-CN" href="${ORIGIN}/guides/keys"`)
    .replace('content="de_DE"', 'content="en_US"')
    .replace(`${ORIGIN}/og.png`, `${ORIGIN}/missing.png`)
    .replace('<meta name="twitter:card" content="summary_large_image">', "")
    .replace('<html lang="de">', '<html lang="en">')
    .replace(/<meta name="theme-color"[^>]*dark[^>]*>/, "")
    .replace('<link rel="apple-touch-icon" href="/apple-touch-icon.png">', '<link rel="apple-touch-icon" href="/touch.png">')
    .replace("</head>", '<script type="application/ld+json">{"@type":"TechArticle"}</script></head>');
  const { problems } = checkSite(site({ "de/guides/keys.html": broken }), ORIGIN);
  const of = problems.filter((p) => p.startsWith("de/guides/keys.html:"));
  assert.deepEqual(of, [
    'de/guides/keys.html: <html lang> is "en", not "de"',
    `de/guides/keys.html: has the canonical address ${ORIGIN}/de/guides/keys/ instead of ${ORIGIN}/de/guides/keys`,
    `de/guides/keys.html: has the zh-CN alternate ${ORIGIN}/guides/keys instead of ${ORIGIN}/zh-CN/guides/keys`,
    'de/guides/keys.html: has og:locale "en_US"; it must be de_DE',
    `de/guides/keys.html: has og:image "${ORIGIN}/missing.png"; it must be an image of ${ORIGIN} that is in the build`,
    "de/guides/keys.html: has twitter:card none; it must be summary_large_image",
    "de/guides/keys.html: has no theme-color for the dark scheme",
    "de/guides/keys.html: links to /touch.png, which is not in the build",
    "de/guides/keys.html: has structured data without a schema.org @context and @type",
  ]);
});

test("finds a description shared by two pages and a title shared within a language", () => {
  const { problems } = checkSite(
    site({
      "de/guides/keys.html": head("de", "/guides/keys", { description: "About Guides in de." }),
      "zh-CN/guides/keys.html": head("zh-CN", "/guides/keys", { title: "Guides (zh-CN)" }),
    }),
    ORIGIN,
  );
  assert.ok(problems.includes('de/guides/keys.html: has the same description as de/guides.html: "About Guides in de."'));
  assert.ok(problems.includes('zh-CN/guides/keys.html: has the same title as the page in its language zh-CN/guides.html: "Guides (zh-CN)"'));
  assert.ok(!problems.some((p) => p.includes("same title") && p.startsWith("de/")));
});

test("checks robots.txt, the sitemap's alternates and the manifest", () => {
  const map = sitemap(["/", "/guides", "/guides/keys"])
    .replace(`hreflang="x-default" href="${ORIGIN}/guides"`, `hreflang="x-default" href="${ORIGIN}/de/guides"`)
    .replace("<url>", "<url><lastmod>yesterday</lastmod>");
  const { problems } = checkSite(
    site({
      "robots.txt": "User-agent: *\nDisallow: /\n",
      "sitemap.xml": map,
      "site.webmanifest": JSON.stringify({ name: "Kaeso", icons: [{ src: "/gone.png" }] }),
    }),
    ORIGIN,
  );
  assert.ok(problems.includes("robots.txt disallows the whole site"));
  assert.ok(problems.includes(`robots.txt does not point to ${ORIGIN}/sitemap.xml`));
  assert.ok(problems.includes(`sitemap.xml gives ${ORIGIN}/ a <lastmod> that is not a W3C date: yesterday`));
  assert.ok(problems.includes(`sitemap.xml gives ${ORIGIN}/guides the x-default alternate ${ORIGIN}/de/guides instead of ${ORIGIN}/guides`));
  assert.ok(problems.includes("the web manifest's icon /gone.png is missing"));
});

test("fails from the command line with every problem, and passes quietly", () => {
  const good = site();
  assert.match(execFileSync("node", [script, good, ORIGIN], { encoding: "utf8" }), /^site: 9 pages, all 9 in sitemap\.xml/);
  rmSync(join(good, "robots.txt"));
  assert.throws(
    () => execFileSync("node", [script, good, ORIGIN], { encoding: "utf8", stdio: "pipe" }),
    (e) => /robots\.txt is missing/.test(String(e.stderr)),
  );
  assert.deepEqual(checkSite(join(good, "nothing"), ORIGIN).problems, [`${join(good, "nothing")} does not exist; build the site first`]);
});
