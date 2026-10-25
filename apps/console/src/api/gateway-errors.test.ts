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

  it("tells the person a request could cross a budget in their language", async () => {
    await speak("de");
    const error = await errorFromResponse(refusal("This request could exceed this key's monthly spend budget of 1 USD; ask for fewer output tokens."));
    expect(error.message).toBe(
      "Diese Anfrage könnte das Limit dieses Schlüssels (Monats-Ausgabenbudget von 1 USD) überschreiten; weniger Ausgabe-Token anfordern.",
    );
  });

  it("keeps an amount that isn't a number as the gateway wrote it", async () => {
    await speak("de");
    const error = await errorFromResponse(refusal("Rate limit reached: the project's request rate of many requests per minute; retry in 3 seconds."));
    expect(error.message).toBe("Ratenlimit des Projekts erreicht (Anfragerate von many Anfragen pro Minute); in 3 Sek. erneut versuchen.");
  });
});

describe("a refusal of a feature a spend budget can't price", () => {
  it("keeps the feature as code in German and Chinese", async () => {
    const message = "The member has a spend budget that can't price `image_generation`; ask an admin.";
    await speak("de");
    expect((await errorFromResponse(refusal(message))).message).toBe(
      "Das Mitglied hat ein Ausgabenbudget, das `image_generation` nicht bepreisen kann; wenden Sie sich an einen Admin.",
    );
    await speak("zh-CN");
    expect((await errorFromResponse(refusal("This key has a spend budget that can't price `service_tier: priority`; ask an admin."))).message).toBe(
      "此密钥有支出预算，无法为 `service_tier: priority` 计价，请联系管理员。",
    );
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
    expect((await errorFromResponse(refusal(keep))).message).toBe(
      "Für 1.200 Träger dieser Rolle setzt `members.invite` das Recht `members.view` voraus; es in der Rolle behalten.",
    );
    expect((await errorFromResponse(refusal(restore))).message).toBe(
      "`members.invite` setzt `members.view` voraus, das 3 Trägern dieser Rolle entzogen wurde; es wiederherstellen.",
    );
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

describe("a refusal from the Playground's capability checks", () => {
  it("joins the effort levels the language's way and keeps the codes", async () => {
    const message = "The model `gpt-5` supports the effort levels `low`, `medium`, `high`; `max` is not one of them.";
    await speak("de");
    expect((await errorFromResponse(refusal(message))).message).toBe(
      "Das Modell `gpt-5` unterstützt die Aufwandsstufen `low`, `medium` und `high`; `max` gehört nicht dazu.",
    );
    await speak("zh-CN");
    expect((await errorFromResponse(refusal(message))).message).toBe("模型 `gpt-5` 支持的推理强度为 `low`、`medium` 和 `high`，`max` 不在其中。");
    const plain = "The model `gpt-5` supports the effort levels low, high; `max` is not one of them.";
    expect((await errorFromResponse(refusal(plain))).message).toBe(plain);
  });

  it("tells which thinking budget the model takes, with the numbers in the language's format", async () => {
    const message = "A `budget_tokens` of 2048 means `low`; `mistral-small-latest` needs at least 16384.";
    await speak("de");
    expect((await errorFromResponse(refusal(message))).message).toBe(
      "`budget_tokens` von 2.048 ergibt `low`; `mistral-small-latest` braucht mindestens 16.384.",
    );
    await speak("zh-CN");
    expect((await errorFromResponse(refusal("A `budget_tokens` of 2048 means `low`; `gpt-5` needs `thinking` off."))).message).toBe(
      "`budget_tokens` 为 2,048 即 `low`；`gpt-5` 需要关闭 `thinking`。",
    );
  });

  it("translates the named tool choice and keeps a choice written as code", async () => {
    await speak("de");
    const named = "The model `o3` does not accept a named function in `tool_choice`; send `auto` and name the tool in the prompt.";
    expect((await errorFromResponse(refusal(named))).message).toBe(
      "Das Modell `o3` nimmt eine benannte Funktion in `tool_choice` nicht an; senden Sie `auto` und nennen Sie das Tool im Prompt.",
    );
    const code = "The model `o3` does not accept `tool_choice` `required`; send `auto` and name the tool in the prompt.";
    expect((await errorFromResponse(refusal(code))).message).toBe(
      "Das Modell `o3` nimmt `tool_choice` `required` nicht an; senden Sie `auto` und nennen Sie das Tool im Prompt.",
    );
    const unknown = "The model `o3` does not accept a brand new choice; send `auto` and name the tool in the prompt.";
    expect((await errorFromResponse(refusal(unknown))).message).toBe(unknown);
  });
});

describe("a refusal while Kaeso updates", () => {
  it("is translated from the gateway and from the proxy in front of it", async () => {
    await speak("de");
    expect((await errorFromResponse(refusal("An update is already running; wait for it to finish."))).message).toBe(
      "Ein Update läuft bereits; warten Sie, bis es abgeschlossen ist.",
    );
    await speak("zh-CN");
    expect((await errorFromResponse(refusal("Kaeso is being updated; try again in a minute."))).message).toBe("Kaeso 正在更新，请一分钟后重试。");
    expect((await errorFromResponse(refusal("10 log deletions are still waiting; try again once they have run."))).message).toBe(
      "仍有 10 个日志删除任务在等待，请在其完成后重试。",
    );
  });
});

describe("the model API's messages, as request logs show them", () => {
  it("translate with the stage a provider ran out of time in", async () => {
    const { translateMessage, knownMessage } = await import("./gateway-errors");
    await speak("de");
    expect(translateMessage("The provider did not answer in time (time to first byte). Kaeso does not retry; try again or raise the model's timeout.")).toBe(
      "Der Anbieter hat nicht rechtzeitig geantwortet (Zeit bis zum ersten Byte). Kaeso wiederholt nicht; erneut versuchen oder das Zeitlimit des Modells erhöhen.",
    );
    expect(translateMessage("The provider did not answer in time (lunch). Kaeso does not retry; try again or raise the model's timeout.")).toBe(
      "The provider did not answer in time (lunch). Kaeso does not retry; try again or raise the model's timeout.",
    );
    await speak("zh-CN");
    expect(translateMessage("The app closed the connection before the reply was complete.")).toBe("回复完成前，应用关闭了连接。");
    expect(translateMessage("The app stopped reading the stream for too long; Kaeso ended it.")).toBe("应用过长时间未读取流，Kaeso 已将其结束。");
    expect(translateMessage("The request body is larger than 1048576 bytes.")).toBe("请求体超过 1,048,576 字节。");
    expect(knownMessage("Provider returned error")).toBeUndefined();
  });

  it("ask a person to open Kaeso at its own address", async () => {
    await speak("de");
    const message = "Open Kaeso at https://kaeso.example.com; it refuses requests from pages at other addresses.";
    expect((await errorFromResponse(new Response(JSON.stringify({ error: { type: "permission_denied", message } }), { status: 403 }))).message).toBe(
      "Kaeso unter https://kaeso.example.com öffnen; Anfragen von Seiten unter anderen Adressen lehnt es ab.",
    );
  });
});
