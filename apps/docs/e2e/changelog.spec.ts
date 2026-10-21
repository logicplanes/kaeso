import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "./support";

const repo = "https://github.com/logicplanes/kaeso";

const LANGUAGES = [
  {
    path: "/changelog",
    nav: "Documentation",
    title: "Changelog",
    unreleased: "Unreleased",
    latest: "Latest",
    prerelease: "Pre-release",
    added: "Added",
    date: "15 Nov 2026",
    note: null,
    compare: "Changes since 1.1.1",
  },
  {
    path: "/de/changelog",
    nav: "Dokumentation",
    title: "Änderungen",
    unreleased: "Unveröffentlicht",
    latest: "Aktuell",
    prerelease: "Vorabversion",
    added: "Hinzugefügt",
    date: "15. Nov. 2026",
    note: "Die Einträge sind auf Englisch verfasst.",
    compare: "Änderungen seit 1.1.1",
  },
  {
    path: "/zh-CN/changelog",
    nav: "文档",
    title: "更新日志",
    unreleased: "未发布",
    latest: "最新",
    prerelease: "预发布版本",
    added: "新增",
    date: "2026年11月15日",
    note: "更新日志以英文撰写。",
    compare: "自 1.1.1 以来的变更",
  },
] as const;

for (const l of LANGUAGES) {
  test(`the changelog at ${l.path} shows the latest releases in its language, with the entries in English`, async ({ page }) => {
    await page.goto(l.path);
    await expect(page.getByRole("heading", { level: 1, name: l.title })).toBeVisible();
    const nav = page.getByRole("navigation", { name: l.nav });
    await expect(nav.getByRole("link", { name: l.title })).toHaveAttribute("aria-current", "page");
    await expect(page.getByRole("heading", { level: 2 })).toHaveText([l.unreleased, "1.3.0-rc.1", "1.2.0"]);
    const latest = page.getByRole("region", { name: "1.2.0" });
    await expect(latest.getByText(l.latest, { exact: true })).toBeVisible();
    await expect(page.getByText(l.latest, { exact: true })).toHaveCount(1);
    await expect(page.getByRole("region", { name: "1.3.0-rc.1" }).getByText(l.prerelease, { exact: true })).toBeVisible();
    await expect(latest.locator("time")).toHaveAttribute("datetime", "2026-11-15");
    await expect(latest.locator("time")).toHaveText(l.date);
    await expect(latest.getByRole("link", { name: l.compare })).toHaveAttribute("href", `${repo}/compare/v1.1.1...v1.2.0`);
    await expect(latest.getByRole("link", { name: /GitHub/ }).first()).toHaveAttribute("href", `${repo}/releases/tag/v1.2.0`);
    await expect(latest.getByRole("heading", { level: 3, name: l.added })).toHaveAttribute("id", "v1.2.0-added");
    await expect(latest.getByRole("img", { name: /1\.2\.0/ }).locator("image")).toHaveAttribute("href", "https://update.kaeso.app/art/1.2.0.svg");
    await expect(latest.getByText("A filter for aliases on the models page.").locator("xpath=ancestor::*[@lang][1]")).toHaveAttribute("lang", "en");
    if (l.note) {
      await expect(page.getByText(l.note)).toBeVisible();
      await expect(latest.getByRole("heading", { level: 4, name: "Model API" })).toHaveAttribute("lang", "en");
    } else await expect(page.getByText("The changelog is written in English.")).toHaveCount(0);
  });
}

