import { describe, expect, it } from "vitest";
import { scriptedReply, slackQuestion, ticket } from "../stack/conversations.mjs";

const tickets = Array.from({ length: 400 }, (_, i) => ticket(i));
const SIGNATURES = { en: "Northwind Customer Care", de: "Ihr Northwind-Kundenservice", fr: "Le service client Northwind" };
const MONTHS =
  /\b(January|February|March|April|June|July|August|September|October|November|December|Januar|Februar|März|Juni|Juli|Oktober|Dezember|janvier|février|mars|avril|juin|juillet|août|octobre|novembre|décembre)\b|\b20\d\d\b/;

describe("the seed's support tickets", () => {
  it("are classified the way the triage prompt asks, with the order the email names", () => {
    for (const t of tickets) {
      const c = JSON.parse(t.classify.reply);
      expect(Object.keys(c)).toEqual(["category", "priority", "language", "sentiment", "order_id", "needs_human", "summary"]);
      expect(c.category).toBe(t.category);
      expect(c.language).toBe(t.language);
      expect(["low", "normal", "high", "urgent"]).toContain(c.priority);
      if (c.order_id) expect(t.classify.user).toContain(c.order_id);
      expect(c.summary.split(" ").length).toBeLessThanOrEqual(25);
    }
  });

  it("get their drafted answer in the customer's language", () => {
    for (const t of tickets) expect(t.draft.reply).toContain(SIGNATURES[t.language]);
  });

  it("are all different, come out the same every time and never name a date", () => {
    expect(new Set(tickets.map((t) => t.classify.user)).size).toBe(tickets.length);
    expect(ticket(17)).toEqual(tickets[17]);
    for (const t of tickets) expect(`${t.classify.user}\n${t.draft.reply}`).not.toMatch(MONTHS);
  });

  it("cover every category in English, German and French", () => {
    expect(new Set(tickets.map((t) => t.category)).size).toBe(11);
    expect(new Set(tickets.map((t) => t.language))).toEqual(new Set(["en", "de", "fr"]));
  });
});

describe("the mock provider's written answers", () => {
  const t = tickets[3];
  const q = slackQuestion(5);

  it("answer the seed's prompts in each request format", () => {
    expect(scriptedReply({ system: "triage", messages: [{ role: "user", content: t.classify.user }] })).toBe(t.classify.reply);
    expect(
      scriptedReply({
        messages: [
          { role: "system", content: "x" },
          { role: "user", content: [{ type: "text", text: t.draft.user }] },
        ],
      }),
    ).toBe(t.draft.reply);
    expect(scriptedReply({ input: q.user })).toBe(q.reply);
    expect(scriptedReply({ input: [{ role: "user", content: [{ type: "input_text", text: q.user }] }] })).toBe(q.reply);
  });

  it("leave every other prompt to the mock's usual reply", () => {
    expect(scriptedReply({ messages: [{ role: "user", content: "Where is order 4471?" }] })).toBeUndefined();
    expect(scriptedReply({ messages: [{ role: "assistant", content: t.classify.user }] })).toBeUndefined();
    expect(scriptedReply({})).toBeUndefined();
  });
});
