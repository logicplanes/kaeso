import AxeBuilder from "@axe-core/playwright";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { EDIT_BASE, FIXTURES } from "../playwright.config";
import { expect, test } from "./support";

test("the home page renders its title, description and text", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1, name: "Kaeso documentation" })).toBeVisible();
  await expect(page.getByText("Fixture pages for the end-to-end suite and the visual review.")).toBeVisible();
  await expect(page).toHaveTitle("Kaeso documentation");
  await expect(page.getByRole("navigation", { name: "Documentation" }).getByRole("link", { name: "Introduction" })).toHaveAttribute("aria-current", "page");
});

test("pages show the version of the release they were built from, in every language", async ({ page }) => {
  const { version } = JSON.parse(readFileSync(new URL("../../../package.json", import.meta.url), "utf8")) as { version: string };
  await page.goto("/");
  await expect(page.getByText(`They describe Kaeso ${version}.`)).toBeVisible();
  await page.goto("/de");
  await expect(page.getByText(`Sie beschreiben Kaeso ${version}.`)).toBeVisible();
  await page.goto("/zh-CN");
  await expect(page.getByText(`它们描述的是 Kaeso ${version}。`)).toBeVisible();
  await expect(page.getByText("{{version}}")).toHaveCount(0);
});

test("every page is built with its own title, description and alternates", async ({ request }) => {
  const html = await (await request.get("/guides/keys-and-limits")).text();
  expect(html).toContain("<title>Keys and limits · Kaeso documentation</title>");
  expect(html).toContain('<meta name="description" content="Create a key for an app and keep its spending inside a budget." />');
  expect(html).toContain('<link rel="canonical" href="https://docs.kaeso.app/guides/keys-and-limits" />');
  expect(html).toContain('<link rel="alternate" hreflang="de" href="https://docs.kaeso.app/de/guides/keys-and-limits" />');
  const german = await (await request.get("/de/guides/keys-and-limits")).text();
  expect(german).toContain('<html lang="de">');
  expect(german).toContain("<title>Schlüssel und Limits · Kaeso-Dokumentation</title>");
  const sitemap = await (await request.get("/sitemap.xml")).text();
  const english = [
    "/",
    "/changelog",
    "/guides",
    "/guides/keys-and-limits",
    "/guides/provider-keys",
    "/reference",
    "/reference/platform-api",
    "/reference/platform-api/keys",
    "/reference/models",
  ];
  const locs = [...sitemap.matchAll(/<loc>https:\/\/docs\.kaeso\.app([^<]*)<\/loc>/g)].map((m) => m[1] || "/");
  expect(locs.filter((l) => !/^\/(de|zh-CN)(\/|$)/.test(l))).toEqual(english);
  expect(locs).toHaveLength(english.length * 3);
  expect(sitemap).not.toContain("<lastmod>");
});

test("the build passes the site check: every page in the sitemap, with its alternates and link previews", () => {
  const docs = fileURLToPath(new URL("..", import.meta.url));
  const out = execFileSync("node", ["../../scripts/check-site.mjs", "dist-e2e", "https://docs.kaeso.app"], { cwd: docs, encoding: "utf8" });
  expect(out).toMatch(/^site: 27 pages, all 27 in sitemap\.xml/);
});

test("a page names its language, its preview image and what it is", async ({ request }) => {
  const html = await (await request.get("/zh-CN/guides/keys-and-limits")).text();
  for (const tag of [
    '<meta property="og:url" content="https://docs.kaeso.app/zh-CN/guides/keys-and-limits" />',
    '<meta property="og:locale" content="zh_CN" />',
    '<meta property="og:locale:alternate" content="en_US" />',
    '<meta property="og:locale:alternate" content="de_DE" />',
    '<meta property="og:image" content="https://docs.kaeso.app/og.png" />',
    '<meta property="og:type" content="website" />',
    '<meta name="twitter:card" content="summary_large_image" />',
    '<link rel="alternate" hreflang="x-default" href="https://docs.kaeso.app/guides/keys-and-limits" />',
  ])
    expect(html).toContain(tag);
  const data = JSON.parse(/<script type="application\/ld\+json">(.*?)<\/script>/.exec(html)?.[1] ?? "{}");
  expect(data).toMatchObject({ "@type": "TechArticle", inLanguage: "zh-CN", url: "https://docs.kaeso.app/zh-CN/guides/keys-and-limits" });
  expect(html).not.toContain("noindex");
  for (const path of ["/404.html", "/de/404.html", "/zh-CN/404.html"]) {
    const notFound = await (await request.get(path)).text();
    expect(notFound).toContain('<meta name="robots" content="noindex" />');
    expect(notFound).not.toContain('rel="canonical"');
  }
  const png = Buffer.from(await (await request.get("/og.png")).body());
  expect([png.readUInt32BE(16), png.readUInt32BE(20)]).toEqual([1200, 630]);
  expect(await (await request.get("/robots.txt")).text()).toContain("Sitemap: https://docs.kaeso.app/sitemap.xml");
  expect(((await (await request.get("/site.webmanifest")).json()) as { name: string }).name).toBe("Kaeso documentation");
});

