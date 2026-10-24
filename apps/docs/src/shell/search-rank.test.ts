import { describe, expect, it } from "vitest";
import type { SearchEntry } from "~/markdown/ast";
import { matchHeadings, rankPages, splitPages } from "./search-rank";

const entry = (path: string, title: string, description: string, headings: string[] = [], words = ""): SearchEntry => ({
  path,
  title,
  description,
  headings: headings.map((label) => ({ id: label.toLowerCase().replace(/ /g, "-"), label })),
  words,
});
const index = [
  entry("/", "Kaeso documentation", "Guides and reference.", [], "gateway keys"),
  entry("/guides/budgets", "Budgets", "Spending limits for keys.", ["Monthly budgets"], "spend month"),
  entry("/guides/keys", "Keys", "Create a key for an app.", ["Create a key", "Rate limits"], "secret expiry"),
];

describe("rankPages", () => {
  it("puts title matches first, then descriptions, then the text", () => {
    expect(rankPages(index, "keys").map((e) => e.path)).toEqual(["/guides/keys", "/guides/budgets", "/"]);
    expect(rankPages(index, "").map((e) => e.path)).toEqual(["/", "/guides/budgets", "/guides/keys"]);
  });
});

describe("splitPages", () => {
  it("keeps pages that only mention the words apart from pages their title or description names", () => {
    const { named, mentioned } = splitPages(index, "keys");
    expect(named.map((e) => e.path)).toEqual(["/guides/keys", "/guides/budgets"]);
    expect(mentioned.map((e) => e.path)).toEqual(["/"]);
    expect(splitPages(index, "").named).toHaveLength(3);
    expect(splitPages(index, "").mentioned).toEqual([]);
  });
});

describe("matchHeadings", () => {
  it("matches a heading by its own words only", () => {
    expect(matchHeadings(index, "rate").map((h) => `${h.entry.path}#${h.id}`)).toEqual(["/guides/keys#rate-limits"]);
    expect(matchHeadings(index, "keys")).toEqual([]);
    expect(matchHeadings(index, "")).toEqual([]);
  });
});
