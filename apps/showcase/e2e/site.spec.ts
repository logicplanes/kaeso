import AxeBuilder from "@axe-core/playwright";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { componentPages, open, overviewPages } from "./support";

const SITE = "https://ui.kaeso.app";
const dist = fileURLToPath(new URL("../dist/", import.meta.url));
const { version } = JSON.parse(readFileSync(fileURLToPath(new URL("../../../package.json", import.meta.url)), "utf8")) as { version: string };
const TAG = `v${version}`;
const LANGS = ["en", "de", "zh-CN"] as const;
const at = (lang: (typeof LANGS)[number], path: string) => (lang === "en" ? path : path === "/" ? `/${lang}` : `/${lang}${path}`);

function builtFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = join(dir, name);
    return statSync(full).isDirectory() ? builtFiles(full) : /\.(html|txt|json|js)$/.test(name) ? [full] : [];
  });
}

test.describe("links to the source", () => {
  for (const lang of LANGS) {
    test(`View source opens the component at the release tag in ${lang}`, async ({ request }) => {
      const html = await (await request.get(at(lang, "/components/select"))).text();
      const links = [...html.matchAll(/href="(https:\/\/github\.com\/[^"]+)"/g)].map((m) => m[1]);
      expect(links).toContain(`https://github.com/logicplanes/kaeso/tree/${TAG}/packages/ui/src/components/select`);
    });
  }

  test("every link into the repository names the release tag, never a branch", () => {
    const pinned = /https:\/\/(?:github\.com\/logicplanes\/kaeso\/(?:tree|blob|raw)|raw\.githubusercontent\.com\/logicplanes\/kaeso)\/([^/"\s)]+)/g;
    const refs = new Set<string>();
    for (const file of builtFiles(dist)) for (const m of readFileSync(file, "utf8").matchAll(pinned)) refs.add(m[1] ?? "");
    expect([...refs]).toEqual([TAG]);
  });

  test("llms.txt points at the release tag", async ({ request }) => {
    expect(await (await request.get("/llms.txt")).text()).toContain(`- Source: https://github.com/logicplanes/kaeso/tree/${TAG}/packages/ui\n`);
  });
});

test.describe("the changelog", () => {
  const words = { en: "Changelog", de: "Änderungen", "zh-CN": "更新日志" } as const;
  for (const lang of LANGS) {
    test(`the sidebar's changelog leads to kaeso.app in ${lang}`, async ({ page }) => {
      await open(page, at(lang, "/"));
      const site = lang === "en" ? "https://kaeso.app" : `https://kaeso.app/${lang}`;
      await expect(page.getByRole("navigation").getByRole("link", { name: words[lang], exact: true })).toHaveAttribute("href", `${site}/changelog`);
    });
  }

  test("the package version in the bar leads to its release on kaeso.app", async ({ page }) => {
    await open(page, "/de");
    await expect(page.getByRole("link", { name: `@logicplanes/kaeso-ui v${version}` })).toHaveAttribute("href", `https://kaeso.app/de/changelog/${version}`);
  });

  test("the site keeps no changelog of its own", async ({ request }) => {
    for (const path of ["/changelog", "/de/changelog", "/zh-CN/changelog"]) expect((await request.get(path)).status(), path).toBe(404);
  });
});

test.describe("legal pages", () => {
  const words = { en: ["Legal", "Privacy", "Imprint"], de: ["Rechtliches", "Datenschutz", "Impressum"], "zh-CN": ["法律信息", "隐私政策", "法律声明"] } as const;
  for (const lang of LANGS) {
    test(`the legal menu at the sidebar's foot links to the privacy policy and the imprint in ${lang}`, async ({ page }) => {
      await open(page, at(lang, "/"));
      const [legal, privacy, imprint] = words[lang];
      await page.getByRole("button", { name: legal, exact: true }).click();
      const site = lang === "en" ? "https://kaeso.app" : `https://kaeso.app/${lang}`;
      await expect(page.getByRole("menuitem", { name: privacy, exact: true })).toHaveAttribute("href", `${site}/datenschutz`);
      await expect(page.getByRole("menuitem", { name: imprint, exact: true })).toHaveAttribute("href", `${site}/impressum`);
    });
  }
});

test.describe("for search engines and link previews", () => {
  test("robots.txt allows everything and names the sitemap", async ({ request }) => {
    expect(await (await request.get("/robots.txt")).text()).toBe(`User-agent: *\nAllow: /\n\nSitemap: ${SITE}/sitemap.xml\n`);
  });

  test("the sitemap lists every page in every language with its alternates, and nothing that isn't indexed", async ({ request }) => {
    const xml = await (await request.get("/sitemap.xml")).text();
    const locs = [...xml.matchAll(/<loc>([^<]+)<\/loc>/g)].map((m) => (m[1] ?? "").slice(SITE.length) || "/");
    const pages = [...overviewPages, ...componentPages];
    expect(new Set(locs)).toEqual(new Set(LANGS.flatMap((lang) => pages.map((p) => at(lang, p)))));
    expect(locs.filter((l) => /\/(examples|states|404)$/.test(l))).toEqual([]);
    const button = /<url>\s*<loc>https:\/\/ui\.kaeso\.app\/de\/components\/button<\/loc>([\s\S]*?)<\/url>/.exec(xml)?.[1] ?? "";
    expect([...button.matchAll(/hreflang="([^"]+)" href="([^"]+)"/g)].map((m) => [m[1], m[2]])).toEqual([
      ["en", `${SITE}/components/button`],
      ["de", `${SITE}/de/components/button`],
      ["zh-CN", `${SITE}/zh-CN/components/button`],
      ["x-default", `${SITE}/components/button`],
    ]);
    expect(xml).not.toContain("<lastmod>");
  });

  test("a page names itself, its languages and its preview image", async ({ page }) => {
    await page.goto("/de/components/button");
    const head = page.locator("head");
    const content = (selector: string) => head.locator(selector).getAttribute("content");
    await expect(page).toHaveTitle("Button · Kaeso UI");
    await expect(head.locator('link[rel="canonical"]')).toHaveAttribute("href", `${SITE}/de/components/button`);
    expect(await content('meta[property="og:url"]')).toBe(`${SITE}/de/components/button`);
    expect(await content('meta[property="og:site_name"]')).toBe("Kaeso UI");
    expect(await content('meta[property="og:locale"]')).toBe("de_DE");
    expect(await head.locator('meta[property="og:locale:alternate"]').evaluateAll((els) => els.map((e) => e.getAttribute("content")))).toEqual(["en_US", "zh_CN"]);
    expect(await content('meta[property="og:image"]')).toBe(`${SITE}/og.png`);
    expect(await content('meta[name="twitter:card"]')).toBe("summary_large_image");
    expect(await content('meta[name="description"]')).toBe(await content('meta[property="og:description"]'));
    expect(await head.locator('meta[name="robots"]').count()).toBe(0);
    expect(await head.locator('script[type="application/ld+json"]').count()).toBe(0);
  });

  test("the home page describes the site as a website in its language", async ({ page }) => {
    await page.goto("/zh-CN");
    const data = JSON.parse((await page.locator('head script[type="application/ld+json"]').textContent()) ?? "{}");
    expect(data).toEqual({
      "@context": "https://schema.org",
      "@type": "WebSite",
      name: "Kaeso UI",
      url: `${SITE}/zh-CN`,
      inLanguage: "zh-CN",
      description: "Kaeso 的 UI 库：纯白与纯黑、胶囊形控件、单色界面。",
    });
  });

  for (const path of ["/404", "/de/404", "/components/button/examples", "/zh-CN/components/button/states"]) {
    test(`${path} is kept out of search results`, async ({ request }) => {
      const html = await (await request.get(path)).text();
      expect(html).toContain('<meta name="robots" content="noindex">');
      expect(html).not.toContain('rel="canonical"');
    });
  }

  test("the manifest, the icons and the preview image are served", async ({ request }) => {
    const manifest = (await (await request.get("/site.webmanifest")).json()) as { name: string; icons: { src: string }[] };
    expect(manifest.name).toBe("Kaeso UI");
    for (const src of ["/favicon.svg", "/favicon-32.png", "/apple-touch-icon.png", ...manifest.icons.map((i) => i.src)])
      expect((await request.get(src)).status(), src).toBe(200);
    const png = Buffer.from(await (await request.get("/og.png")).body());
    expect([png.readUInt32BE(16), png.readUInt32BE(20)]).toEqual([1200, 630]);
  });
});

test.describe("search on a phone", () => {
  test.use({ viewport: { width: 375, height: 812 }, hasTouch: true, isMobile: true });

  const words = {
    en: { search: "Search", nav: "Open navigation", placeholder: "Search pages, components and props" },
    de: { search: "Suchen", nav: "Navigation öffnen", placeholder: "Seiten, Komponenten und Props suchen" },
    "zh-CN": { search: "搜索", nav: "打开导航", placeholder: "搜索页面、组件和属性" },
  } as const;

  for (const lang of LANGS) {
    test(`opens from the header in ${lang}`, async ({ page }) => {
      const w = words[lang];
      await open(page, at(lang, "/components/button"));
      const header = page.locator("header").filter({ has: page.getByRole("button", { name: w.nav }) });
      const search = header.getByRole("button", { name: w.search });
      await expect(search).toBeVisible();
      await expect(search).toHaveAttribute("aria-haspopup", "dialog");
      const [searchBox, navBox] = [await search.boundingBox(), await header.getByRole("button", { name: w.nav }).boundingBox()];
      expect(searchBox && navBox && searchBox.x + searchBox.width <= navBox.x && searchBox.y === navBox.y).toBe(true);
      await search.click();
      const input = page.getByPlaceholder(w.placeholder);
      await expect(input).toBeFocused();
      await input.fill("Select");
      await page.getByRole("option", { name: /^Select(?!\w)/ }).click();
      await expect(page).toHaveURL(at(lang, "/components/select"));
    });
  }

  test("the keyboard shortcut still opens it", async ({ page }) => {
    await open(page, "/");
    await page.keyboard.press("ControlOrMeta+k");
    await expect(page.getByPlaceholder(words.en.placeholder)).toBeFocused();
  });

  for (const theme of ["light", "dark"] as const) {
    test(`the header passes axe in ${theme}`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: theme });
      await open(page, "/de/installation");
      const { violations } = await new AxeBuilder({ page }).include("header").withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"]).analyze();
      expect(violations.map((v) => v.id)).toEqual([]);
    });
  }
});