test("the sidebar opens pages in place, with their title and contents", async ({ page }) => {
  await page.goto("/");
  const nav = page.getByRole("navigation", { name: "Documentation" });
  await nav.getByRole("link", { name: "Keys and limits" }).click();
  await expect(page).toHaveURL("/guides/keys-and-limits");
  await expect(page.getByRole("heading", { level: 1, name: "Keys and limits" })).toBeVisible();
  await expect(page).toHaveTitle("Keys and limits · Kaeso documentation");
  await expect(nav.getByRole("link", { name: "Keys and limits" })).toHaveAttribute("aria-current", "page");
  const toc = page.getByRole("navigation", { name: "On this page" });
  await expect(toc.getByRole("link")).toHaveText(["Create a key", "From an SDK", "Limits", "Rate limits", "Budgets", "Checklist", "Notes"]);
  await expect(page.getByRole("heading", { name: "Keys and limits" })).toHaveCount(1);
});

test("a relative link between two Markdown files opens the page at its heading", async ({ page }) => {
  await page.goto("/guides/provider-keys");
  await page.getByRole("link", { name: "rate limits" }).click();
  await expect(page).toHaveURL("/guides/keys-and-limits#rate-limits");
  await expect(page.getByRole("heading", { level: 3, name: "Rate limits" })).toBeInViewport();
});

test("a link to a heading opens there on first load too", async ({ page }) => {
  await page.goto("/guides/keys-and-limits#checklist");
  await expect(page.getByRole("heading", { level: 2, name: "Checklist" })).toBeInViewport();
});

test("previous and next follow the sidebar", async ({ page }) => {
  await page.goto("/guides/keys-and-limits");
  const steps = page.getByRole("navigation", { name: "Previous and next page" });
  await expect(steps.getByRole("link", { name: "Previous: Guides" })).toHaveAttribute("href", "/guides");
  await steps.getByRole("link", { name: "Next: Provider keys" }).click();
  await expect(page).toHaveURL("/guides/provider-keys");
});

test("Edit on GitHub opens the page's file", async ({ page }) => {
  await page.goto("/reference/platform-api/keys");
  const edit = page.getByRole("link", { name: /Edit on GitHub/ });
  await expect(edit).toHaveAttribute("href", `${EDIT_BASE}${FIXTURES}/en/reference/platform-api/keys.md`);
  await expect(edit).toHaveAttribute("href", /^https:\/\/github\.com\/logicplanes\/kaeso\/edit\/dev\//);
  await expect(edit).toHaveAttribute("target", "_blank");
});

test("search finds pages and headings, and says when nothing matches", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Search" }).click();
  const input = page.getByPlaceholder("Search the documentation");
  await expect(input).toBeFocused();
  await input.fill("rate");
  await expect(page.getByRole("option", { name: /Rate limits/ })).toBeVisible();
  await page.getByRole("option", { name: /Rate limits/ }).click();
  await expect(page).toHaveURL("/guides/keys-and-limits#rate-limits");
  await expect(page.getByRole("heading", { level: 3, name: "Rate limits" })).toBeInViewport();

  await page.keyboard.press("ControlOrMeta+k");
  await input.fill("zzzqx");
  await expect(page.getByText("Nothing matches. Try another word.")).toBeVisible();
  await input.fill("alias");
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL("/reference/models");
});

test("what is typed right after ⌘K, before the search has loaded, becomes its query", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  await page.keyboard.press("ControlOrMeta+k");
  await page.keyboard.type("alias");
  await expect(page.getByPlaceholder("Search the documentation")).toHaveValue("alias");
  await expect(page.getByRole("option", { name: /^Aliases/ })).toBeVisible();
});

