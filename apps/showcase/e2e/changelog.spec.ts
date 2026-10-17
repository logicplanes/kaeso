import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { history } from "../../../scripts/changelog.mjs";
import { open } from "./support";

const log = history(readFileSync(new URL("../../../CHANGELOG.md", import.meta.url), "utf8"));
const latest = log.releases.find((r) => r.latest);
const { version } = JSON.parse(readFileSync(new URL("../../../packages/ui/package.json", import.meta.url), "utf8")) as { version: string };

const LANGUAGES = [
  { path: "/changelog", lang: "en", title: "Changelog", unreleased: "Unreleased", latest: "Latest", release: "Release on GitHub", added: "Added", note: null },
  {
    path: "/de/changelog",
    lang: "de",
    title: "Änderungen",
    unreleased: "Unveröffentlicht",
    latest: "Aktuell",
    release: "Release auf GitHub",
    added: "Hinzugefügt",
    note: "Die Änderungen sind auf Englisch geschrieben.",
  },
  {
    path: "/zh-CN/changelog",
    lang: "zh-CN",
    title: "更新日志",
    unreleased: "未发布",
    latest: "最新",
    release: "GitHub 上的版本",
    added: "新增",
    note: "更新日志以英文撰写。",
  },
] as const;

const shortDate = (lang: string, iso: string) => {
  const [y, m, d] = iso.split("-").map(Number);
  return new Intl.DateTimeFormat(lang === "en" ? "en-GB" : lang, { day: "numeric", month: "short", year: "numeric" }).format(new Date(y ?? 0, (m ?? 1) - 1, d ?? 1));
};

test("the changelog has releases to show", () => {
  expect(log.releases.length).toBeGreaterThan(0);
  expect(latest).toBeDefined();
});

for (const l of LANGUAGES) {
  test(`${l.path} shows every release of CHANGELOG.md in ${l.lang}, with the entries in English`, async ({ page }) => {
    await open(page, l.path);
    await expect(page.getByRole("heading", { level: 1, name: l.title })).toBeVisible();
    await expect(page.locator("main h2")).toHaveText([...(log.unreleased ? [l.unreleased] : []), ...log.releases.map((r) => r.version)]);
    for (const release of log.releases) {
      const section = page.getByRole("region", { name: release.version });
      await expect(section.getByRole("heading", { level: 2 })).toHaveAttribute("id", `v${release.version}`);
      if (release.date) {
        await expect(section.locator("time").first()).toHaveAttribute("datetime", release.date);
        await expect(section.locator("time").first()).toHaveText(shortDate(l.lang, release.date));
      }
      await expect(section.getByRole("link", { name: l.release })).toHaveAttribute("href", `https://github.com/logicplanes/kaeso/releases/tag/v${release.version}`);
    }
    await expect(page.getByRole("region", { name: latest?.version ?? "" }).getByText(l.latest, { exact: true })).toBeVisible();
    await expect(page.getByText(l.latest, { exact: true })).toHaveCount(1);
    const first = log.releases[0];
    if (first?.sections.some((s) => s.name === "Added")) await expect(page.getByRole("region", { name: first.version }).getByRole("heading", { level: 3, name: l.added })).toBeVisible();
    const entries = page.locator("main .prose-k");
    await expect(entries.first()).toBeVisible();
    for (const lang of await entries.evaluateAll((els) => els.map((el) => el.getAttribute("lang")))) expect(lang).toBe(l.note ? "en" : null);
    if (l.note) await expect(page.getByText(l.note)).toBeVisible();
    else await expect(page.getByText("The changelog is written in English.")).toHaveCount(0);
  });
}

test("Unreleased shows only when CHANGELOG.md has lines under it, and says they are not released", async ({ page }) => {
  await open(page, "/changelog");
  const unreleased = page.getByRole("region", { name: "Unreleased" });
  if (!log.unreleased) {
    await expect(unreleased).toHaveCount(0);
    return;
  }
  await expect(unreleased.getByText("Not in a release yet; these changes come with the next one.")).toBeVisible();
  await expect(unreleased.getByRole("link", { name: `Changes since ${log.unreleased.since}` })).toHaveAttribute("href", log.unreleased.compare ?? "");
  await expect(unreleased.locator("time")).toHaveCount(0);
});

test("the package version in the bar opens its release in the changelog", async ({ page }) => {
  await open(page, "/de");
  await page.getByRole("link", { name: `@logicplanes/kaeso-ui v${version}` }).click();
  await expect(page).toHaveURL(new RegExp(`/de/changelog#v${version.replace(/\./g, "\\.")}$`));
  await expect(page.getByRole("heading", { level: 2, name: version })).toBeInViewport();
});

test("the changelog names GitHub's release feed", async ({ request }) => {
  const html = await (await request.get("/zh-CN/changelog")).text();
  expect(html).toContain('<link rel="alternate" type="application/atom+xml" title="Kaeso 版本" href="https://github.com/logicplanes/kaeso/releases.atom">');
});
