import { afterEach, describe, expect, it } from "vitest";
import { formatDate, setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { apply, load, type Locale } from "~/i18n";
import { errorFromResponse } from "./errors";

const speak = async (l: Locale) => {
  apply(await load(l));
  setFormatLocale(l);
};
afterEach(() => speak("en"));

const refusal = (message: string) => new Response(JSON.stringify({ error: { type: "invalid_request", message } }), { status: 409 });

describe("a refusal that lists limits", () => {
  it("names each owner and amount in German, joined the German way", async () => {
    await speak("de");
    const error = await errorFromResponse(
      refusal("First lower the limits that allow more than this one: member usr_01 (1200), key key_1 (5), the project (8)."),
    );
    expect(error.message).toBe("Zuerst die Limits senken, die mehr erlauben als dieses: Mitglied usr_01 (1.200), Schlüssel key_1 (5) und Projekt (8).");
  });

  it("stays as the gateway wrote it when one owner is unknown", async () => {
    await speak("de");
    const message = "First lower the limits that allow more than this one: member usr_01 (1200), the moon (3).";
    expect((await errorFromResponse(refusal(message))).message).toBe(message);
  });
});

describe("a refusal with a budget or a rate", () => {
  it("shows the reset time in the person's format, and a time it can't read as written", async () => {
    await speak("de");
    const resets = await errorFromResponse(refusal("Budget used up: the project's monthly spend budget of 5 USD; it resets at 2026-11-01T00:00:00Z."));
    expect(resets.message).toBe(
      `Budget des Projekts aufgebraucht (Monats-Ausgabenbudget von 5 USD); es wird am ${formatDate("2026-11-01T00:00:00Z", { time: true })} zurückgesetzt.`,
    );
    const unreadable = await errorFromResponse(refusal("Budget used up: the project's monthly spend budget of 5 USD; it resets at soon."));
    expect(unreadable.message).toBe("Budget des Projekts aufgebraucht (Monats-Ausgabenbudget von 5 USD); es wird am soon zurückgesetzt.");
  });

  it("keeps an amount that isn't a number as the gateway wrote it", async () => {
    await speak("de");
    const error = await errorFromResponse(refusal("Rate limit reached: the project's request rate of many requests per minute; retry in 3 seconds."));
    expect(error.message).toBe("Ratenlimit des Projekts erreicht (Anfragerate von many Anfragen pro Minute); in 3 Sek. erneut versuchen.");
  });
});

describe("a refusal of a spend budget that is too large", () => {
  it("is translated in German and Chinese", async () => {
    const message = "The amount must be positive and at most 1,000,000,000 USD.";
    await speak("de");
    expect((await errorFromResponse(refusal(message))).message).toBe("Der Betrag muss positiv und höchstens 1.000.000.000 USD sein.");
    await speak("zh-CN");
    expect((await errorFromResponse(refusal(message))).message).toBe("数额必须为正数且不超过 1,000,000,000 USD。");
  });
});

describe("a role change that would leave its holders without a right they need", () => {
  it("names the right, its need and the holders in German and Chinese", async () => {
    const keep = "`members.invite` needs `members.view` for 1200 of this role's holders; keep it in the role.";
    const restore = "`members.invite` needs `members.view`, removed from 3 of this role's holders; restore it.";
    await speak("de");
    expect((await errorFromResponse(refusal(keep))).message).toBe("`members.invite` setzt `members.view` voraus – für 1.200 Inhaber; in der Rolle behalten.");
    expect((await errorFromResponse(refusal(restore))).message).toBe("`members.invite` setzt `members.view` voraus – für 3 Inhaber entfernt; zurückgeben.");
    await speak("zh-CN");
    expect((await errorFromResponse(refusal(keep))).message).toBe("此角色有 1,200 个持有者的 `members.invite` 需要 `members.view`；请在角色中保留它。");
    expect((await errorFromResponse(refusal(restore))).message).toBe("此角色有 3 个持有者被移除了 `members.invite` 所需的 `members.view`；请先恢复。");
  });
});

describe("a refusal in English", () => {
  it("keeps the gateway's own numbers, dates and lists exactly as written", async () => {
    const limits = "First lower the limits that allow more than this one: member usr_01 (1200), key key_1 (5), the project (8).";
    expect((await errorFromResponse(refusal(limits))).message).toBe(limits);
    const budget = "Budget used up: the project's monthly spend budget of 5000 USD; it resets at 2026-11-01T00:00:00Z.";
    expect((await errorFromResponse(refusal(budget))).message).toBe(budget);
  });
});