test("the theme switches and stays switched", async ({ page }) => {
  await page.goto("/");
  const theme = page.getByRole("combobox", { name: "Theme" });
  await expect(theme).toHaveText("System");
  await theme.click();
  await page.getByRole("option", { name: "Dark" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect(theme).toHaveText("Dark");
  await theme.click();
  await page.getByRole("option", { name: "System" }).click();
  await expect(page.locator("html")).not.toHaveAttribute("data-theme");
});

test.describe("on a phone", () => {
  test.use({ viewport: { width: 375, height: 812 }, hasTouch: true, isMobile: true });

  test("the navigation is a sheet that closes when a page opens", async ({ page }) => {
    await page.goto("/guides/keys-and-limits");
    await expect(page.getByRole("navigation", { name: "On this page" })).toBeHidden();
    const toggle = page.getByRole("button", { name: "Open navigation" });
    await toggle.click();
    const sheet = page.getByRole("dialog", { name: "Navigation" });
    await expect(sheet).toBeVisible();
    await expect(sheet.getByRole("combobox", { name: "Language" })).toHaveText("English");
    await expect(sheet.getByRole("combobox", { name: "Theme" })).toHaveText("System");
    await sheet.getByRole("link", { name: "Models" }).click();
    await expect(page).toHaveURL("/reference/models");
    await expect(sheet).toBeHidden();
    await expect(page.getByRole("heading", { level: 1, name: "Models" })).toBeVisible();
  });

  for (const [path, search, navigation, placeholder, query, option] of [
    ["/", "Search", "Open navigation", "Search the documentation", "alias", /^Aliases/],
    ["/de", "Suchen", "Navigation öffnen", "Dokumentation durchsuchen", "Anbieter", /^Anbieter-Schlüssel/],
    ["/zh-CN", "搜索", "打开导航", "搜索文档", "密钥", /^服务商密钥/],
  ] as const) {
    test(`the search opens from the header at ${path}, beside the navigation toggle`, async ({ page }) => {
      await page.goto(path);
      await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
      const bar = page.getByRole("banner");
      const button = bar.getByRole("button", { name: search, exact: true });
      await expect(button).toBeVisible();
      await expect(button).toHaveAttribute("aria-haspopup", "dialog");
      const [own, toggle] = [await button.boundingBox(), await bar.getByRole("button", { name: navigation }).boundingBox()];
      expect(own && toggle && own.x + own.width <= toggle.x && own.y === toggle.y).toBe(true);
      await button.click();
      const input = page.getByPlaceholder(placeholder);
      await expect(input).toBeFocused();
      await input.fill(query);
      await page.getByRole("option", { name: option }).first().click();
      await expect(input).toBeHidden();
      await expect(page.getByRole("heading", { level: 1 })).not.toHaveText(/^Kaeso/);
    });
  }

  test("⌘K still opens the search on a phone", async ({ page }) => {
    await page.goto("/guides");
    await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
    await page.keyboard.press("ControlOrMeta+k");
    await expect(page.getByPlaceholder("Search the documentation")).toBeFocused();
  });

  for (const theme of ["light", "dark"] as const) {
    test(`the header passes axe in the ${theme} theme`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: theme });
      await page.goto("/de/guides");
      await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
      const results = await new AxeBuilder({ page }).include("header").withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"]).analyze();
      expect(results.violations.map((v) => v.id)).toEqual([]);
    });
  }

  test("a wide table scrolls inside the page, which never scrolls sideways", async ({ page }) => {
    await page.goto("/guides/keys-and-limits");
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(375);
    const region = page.getByRole("region", { name: "Table: Level, Requests per minute, Tokens per day, Spend per month, Applies to" });
    await expect(region).toHaveAttribute("tabindex", "0");
    await region.scrollIntoViewIfNeeded();
    const { inner, outer } = await region.evaluate((el) => ({ inner: el.scrollWidth, outer: el.clientWidth }));
    expect(inner).toBeGreaterThan(outer);
  });

  for (const path of ["/guides", "/de/guides", "/zh-CN/guides", "/guides/keys-and-limits", "/de/guides/keys-and-limits", "/zh-CN/reference/models"]) {
    test(`no word in a table at ${path} breaks inside itself, and the page never scrolls sideways`, async ({ page }) => {
      await page.goto(path);
      await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
      const split = await page.locator("article table").evaluateAll((tables) => {
        const broken: string[] = [];
        const han = /\p{Script=Han}/u;
        for (const table of tables) {
          const walker = document.createTreeWalker(table, NodeFilter.SHOW_TEXT);
          for (let node = walker.nextNode(); node; node = walker.nextNode()) {
            for (const word of (node.nodeValue ?? "").matchAll(/[\p{L}\p{N}]+/gu)) {
              if (han.test(word[0])) continue;
              const range = document.createRange();
              range.setStart(node, word.index);
              range.setEnd(node, word.index + word[0].length);
              const lines = new Set([...range.getClientRects()].filter((r) => r.width > 0).map((r) => Math.round(r.top)));
              if (lines.size > 1) broken.push(word[0]);
            }
          }
        }
        return broken;
      });
      expect(split).toEqual([]);
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(375);
    });
  }

  for (const path of ["/guides", "/de/guides", "/zh-CN/guides"]) {
    test(`the pages a table at ${path} links to stay on one line each`, async ({ page }) => {
      await page.goto(path);
      const links = page.locator("article tbody tr td:first-child a");
      await expect(links).toHaveCount(2);
      for (const link of await links.all()) {
        const { height, line } = await link.evaluate((el) => ({ height: el.getBoundingClientRect().height, line: parseFloat(getComputedStyle(el).lineHeight) }));
        expect(height).toBeLessThanOrEqual(line);
      }
      const region = page.getByRole("region", { name: /Table|Tabelle|表格/ });
      expect(await region.evaluate((el) => el.scrollWidth - el.clientWidth)).toBe(0);
    });
  }

  for (const [path, previous, next] of [
    ["/guides/keys-and-limits", "Previous: Guides", "Next: Provider keys"],
    ["/de/guides/keys-and-limits", "Zurück: Anleitungen", "Weiter: Anbieter-Schlüssel"],
    ["/zh-CN/guides/keys-and-limits", "上一页：指南", "下一页：服务商密钥"],
  ] as const) {
    test(`previous and next show their label above the title at ${path}`, async ({ page }) => {
      await page.goto(path);
      for (const [name, edge] of [
        [previous, "left"],
        [next, "right"],
      ] as const) {
        const link = page.getByRole("link", { name, exact: true });
        await expect(link).toBeVisible();
        const box = await link.evaluate((el, side) => {
          const [label, title] = [...el.children].map((c) => c.getBoundingClientRect());
          const nav = el.closest("nav")?.getBoundingClientRect();
          return { stacked: !!label && !!title && label.bottom <= title.top, aligned: !!nav && !!label && !!title && Math.abs(label[side] - title[side]) < 1 && Math.abs(title[side] - nav[side]) < 1 };
        }, edge);
        expect(box).toEqual({ stacked: true, aligned: true });
      }
    });
  }
});