test("the changelog follows the introduction in the sidebar and is built as its own page", async ({ page, request }) => {
  await page.goto("/");
  const nav = page.getByRole("navigation", { name: "Documentation" });
  await expect(nav.getByRole("link", { name: "Changelog" })).toBeVisible();
  const links = await nav.getByRole("link").allTextContents();
  expect(links[links.indexOf("Introduction") + 1]).toBe("Changelog");
  await nav.getByRole("link", { name: "Changelog" }).click();
  await expect(page).toHaveURL("/changelog");
  await expect(page).toHaveTitle("Changelog · Kaeso documentation");
  await expect(page.getByRole("link", { name: /Every release on GitHub/ })).toHaveAttribute("href", `${repo}/releases`);
  await expect(page.getByRole("link", { name: /Edit on GitHub/ })).toHaveCount(0);
  const html = await (await request.get("/de/changelog")).text();
  expect(html).toContain("<title>Änderungen · Kaeso-Dokumentation</title>");
  expect(html).toContain('<link rel="alternate" type="application/atom+xml" title="Kaeso-Releases" href="https://github.com/logicplanes/kaeso/releases.atom" />');
  expect(await (await request.get("/sitemap.xml")).text()).toContain("<loc>https://docs.kaeso.app/zh-CN/changelog</loc>");
});

test("each release shows its artwork from the update service, in a frame kept while it loads", async ({ page, releaseArt }) => {
  await page.goto("/changelog");
  const art = page.getByRole("region", { name: "1.2.0" }).getByRole("img", { name: "Artwork of Kaeso 1.2.0" });
  await expect(art.locator("image")).toHaveAttribute("href", "https://update.kaeso.app/art/1.2.0.svg");
  const box = await art.boundingBox();
  expect(Math.round(((box?.width ?? 0) / (box?.height ?? 1)) * 9)).toBe(21);
  await expect.poll(() => releaseArt).toContain("https://update.kaeso.app/art/1.2.0.svg");
  await expect(page.getByRole("region", { name: "Unreleased" }).getByRole("img")).toHaveCount(0);
});

test("the lines under Unreleased say they are not released yet", async ({ page }) => {
  await page.goto("/changelog");
  const unreleased = page.getByRole("region", { name: "Unreleased" });
  await expect(unreleased.getByText("Not in a release yet; these changes come with the next one.")).toBeVisible();
  await expect(unreleased.getByRole("link", { name: "Changes since 1.3.0-rc.1" })).toHaveAttribute("href", `${repo}/compare/v1.3.0-rc.1...HEAD`);
  await expect(unreleased.locator("time")).toHaveCount(0);
});

test("a link to a version opens the page of its series there, and the versions link to each other", async ({ page, request }) => {
  await page.goto("/de/changelog#v1.1.0");
  await expect(page).toHaveURL("/de/changelog/1.1#v1.1.0");
  await expect(page.getByRole("heading", { level: 1, name: "Änderungen 1.1" })).toBeVisible();
  await expect(page.getByRole("heading", { level: 2, name: "1.1.0" })).toBeInViewport();
  await page.getByRole("region", { name: "1.1.0" }).getByRole("link", { name: "1.0.0", exact: true }).click();
  await expect(page).toHaveURL("/de/changelog/1.0#v1.0.0");
  await expect(page.getByRole("heading", { level: 2, name: "1.0.0" })).toBeInViewport();
  await page.getByRole("navigation", { name: "Weitere Releases" }).getByRole("link", { name: "Neueste Releases" }).click();
  await expect(page).toHaveURL("/de/changelog");
  const html = await (await request.get("/zh-CN/changelog/1.1")).text();
  expect(html).toContain("<title>更新日志 1.1 · Kaeso 文档</title>");
  expect(await (await request.get("/sitemap.xml")).text()).toContain("<loc>https://docs.kaeso.app/changelog/1.0</loc>");
});

test("the changelog fits a phone without scrolling sideways", async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 });
  for (const path of ["/changelog", "/de/changelog", "/zh-CN/changelog", "/changelog/1.1"]) {
    await page.goto(path);
    await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(375);
  }
});

for (const theme of ["light", "dark"] as const) {
  test(`the changelog passes axe in every language in the ${theme} theme`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: theme });
    for (const path of [...LANGUAGES.map((l) => l.path), "/de/changelog/1.1"]) {
      await page.goto(path);
      await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
      const results = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"]).analyze();
      expect(results.violations.map((v) => `${path}: ${v.id} (${v.nodes.length})`)).toEqual([]);
    }
  });
}