test.describe("languages", () => {
  test("the language select opens the same page in another language, which the next visit remembers", async ({ page }) => {
    await page.goto("/guides/keys-and-limits");
    await expect(page.getByRole("combobox", { name: "Language" })).toHaveText("English");
    await page.getByRole("combobox", { name: "Language" }).click();
    await page.getByRole("option", { name: "Deutsch" }).click();
    await expect(page).toHaveURL("/de/guides/keys-and-limits");
    await expect(page.getByRole("heading", { level: 1, name: "Schlüssel und Limits" })).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute("lang", "de");
    await expect(page).toHaveTitle("Schlüssel und Limits · Kaeso-Dokumentation");
    await page.goto("/reference/models");
    await expect(page).toHaveURL("/de/reference/models");
    await expect(page.getByRole("combobox", { name: "Sprache" })).toHaveText("Deutsch");
    await page.getByRole("combobox", { name: "Sprache" }).click();
    await page.getByRole("option", { name: "简体中文" }).click();
    await expect(page).toHaveURL("/zh-CN/reference/models");
    await expect(page.getByRole("navigation", { name: "文档" })).toBeVisible();
  });

  test.describe("for a browser in German", () => {
    test.use({ locale: "de-DE" });

    test("the first visit follows the browser's language", async ({ page }) => {
      await page.goto("/guides/provider-keys");
      await expect(page).toHaveURL("/de/guides/provider-keys");
      await expect(page.getByRole("navigation", { name: "Dokumentation" }).getByRole("link", { name: "Anbieter-Schlüssel" })).toHaveAttribute(
        "aria-current",
        "page",
      );
    });

    test("an address with a query and a heading keeps both in German", async ({ page }) => {
      await page.goto("/guides/keys-and-limits?from=mail#limits");
      await expect(page).toHaveURL("/de/guides/keys-and-limits?from=mail#limits");
      await expect(page.getByRole("heading", { level: 2, name: "Limits" })).toBeInViewport();
    });
  });
});

test("an address without a page says so inside the frame", async ({ page }) => {
  await page.goto("/guides/nothing-here");
  await expect(page.getByText("This page doesn't exist.")).toBeVisible();
  await expect(page).toHaveTitle("Not found · Kaeso documentation");
  await page.getByRole("link", { name: "Go to the start" }).click();
  await expect(page).toHaveURL("/");
});

test.describe("tables and inline code", () => {
  const MODELS = ["/reference/models", "/de/reference/models", "/zh-CN/reference/models"];
  const WIDTHS = [320, 344, 375, 392, 414, 480, 1440];

  for (const path of MODELS) {
    for (const width of [375, 1440]) {
      test(`a table that fits does not scroll or show a scrollbar at ${path}, ${width} px wide`, async ({ page }) => {
        await page.setViewportSize({ width, height: 900 });
        await page.goto(path);
        const region = page.getByRole("region", { name: /Table|Tabelle|表格/ });
        await expect(region).toBeVisible();
        const state = await region.evaluate((el) => {
          el.scrollTop = 40;
          el.scrollLeft = 40;
          const style = getComputedStyle(el);
          return {
            width: [el.scrollWidth, el.clientWidth],
            height: [el.scrollHeight, el.clientHeight],
            top: el.scrollTop,
            left: el.scrollLeft,
            overflowY: style.overflowY,
            bars: el.parentElement?.querySelectorAll("[data-orientation]").length ?? 0,
          };
        });
        expect(state.width[0]).toBe(state.width[1]);
        expect(state.height[0]).toBe(state.height[1]);
        expect([state.top, state.left, state.overflowY, state.bars]).toEqual([0, 0, "hidden", 0]);
        await expect(region).toHaveAttribute("tabindex", "-1");
      });
    }
  }

  test("a table wider than the column scrolls sideways only", async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 812 });
    await page.goto("/guides/keys-and-limits");
    const region = page.getByRole("region", { name: /^Table: Level/ });
    await region.scrollIntoViewIfNeeded();
    const state = await region.evaluate((el) => {
      el.scrollTop = 40;
      el.scrollLeft = 40;
      return { wider: el.scrollWidth > el.clientWidth, left: el.scrollLeft, top: el.scrollTop, overflowY: getComputedStyle(el).overflowY, tall: el.scrollHeight - el.clientHeight };
    });
    expect(state).toEqual({ wider: true, left: 40, top: 0, overflowY: "hidden", tall: 0 });
  });

  for (const path of MODELS) {
    test(`a code chip stays whole and hugs the punctuation after it, which never starts a line, at ${path}`, async ({ page }) => {
      await page.goto(path);
      await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
      await page.evaluate(() => document.fonts.ready);
      for (const width of WIDTHS) {
        await page.setViewportSize({ width, height: 900 });
        const found = await page.locator("article code").evaluateAll((chips) => {
          const punctuation = /^[,.;:!?)\]、。，；：！？）]/;
          const loose: string[] = [];
          const detached: string[] = [];
          const margins: string[] = [];
          const split: string[] = [];
          let followed = 0;
          for (const chip of chips) {
            const style = getComputedStyle(chip);
            if ((chip.textContent ?? "").length <= 16 && chip.getClientRects().length > 1) split.push(chip.textContent ?? "");
            if (style.marginLeft !== "0px" || style.marginRight !== "0px") margins.push(chip.textContent ?? "");
            const next = chip.nextSibling;
            if (next?.nodeType !== Node.TEXT_NODE || !punctuation.test(next.nodeValue ?? "")) continue;
            followed += 1;
            const range = document.createRange();
            range.setStart(next, 0);
            range.setEnd(next, 1);
            const mark = range.getBoundingClientRect();
            const rects = [...chip.getClientRects()];
            const last = rects[rects.length - 1];
            if (!last) continue;
            if (Math.abs(mark.left - last.right) > 0.5) detached.push(`${chip.textContent}${next.nodeValue?.[0]}`);
            if (mark.top + mark.height / 2 < last.top || mark.top + mark.height / 2 > last.bottom) loose.push(`${chip.textContent}${next.nodeValue?.[0]}`);
          }
          return { followed, detached, loose, margins, split };
        });
        expect(found.followed, `chips followed by punctuation at ${width} px`).toBeGreaterThan(0);
        expect(found, `at ${width} px`).toEqual({ followed: found.followed, detached: [], loose: [], margins: [], split: [] });
      }
    });
  }
});

test.describe("accessibility", () => {
  for (const theme of ["light", "dark"] as const) {
    test(`pages in every language pass axe in the ${theme} theme`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: theme });
      for (const path of ["/", "/guides/keys-and-limits", "/de/guides", "/de/reference/models", "/zh-CN/guides/provider-keys"]) {
        await page.goto(path);
        await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
        const results = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"]).analyze();
        expect(results.violations.map((v) => `${path}: ${v.id} (${v.nodes.length})`)).toEqual([]);
      }
    });
  }
});
